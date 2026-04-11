use crate::manifest::ModelManifest;
use crate::types::ModelId;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use uuid::Uuid;

/// Erreurs liées à la gestion du catalogue
#[derive(thiserror::Error, Debug)]
pub enum RegistryError {
    #[error("Erreur I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("Erreur de Parsing TOML: {0}")]
    ParseToml(#[from] toml::de::Error),
    #[error("Manifest Invalide ou Manquant: {0}")]
    InvalidManifest(String),
    #[error("Modèle introuvable: {0}")]
    NotFound(String),
}

/// Gestionnaire MLOps de la Forge. I/O FileSystem Adapter.
pub struct ModelRegistry {
    pub base_path: PathBuf,
}

impl ModelRegistry {
    /// Initialise le registre en pointant vers un dossier racine (ex: `/workspace/models`)
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    /// Tente de lire le manifeste dans un dossier spécifique (Asynchrone + Isolé)
    pub async fn load_manifest<P: AsRef<Path>>(
        &self,
        model_dir: P,
    ) -> Result<ModelManifest, RegistryError> {
        let manifest_file = model_dir.as_ref().join("manifest.toml");

        // Isolation contre la famine du thread asynchrone (Starvation)
        let manifest_result =
            tokio::task::spawn_blocking(move || -> Result<ModelManifest, RegistryError> {
                let metadata = fs::metadata(&manifest_file).map_err(|_| {
                    RegistryError::InvalidManifest(
                        "manifest.toml est introuvable ou inaccessible".to_string(),
                    )
                })?;

                // Sécurité Zero-Trust: Refuser les fichiers manifestes gigantesques
                if metadata.len() > 128 * 1024 {
                    // Limite 128 KB
                    return Err(RegistryError::InvalidManifest(
                        "Le fichier manifest.toml dépasse la limite de sécurité de 128 Ko"
                            .to_string(),
                    ));
                }

                let content = fs::read_to_string(&manifest_file)?;

                // Protection : deserializer limiter pour toml via serde (Limitation de la profondeur implicitement
                // traitée par l'implémentation ToML de base avec des listes / strings courtes dans notre structure)
                let manifest: ModelManifest = toml::from_str(&content)?;
                Ok(manifest)
            })
            .await;

        let manifest = manifest_result.map_err(|e| {
            tracing::error!("Erreur de spawn_blocking: {}", e);
            RegistryError::InvalidManifest("Panique dans le thread I/O".to_string())
        })??; // Double unwrap (spawn_blocking Result + Inner Result)

        Ok(manifest)
    }

    /// Scan récursif pour indexer tous les modèles disponibles dans la base_path
    /// C'est le "Pipe" principal pour la future interface Front-end !
    pub async fn catalog(&self) -> Vec<(PathBuf, ModelManifest)> {
        let mut available_models = Vec::new();

        if !self.base_path.exists() {
            warn!(
                "Le dossier du registre models n'existe pas encore: {:?}",
                self.base_path
            );
            return available_models;
        }

        // On peut encapsuler la lecture du FileSystem pur
        let base_path_clone = self.base_path.clone();
        let family_and_model_dirs_res = tokio::task::spawn_blocking(move || -> Vec<PathBuf> {
            let mut dirs = Vec::new();
            if let Ok(families) = fs::read_dir(&base_path_clone) {
                for family_dir in families.flatten() {
                    if family_dir.path().is_dir() {
                        if let Ok(models) = fs::read_dir(family_dir.path()) {
                            for model_dir in models.flatten() {
                                if model_dir.path().is_dir() {
                                    dirs.push(model_dir.path());
                                }
                            }
                        }
                    }
                }
            }
            dirs
        })
        .await;

        let family_and_model_dirs = family_and_model_dirs_res.unwrap_or_default();

        for model_dir in family_and_model_dirs {
            match self.load_manifest(&model_dir).await {
                Ok(manifest) => {
                    info!(
                        "Modèle découvert: {} ({})",
                        manifest.identity.name, manifest.identity.uuid
                    );
                    available_models.push((model_dir, manifest));
                }
                Err(e) => {
                    warn!("Ignoré (Metadata Incomplètes) - {:?}: {}", model_dir, e);
                    println!("Error loading manifest: {:?}", e);
                }
            }
        }

        available_models
    }

    /// Recherche un modèle par son UUID
    pub async fn find_by_uuid(&self, uuid: &Uuid) -> Option<(PathBuf, ModelManifest)> {
        self.catalog()
            .await
            .into_iter()
            .find(|(_, manifest)| &manifest.identity.uuid == uuid)
    }

    /// Recherche un modèle par son Nom Exact
    pub async fn find_by_name(&self, name: &ModelId) -> Option<(PathBuf, ModelManifest)> {
        self.catalog()
            .await
            .into_iter()
            .find(|(_, manifest)| &manifest.identity.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_registry_manifest_parsing() {
        let dir = tempdir().unwrap();
        let family_dir = dir.path().join("bitmamba");
        let model_dir = family_dir.join("chimera_1");
        fs::create_dir_all(&model_dir).unwrap();

        let manifest_content = r#"
format = "causal_lm"

[identity]
uuid = "a3bc9dfc-ab84-489e-8c34-eb54e3d3b76a"
name = "Chimera-Test"
version = "1.0.0"
family = "bitmamba"
domain_role = "generator"

[topology]
architecture = "SSM"
quantization = "1.58b"
backend = "LocalCandle"
device = "cpu"

[storage]

[metrics]
optimal_tasks = ["reasoning"]
"#;
        let mut file = fs::File::create(model_dir.join("manifest.toml")).unwrap();
        file.write_all(manifest_content.as_bytes()).unwrap();

        let registry = ModelRegistry::new(dir.path());
        let catalog = registry.catalog().await;

        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].1.identity.name.0, "Chimera-Test");
        assert_eq!(catalog[0].1.topology.architecture, "SSM");
        assert_eq!(
            catalog[0].1.topology.quantization,
            crate::types::QuantizationLevel::Bit1_58
        );
    }
}
