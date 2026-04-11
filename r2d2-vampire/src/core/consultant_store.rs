use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsultantData {
    pub url: Option<String>,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<serde_json::Map<String, Value>>,
}

pub struct ConsultantStore {
    db_path: PathBuf,
    pub data: RwLock<HashMap<String, ConsultantData>>,
}

impl ConsultantStore {
    pub fn new() -> Arc<Self> {
        let base_dir =
            if let Some(proj_dirs) = directories::ProjectDirs::from("com", "R2D2", "Vampire") {
                proj_dirs.config_dir().to_path_buf()
            } else {
                std::env::current_dir()
                    .unwrap_or_default()
                    .join(".r2d2-vampire")
            };

        if !base_dir.exists() {
            let _ = fs::create_dir_all(&base_dir);
        }

        // Fichier JSON standard en clair
        let db_path = base_dir.join("consultants.json");
        let mut map = HashMap::new();

        if db_path.exists() {
            if let Ok(content) = fs::read_to_string(&db_path) {
                if let Ok(parsed) = serde_json::from_str(&content) {
                    map = parsed;
                }
            }
        }

        let store = Self {
            db_path,
            data: RwLock::new(map),
        };
        store.save_disk(); // Sauvegarde initiale
        Arc::new(store)
    }

    pub fn save_disk(&self) {
        if let Ok(guard) = self.data.read() {
            if let Ok(json) = serde_json::to_string_pretty(&*guard) {
                let _ = fs::write(&self.db_path, json);
            }
        }
    }

    pub fn list_enabled_names(&self) -> Vec<String> {
        if let Ok(guard) = self.data.read() {
            guard
                .iter()
                .filter(|(_, v)| v.enabled)
                .map(|(k, _)| k.clone())
                .collect()
        } else {
            vec![]
        }
    }
}
