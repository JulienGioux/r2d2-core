use async_trait::async_trait;
use r2d2_mcp_core::McpTool;
use serde_json::{json, Value};
use std::time::Duration;
use tracing::{info, warn};

pub struct AuthenticateProviderTool;

#[derive(serde::Deserialize, Clone)]
struct ProviderConfig {
    active: bool,
    auth_url: String,
    success_selector: String,
}

#[async_trait]
impl McpTool for AuthenticateProviderTool {
    fn name(&self) -> String {
        "authenticate_provider".to_string()
    }

    fn description(&self) -> String {
        "Lance la procédure Interactive Hand-Off pour authentifier un fournisseur SSO (Google, Github, etc.).".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "provider": { "type": "string", "description": "L'identifiant du fournisseur (ex: google, github, openai)" }
            },
            "required": ["provider"]
        })
    }

    async fn call(&self, arguments: Value) -> Result<Value, anyhow::Error> {
        let provider_id = arguments
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_lowercase();

        let path = dirs::config_dir()
            .unwrap_or_default()
            .join("vampire")
            .join("providers.json");
        if !path.exists() {
            return Err(anyhow::anyhow!(
                "Le registre des Identity Providers est introuvable."
            ));
        }

        let content = std::fs::read_to_string(path)?;
        let providers: std::collections::HashMap<String, ProviderConfig> =
            serde_json::from_str(&content)?;

        let config = match providers.get(&provider_id) {
            Some(c) if c.active => c.clone(),
            Some(_) => {
                return Err(anyhow::anyhow!(
                    "Le fournisseur '{}' est désactivé dans la configuration.",
                    provider_id
                ))
            }
            None => return Err(anyhow::anyhow!("Fournisseur inрияconnu : {}", provider_id)),
        };

        info!(
            "🛡️ Séquence d'Authentification Interactive lancée pour {}",
            provider_id
        );

        let profile_name = format!("Chrome_{}", provider_id.to_uppercase());

        // Polling loop asynchrone (Bloquant jusqu'à la réussite)
        let res = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let browser = r2d2_browser::SovereignBrowser::connect(&profile_name)?;
            let tab = browser.new_tab()?;
            tab.navigate_to(&config.auth_url)?;

            info!(
                "⏳ En attente de la résolution HUMAINE du SSO pour {} ...",
                provider_id
            );

            // Boucle de Polling pour détecter la connexion
            for _ in 0..120 {
                // Attente max 10 minutes (120 * 5s)
                std::thread::sleep(Duration::from_secs(5));

                let check_js = format!(
                    "document.querySelector(\"{}\") !== null",
                    config.success_selector
                );
                if let Ok(eval) = tab.evaluate(&check_js, false) {
                    if let Some(Value::Bool(true)) = eval.value {
                        info!(
                            "✅ Validation Cryptographique : Authentification {} réussie !",
                            provider_id
                        );
                        let _ = tab.close_target();
                        return Ok(format!(
                            "Succès de l'authentification native. Profil scellé: {}",
                            profile_name
                        ));
                    }
                }
                warn!(
                    "... En attente du relais humain sur Chrome (Connexion {} requise)",
                    provider_id
                );
            }

            Err(anyhow::anyhow!(
                "Timeout: L'utilisateur n'a pas finalisé l'authentification dans le temps imparti."
            ))
        })
        .await??;

        Ok(json!(res))
    }
}
