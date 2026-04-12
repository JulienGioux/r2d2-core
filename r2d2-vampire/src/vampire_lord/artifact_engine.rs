use crate::vampire_lord::notebook_api::NotebookApi;
use crate::vampire_lord::types::{ArtifactStatus, QuizDifficulty, QuizQuantity};
use anyhow::Result;
use serde_json::Value;
use std::time::Duration;
use tracing::{info, warn};

/// Orchestrateur Applicatif pour extraire les Artefacts NotebookLM
pub struct ArtifactEngine {
    api: NotebookApi,
}

impl ArtifactEngine {
    pub fn new(api: NotebookApi) -> Self {
        Self { api }
    }

    /// Exécute souverainement l'ensemble de la machinerie: Création -> Polling -> Téléchargement
    pub async fn forge_flashcards(
        &self,
        notebook_uuid: String,
        prompt: String,
        quantity: QuizQuantity,
        difficulty: QuizDifficulty,
    ) -> Result<Value> {
        info!(
            "Début de la Forge Asynchrone des Flashcards (Niveau: {:?})",
            difficulty
        );

        let artifact_id = self
            .api
            .create_artifact(&notebook_uuid, Some(&prompt), quantity, difficulty)
            .await?;

        info!(
            "Flashcard Request Queued! Obtenu Artifact ID: {}",
            artifact_id
        );

        let mut attempt = 1;
        let max_attempts = 30; // 30 * 15s = 7.5 minutes timeout

        loop {
            if attempt > max_attempts {
                return Err(anyhow::anyhow!(
                    "Timeout lors du polling de l'artefact {}",
                    artifact_id
                ));
            }

            tokio::time::sleep(Duration::from_secs(15)).await;
            info!(
                "Polling statut pour l'artefact {} (Essai {}/{})",
                artifact_id, attempt, max_attempts
            );

            match self.api.list_artifacts(&notebook_uuid).await {
                Ok(artifacts) => {
                    let status_opt = artifacts
                        .iter()
                        .find(|a| a.id == artifact_id)
                        .map(|a| a.status);
                    if let Some(status) = status_opt {
                        info!("Statut actuel: {}", status);
                        match status {
                            ArtifactStatus::Completed => {
                                info!(
                                    "✅ Artefact {} terminé ! Téléchargement en cours...",
                                    artifact_id
                                );
                                break;
                            }
                            ArtifactStatus::Failed => {
                                return Err(anyhow::anyhow!(
                                    "La génération de l'artefact a échoué côté serveur."
                                ));
                            }
                            _ => {
                                // Continue polling
                            }
                        }
                    } else {
                        info!("Artefact {} non encore listé...", artifact_id);
                    }
                }
                Err(e) => {
                    warn!(
                        "Erreur lors du polling, on retente au prochain cycle : {}",
                        e
                    );
                }
            }
            attempt += 1;
        }
        let data = self
            .api
            .fetch_artifact_data(&notebook_uuid, &artifact_id)
            .await?;

        info!("Payload JSON métier pur intercepté.");

        Ok(data)
    }
}
