pub mod ipc_pipeline;
pub mod mcp_types;
pub mod stdio_pipeline;

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Outil compatible Modèle MCP
#[async_trait]
pub trait McpTool: Send + Sync {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn input_schema(&self) -> Value;
    async fn call(&self, arguments: Value) -> Result<Value, anyhow::Error>;
}

/// Interface pour autoriser la résolution dynamique Just-In-Time d'outils
#[async_trait]
pub trait DynamicToolResolver: Send + Sync {
    async fn list_dynamic_tools(&self) -> Vec<serde_json::Value>;
    async fn call_dynamic_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Option<Result<serde_json::Value, anyhow::Error>>;
}

/// Interface pour autoriser la résolution dynamique Just-In-Time de Ressources et Templates MCP
#[async_trait]
pub trait DynamicResourceResolver: Send + Sync {
    async fn list_dynamic_resources(&self) -> Vec<serde_json::Value>;
    async fn list_dynamic_resource_templates(&self) -> Vec<serde_json::Value>;
    async fn read_dynamic_resource(&self, uri: &str) -> Option<Result<String, anyhow::Error>>;
}

/// Orchestrateur des Outils MCP
pub struct SuperMcpServer {
    pub name: String,
    pub version: String,
    pub static_tools: HashMap<String, Arc<dyn McpTool>>,
    pub dynamic_resolver: Option<Arc<dyn DynamicToolResolver>>,
    pub dynamic_resource_resolver: Option<Arc<dyn DynamicResourceResolver>>,
}

impl SuperMcpServer {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            static_tools: HashMap::new(),
            dynamic_resolver: None,
            dynamic_resource_resolver: None,
        }
    }

    pub fn register_tool(&mut self, tool: Arc<dyn McpTool>) {
        self.static_tools.insert(tool.name().to_string(), tool);
    }

    /// Cœur de Domaine Hexagonal Pur (Indépendant du Transport JSON-RPC ou Rkyv)
    pub async fn execute_domain(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, (i32, String)> {
        match method {
            "initialize" => Ok(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {}, "prompts": {}, "resources": {} },
                "serverInfo": {
                    "name": self.name,
                    "version": self.version
                }
            })),
            "tools/list" => {
                let mut tool_list = Vec::new();
                for tool in self.static_tools.values() {
                    tool_list.push(serde_json::json!({
                        "name": tool.name(),
                        "description": tool.description(),
                        "inputSchema": tool.input_schema()
                    }));
                }
                if let Some(resolver) = &self.dynamic_resolver {
                    let dynamic_list = resolver.list_dynamic_tools().await;
                    tool_list.extend(dynamic_list);
                }
                Ok(serde_json::json!({ "tools": tool_list }))
            }
            "prompts/list" => {
                let mut prompt_list = Vec::new();
                for tool in self.static_tools.values() {
                    prompt_list.push(serde_json::json!({
                        "name": tool.name(),
                        "description": tool.description(),
                        "arguments": []
                    }));
                }
                if let Some(resolver) = &self.dynamic_resolver {
                    let dyn_tools = resolver.list_dynamic_tools().await;
                    for dyn_t in dyn_tools {
                        if let (Some(n), Some(d)) = (dyn_t.get("name"), dyn_t.get("description")) {
                            if let (Some(name_str), Some(desc_str)) = (n.as_str(), d.as_str()) {
                                prompt_list.push(serde_json::json!({
                                    "name": name_str,
                                    "description": desc_str,
                                    "arguments": []
                                }));
                            }
                        }
                    }
                }
                Ok(serde_json::json!({ "prompts": prompt_list }))
            }
            "prompts/get" => {
                let params = params.unwrap_or(serde_json::json!({}));
                let prompt_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                Ok(serde_json::json!({
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
                }))
            }
            "resources/list" => {
                let mut resource_list = Vec::new();
                if let Some(resolver) = &self.dynamic_resource_resolver {
                    resource_list = resolver.list_dynamic_resources().await;
                }
                Ok(serde_json::json!({ "resources": resource_list }))
            }
            "resources/templates/list" => {
                let mut template_list = Vec::new();
                if let Some(resolver) = &self.dynamic_resource_resolver {
                    template_list = resolver.list_dynamic_resource_templates().await;
                }
                Ok(serde_json::json!({ "resourceTemplates": template_list }))
            }
            "resources/read" => {
                let params = params.unwrap_or(serde_json::json!({}));
                let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");
                if let Some(resolver) = &self.dynamic_resource_resolver {
                    match resolver.read_dynamic_resource(uri).await {
                        Some(Ok(text)) => Ok(serde_json::json!({
                            "contents": [
                                {
                                    "uri": uri,
                                    "mimeType": "text/markdown",
                                    "text": text
                                }
                            ]
                        })),
                        Some(Err(e)) => Err((-32603, format!("Erreur exécution: {}", e))),
                        None => Err((
                            -32601,
                            format!("Ressource introuvable ou non supportée: {}", uri),
                        )),
                    }
                } else {
                    Err((
                        -32601,
                        "Aucun résolveur de ressources n'est configuré.".to_string(),
                    ))
                }
            }
            "tools/call" => {
                let params = params.unwrap_or(serde_json::json!({}));
                let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                if let Some(tool) = self.static_tools.get(tool_name) {
                    match tool.call(arguments).await {
                        Ok(res) => {
                            let text_val = match res.as_str() {
                                Some(s) => s.to_string(),
                                None => res.to_string(),
                            };
                            Ok(serde_json::json!({
                                "content": [{ "type": "text", "text": text_val }]
                            }))
                        }
                        Err(e) => Err((-32603, format!("Erreur exécution: {}", e))),
                    }
                } else if let Some(resolver) = &self.dynamic_resolver {
                    match resolver.call_dynamic_tool(tool_name, arguments).await {
                        Some(Ok(res)) => {
                            let text_val = match res.as_str() {
                                Some(s) => s.to_string(),
                                None => res.to_string(),
                            };
                            Ok(serde_json::json!({
                                "content": [{ "type": "text", "text": text_val }]
                            }))
                        }
                        Some(Err(e)) => Err((-32603, format!("Erreur dynamique: {}", e))),
                        None => Err((
                            -32601,
                            format!("Outil dynamique introuvable: {}", tool_name),
                        )),
                    }
                } else {
                    Err((-32601, format!("Outil introuvable: {}", tool_name)))
                }
            }
            _ => Err((-32601, "Method not found".to_string())),
        }
    }
}
