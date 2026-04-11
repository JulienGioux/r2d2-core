use crate::core::consultant_store::{ConsultantData, ConsultantStore};
use crate::core::McpTool;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

pub struct SyncNotebooksTool {
    store: Arc<ConsultantStore>,
}

impl SyncNotebooksTool {
    pub fn new(store: Arc<ConsultantStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl McpTool for SyncNotebooksTool {
    fn name(&self) -> String {
        "sync_notebooks".to_string()
    }

    fn description(&self) -> String {
        "Ouvre une connexion souveraine vers Google pour s'authentifier et aspirer la liste de tous vos carnets NotebookLM.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn call(&self, _arguments: Value) -> Result<Value, anyhow::Error> {
        let base_dir =
            if let Some(proj_dirs) = directories::ProjectDirs::from("com", "R2D2", "Vampire") {
                proj_dirs.config_dir().to_path_buf()
            } else {
                std::env::current_dir()
                    .unwrap_or_default()
                    .join(".r2d2-vampire")
            };
        let _ = std::fs::create_dir_all(&base_dir);
        let profile_dir = base_dir.join("chrome-profile");

        info!(
            "🚀 Lancement du Sync Agent (Chrome Profile: {:?})",
            profile_dir
        );

        let browser = r2d2_browser::SovereignBrowser::connect("chrome-profile").await.map_err(|e| anyhow::anyhow!("Échec critique de la liaison au navigateur (CDP Bridge déconnecté ?). Contexte: {}", e))?;
        let tab = r2d2_browser::SovereignBrowser::get_or_new_tab(&browser, "notebooklm")
            .await
            .map_err(|e| anyhow::anyhow!("Erreur Tab: {:?}", e))?;

        info!("Mise en orbite vers notebooklm.google.com...");
        tab.goto("https://notebooklm.google.com/")
            .await
            .map_err(|e| anyhow::anyhow!("Erreur Navigate: {}", e))?;

        // Attente de stabilisation initiale (Redirection Auth possible)
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Boucle de surveillance visuelle (Attente du Login Utilisateur)
        let mut logged_in = false;
        for i in 0..120 {
            // 120 * 3s = 6 minutes max pour taper son mot de passe
            let url = tab.url().await.unwrap_or_default();
            let safe_url = url.unwrap_or_default();
            if safe_url.contains("notebooklm.google.com") && !safe_url.contains("accounts.google") {
                logged_in = true;
                break;
            }
            if i % 5 == 0 {
                warn!("ATTENTION : R2D2 attend votre connexion Google sur la fenêtre ouverte...");
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }

        if !logged_in {
            return Err(anyhow::anyhow!(
                "Timeout: Connexion Google non établie. Relancez la commande."
            ));
        }

        info!("✅ Connexion Google VÉRIFIÉE. Aspiration du DOM et de l'inventaire (Adaptive Polling)...");

        let extract_js = r#"
            (function() {
                let uniques = {};
                let links = document.querySelectorAll('a[href*="/notebook/"]');
                for (let a of links) {
                    let card = a.closest('project-button, mat-card');
                    if (card) {
                        let titleEl = card.querySelector('.project-button-title');
                        if (titleEl) {
                            let name = titleEl.innerText.trim().replace(/[^a-zA-Z0-9_\-\s]/g, "");
                            if (name.length > 0 && a.href.startsWith('http')) {
                                uniques[name] = a.href;
                            }
                        }
                    }
                }
                return JSON.stringify(uniques);
            })()
        "#;

        let mut found_count = 0;
        // Adaptive Polling: 15 essais * 2s = 30s
        for _ in 0..15 {
            tokio::time::sleep(Duration::from_secs(2)).await;

            if let Ok(result) = tab.evaluate(extract_js).await {
                if let Some(serde_json::Value::String(val_ref)) = result.value() {
                    if let Ok(Value::Object(map)) = serde_json::from_str(val_ref) {
                        if !map.is_empty() {
                            let mut guard = match self.store.data.write() {
                                Ok(g) => g,
                                Err(poisoned) => poisoned.into_inner(),
                            };
                            for (name, url_val) in map {
                                if let Some(url) = url_val.as_str() {
                                    info!("✅ Découverte Souveraine : '{}' -> {}", name, url);
                                    let data = ConsultantData {
                                        url: Some(url.to_string()),
                                        enabled: true,
                                        variables: None,
                                    };
                                    let key = name.to_lowercase().replace(" ", "_");
                                    guard.insert(key, data);
                                    found_count += 1;
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }

        if found_count == 0 {
            let dump_js = "document.body.innerHTML";
            if let Ok(dump_res) = tab.evaluate(dump_js).await {
                if let Some(serde_json::Value::String(html)) = dump_res.value() {
                    let _ = tokio::fs::write("/tmp/notebook_dom.html", html).await;
                    error!("DOM dumpe dans /tmp/notebook_dom.html pour diagnostique.");
                }
            }
            return Err(anyhow::anyhow!("Timeout : Aucun carnet trouvé après 30s. L'élément DOM a peut-être changé côté Google."));
        }

        // On libère la base
        self.store.save_disk();

        let res_msg = format!(
            "Aspiration terminée. {} Notebooks ont été synchronisés et écrits.",
            found_count
        );
        Ok(json!(res_msg))
    }
}
