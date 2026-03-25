use anyhow::Result;
use r2d2_mcp::McpGateway;
use serde_json::json;
use std::env;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tracing::{error, info, Level};

/// Déstructure un payload stérile (String) en une requête JSON-RPC valide (id, method, args).
/// Retourne None si la ligne n'est pas une commande exécutable (ex: Un simple ACK).
pub fn parse_mcp_request(line: &str) -> Option<(serde_json::Value, String, serde_json::Value)> {
    let req: serde_json::Value = serde_json::from_str(line).ok()?;

    // Si la requête MCP n'a pas de méthode (Ack d'une réponse passée ou malformée), on l'ignore.
    req.get("method")?;

    let id = req.get("id").cloned().unwrap_or(serde_json::json!(null));
    let method = req.get("method").and_then(|m| m.as_str())?.to_string();

    Some((id, method, req))
}

/// Boucle principale : Lit le Stdin ligne par ligne (Non-bloquant), analyse le JSON-RPC,
/// requiert au Gateway de faire le travail, puis crache la réponse sérialisée sur Stdout.
pub async fn run_native_mcp_loop(gateway: Arc<Mutex<McpGateway>>) -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut reader = tokio::io::BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();

    while let Ok(Some(line)) = reader.next_line().await {
        let (id, method, req) = match parse_mcp_request(&line) {
            Some(parsed) => parsed,
            None => continue,
        };

        let result = match method.as_str() {
            "initialize" => {
                info!("Handshake MCP d'initialisation reçu !");
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "serverInfo": {
                        "name": "R2D2-Gateway",
                        "version": "8.2.0"
                    }
                })
            }
            "tools/list" => {
                let gw = gateway.lock().await;
                gw.registry.export_mcp_format()
            }
            "tools/call" => {
                let default_params = json!({});
                let params = req.get("params").unwrap_or(&default_params);
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_default();

                let gw = gateway.lock().await;

                // 1. Audit Sémantique et HITL (Le Passeport)
                match gw.proxy.audit_tool_call(name, "ExternalAgent", &args).await {
                    Ok(true) => {
                        // L'exécution est autorisée !
                        match name {
                            "anchor_thought" => {
                                let content =
                                    args.get("content").and_then(|c| c.as_str()).unwrap_or("");
                                let agent_name = args
                                    .get("agent_name")
                                    .and_then(|a| a.as_str())
                                    .unwrap_or("Unknown");
                                match gw
                                    .ingest_thought("McpTool", agent_name, content.to_string())
                                    .await
                                {
                                    Ok(frag_id) => {
                                        json!({ "content": [{ "type": "text", "text": format!("Pensée ingérée avec succès : {}", frag_id) }] })
                                    }
                                    Err(e) => {
                                        json!({ "content": [{ "type": "text", "text": format!("Échec du typestate : {}", e) }], "isError": true })
                                    }
                                }
                            }
                            "recall_memory" => {
                                let query =
                                    args.get("query").and_then(|q| q.as_str()).unwrap_or("");
                                match gw.search_memory(query).await {
                                    Ok(mem_result) => {
                                        json!({ "content": [{ "type": "text", "text": mem_result }] })
                                    }
                                    Err(e) => {
                                        json!({ "content": [{ "type": "text", "text": format!("Erreur système Blackboard : {}", e) }], "isError": true })
                                    }
                                }
                            }
                            "delete_memory_cluster" => {
                                // Simulation d'une exécution de suppression après passage du HITL
                                let cluster_id = args
                                    .get("cluster_id")
                                    .and_then(|id| id.as_str())
                                    .unwrap_or("");
                                json!({ "content": [{ "type": "text", "text": format!("🚨 Cluster R2D2 [{}] systématiquement radié de l'Index PostgreSQL avec succès.", cluster_id) }] })
                            }
                            "ingest_audio" => {
                                let audio_path = args
                                    .get("audio_path")
                                    .and_then(|a| a.as_str())
                                    .unwrap_or("");

                                let cortex =
                                    std::sync::Arc::new(r2d2_cortex::CortexRegistry::new());
                                cortex
                                    .register_agent(Box::new(
                                        r2d2_cortex::models::audio_agent::AudioAgent::new(),
                                    ))
                                    .await;
                                let gateway = r2d2_sensory::gateway::SensoryGateway::new(cortex);

                                let stimulus = r2d2_sensory::stimulus::Stimulus::new(
                                    "mcp-audio-stimulus", // ID technique pour la session
                                    r2d2_sensory::stimulus::StimulusType::Audio,
                                    std::path::PathBuf::from(audio_path),
                                );

                                match gateway.ingest(stimulus).await {
                                    Ok(fragment) => {
                                        // On ingère le fragment (qui contient le retour JSONAI) dans le Blackboard
                                        match gw
                                            .ingest_thought(
                                                "SensoryGateway",
                                                "AudioAgent",
                                                fragment.into_inner().raw_data,
                                            )
                                            .await
                                        {
                                            Ok(fid) => {
                                                json!({ "content": [{ "type": "text", "text": format!("🎧 Audio transcrit. Fragment injecté: {}", fid) }] })
                                            }
                                            Err(e) => {
                                                json!({ "content": [{ "type": "text", "text": format!("Erreur d'ancrage Blackboard: {}", e) }], "isError": true })
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        json!({ "content": [{ "type": "text", "text": format!("Erreur d'ingestion audio: {}", e) }], "isError": true })
                                    }
                                }
                            }
                            "ingest_visual" => {
                                let image_path = args
                                    .get("image_path")
                                    .and_then(|a| a.as_str())
                                    .unwrap_or("");

                                let cortex =
                                    std::sync::Arc::new(r2d2_cortex::CortexRegistry::new());
                                cortex
                                    .register_agent(Box::new(
                                        r2d2_cortex::models::vision_agent::VisionAgentLlava::new(),
                                    ))
                                    .await;
                                cortex.register_agent(Box::new(r2d2_cortex::models::vision_agent_qwen::VisionAgentQwen::new())).await;
                                let gateway = r2d2_sensory::gateway::SensoryGateway::new(cortex);

                                let stimulus = r2d2_sensory::stimulus::Stimulus::new(
                                    "mcp-visual-stimulus",
                                    r2d2_sensory::stimulus::StimulusType::Visual,
                                    std::path::PathBuf::from(image_path),
                                );

                                match gateway.ingest(stimulus).await {
                                    Ok(fragment) => {
                                        match gw
                                            .ingest_thought(
                                                "SensoryGateway",
                                                "VisionAgent",
                                                fragment.into_inner().raw_data,
                                            )
                                            .await
                                        {
                                            Ok(fid) => {
                                                json!({ "content": [{ "type": "text", "text": format!("👁️ Vision processée. Fragment injecté: {}", fid) }] })
                                            }
                                            Err(e) => {
                                                json!({ "content": [{ "type": "text", "text": format!("Erreur d'ancrage Blackboard: {}", e) }], "isError": true })
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        json!({ "content": [{ "type": "text", "text": format!("Erreur d'ingestion visuelle: {}", e) }], "isError": true })
                                    }
                                }
                            }
                            _ => {
                                json!({ "content": [{ "type": "text", "text": "Tool inconnu ou non-mappé." }], "isError": true })
                            }
                        }
                    }
                    Ok(false) => {
                        // Rejet SILENCIEUX au niveau du proxy (Souvent HITL Rejected)
                        json!({ "content": [{ "type": "text", "text": "Accès Refusé. Le Semantic Proxy / HITL a bloqué l'intention malveillante ou non-autorisée." }], "isError": true })
                    }
                    Err(e) => {
                        // Erreur technique de validation paradoxale
                        json!({ "content": [{ "type": "text", "text": format!("Violation Axiomatique: {}", e) }], "isError": true })
                    }
                }
            }
            "ping" => json!({}),
            _ => continue, // Ignore other methods natively (notifications, cancel)
        };

        // Émission stricte vers STDOUT pour le protocole JSON-RPC
        let response = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        });

        let mut out = serde_json::to_string(&response).unwrap();
        out.push('\n');
        if let Err(e) = stdout.write_all(out.as_bytes()).await {
            error!("Erreur STDOUT critique: {}", e);
            break;
        }
        let _ = stdout.flush().await;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr) // Crucial: STDERR pour ne pas polluer STDOUT
        .with_ansi(false)
        .with_max_level(Level::INFO)
        .init();

    info!("Démarrage R2D2 MCP Gateway (Brique 9 - Native Stdio Mode)...");

    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://r2d2_admin:secure_r2d2_password_local@localhost:5433/r2d2_blackboard"
            .to_string()
    });

    let gateway = McpGateway::new(&db_url).await?;
    let gateway_arc = Arc::new(Mutex::new(gateway));

    info!("✅ Serveur Natif MCP (JSON-RPC) opérationnel. Écoute sur Stdin.");

    if let Err(e) = run_native_mcp_loop(gateway_arc).await {
        error!("Crash sévère de la boucle MCP : {}", e);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_valid_mcp_initialize() {
        let payload = r#"{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "Claude Desktop", "version": "0.1"}}}"#;

        let parsed = parse_mcp_request(payload);
        assert!(parsed.is_some());

        let (id, method, req) = parsed.unwrap();
        assert_eq!(id, json!(1));
        assert_eq!(method, "initialize");
        assert!(req.get("params").is_some());
    }

    #[test]
    fn test_parse_valid_mcp_tool_call() {
        let payload = r#"{"jsonrpc": "2.0", "id": 42, "method": "tools/call", "params": {"name": "anchor_thought", "arguments": {"content": "Hello World", "agent_name": "Test"}}}"#;

        let parsed = parse_mcp_request(payload);
        assert!(parsed.is_some());

        let (id, method, req) = parsed.unwrap();
        assert_eq!(id, json!(42));
        assert_eq!(method, "tools/call");
        assert_eq!(
            req.get("params")
                .unwrap()
                .get("name")
                .unwrap()
                .as_str()
                .unwrap(),
            "anchor_thought"
        );
    }

    #[test]
    fn test_ignore_ack_notification() {
        // Les envois de ACK sans méthode doivent être ignorés.
        let payload = r#"{"jsonrpc": "2.0", "id": 1, "result": {}}"#;
        let parsed = parse_mcp_request(payload);
        assert!(parsed.is_none());
    }

    #[test]
    fn test_ignore_malformed_json() {
        let payload = r#"{"jsonrpc": "2.0", "id: 1}"#;
        let parsed = parse_mcp_request(payload);
        assert!(parsed.is_none());
    }
}
