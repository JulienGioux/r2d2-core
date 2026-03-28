use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use std::sync::{Mutex, OnceLock};

fn in_memory_vault() -> &'static Mutex<HashMap<String, String>> {
    static IN_MEMORY_VAULT: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    IN_MEMORY_VAULT.get_or_init(|| Mutex::new(HashMap::new()))
}

const VAULT_PATH_STR: &str = "data/.secrets.json";

#[derive(Serialize, Deserialize, Default)]
struct SecretsVault {
    keys: HashMap<String, String>,
}

pub struct Vault;

impl Vault {
    /// Retourne une clé API depuis le Vault sécurisé, la RAM Paranoïaque ou fallback sur les variables d'environnement.
    pub fn get_api_key(identifier: &str) -> Option<String> {
        if let Some(val) = in_memory_vault().lock().unwrap().get(identifier) {
            return Some(val.clone());
        }
        
        let vault = Self::load_vault();
        if let Some(val) = vault.keys.get(identifier) {
            return Some(val.clone());
        }
        
        // Fallback transparent (rétrocompatibilité)
        std::env::var(identifier).ok()
    }

    /// Enregistre ou met à jour une clé API dans le Vault sécurisé
    pub fn set_api_key(identifier: &str, value: &str) -> bool {
        let mut vault = Self::load_vault();
        if value.trim().is_empty() {
            vault.keys.remove(identifier);
        } else {
            vault.keys.insert(identifier.to_string(), value.to_string());
        }
        Self::save_vault(&vault)
    }

    /// Enregistre ou met à jour une clé API dans le Vault MEMORY SEULEMENT (Paranoia Mode)
    pub fn set_in_memory_key(identifier: &str, value: &str) {
        let mut mem = in_memory_vault().lock().unwrap();
        if value.trim().is_empty() {
            mem.remove(identifier);
            info!("Clé {} révoquée de la RAM (Mode Paranoïaque).", identifier);
        } else {
            mem.insert(identifier.to_string(), value.to_string());
            info!("Clé {} injectée en RAM uniquement (Mode Paranoïaque activé).", identifier);
        }
    }

    /// Extrait les clés enregistrées (masquées) pour l'interface d'administration
    pub fn get_masked_keys() -> HashMap<String, String> {
        let vault = Self::load_vault();
        let mem = in_memory_vault().lock().unwrap();
        let mut masked = HashMap::new();
        
        // Keys in disk
        for (k, v) in vault.keys {
            if v.len() > 8 {
                let mask = format!("{}****{}", &v[..4], &v[v.len()-4..]);
                masked.insert(k, mask);
            } else if v.is_empty() {
                masked.insert(k, "NON DÉFINIE".to_string());
            } else {
                masked.insert(k, "****".to_string());
            }
        }
        
        // Override with RAM keys
        for (k, v) in mem.iter() {
            if v.len() > 8 {
                let mask = format!("{}****{} (IN-RAM)", &v[..4], &v[v.len()-4..]);
                masked.insert(k.clone(), mask);
            } else if v.is_empty() {
                masked.insert(k.clone(), "NON DÉFINIE (IN-RAM)".to_string());
            } else {
                masked.insert(k.clone(), "**** (IN-RAM)".to_string());
            }
        }
        masked
    }

    /// Construit une Map d'environnement sécurisée pour un sous-processus spécifique, isolant les clés non requises
    pub fn get_runtime_env(provider: &str) -> HashMap<String, String> {
        let vault = Self::load_vault();
        let mem = in_memory_vault().lock().unwrap();
        let mut env = HashMap::new();
        
        match provider {
            "github" | "@modelcontextprotocol/server-github" => {
                if let Some(token) = mem.get("GITHUB_PERSONAL_ACCESS_TOKEN") {
                    env.insert("GITHUB_PERSONAL_ACCESS_TOKEN".to_string(), token.clone());
                } else if let Some(token) = vault.keys.get("GITHUB_PERSONAL_ACCESS_TOKEN") {
                    env.insert("GITHUB_PERSONAL_ACCESS_TOKEN".to_string(), token.clone());
                } else if let Ok(val) = std::env::var("GITHUB_PERSONAL_ACCESS_TOKEN") {
                    env.insert("GITHUB_PERSONAL_ACCESS_TOKEN".to_string(), val);
                }
            },
            "gemini" => {
                if let Some(token) = mem.get("GEMINI_API_KEY") {
                    env.insert("GEMINI_API_KEY".to_string(), token.clone());
                } else if let Some(token) = vault.keys.get("GEMINI_API_KEY") {
                    env.insert("GEMINI_API_KEY".to_string(), token.clone());
                } else if let Ok(val) = std::env::var("GEMINI_API_KEY") {
                    env.insert("GEMINI_API_KEY".to_string(), val);
                }
            },
            _ => {}
        }
        
        env
    }

    fn load_vault() -> SecretsVault {
        let vault_path = Path::new(VAULT_PATH_STR);
        if !vault_path.exists() {
            return SecretsVault::default();
        }

        match fs::read_to_string(vault_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                error!("Format Vault Corrompu : {:?}", e);
                SecretsVault::default()
            }),
            Err(e) => {
                warn!("Impossible de lire le Vault {:?} : {:?}", vault_path, e);
                SecretsVault::default()
            }
        }
    }

    fn save_vault(vault: &SecretsVault) -> bool {
        let vault_path = Path::new(VAULT_PATH_STR);
        // S'assurer que le parent est créé
        if let Some(parent) = vault_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let json = match serde_json::to_string_pretty(vault) {
            Ok(j) => j,
            Err(e) => {
                error!("Echec de la sérialisation Vault : {:?}", e);
                return false;
            }
        };

        match fs::write(vault_path, json) {
            Ok(_) => {
                // Tenter de séciruser le fichier chmod 600 sur Unix si possible
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(mut perms) = fs::metadata(vault_path).map(|m| m.permissions()) {
                        perms.set_mode(0o600);
                        let _ = fs::set_permissions(vault_path, perms);
                    }
                }
                info!("Vault {} statique mis à jour avec succès.", vault_path.display());
                true
            }
            Err(e) => {
                error!("Echec lors de l'écriture sécurisée dans le Vault : {:?}", e);
                false
            }
        }
    }
}
