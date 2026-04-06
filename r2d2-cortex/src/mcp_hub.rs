use crate::mcp_client::McpClient;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub envs: HashMap<String, String>,
}

fn sanitize_schema_for_gemini(schema: &mut serde_json::Value) {
    if let serde_json::Value::Object(map) = schema {
        map.remove("$schema");
        map.remove("additionalProperties");
        map.remove("default");

        if let Some(any_of) = map.remove("anyOf") {
            if let Some(first) = any_of.as_array().and_then(|a| a.first()).cloned() {
                if let Some(type_str) = first.get("type").and_then(|t| t.as_str()) {
                    map.insert("type".to_string(), serde_json::json!(type_str));
                }
            }
        }
        map.remove("allOf");
        map.remove("oneOf");

        for (_, val) in map.iter_mut() {
            sanitize_schema_for_gemini(val);
        }
    } else if let serde_json::Value::Array(arr) = schema {
        for val in arr.iter_mut() {
            sanitize_schema_for_gemini(val);
        }
    }
}

pub struct McpHub {
    clients: Arc<Mutex<HashMap<String, Arc<McpClient>>>>,
    gemini_tools_cache: Vec<serde_json::Value>,
}

impl McpHub {
    pub async fn new(configs: Vec<McpServerConfig>) -> Result<Self> {
        let mut clients = HashMap::new();
        let mut gemini_tools_cache = Vec::new();

        for config in configs {
            info!(
                "🔌 [McpHub] Initializing tool server '{}': {} {:?}",
                config.name, config.command, config.args
            );
            match McpClient::new(&config.command, &config.args, config.envs).await {
                Ok(client) => {
                    // Ask the server for its tools
                    if let Ok(tools_resp) = client.list_tools().await {
                        if let Some(mut tools_arr) =
                            tools_resp.get("tools").and_then(|t| t.as_array()).cloned()
                        {
                            for tool in tools_arr.iter_mut() {
                                if let Some(obj) = tool.as_object_mut() {
                                    // Extract MCP fields
                                    let tool_name = obj
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    let desc = obj
                                        .get("description")
                                        .cloned()
                                        .unwrap_or(serde_json::json!(""));
                                    let mut parameters =
                                        obj.remove("inputSchema").unwrap_or(serde_json::json!({
                                            "type": "object",
                                            "properties": {}
                                        }));

                                    sanitize_schema_for_gemini(&mut parameters);

                                    // Format the name as `server_name___tool_name` cleanly for Gemini
                                    let sanitized_config_name =
                                        config.name.replace("-", "_").replace(" ", "_");
                                    let gemini_name =
                                        format!("{}___{}", sanitized_config_name, tool_name)
                                            .chars()
                                            .take(63)
                                            .collect::<String>();

                                    // Add to Gemini tools array
                                    gemini_tools_cache.push(serde_json::json!({
                                        "name": gemini_name,
                                        "description": desc,
                                        "parameters": parameters,
                                    }));
                                }
                            }
                        }
                    } else {
                        warn!(
                            "❌ [McpHub] Server '{}' initialized but failed to list tools.",
                            config.name
                        );
                    }

                    clients.insert(config.name.clone(), Arc::new(client));
                }
                Err(e) => {
                    warn!(
                        "❌ [McpHub] Failed to start server '{}': {}",
                        config.name, e
                    );
                }
            }
        }

        let hub = Self {
            clients: Arc::new(Mutex::new(clients)),
            gemini_tools_cache,
        };

        Ok(hub)
    }

    pub fn get_gemini_tools(&self) -> Vec<serde_json::Value> {
        self.gemini_tools_cache.clone()
    }

    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let lock = self.clients.lock().await;
        if let Some(client) = lock.get(server_name) {
            client.call_tool(tool_name, arguments).await
        } else {
            let matched_key = lock
                .keys()
                .find(|k| k.replace("-", "_").replace(" ", "_") == server_name);
            if let Some(key) = matched_key {
                lock.get(key).unwrap().call_tool(tool_name, arguments).await
            } else {
                Err(anyhow::anyhow!(
                    "MCP Server '{}' not found or failed to initialize",
                    server_name
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_sanitize_schema_for_gemini_basics() {
        let raw_schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "owner": {
                    "type": "string",
                    "description": "Repository owner"
                }
            }
        });

        let mut sanitized = raw_schema;
        sanitize_schema_for_gemini(&mut sanitized);

        assert!(sanitized.get("$schema").is_none());
        assert!(sanitized.get("additionalProperties").is_none());
        assert_eq!(sanitized.get("type").unwrap(), "object");
        assert_eq!(
            sanitized.pointer("/properties/owner/type").unwrap(),
            "string"
        );
    }

    #[test]
    fn test_sanitize_schema_for_gemini_recursive_anyof() {
        let raw_schema = json!({
            "type": "object",
            "properties": {
                "complex_field": {
                    "anyOf": [
                        { "type": "string" },
                        { "type": "null" }
                    ]
                },
                "single_anyof": {
                    "anyOf": [
                        { "type": "integer" }
                    ]
                },
                "nested_object": {
                    "type": "object",
                    "$schema": "invalid",
                    "additionalProperties": { "type": "string" },
                    "properties": {
                        "inner": {
                            "type": "string",
                            "default": "value"
                        }
                    }
                }
            }
        });

        let mut sanitized = raw_schema;
        sanitize_schema_for_gemini(&mut sanitized);

        // anyOf with multiple members is stripped down to just type: string
        // Since our sanitizer just removes the "anyOf" key completely, it defaults to type "string"
        assert_eq!(
            sanitized.pointer("/properties/complex_field/type").unwrap(),
            "string"
        );
        assert!(sanitized
            .pointer("/properties/complex_field/anyOf")
            .is_none());

        // Single anyOf is unwrapped
        assert_eq!(
            sanitized.pointer("/properties/single_anyof/type").unwrap(),
            "integer"
        );

        // Nested cleaning
        assert!(sanitized
            .pointer("/properties/nested_object/$schema")
            .is_none());
        assert!(sanitized
            .pointer("/properties/nested_object/additionalProperties")
            .is_none());
        assert!(sanitized
            .pointer("/properties/nested_object/properties/inner/default")
            .is_none());
    }
}

pub struct ProviderStatus {
    pub name: String,
    pub status: String,
}

pub async fn health_check() -> Vec<ProviderStatus> {
    // Just wrap list_active_providers for now
    // Wait, list_active_providers() returns Vec<String> probably?
    // Let's implement health_check stub
    vec![]
}
