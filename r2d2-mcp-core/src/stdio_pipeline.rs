use crate::mcp_types::{McpRequest, McpResponse};
use crate::SuperMcpServer;
use futures::StreamExt;
use serde_json::json;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio_util::codec::{FramedRead, LinesCodec};
use tracing::{debug, error, info};

pub async fn start_mcp_pipeline(server: Arc<SuperMcpServer>) {
    // Canaux de communication entre Acteurs
    let (tx_in, mut rx_in) = mpsc::channel::<String>(100);
    let (tx_out, mut rx_out) = mpsc::channel::<McpResponse>(100);

    // 1. ACTEUR INGESTION (Stdin -> tx_in)
    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        // [ISSUE-SEC-P1] Limite stricte de 10 Mo pour prévenir les OOM Stdio
        let codec = LinesCodec::new_with_max_length(10 * 1024 * 1024);
        let mut framed_reader = FramedRead::new(stdin, codec);

        while let Some(line_res) = framed_reader.next().await {
            match line_res {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    if tx_in.send(line.to_string()).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    error!("[SECURITY] Ligne Stdio rejetée (Dépassement 10 Mo potentiellement hostile) : {}", e);
                    continue; // Skip the malicious payload without crashing the agent
                }
            }
        }
        debug!("Acteur Ingestion (Stdin) terminé.");
    });

    // 2. ACTEUR ÉMISSION (rx_out -> Stdout)
    tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();

        while let Some(response) = rx_out.recv().await {
            match serde_json::to_string(&response) {
                Ok(mut serialized) => {
                    serialized.push('\n');
                    if let Err(e) = stdout.write_all(serialized.as_bytes()).await {
                        error!("Échec d'écriture Stdout: {}", e);
                    }
                    if let Err(e) = stdout.flush().await {
                        error!("Échec flush Stdout: {}", e);
                    }
                }
                Err(e) => {
                    error!("Erreur critique de sérialisation JSON: {}", e);
                }
            }
        }
        debug!("Acteur Émission (Stdout) terminé.");
    });

    // 3. ACTEUR ORCHESTRATEUR (Cœur de Dispatch Hexagonal)
    info!(
        "🚀 Serveur Stdio '{}' En Écoute. [Architecture Actor MPSC Active]",
        server.name
    );

    let orchestrator_tx_out = tx_out.clone();

    while let Some(msg) = rx_in.recv().await {
        let request: McpRequest = match serde_json::from_str(&msg) {
            Ok(r) => r,
            Err(e) => {
                error!("Parsing échoué (Requête JSON-RPC invalide) : {}", e);
                let err_res = McpResponse::error(json!(null), -32700, "Parse error");
                let _ = orchestrator_tx_out.send(err_res).await;
                continue;
            }
        };

        let server_ref = Arc::clone(&server);
        let resp_tx = orchestrator_tx_out.clone();

        tokio::spawn(async move {
            // [ISSUE-PROTO-P2] Gestion des Notifications JSON-RPC 2.0 (sans ID)
            let id = match request.id {
                Some(val) => val,
                None => {
                    if request.method == "notifications/initialized" {
                        info!("Handshake MCP Complété (Notification initialized).");
                    } else {
                        debug!(
                            "Notification ignorée discrètement (Aucun id fourni) : {}",
                            request.method
                        );
                    }
                    return; // ⛔ Pas de format de réponse pour les notifications.
                }
            };

            // Adaptateur Hexagonal : Invoque le port du domaine pur
            let response = match server_ref
                .execute_domain(&request.method, request.params)
                .await
            {
                Ok(res_payload) => McpResponse::success(id, res_payload),
                Err((code, msg)) => McpResponse::error(id, code, &msg),
            };

            let _ = resp_tx.send(response).await;
        });
    }

    info!("🛑 Serveur arrêté.");
}
