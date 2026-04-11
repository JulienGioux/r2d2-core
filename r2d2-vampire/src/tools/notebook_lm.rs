use crate::core::McpTool;
use crate::vampire_lord::cdp_supervisor::{get_supervisor, SupervisorCommand};
use async_trait::async_trait;
use r2d2_browser::SovereignBrowser;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};

pub struct NotebookLmTool {
    pub expert_name: String,
    pub target_url: String,
}

impl NotebookLmTool {
    pub fn new(expert_name: String, target_url: String) -> Self {
        Self {
            expert_name,
            target_url,
        }
    }
}

#[async_trait]
impl McpTool for NotebookLmTool {
    fn name(&self) -> String {
        format!("ask_{}", self.expert_name.to_lowercase())
    }

    fn description(&self) -> String {
        format!(
            "Consulte l'expert '{}' via son onglet géré par le Superviseur OTP NotebookLM.",
            self.expert_name
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": format!("Le contexte/la question posée à {}", self.expert_name) }
            },
            "required": ["prompt"]
        })
    }

    async fn call(&self, arguments: Value) -> Result<Value, anyhow::Error> {
        let prompt = arguments
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        info!(
            "🦇 Routing Zero-Scraping NotebookLM (via OTP Supervisor) : '{}'",
            self.expert_name
        );

        let tx = get_supervisor();
        let (resp_tx, resp_rx) = oneshot::channel();

        let send_result = tx
            .send(SupervisorCommand::AskExpert {
                expert_name: self.expert_name.clone(),
                target_url: self.target_url.clone(),
                prompt,
                responder: resp_tx,
            })
            .await;

        if send_result.is_err() {
            return Err(anyhow::anyhow!(
                "Le Superviseur CDP est indisponible (Canal Hub crashé)."
            ));
        }

        let response = resp_rx.await??;
        Ok(json!(response))
    }
}

/// Boucle Asynchrone Tokio de l'Acteur. Détenue et monitorée par le VampireSupervisor.
pub async fn chrome_actor_loop(
    mut rx: mpsc::Receiver<SupervisorCommand>,
    expert_name: String,
    browser: Arc<chromiumoxide::Browser>,
) -> anyhow::Result<()> {
    let mut active_tab: Option<Arc<chromiumoxide::Page>> = None;

    while let Some(command) = rx.recv().await {
        let SupervisorCommand::AskExpert {
            target_url,
            prompt,
            responder,
            ..
        } = command;
        // 2. Récupération ou ancrage
        let notebook_uuid = target_url
            .split("/notebook/")
            .nth(1)
            .unwrap_or("")
            .split('/')
            .next()
            .unwrap_or(if target_url.contains("notebooklm") {
                "notebooklm"
            } else {
                ""
            });

        if active_tab.is_none() {
            info!(
                "Ouverture/Reuse de l'onglet expert distant : {} (Matcher: {})",
                expert_name, notebook_uuid
            );

            match SovereignBrowser::get_or_new_tab(&browser, notebook_uuid).await {
                Ok(t) => active_tab = Some(t),
                Err(e) => {
                    let _ = responder.send(Err(anyhow::anyhow!(
                        "Échec CDP get_or_new_tab Async: {}",
                        e
                    )));
                    continue;
                }
            }
        }

        let tab = active_tab.as_ref().unwrap();

        let current_url = tab.url().await.unwrap_or_default();
        let url_str: &str = current_url.as_deref().unwrap_or("");

        if !url_str.starts_with(&target_url) {
            info!("Redirection du Pont CDP vers l'Expert : {}", target_url);
            let _ = tab.goto(&target_url).await;
            let _ = tab.wait_for_navigation().await;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        let api = crate::vampire_lord::notebook_api::NotebookApi::new(tab.clone()).await;

        // Exécution RPC pure Asynchrone Zero-Scraping
        let rpc_result = api.chat_ask(notebook_uuid, &prompt).await;

        match rpc_result {
            Ok(response_text) => {
                // Écriture du résultat dans output.txt pour le Chef
                if let Err(e) = std::fs::write("/home/jgx/source/R2D2/output.txt", &response_text) {
                    error!("Impossible d'écrire dans output.txt : {}", e);
                } else {
                    info!(
                        "✅ Résultat NotebookLM sauvegardé dans /home/jgx/source/R2D2/output.txt"
                    );
                }
                let _ = responder.send(Ok(response_text));
            }
            Err(e) => {
                // Si l'erreur provient d'un context invalidé (par ex page fermée), reset active_tab
                // pour forcer la réouverture au prochain appel ! (Auto-Heal local)
                if e.to_string().contains("Session closed")
                    || e.to_string().contains("Target closed")
                {
                    active_tab = None;
                }
                let _ = responder.send(Err(e));
            }
        }
    }

    Ok(())
}
