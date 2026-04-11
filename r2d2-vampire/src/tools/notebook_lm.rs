use crate::core::McpTool;
use async_trait::async_trait;
use r2d2_browser::SovereignBrowser;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};

/// Commandes asynchrones dirigées vers l'Acteur Chrome
pub enum ChromeCommand {
    NavigateAndAsk {
        target_url: String,
        prompt: String,
        responder: oneshot::Sender<anyhow::Result<String>>,
    },
}

/// Outil Autonome "Stateful" par Expert (Génération Dynamique)
pub struct NotebookLmTool {
    pub expert_name: String,
    pub target_url: String,
    tx: mpsc::Sender<ChromeCommand>,
}

impl NotebookLmTool {
    pub fn new(expert_name: String, target_url: String) -> Self {
        let (tx, rx) = mpsc::channel(20);
        let name_clone = expert_name.clone();

        tokio::task::spawn_blocking(move || {
            info!(
                "Moteur CDP MPSC en standby pour {}. Prêt au Lazy-Load.",
                name_clone
            );
            if let Err(e) = chrome_actor_loop(rx, name_clone) {
                error!("L'Acteur Global Chrome a crashé. Erreur: {}", e);
            }
        });

        Self {
            expert_name,
            target_url,
            tx,
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

        info!("🦇 Routing Scraping NotebookLM : '{}'", self.expert_name);

        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(ChromeCommand::NavigateAndAsk {
                target_url: self.target_url.clone(),
                prompt,
                responder: resp_tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("Canal Chrome MPSC crashé"))?;

        let response = resp_rx.await??;
        Ok(json!(response))
    }
}

/// Boucle synchrone de l'Acteur. Se connecte au Host Chrome (Port 9222) au premier appel.
fn chrome_actor_loop(
    mut rx: mpsc::Receiver<ChromeCommand>,
    expert_name: String,
) -> anyhow::Result<()> {
    let mut browser: Option<headless_chrome::Browser> = None;
    let mut active_tab: Option<std::sync::Arc<headless_chrome::Tab>> = None;

    while let Some(command) = rx.blocking_recv() {
        match command {
            ChromeCommand::NavigateAndAsk {
                target_url,
                prompt,
                responder,
            } => {
                if browser.is_none() {
                    info!("⏳ Lancement du Sovereign Chromium (CDP Relay) pour NotebookLM...");
                    let b = match SovereignBrowser::connect("chrome-profile") {
                        Ok(b) => b,
                        Err(e) => {
                            let _ = responder.send(Err(anyhow::anyhow!("Échec critique de la liaison au navigateur (CDP Bridge déconnecté ?). Contexte technique: {}", e)));
                            continue;
                        }
                    };
                    browser = Some(b);
                }

                let b = browser.as_ref().unwrap();

                let tab = active_tab.get_or_insert_with(|| {
                    info!("Ouverture/Reuse du canal expert distant : {}", expert_name);
                    let t =
                        r2d2_browser::SovereignBrowser::get_or_new_tab(b, "notebooklm").unwrap();

                    // Si on est déjà sur notebooklm (réutilisation), on évite un fastidieux navigate_to
                    let current_url = t.get_url();
                    if !current_url.contains("notebooklm") {
                        // Set a massive 120s timeout because Google Angular takes forever to fully load sometimes
                        t.set_default_timeout(std::time::Duration::from_secs(120));
                        let _ = t.navigate_to("https://notebooklm.google.com/");
                        let _ = t.wait_until_navigated();
                    }
                    t
                });

                let api = crate::vampire_lord::notebook_api::NotebookApi::new(tab.clone());

                let notebook_uuid = target_url
                    .split("/notebook/")
                    .nth(1)
                    .unwrap_or("")
                    .split('/')
                    .next()
                    .unwrap_or("");

                // Exécution RPC pure (Zero-Class / Sovereign Hexagonal)
                let rpc_result = api.chat_ask(notebook_uuid, &prompt);

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
