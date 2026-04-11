use crate::manifest::ModelManifest;
use anyhow::{Context, Result};
use hf_hub::{api::tokio::ApiBuilder, Repo, RepoType};
// removed std::path::{Path, PathBuf}
use tracing::{info, instrument};

/// Agent Logistique chargé de télécharger les modèles depuis le Hub HuggingFace (Zero-Config).
/// Cette classe garantit la séparation des préoccupations : elle provisionne les fichiers sur disque,
/// sans jamais instancier de tenseurs en RAM.
pub struct ModelFetcher;

impl ModelFetcher {
    /// Télécharge (uniquement si manquant) les artefacts nécessaires à partir de HuggingFace.
    /// Renvoie une copie du `ModelManifest` modifié où la `StorageConfig` est peuplée avec les vrais chemins.
    #[instrument(skip(manifest))]
    pub async fn ensure_downloaded(
        manifest: &ModelManifest,
        repo_id: &str,
        revision: &str,
        weights_file: &str,
    ) -> Result<ModelManifest> {
        let mut new_manifest = manifest.clone();

        info!(
            "🛡️ [MODEL-FETCHER] Vérification des poids pour '{}'",
            repo_id
        );

        let token = std::env::var("HF_TOKEN").ok(); // Utilise la variable d'env s'il y en a une (Vault)

        let api = ApiBuilder::new()
            .with_token(token)
            .build()
            .context("Failed to build HF API client")?;

        let repo = api.repo(Repo::with_revision(
            repo_id.to_string(),
            RepoType::Model,
            revision.to_string(),
        ));

        info!("   [MODEL-FETCHER] Résolution des poids ({weights_file})...");
        let w_path = repo
            .get(weights_file)
            .await
            .context("Failed to fetch weights")?;

        info!("   [MODEL-FETCHER] Résolution du dictionnaire Tokenizer (tokenizer.json)...");
        let t_path = repo
            .get("tokenizer.json")
            .await
            .context("Failed to fetch tokenizer")?;

        info!("   [MODEL-FETCHER] Résolution de la configuration matérielle (config.json)...");
        let c_path = repo
            .get("config.json")
            .await
            .context("Failed to fetch config")?;

        info!("✅ [MODEL-FETCHER] Apprêté sur le disque ! Fichiers sauvegardés/mis en cache par l'OS.");

        // Mise à jour du Manifeste
        new_manifest.storage.weights_path = Some(w_path.to_string_lossy().to_string());
        new_manifest.storage.tokenizer_path = Some(t_path.to_string_lossy().to_string());
        new_manifest.storage.config_path = Some(c_path.to_string_lossy().to_string());

        Ok(new_manifest)
    }
}
