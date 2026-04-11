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

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// Global Registry pour implémenter l'Auto-Healing et empêcher le "Let it Crash Zombies"
static EXPERT_REGISTRY: OnceLock<RwLock<HashMap<String, mpsc::Sender<ChromeCommand>>>> =
    OnceLock::new();

fn get_registry() -> &'static RwLock<HashMap<String, mpsc::Sender<ChromeCommand>>> {
    EXPERT_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Outil Autonome "Stateful" par Expert (Génération Dynamique)
pub struct NotebookLmTool {
    pub expert_name: String,
    pub target_url: String,
    tx: mpsc::Sender<ChromeCommand>,
}

impl NotebookLmTool {
    pub fn new(expert_name: String, target_url: String) -> Self {
        let registry_guard = get_registry().write();
        let mut registry = match registry_guard {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        let tx = if let Some(existing_tx) = registry.get(&expert_name) {
            existing_tx.clone()
        } else {
            // Création du canal avec Backpressure forte (limite de 10 messages)
            let (new_tx, rx) = mpsc::channel(10);
            let name_clone = expert_name.clone();

            // Remplacement formel de spawn_blocking par un Thread dédié (RAM gérée par l'OS pour éviter les Stack Overflows avec Tokio Futures lourdes)
            std::thread::Builder::new()
                .name(format!("cdp-{}", name_clone))
                .spawn(move || {
                    info!("Actor CDP démarré pour {}", name_clone);
                    if let Err(e) = chrome_actor_loop(rx, name_clone) {
                        error!("Acteur CDP a crashé. Erreur: {}", e);
                    }
                })
                .expect("Failed to spawn OS thread for Chrome Actor");

            registry.insert(expert_name.clone(), new_tx.clone());
            new_tx
        };

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
        let send_result = self
            .tx
            .send(ChromeCommand::NavigateAndAsk {
                target_url: self.target_url.clone(),
                prompt,
                responder: resp_tx,
            })
            .await;

        if send_result.is_err() {
            // Nettoyage Auto-Healing: Si le canal est mort, on l'arrache du registre.
            let mut registry = match get_registry().write() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            registry.remove(&self.expert_name);
            return Err(anyhow::anyhow!(
                "Canal Chrome MPSC crashé. Acteur purgé du cache, nouvel essai au prochain appel."
            ));
        }

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

                let b = if let Some(ref br) = browser {
                    br
                } else {
                    continue; // Never happens due to previous check, but satisfies rustc and "zero panic"
                };

                // 1. Validation de la vivacité de l'onglet (Auto-Healing)
                let mut reset_tab = false;
                if let Some(tab) = active_tab.as_ref() {
                    // Un evaluate trivial pour vérifier si la socket CDP est toujours active vers cet onglet
                    if tab.evaluate("1", false).is_err() {
                        info!("L'onglet associé à {} semble mort ou a été fermé. Réinitialisation du lien.", expert_name);
                        reset_tab = true;
                    }
                }
                if reset_tab {
                    active_tab = None;
                }

                // 2. Extraction fine du matcher d'URL (UUID si possible)
                let notebook_uuid = target_url
                    .split("/notebook/")
                    .nth(1)
                    .unwrap_or("")
                    .split('/')
                    .next()
                    .unwrap_or("");

                let url_matcher = if notebook_uuid.is_empty() {
                    "notebooklm" // Fallback pour la page d'accueil ou URL générique
                } else {
                    notebook_uuid // L'ID Unique du carnet isole parfaitement notre Expert sur son propre onglet
                };

                // 3. Récupération ou ancrage
                let mut tab_result = Ok(None);
                if active_tab.is_none() {
                    info!(
                        "Ouverture/Reuse du canal expert distant : {} (Matcher: {})",
                        expert_name, url_matcher
                    );
                    // Si l'onglet existe déjà pour cet Expert, on le récupère. Sinon, le Browser en ouvre un vierge.
                    tab_result = r2d2_browser::SovereignBrowser::get_or_new_tab(b, url_matcher)
                        .or_else(|_| b.new_tab())
                        .map(Some);
                }

                match tab_result {
                    Ok(Some(t)) => active_tab = Some(t),
                    Ok(None) => {} // Already had active_tab
                    Err(e) => {
                        let _ = responder.send(Err(anyhow::anyhow!(
                            "Échec CDP new_tab. L'hôte Chrome est fermé ? : {}",
                            e
                        )));
                        browser = None; // Force reset of browser connection next time
                        continue;
                    }
                }

                let tab = if let Some(t) = active_tab.as_ref() {
                    t
                } else {
                    let _ = responder.send(Err(anyhow::anyhow!(
                        "Impossible d'acquérir le pont CDP vers le Tab Chrome."
                    )));
                    continue;
                };

                let current_url = tab.get_url();
                if !current_url.starts_with(&target_url) {
                    tab.set_default_timeout(std::time::Duration::from_secs(120));
                    info!("Redirection du Pont CDP vers l'Expert : {}", target_url);
                    let _ = tab.navigate_to(&target_url);
                    let _ = tab.wait_until_navigated();
                    std::thread::sleep(std::time::Duration::from_secs(2)); // Pause SPA logic
                }

                let api = crate::vampire_lord::notebook_api::NotebookApi::new(tab.clone());

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
