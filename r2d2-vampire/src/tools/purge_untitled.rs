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
        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<Value> {
            let browser = SovereignBrowser::connect("Chrome_GOOGLE")?;

            // Attendre la stabilisation CDP
            std::thread::sleep(std::time::Duration::from_millis(500));

            let tabs = browser.get_tabs().lock().unwrap().clone();
            let legit_tab = tabs.iter().find(|t| t.get_url().contains("notebooklm.google.com"));

            let tab = if let Some(t) = legit_tab {
                t.clone()
            } else {
                r2d2_browser::SovereignBrowser::get_or_new_tab(&browser, "notebooklm.google.com")?
            };

            let api = NotebookApi::new(tab);

            let all_nbs = api.list_notebooks()?;
            let count_before = all_nbs.len();

            let deleted_count = api.purge_untitled_notebooks()?;

            let all_nbs_after = api.list_notebooks()?;
            let count_after = all_nbs_after.len();

            info!("Purge terminée. {} / {} supprimés.", deleted_count, count_before);

            Ok(json!({
                "status": "success",
                "message": format!("Purge RPC exécutée. {} carnets indésirables éliminés. Nombre de carnets total ramené de {} à {}.", deleted_count, count_before, count_after),
                "deleted": deleted_count
            }))
        }).await??;

        Ok(result)
    }
}
