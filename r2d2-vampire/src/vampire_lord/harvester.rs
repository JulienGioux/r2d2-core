use super::notebook_api::NotebookApi;
use r2d2_blackboard::{FlashcardTaskRow, PostgresBlackboard, TaskState};
use std::sync::Arc;
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
                    .update_flashcard_task_status(&task.id, TaskState::Expired, None)
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
                            )
                            .await;
                    }
                    Ok(Err(e)) => {
                        error!("❌ Erreur RPC Google: {}", e);
                        let _ = self
                            .blackboard
                            .update_flashcard_task_status(&task.id, TaskState::Failed, None)
                            .await;
                    }
                    Err(_) => {
                        error!("⏱️ Timeout Circuit Breaker pendant `generate_flashcards` !");
                        let _ = self
                            .blackboard
                            .update_flashcard_task_status(&task.id, TaskState::Failed, None)
                            .await;
                    }
                }
            }
            TaskState::Generating => {
                // Poll des artifacts pour voir s'il est prêt
                info!("🔍 Vérification du statut de la génération de {}", task.id);
                // Dans notebooklm, list_artifacts contient un statut (is_completed, is_generating)
                let poll_future = api.list_artifacts(&task.expert_id);
                if let Ok(Ok(_list_res)) = timeout(Duration::from_secs(10), poll_future).await {
                    // Si on repère que la flashcard est prete, on met "Completed"
                    // (Simplification pour l'instant : on s'attend à ce que le Worker télécharge ou marque)
                }
            }
            _ => {}
        }
    }
}
