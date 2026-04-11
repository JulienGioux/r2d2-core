use crate::core::McpTool;
use async_trait::async_trait;
use r2d2_browser::SovereignBrowser;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info}; // Changement pour le Async RwLock

/// Commandes asynchrones dirigées vers l'Acteur Chrome (Chromiumoxide)
pub enum ChromeCommand {
    NavigateAndAsk {
        target_url: String,
        prompt: String,
        responder: oneshot::Sender<anyhow::Result<String>>,
    },
}

/// Global Registry pour implémenter l'Auto-Healing Async
static EXPERT_REGISTRY: OnceLock<RwLock<HashMap<String, mpsc::Sender<ChromeCommand>>>> =
    OnceLock::new();

fn get_registry() -> &'static RwLock<HashMap<String, mpsc::Sender<ChromeCommand>>> {
    EXPERT_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Outil Autonome "Stateful" par Expert (Génération Dynamique Async)
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

    async fn get_or_spawn_tx(&self) -> mpsc::Sender<ChromeCommand> {
        let mut registry = get_registry().write().await;

        if let Some(existing_tx) = registry.get(&self.expert_name) {
            return existing_tx.clone();
        }

        // Création du canal avec Backpressure forte (limite de 10 messages)
        let (new_tx, rx) = mpsc::channel(10);
        let name_clone = self.expert_name.clone();

        // Remplacement de spawn_blocking (OS Thread) par pur tokio::spawn
        tokio::spawn(async move {
            info!("Actor CDP (Chromiumoxide) démarré pour {}", name_clone);
            if let Err(e) = chrome_actor_loop(rx, name_clone).await {
                error!("Acteur CDP a crashé. Erreur critique: {}", e);
            }
        });

        registry.insert(self.expert_name.clone(), new_tx.clone());
        new_tx
    }
}

#[async_trait]
impl McpTool for NotebookLmTool {
    fn name(&self) -> String {
        format!("ask_{}", self.expert_name.to_lowercase())
    }

    fn description(&self) -> String {
        format!(
            "Consulte l'expert '{}' via son onglet connecté Stateful NotebookLM.",
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
            "🦇 Routing Zero-Scraping NotebookLM : '{}'",
            self.expert_name
        );

        let tx = self.get_or_spawn_tx().await;
        let (resp_tx, resp_rx) = oneshot::channel();

        let send_result = tx
            .send(ChromeCommand::NavigateAndAsk {
                target_url: self.target_url.clone(),
                prompt,
                responder: resp_tx,
            })
            .await;

        if send_result.is_err() {
            // Nettoyage Auto-Healing (Async)
            let mut registry = get_registry().write().await;
            registry.remove(&self.expert_name);
            return Err(anyhow::anyhow!(
                "Canal Chrome MPSC crashé. Acteur purgé du cache, nouvel essai au prochain appel."
            ));
        }

        let response = resp_rx.await??;
        Ok(json!(response))
    }
}

/// Boucle Asynchrone Tokio de l'Acteur. Mainteneur persistant d'Onglet (Page) Chromiumoxide.
async fn chrome_actor_loop(
    mut rx: mpsc::Receiver<ChromeCommand>,
    expert_name: String,
) -> anyhow::Result<()> {
    let mut browser: Option<chromiumoxide::Browser> = None;
    let mut active_tab: Option<Arc<chromiumoxide::Page>> = None;

    while let Some(command) = rx.recv().await {
        match command {
            ChromeCommand::NavigateAndAsk {
                target_url,
                prompt,
                responder,
            } => {
                if browser.is_none() {
                    info!("⏳ Lancement Asynchrone Chromiumoxide pour NotebookLM...");
                    match SovereignBrowser::connect("chrome-profile").await {
                        Ok(b) => {
                            browser = Some(b);
                        }
                        Err(e) => {
                            let _ =
                                responder.send(Err(anyhow::anyhow!("Échec connexion CDP: {}", e)));
                            continue;
                        }
                    }
                }

                let b = browser.as_ref().unwrap();

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
                        "Ouverture/Reuse du canal expert distant : {} (Matcher: {})",
                        expert_name, notebook_uuid
                    );

                    match SovereignBrowser::get_or_new_tab(b, notebook_uuid).await {
                        Ok(t) => active_tab = Some(t),
                        Err(e) => {
                            let _ = responder
                                .send(Err(anyhow::anyhow!("Échec CDP new_tab Async: {}", e)));
                            browser = None; // Force reset au prochain tour
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
                        if let Err(e) =
                            std::fs::write("/home/jgx/source/R2D2/output.txt", &response_text)
                        {
                            error!("Impossible d'écrire dans output.txt : {}", e);
                        } else {
                            info!("✅ Résultat NotebookLM sauvegardé dans /home/jgx/source/R2D2/output.txt");
                        }
                        let _ = responder.send(Ok(response_text));
                    }
                    Err(e) => {
                        let _ = responder.send(Err(e));
                    }
                }
            }
        }
    }

    Ok(())
}
