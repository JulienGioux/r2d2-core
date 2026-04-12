use super::notebook_api::NotebookApi;
use r2d2_blackboard::{FlashcardTaskRow, PostgresBlackboard, TaskState};
use std::sync::Arc;
use tokio::fs;
use std::path::Path;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout, Duration};
use tracing::{error, info, instrument, warn};

/// Le Harvester est le Worker asynchrone ("Spawn & Abort" pattern).
/// Il se réveille sur notification MPSC ou périodiquement (Fallback).
pub struct Harvester {
    blackboard: PostgresBlackboard,
    notebook_api: Option<Arc<NotebookApi>>, // Optionnel car le Harvester doit être résilient même sans CDP initialisé au démarrage
    notify_rx: mpsc::Receiver<()>,
}

impl Harvester {
    pub fn new(blackboard: PostgresBlackboard, notify_rx: mpsc::Receiver<()>) -> Self {
        Self {
            blackboard,
            notebook_api: None,
            notify_rx,
        }
    }

    pub fn attach_cdp(&mut self, api: Arc<NotebookApi>) {
        self.notebook_api = Some(api);
    }

    /// Boucle Principale du Harvester
    pub async fn run(mut self) {
        info!("🌾 Harvester: Orchestrateur NotebookLM Démarré en arrière-plan.");

        loop {
            // Wait for signal OR fallback timeout (30 seconds)
            tokio::select! {
                _ = self.notify_rx.recv() => {
                    info!("🌾 Harvester réveillé par une notification MPSC !");
                }
                _ = sleep(Duration::from_secs(30)) => {
                    // Periodic poll fallback
                }
            }

            self.process_pending_tasks().await;
        }
    }

    async fn process_pending_tasks(&self) {
        if self.notebook_api.is_none() {
            warn!(
                "🌾 Harvester: NotebookApi non attaché, impossible de traiter les tâches Google."
            );
            return;
        }
        let api = self.notebook_api.as_ref().unwrap();

        match self.blackboard.get_pending_flashcard_tasks().await {
            Ok(tasks) => {
                for task in tasks {
                    self.process_single_task(api, task).await;
                }
            }
            Err(e) => error!("🌾 Harvester DB Error: {}", e),
        }
    }

    #[instrument(skip(self, api))]
    async fn process_single_task(&self, api: &NotebookApi, task: FlashcardTaskRow) {
        // Validation CircuitBreaker: Si la tâche a dépassé son expiration, on la marque Failed/Expired
        if let Some(exp) = task.expires_at {
            if chrono::Utc::now() > exp {
                warn!("⏳ Tâche Flashcards {} expirée !", task.id);
                let _ = self
                    .blackboard
                    .update_flashcard_task_status(&task.id, TaskState::Expired, None, None)
                    .await;
                return;
            }
        }

        match task.status {
            TaskState::Queued => {
                info!(
                    "🚀 Lancement du Batch: Flashcards sur l'expert {}",
                    task.expert_id
                );
                // On utilise tokio::time::timeout pour le circuit breaker au niveau RPC
                let generation_future = api.create_artifact(
                    &task.expert_id,
                    None,
                    crate::vampire_lord::types::QuizQuantity::Standard,
                    crate::vampire_lord::types::QuizDifficulty::Medium,
                );

                match timeout(Duration::from_secs(30), generation_future).await {
                    Ok(Ok(res_id)) => {
                        // NotebookLM "batchExecute" pour Generate_Flashcards renvoie l'objet Google Task Id
                        let google_task_id = Some(res_id); // res_id est maintenant String

                        let _ = self
                            .blackboard
                            .update_flashcard_task_status(
                                &task.id,
                                TaskState::Generating,
                                google_task_id,
                                None,
                            )
                            .await;
                    }
                    Ok(Err(e)) => {
                        error!("❌ Erreur RPC Google: {}", e);
                        let _ = self
                            .blackboard
                            .update_flashcard_task_status(&task.id, TaskState::Failed, None, None)
                            .await;
                    }
                    Err(_) => {
                        error!("⏱️ Timeout Circuit Breaker pendant `generate_flashcards` !");
                        let _ = self
                            .blackboard
                            .update_flashcard_task_status(&task.id, TaskState::Failed, None, None)
                            .await;
                    }
                }
            }
            TaskState::Generating => {
                let google_task_id = match &task.google_task_id {
                    Some(id) => id,
                    None => {
                        error!("Tâche {} bloque en Generating sans google_task_id !", task.id);
                        let _ = self.blackboard.update_flashcard_task_status(&task.id, TaskState::Failed, None, None).await;
                        return;
                    }
                };

                info!("🔍 Vérification de l'artéfact {} pour la tâche {}", google_task_id, task.id);
                // Le filtre réseau n'a besoin que d'un &str, on évite le clone!
                let expert_id_ref: &str = &task.expert_id;
                let poll_future = api.list_artifacts(expert_id_ref);

                match timeout(Duration::from_secs(10), poll_future).await {
                    Ok(Ok(artifacts)) => {
                        let artifact_opt = artifacts.into_iter().find(|a| a.id == *google_task_id);
                        if let Some(artifact) = artifact_opt {
                            match artifact.status {
                                crate::vampire_lord::types::ArtifactStatus::Completed => {
                                    info!("✅ Artefact {} terminé ! Téléchargement en cours...", google_task_id);
                                    match api.fetch_artifact_data(expert_id_ref, google_task_id).await {
                                        Ok(data) => {
                                            // Option A: Sauvegarde en pur fichier .json et traçabilité en DB
                                            let dir_path = format!("data/artifacts/{}", expert_id_ref);
                                            if let Err(e) = fs::create_dir_all(&dir_path).await {
                                                error!("Impossible de créer le dossier de stockage: {}", e);
                                                return;
                                            }
                                            
                                            let file_path = format!("{}/{}.json", dir_path, google_task_id);
                                            match fs::write(&file_path, serde_json::to_string_pretty(&data).unwrap_or_default()).await {
                                                Ok(_) => {
                                                    info!("💾 Artefact sauvegardé avec succès dans: {}", file_path);
                                                    // On garde le path dans le JSON du payload de la DB
                                                    let db_result = serde_json::json!({
                                                        "storage_path": file_path,
                                                        "type": "google_flashcard"
                                                    });
                                                    let _ = self.blackboard.update_flashcard_task_status(&task.id, TaskState::Completed, task.google_task_id.clone(), Some(db_result)).await;
                                                    // NB: On devrait aussi attacher db_result à la tâche. (Actuellement update_flashcard_task_status ne prend que task_id, state, google_id)
                                                    // Idéalement on ajoute une méthode `complete_task_with_result`.
                                                }
                                                Err(e) => error!("❌ Erreur d'écriture de l'artefact: {}", e),
                                            }
                                        }
                                        Err(e) => {
                                            error!("❌ L'artefact est prêt mais le téléchargement a échoué: {}", e);
                                        }
                                    }
                                }
                                crate::vampire_lord::types::ArtifactStatus::Failed => {
                                    error!("❌ NotebookLM annonce un échec (status: Failed) sur {}", google_task_id);
                                    let _ = self.blackboard.update_flashcard_task_status(&task.id, TaskState::Failed, None, None).await;
                                }
                                _ => {
                                    // Generating ou autre : on continue d'attendre au prochain cycle.
                                }
                            }
                        } else {
                            warn!("🤔 L'artefact {} n'est pas encore visible dans la liste.", google_task_id);
                        }
                    }
                    Ok(Err(e)) => {
                        error!("❌ Erreur réseau ou CDP lors du poll list_artifacts: {}", e);
                        // On pourrait compter les erreurs ou passer en failed.
                    }
                    Err(_) => {
                        warn!("⏱️ Polling Timeout (Circuit Breaker atteint). L'API rame, on retentera au cycle suivant.");
                        // Le Swallow est ici remplacé par un log explicite de Backoff.
                    }
                }
            }
            _ => {}
        }
    }
}
