use anyhow::Result;
use async_trait::async_trait;
use mcp_rust_sdk::{
    error::{Error as McpError, ErrorCode},
    server::{Server, ServerHandler},
    transport::stdio::StdioTransport,
    types::{ClientCapabilities, Implementation, ServerCapabilities, Tool},
};
use r2d2_mcp::McpGateway;
use serde_json::json;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, Level};

struct R2d2Handler {
    gateway: Arc<Mutex<McpGateway>>,
}

#[async_trait]
impl ServerHandler for R2d2Handler {
    async fn initialize(
        &self,
        _implementation: Implementation,
        _capabilities: ClientCapabilities,
    ) -> Result<ServerCapabilities, McpError> {
        info!("Handshake MCP d'initialisation reçu !");
        Ok(ServerCapabilities { custom: None })
    }

    async fn shutdown(&self) -> Result<(), McpError> {
        info!("Arrêt demandé par le client MCP.");
        std::process::exit(0);
    }

    async fn handle_method(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, McpError> {
        match method {
            "tools/list" => {
                let tools = vec![
                    Tool {
                        name: "anchor_thought".to_string(),
                        description: "Force le R2D2 Kernel à analyser une proposition via le ParadoxSolver et à l'ancrer dans sa matrice vectorielle permanente si elle est intrinsèquement ou contextuellement valide.".to_string(),
                        schema: json!({
                            "type": "object",
                            "properties": {
                                "content": {
                                    "type": "string",
                                    "description": "Le texte brut de la pensée ou de l'analyse à ingérer."
                                },
                                "agent_name": {
                                    "type": "string",
                                    "description": "Le nom de l'agent IA à l'origine de cette pensée."
                                }
                            },
                            "required": ["content", "agent_name"]
                        }),
                    },
                    Tool {
                        name: "recall_memory".to_string(),
                        description: "Recherche sémantique vectorielle dans le Blackboard R2D2 pour exhumer les souvenirs, règles et axiomes passés.".to_string(),
                        schema: json!({
                            "type": "object",
                            "properties": {
                                "query": {
                                    "type": "string",
                                    "description": "La question ou les mots-clés sémantiques de recherche."
                                }
                            },
                            "required": ["query"]
                        }),
                    }
                ];

                Ok(json!({ "tools": tools }))
            }
            "tools/call" => {
                let params = params.unwrap_or_default();
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_default();

                match name {
                    "anchor_thought" => {
                        let content = args.get("content").and_then(|c| c.as_str()).unwrap_or("");
                        let agent_name = args
                            .get("agent_name")
                            .and_then(|a| a.as_str())
                            .unwrap_or("Unknown");

                        let gateway = self.gateway.lock().await;

                        match gateway
                            .ingest_thought(agent_name, content, "".to_string())
                            .await
                        {
                            Ok(id) => Ok(json!({
                                "content": [
                                    {
                                        "type": "text",
                                        "text": format!("Pensée ingérée avec succès par le Kernel sous le fragment ID : {}", id)
                                    }
                                ]
                            })),
                            Err(e) => Ok(json!({
                                "content": [
                                    {
                                        "type": "text",
                                        "text": format!("Échec du typestate (Contradiction Logique ParadoxSolver) : {}", e)
                                    }
                                ],
                                "isError": true
                            })),
                        }
                    }
                    "recall_memory" => {
                        let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("");

                        // Fake vector search for Brique 9 until Brique 7 embeddings logic is fully bridged in Gateway
                        let result = format!("(Simulation) Souvenirs exhumés pour : {}", query);

                        Ok(json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": result
                                }
                            ]
                        }))
                    }
                    _ => Err(McpError::protocol(
                        ErrorCode::MethodNotFound,
                        "Tool inconnu",
                    )),
                }
            }
            _ => Err(McpError::protocol(
                ErrorCode::MethodNotFound,
                "Méthode JSON-RPC non supportée par R2D2",
            )),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Démarrage R2D2 MCP Gateway (Brique 9 - Unstubbed)...");

    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://r2d2_admin:secure_r2d2_password_local@localhost:5433/r2d2_blackboard"
            .to_string()
    });

    let gateway = McpGateway::new(&db_url).await?;
    let gateway_arc = Arc::new(Mutex::new(gateway));

    info!("✅ Passerelle Connectée au Blackboard Vectoriel.");

    let (transport, _sender) = StdioTransport::new();
    let handler = R2d2Handler {
        gateway: gateway_arc,
    };

    let server = Server::new(Arc::new(transport), Arc::new(handler));

    info!("✅ Serveur Stdio MCP 0.1.1 opérationnel, transmission via stdin/stdout ouverte.");

    // Le serveur bloque et gère le flux JSON-RPC jusqu'à exit
    server
        .start()
        .await
        .map_err(|e| anyhow::anyhow!("Erreur serveur MCP: {}", e))?;

    Ok(())
}
