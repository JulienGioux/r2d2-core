use crate::mcp_types::{IpcMcpRequest, IpcMcpResponse};
use rkyv::check_archived_root;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, error, info};

pub async fn start_unix_socket(server: Arc<crate::SuperMcpServer>, socket_path: &str) {
    let _ = std::fs::remove_file(socket_path); // Nettoyage agressif d'un vieil artefact
    let listener =
        UnixListener::bind(socket_path).expect("Echec critique du bind Unix Socket (IPC)");
    info!(
        "🚀 Neural-Link IPC Actif. [Zero-Copy Unix Socket: {}]",
        socket_path
    );

    while let Ok((mut socket, _)) = listener.accept().await {
        let srv = Arc::clone(&server);
        tokio::spawn(async move {
            debug!("Nouvelle connexion physique détectée sur le socket IPC.");
            if let Err(e) = handle_ipc_client(&mut socket, srv).await {
                error!("Erreur de liaison Client IPC: {}", e);
            }
            debug!("Connexion IPC terminée.");
        });
    }
}

async fn handle_ipc_client(
    socket: &mut UnixStream,
    server: Arc<crate::SuperMcpServer>,
) -> anyhow::Result<()> {
    loop {
        let mut len_buf = [0u8; 4];
        // Si la lecture échoue, le client a fermé la connexion, on débranche doucement.
        if socket.read_exact(&mut len_buf).await.is_err() {
            return Ok(());
        }

        let length = u32::from_le_bytes(len_buf) as usize;
        // On pourrait pré-allouer un Obj Pool Buffer ici pour perf absolue
        let mut payload = vec![0u8; length];
        socket.read_exact(&mut payload).await?;

        // 1. Zéro-Copie : Validation stricte et overlay immuable sur les octets. O(1) de coût.
        let archived = match check_archived_root::<IpcMcpRequest>(&payload) {
            Ok(arch) => arch,
            Err(e) => {
                error!("Trame binaire rkyv corrompue rejetée: {}", e);
                continue;
            }
        };

        let method = archived.method.as_str(); // ZCO: Zero-copy pointer string
        let params_raw_slice = archived.params_raw.as_slice(); // ZCO: Zero-copy slice reference

        if method == "tools/call" {
            // JIT Opaque Payload Bypass (Parsing Just-in-time)
            let arguments: serde_json::Value =
                serde_json::from_slice(params_raw_slice).unwrap_or(serde_json::json!({}));

            let tool_name = arguments.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let internal_args = arguments
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::json!({}));

            let mut response = IpcMcpResponse {
                success: false,
                result_raw: vec![],
            };

            // Routing d'exécution asynchrone sur les Outils Partagés
            if let Some(tool) = server.static_tools.get(tool_name) {
                match tool.call(internal_args).await {
                    Ok(res) => {
                        response.success = true;
                        response.result_raw = serde_json::to_vec(&res).unwrap_or_default();
                    }
                    Err(e) => {
                        response.result_raw = e.to_string().into_bytes();
                    }
                }
            } else if let Some(resolver) = &server.dynamic_resolver {
                match resolver.call_dynamic_tool(tool_name, internal_args).await {
                    Some(Ok(res)) => {
                        response.success = true;
                        response.result_raw = serde_json::to_vec(&res).unwrap_or_default();
                    }
                    Some(Err(e)) => {
                        response.result_raw = e.to_string().into_bytes();
                    }
                    None => {
                        response.result_raw =
                            "Outil dynamique introuvable".to_string().into_bytes();
                    }
                }
            } else {
                response.result_raw = "Outil cible introuvable".to_string().into_bytes();
            }

            // 2. Sérialisation Zero-Copy en sortie
            let bytes = match rkyv::to_bytes::<_, 256>(&response) {
                Ok(b) => b,
                Err(e) => {
                    error!("Erreur de sérialisation interne rkyv : {:?}", e);
                    continue; // Skip silently to keep IPC engine alive
                }
            };

            let res_len = (bytes.len() as u32).to_le_bytes();
            socket.write_all(&res_len).await?;
            socket.write_all(&bytes).await?;
        }
    }
}
