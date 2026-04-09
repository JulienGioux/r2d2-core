use r2d2_cortex::mcp_client::McpClient;
use r2d2_registry::RawDataArtifact;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::mpsc::SyncSender;
use std::time::Duration;
use tracing::{error, info, warn};

pub struct VampireWorker {
    pub endpoint: String,
}

impl VampireWorker {
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
        }
    }

    /// Boucle de moissonnage réseau.
    /// Utilise la contre-pression : si tx.send() bloque, le thread de l'OS
    /// s'endort sans consommer de CPU (Bare-Metal Backpressure).
    pub fn harvest_loop(&mut self, tx: SyncSender<RawDataArtifact>) {
        info!("🦇 [VAMPIRE] Initialisation. Démarrage du Micro-Runtime Tokio Ingress...");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            self.run_async_harvest(tx).await;
        });

        info!("🦇 [VAMPIRE] Thread et Micro-Runtime désintégrés proprement.");
    }

    async fn run_async_harvest(&self, tx: SyncSender<RawDataArtifact>) {
        info!("🦇 [VAMPIRE] Connexion à NotebookLM Proxy via MCP...");

        let envs = HashMap::new();
        // Optionnel : Passer le port si necessaire

        let client = match McpClient::new(
            "node",
            &["tools/mcp-consultant-bridge/index.js".to_string()],
            envs,
        )
        .await
        {
            Ok(c) => c,
            Err(e) => {
                error!("🦇 [VAMPIRE] Impossible de booter le MCP Node Proxy: {}", e);
                return;
            }
        };

        info!("🦇 [VAMPIRE] Liaison MCP établie ! Lancement de la Combinatoire RAG.");

        // Générateur Combinatoire
        let concepts = [
            "Pattern Actor",
            "Gestion de la Pile",
            "Zéro-Allocation",
            "Backpressure",
            "Sockets TCP non-bloquantes",
        ];
        let contextes = [
            "en Rust pur SANS crate Async",
            "pour un Cerveau CUDA C++",
            "en environnement Bare-Metal Strict",
            "pour une Forteresse Air-Gapped",
            "lorsqu'on cible une latence de 5us",
        ];
        let taches = [
            "Écris l'implémentation de la structure minimale stricte",
            "Explique les pannes possibles et la sécurité",
            "Conçois l'architecture de données de bout en bout",
            "Produis une critique des approches modernes vs cette approche",
        ];

        let mut chunk_id = 0;
        let limit = 1000;

        // Bouclage itératif infini (bridé par limit)
        for t in taches {
            for c in concepts {
                for ctx in contextes {
                    chunk_id += 1;
                    if chunk_id > limit {
                        info!("🦇 [VAMPIRE] Limite de {} échantillons atteinte ! Extinction de la matrice.", limit);
                        return;
                    }

                    let prompt = format!("{} sur le sujet de [{}] appliqués [{}]. Réponse exigée : pointue, formatée, et destinée à des experts Rust/Souveraineté (Zero-Bloat).", t, c, ctx);
                    info!(
                        "🦇 [VAMPIRE] ({}/{}) Appel RAG envoyé: {}",
                        chunk_id, limit, prompt
                    );

                    let args = serde_json::json!({
                        "prompt": prompt
                    });

                    match client.call_tool("ask_rustymaster", args).await {
                        Ok(response) => {
                            if let Some(text_content) = response
                                .as_array()
                                .and_then(|arr| arr.first())
                                .and_then(|obj| obj.get("text"))
                                .and_then(|t| t.as_str())
                            {
                                let artifact = RawDataArtifact {
                                    payload: Cow::Owned(text_content.to_string()),
                                    source_hash: format!(
                                        "RAG_{}_{}_{}",
                                        t.len(),
                                        c.len(),
                                        ctx.len()
                                    ),
                                };

                                // Transmission sécurisée par Contre-Pression
                                // On isole l'appel bloquant (MPSC) pour ne pas figer le runtime async (Anti-Starvation)
                                let tx_clone = tx.clone();
                                let send_result =
                                    tokio::task::spawn_blocking(move || tx_clone.send(artifact))
                                        .await;

                                match send_result {
                                    Ok(Ok(_)) => info!(
                                        "🦇 [VAMPIRE] Réponse digérée et propulsée dans le Canal."
                                    ),
                                    Ok(Err(e)) => {
                                        warn!("⚠️ [VAMPIRE] Rupture du Canal Sync : L'Orchestrateur ordonne la fin. Détail: {}", e);
                                        return;
                                    }
                                    Err(e) => {
                                        error!("❌ [VAMPIRE] Le thread bloqueur de Tokio a paniqué: {}", e);
                                        return;
                                    }
                                }
                            } else {
                                warn!("⚠️ [VAMPIRE] Format MCP Inattendu ! Content absent.");
                            }
                        }
                        Err(e) => {
                            error!("❌ [VAMPIRE] Erreur RPC lors du tir RAG : {}", e);
                            tokio::time::sleep(Duration::from_secs(5)).await; // Temporisation punitive
                        }
                    }

                    // Respiration anti-spam
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
}
