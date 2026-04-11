use crate::vampire_lord::notebook_api::NotebookApi;
use r2d2_browser::SovereignBrowser;
use r2d2_mcp_core::McpTool;
use serde_json::{json, Value};
use tracing::info;

pub struct PurgeUntitledTool;

#[async_trait::async_trait]
impl McpTool for PurgeUntitledTool {
    fn name(&self) -> String {
        "mcp_vampire_lord_purge_untitled".to_string()
    }

    fn description(&self) -> String {
        "Vampire Lord: Purge definitivement tous les carnets (notebooks) contenant 'untitled notebook' ou 'carnet sans titre'.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn call(&self, _args: Value) -> Result<Value, anyhow::Error> {
        let browser = SovereignBrowser::connect("Chrome_GOOGLE").await?;

        // Attendre la stabilisation CDP
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let tab = r2d2_browser::SovereignBrowser::get_or_new_tab(&browser, "notebooklm").await?;
        let api = NotebookApi::new(tab).await;

        let all_nbs = api.list_notebooks().await?;
        let count_before = all_nbs.len();

        let deleted_count = api.purge_untitled_notebooks().await?;

        let all_nbs_after = api.list_notebooks().await?;
        let count_after = all_nbs_after.len();

        info!(
            "Purge terminée. {} / {} supprimés.",
            deleted_count, count_before
        );

        Ok(json!({
            "status": "success",
            "message": format!("Purge RPC exécutée. {} carnets indésirables éliminés. Nombre de carnets total ramené de {} à {}.", deleted_count, count_before, count_after),
            "deleted": deleted_count
        }))
    }
}
