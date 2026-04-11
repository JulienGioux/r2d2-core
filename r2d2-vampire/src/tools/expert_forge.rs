use crate::core::consultant_store::{ConsultantData, ConsultantStore};
use crate::vampire_lord::notebook_api::NotebookApi;
use async_trait::async_trait;
use r2d2_browser::SovereignBrowser;
use r2d2_mcp_core::McpTool;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{error, info};

pub struct ForgeExpertTool {
    pub store: Arc<ConsultantStore>,
}

#[async_trait]
impl McpTool for ForgeExpertTool {
    fn name(&self) -> String {
        "mcp_vampire_lord_forge_expert".to_string()
    }

    fn description(&self) -> String {
        "Vampire Lord: Forge dynamiquement un nouvel Expert NotebookLM (Création R2D2 + Deep Search) pour l'ajouter au registre consultable.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "topic": { "type": "string", "description": "Sujet ou domaine de l'Expert (ex: 'Cuda 12.5 Rust')" },
                "deep_search_queries": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Liste de requêtes textuelles précises pour alimenter le Deep Search"
                }
            },
            "required": ["topic", "deep_search_queries"]
        })
    }

    async fn call(&self, arguments: Value) -> Result<Value, anyhow::Error> {
        let topic = arguments
            .get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let queries: Vec<String> = arguments
            .get("deep_search_queries")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|i| i.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if topic.is_empty() {
            return Err(anyhow::anyhow!("Topic manquant"));
        }

        info!("🦇 Déclenchement de la Forge Rust Souveraine : {}", topic);

        let topic_clone = topic.to_string();
        let result_url = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            // Pont automatisé RUST (SovereignBrowser prend en charge le WSL transparent)
            let browser = SovereignBrowser::connect("Chrome_GOOGLE")?;
            let tab = r2d2_browser::SovereignBrowser::get_or_new_tab(&browser, "notebooklm")?;
            let api = NotebookApi::new(tab.clone());

            let url = api.create_notebook(&topic_clone)?;
            let uuid = url.split("/notebook/").last().unwrap_or("");
            for q in queries.iter() {
                if let Err(e) = api.add_deep_search_source(uuid, q) {
                    error!("Avertissement: Échec Deep Search pour '{}': {}", q, e);
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            }

            let _ = tab.close_target();
            Ok(url)
        })
        .await??;

        // Auto-Indexation
        {
            let mut guard = self.store.data.write().unwrap();
            let snake_name = topic.to_lowercase().replace(" ", "_");
            guard.insert(
                snake_name.clone(),
                ConsultantData {
                    url: Some(result_url.clone()),
                    enabled: true,
                    variables: None,
                },
            );
        }
        self.store.save_disk();

        let answer = format!("✅ Expert '{}' Forgé et Opérationnel. L'Agent Vampire a le feu vert pour le consulter via l'outil 'ask_{}'.", topic, topic.to_lowercase().replace(" ", "_"));
        Ok(json!(answer))
    }
}
