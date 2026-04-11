use crate::mcp_types::{McpRequest, McpResponse};
use crate::SuperMcpServer;
use serde_json::json;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

pub async fn start_mcp_pipeline(server: Arc<SuperMcpServer>) {
    // Canaux de communication entre Acteurs
    let (tx_in, mut rx_in) = mpsc::channel::<String>(100);
    let (tx_out, mut rx_out) = mpsc::channel::<McpResponse>(100);

    // 1. ACTEUR INGESTION (Stdin -> tx_in)
    // S'endort physiquement a 0% CPU si aucune ligne n'est soumise.
    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();

        while let Ok(Some(line)) = reader.next_line().await {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if tx_in.send(line.to_string()).await.is_err() {
                break; // Le canal est ferme, on sort
            }
        }
        debug!("Acteur Ingestion (Stdin) terminé.");
    });

    // 2. ACTEUR ÉMISSION (rx_out -> Stdout)
    // Seul garant de l'écriture Thread-Safe sur la sortie standard.
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

    // 3. ACTEUR ORCHESTRATEUR (Cœur de Dispatch)
    info!(
        "🚀 Serveur Stdio '{}' En Écoute. [Architecture Actor MPSC Active]",
        server.name
    );

    // Pour des raisons de confort MCP, on pré-crée le Sender Clone
    let orchestrator_tx_out = tx_out.clone();

    while let Some(msg) = rx_in.recv().await {
        // Tentative de Parsing Strict
        let request: McpRequest = match serde_json::from_str(&msg) {
            Ok(r) => r,
            Err(e) => {
                error!(
                    "Parsing échoué (Requête JSON-RPC invalide ou non reconnue) : {}",
                    e
                );
                // On peut décider de rejeter en renvoyant une erreur au client si un ID était passable,
                // mais si c'est du bruit on l'ignore silencieusement.
                let err_res = McpResponse::error(json!(null), -32700, "Parse error");
                let _ = orchestrator_tx_out.send(err_res).await;
                continue;
            }
        };

        let server_ref = Arc::clone(&server);
        let resp_tx = orchestrator_tx_out.clone();

        // Spawn d'une micro-tâche Tokio pour ne JAMAIS bloquer l'ingestion Core
        tokio::spawn(async move {
            let res = handle_request(server_ref, request).await;
            if let Some(valid_res) = res {
                let _ = resp_tx.send(valid_res).await;
            }
        });
    }

    info!("🛑 Serveur arrêté.");
}

/// Dispatcheur central de requêtes JSON-RPC
async fn handle_request(server: Arc<SuperMcpServer>, req: McpRequest) -> Option<McpResponse> {
    let id = req.id.unwrap_or(json!(null));

    match req.method.as_str() {
        "initialize" => Some(McpResponse::success(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {}, "prompts": {}, "resources": {} },
                "serverInfo": {
                    "name": server.name,
                    "version": server.version
                }
            }),
        )),
        "notifications/initialized" => {
            None // One-way notification
        }
        "tools/list" => {
            let mut tool_list = Vec::new();
            for tool in server.static_tools.values() {
                tool_list.push(json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "inputSchema": tool.input_schema()
                }));
            }
            if let Some(resolver) = &server.dynamic_resolver {
                let dynamic_list = resolver.list_dynamic_tools().await;
                tool_list.extend(dynamic_list);
            }
            Some(McpResponse::success(id, json!({ "tools": tool_list })))
        }
        "prompts/list" => {
            let mut prompt_list = Vec::new();
            for tool in server.static_tools.values() {
                prompt_list.push(json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "arguments": []
                }));
            }

            if let Some(resolver) = &server.dynamic_resolver {
                let dyn_tools = resolver.list_dynamic_tools().await;
                for dyn_t in dyn_tools {
                    if let (Some(n), Some(d)) = (dyn_t.get("name"), dyn_t.get("description")) {
                        if let (Some(name_str), Some(desc_str)) = (n.as_str(), d.as_str()) {
                            prompt_list.push(json!({
                                "name": name_str,
                                "description": desc_str,
                                "arguments": []
                            }));
                        }
                    }
                }
            }
            Some(McpResponse::success(id, json!({ "prompts": prompt_list })))
        }
        "prompts/get" => {
            let params = req.params.unwrap_or(json!({}));
            let prompt_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            Some(McpResponse::success(
                id,
                json!({
                    "description": format!("Exécuter l'action {}", prompt_name),
                    "messages": [
                        {
                            "role": "user",
                            "content": {
                                "type": "text",
                                "text": format!("S'il te plaît, déploie et utilise l'outil MCP '{}' maintenant sérieusement.", prompt_name)
                            }
                        }
                    ]
                }),
            ))
        }
        "resources/list" => {
            let mut resource_list = Vec::new();
            if let Some(resolver) = &server.dynamic_resource_resolver {
                resource_list = resolver.list_dynamic_resources().await;
            }
            Some(McpResponse::success(
                id,
                json!({ "resources": resource_list }),
            ))
        }
        "resources/templates/list" => {
            let mut template_list = Vec::new();
            if let Some(resolver) = &server.dynamic_resource_resolver {
                template_list = resolver.list_dynamic_resource_templates().await;
            }
            Some(McpResponse::success(
                id,
                json!({ "resourceTemplates": template_list }),
            ))
        }
        "resources/read" => {
            let params = req.params.unwrap_or(json!({}));
            let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            if let Some(resolver) = &server.dynamic_resource_resolver {
                match resolver.read_dynamic_resource(uri).await {
                    Some(Ok(text)) => Some(McpResponse::success(
                        id,
                        json!({
                            "contents": [
                                {
                                    "uri": uri,
                                    "mimeType": "text/markdown",
                                    "text": text
                                }
                            ]
                        }),
                    )),
                    Some(Err(e)) => {
                        error!(
                            "Erreur asynchrone lors de la lecture de la ressource '{}': {}",
                            uri, e
                        );
                        Some(McpResponse::error(
                            id,
                            -32603,
                            &format!("Erreur execution: {}", e),
                        ))
                    }
                    None => Some(McpResponse::error(
                        id,
                        -32601,
                        &format!("Ressource introuvable ou non supportée: {}", uri),
                    )),
                }
            } else {
                Some(McpResponse::error(
                    id,
                    -32601,
                    "Aucun résolveur de ressources n'est configuré.",
                ))
            }
        }
        "tools/call" => {
            let params = req.params.unwrap_or(json!({}));
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            if let Some(tool) = server.static_tools.get(tool_name) {
                // Exécution interne (Asynchrone statique)
                match tool.call(arguments).await {
                    Ok(res) => {
                        let text_val = match res.as_str() {
                            Some(s) => s.to_string(),
                            None => res.to_string(),
                        };
                        Some(McpResponse::success(
                            id,
                            json!({
                                "content": [{ "type": "text", "text": text_val }]
                            }),
                        ))
                    }
                    Err(e) => {
                        error!("Erreur asynchrone dans l'outil: {}", e);
                        Some(McpResponse::error(
                            id,
                            -32603,
                            &format!("Erreur execution: {}", e),
                        ))
                    }
                }
            } else if let Some(resolver) = &server.dynamic_resolver {
                // Dispatch Dynamique
                match resolver.call_dynamic_tool(tool_name, arguments).await {
                    Some(Ok(res)) => {
                        let text_val = match res.as_str() {
                            Some(s) => s.to_string(),
                            None => res.to_string(),
                        };
                        Some(McpResponse::success(
                            id,
                            json!({
                                "content": [{ "type": "text", "text": text_val }]
                            }),
                        ))
                    }
                    Some(Err(e)) => {
                        error!("Erreur dynamique dans l'outil: {}", e);
                        Some(McpResponse::error(
                            id,
                            -32603,
                            &format!("Erreur execution dynamique: {}", e),
                        ))
                    }
                    None => Some(McpResponse::error(
                        id,
                        -32601,
                        &format!("Outil dynamique introuvable: {}", tool_name),
                    )),
                }
            } else {
                Some(McpResponse::error(
                    id,
                    -32601,
                    &format!("Outil introuvable: {}", tool_name),
                ))
            }
        }
        _ => Some(McpResponse::error(id, -32601, "Method not found")),
    }
}
