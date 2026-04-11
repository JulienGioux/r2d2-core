use async_trait::async_trait;
use r2d2_vampire::core::{McpTool, SuperMcpServer};
use serde_json::{json, Value};
use std::sync::Arc;

struct MockTool;

#[async_trait]
impl McpTool for MockTool {
    fn name(&self) -> String {
        "mock_tool".to_string()
    }
    fn description(&self) -> String {
        "A mock tool for integration testing".to_string()
    }
    fn input_schema(&self) -> Value {
        json!({"type": "object"})
    }
    async fn call(&self, _arguments: Value) -> Result<Value, anyhow::Error> {
        Ok(json!("Mission Accomplished"))
    }
}

#[tokio::test]
async fn test_super_server_initialization() {
    let mut server = SuperMcpServer::new("test-server", "1.0.0");
    server.register_tool(Arc::new(MockTool));
    assert_eq!(server.static_tools.len(), 1);
    assert!(server.static_tools.contains_key("mock_tool"));
}
