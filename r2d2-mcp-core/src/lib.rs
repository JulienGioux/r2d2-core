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

/// Orchestrateur des Outils MCP
pub struct SuperMcpServer {
    pub name: String,
    pub version: String,
    pub static_tools: HashMap<String, Arc<dyn McpTool>>,
    pub dynamic_resolver: Option<Arc<dyn DynamicToolResolver>>,
}

impl SuperMcpServer {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            static_tools: HashMap::new(),
            dynamic_resolver: None,
        }
    }

    pub fn register_tool(&mut self, tool: Arc<dyn McpTool>) {
        self.static_tools.insert(tool.name().to_string(), tool);
    }
}
