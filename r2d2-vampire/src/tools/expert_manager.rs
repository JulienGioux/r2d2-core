use crate::core::consultant_store::{ConsultantData, ConsultantStore};
use crate::core::McpTool;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct AddNotebookExpertTool {
    pub store: Arc<ConsultantStore>,
}

#[async_trait]
impl McpTool for AddNotebookExpertTool {
    fn name(&self) -> String {
        "add_notebook_expert".to_string()
    }

    fn description(&self) -> String {
        "Ajoute manuellement un nouvel expert NotebookLM au registre via son URL.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Nom de l'expert (ex: 'rustymaster')" },
                "url": { "type": "string", "description": "L'URL complète du projet NotebookLM" }
            },
            "required": ["name", "url"]
        })
    }

    async fn call(&self, arguments: Value) -> Result<Value, anyhow::Error> {
        let name = arguments
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let url = arguments
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        if name.is_empty() {
            return Err(anyhow::anyhow!("Le paramètre 'name' est requis"));
        }
        if url.is_empty() || !url.starts_with("http") {
            return Err(anyhow::anyhow!("L'URL est invalide ou manquante."));
        }

        let snake_name = name.to_lowercase().replace(" ", "_");

        let mut guard = self.store.data.write().unwrap();
        guard.insert(
            snake_name.clone(),
            ConsultantData {
                url: Some(url.to_string()),
                enabled: true,
                variables: None,
            },
        );
        drop(guard);
        self.store.save_disk();

        Ok(json!(format!(
            "✅ L'Expert '{}' a été ajouté avec succès. Il est désormais prêt à l’appel (ask_{}).",
            snake_name, snake_name
        )))
    }
}

pub struct RemoveNotebookExpertTool {
    pub store: Arc<ConsultantStore>,
}

#[async_trait]
impl McpTool for RemoveNotebookExpertTool {
    fn name(&self) -> String {
        "remove_notebook_expert".to_string()
    }

    fn description(&self) -> String {
        "Supprime un expert NotebookLM du registre local.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Nom de l'expert à désintégrer (ex: 'rustymaster')" }
            },
            "required": ["name"]
        })
    }

    async fn call(&self, arguments: Value) -> Result<Value, anyhow::Error> {
        let name = arguments
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_lowercase();
        if name.is_empty() {
            return Err(anyhow::anyhow!("Le paramètre 'name' est requis"));
        }

        let mut guard = self.store.data.write().unwrap();
        let removed = guard.remove(&name);
        drop(guard);
        self.store.save_disk();

        if removed.is_some() {
            Ok(json!(format!(
                "🔥 Expert '{}' définitivement désintégré du registre MCP.",
                name
            )))
        } else {
            Err(anyhow::anyhow!(
                "L'Expert '{}' n'existe pas dans le registre.",
                name
            ))
        }
    }
}

pub struct ListNotebookExpertsTool {
    pub store: Arc<ConsultantStore>,
}

#[async_trait]
impl McpTool for ListNotebookExpertsTool {
    fn name(&self) -> String {
        "list_notebook_experts".to_string()
    }

    fn description(&self) -> String {
        "Affiche la liste courante des experts NotebookLM autorisés et persistés dans le registre."
            .to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn call(&self, _arguments: Value) -> Result<Value, anyhow::Error> {
        let guard = self.store.data.read().unwrap();
        let mut list = Vec::new();
        for (name, data) in guard.iter() {
            list.push(format!(
                "- {} (Actif: {}, Url: {})",
                name,
                data.enabled,
                data.url.as_deref().unwrap_or("N/A")
            ));
        }

        if list.is_empty() {
            Ok(json!("ℹ️ Aucun expert n'est actuellement inscrit dans le registre. Tu peux en ajouter avec 'add_notebook_expert' !"))
        } else {
            Ok(json!(list.join("\n")))
        }
    }
}

pub struct ToggleCdpBridgeTool;

#[async_trait]
impl McpTool for ToggleCdpBridgeTool {
    fn name(&self) -> String {
        "toggle_cdp_bridge".to_string()
    }

    fn description(&self) -> String {
        "Active ou désactive le Daemon r2d2-bridge (qui ouvre la connexion au navigateur Chrome Hôte depuis WSL).".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "active": { "type": "boolean", "description": "Lancer (true) ou fermer (false) le pont CDP Windows." },
                "profile": { "type": "string", "description": "Nom du profil Chrome pour isoler le SSO (ex: 'Chrome_Google')" }
            },
            "required": ["active"]
        })
    }

    async fn call(&self, arguments: Value) -> Result<Value, anyhow::Error> {
        let active = arguments
            .get("active")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if active {
            let exe_path = std::env::current_dir()
                .unwrap_or_default()
                .join("target")
                .join("x86_64-pc-windows-msvc")
                .join("release")
                .join("r2d2-bridge.exe");
            if !exe_path.exists() {
                return Err(anyhow::anyhow!("Le binaire Windows n'existe pas : {}. As-tu configuré le module et lancé `cargo xwin` ?", exe_path.display()));
            }

            let profile = arguments
                .get("profile")
                .and_then(|v| v.as_str())
                .unwrap_or("Chrome_WSL_Debug");

            std::process::Command::new(exe_path)
                .arg("--profile")
                .arg(profile)
                .spawn()
                .map_err(|e| anyhow::anyhow!("Erreur Interop WSL: {}", e))?;

            Ok(json!(format!(
                "✅ Le pont R2D2-Bridge a été démarré avec le profil '{}' !",
                profile
            )))
        } else {
            let _ = std::process::Command::new("taskkill.exe")
                .args(["/IM", "r2d2-bridge.exe", "/F"])
                .output();
            Ok(json!(
                "🛑 Le pont R2D2-Bridge a été éteint sur l'Hôte Windows."
            ))
        }
    }
}
