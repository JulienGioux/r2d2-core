use async_trait::async_trait;
use r2d2_mcp_core::{stdio_pipeline::start_mcp_pipeline, McpTool, SuperMcpServer};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;
mod tools;

struct SurfWebTool;

#[async_trait]
impl McpTool for SurfWebTool {
    fn name(&self) -> String {
        "surf_web".to_string()
    }

    fn description(&self) -> String {
        "Navigue vers une URL et extrait le texte brut de la page.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "L'URL de la page web à consulter"
                }
            },
            "required": ["url"]
        })
    }

    async fn call(&self, arguments: Value) -> Result<Value, anyhow::Error> {
        let url = arguments
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Argument 'url' manquant ou invalide"))?
            .to_string();

        info!("SurfWeb: Navigation vers {}", url);

        let browser = r2d2_browser::SovereignBrowser::connect("surfer-profile").await?;
        let tab = browser.new_page(&url).await?;
        tab.wait_for_navigation().await?;

        let js = "document.body.innerText || document.body.textContent";
        let eval = tab.evaluate(js).await?;

        let result = if let Some(serde_json::Value::String(s)) = eval.value() {
            s.clone()
        } else {
            "Aucun texte trouvé".to_string()
        };

        // Isolation Sémantique (JSONAI V3.0) pour stopper le Prompt Injection
        Ok(json!({
            "belief_state": { "is_fact": false },
            "untrusted_web_content": result
        }))
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .init();

    let mut server = SuperMcpServer::new("r2d2-surfer", "1.0.0");
    server.register_tool(Arc::new(SurfWebTool));
    server.register_tool(Arc::new(tools::authenticate::AuthenticateProviderTool));

    info!("🚀 Démarrage du Serveur MCP Web Surfer");
    start_mcp_pipeline(Arc::new(server)).await;
}
