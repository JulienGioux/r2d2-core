//! # Brique 7 : Le Blackboard Persistant
//!
//! Le pattern Blackboard gère l'ancrage des Fragments Validés dans la base de données.
//! Cette version utilise PostgreSQL 16+ et pgvector.
//! Désormais propulsé par un Acteur MPSC Isolé pour garantir zéro "Task Starvation".

pub mod actor;

use actor::{AnchorRequest, BlackboardActor, BlackboardCommand};
use anyhow::Result;
use async_trait::async_trait;
use pgvector::Vector;
use r2d2_jsonai::{ConsensusLevel, JsonAiV3};
use r2d2_kernel::Persistent;
use r2d2_secure_mem::SecureMemGuard;
use sqlx::postgres::PgPoolOptions;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, instrument};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelDbRow {
    pub id: String,
    pub name: String,
    pub model_type: String,
    pub provider: String,
    pub config_json: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpToolDbRow {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args_json: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum TaskState {
    Queued,
    Generating,
    Completed,
    Failed,
    Expired,
}

impl std::str::FromStr for TaskState {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "QUEUED" => Ok(TaskState::Queued),
            "GENERATING" => Ok(TaskState::Generating),
            "COMPLETED" => Ok(TaskState::Completed),
            "FAILED" => Ok(TaskState::Failed),
            "EXPIRED" => Ok(TaskState::Expired),
            _ => Err(format!("Statut de tâche inconnu: {}", s)),
        }
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TaskState::Queued => "QUEUED",
            TaskState::Generating => "GENERATING",
            TaskState::Completed => "COMPLETED",
            TaskState::Failed => "FAILED",
            TaskState::Expired => "EXPIRED",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FlashcardTaskRow {
    pub id: String, // UUID
    pub expert_id: String,
    pub prompt: Option<String>,
    pub difficulty: Option<i32>,
    pub quantity: Option<i32>,
    pub status: TaskState,
    pub google_task_id: Option<String>,
    pub created_at: sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>,
    pub expires_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
}

#[derive(Debug, Error)]
pub enum BlackboardError {
    #[error("Erreur de connexion à la base de données: {0}")]
    ConnectionError(String),
    #[error("Erreur SQL: {0}")]
    QueryError(#[from] sqlx::Error),
    #[error("Erreur de sérialisation JSON: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Le moteur BlackboardActor est hors ligne")]
    ActorDead,
}

#[async_trait]
pub trait GlobalBlackboard {
    async fn anchor_fragment(
        &self,
        guard: SecureMemGuard<Persistent>,
    ) -> Result<String, BlackboardError>;
}

/// Implémentation Proxy du Blackboard (Zero-Logic, Fire-and-Forget)
#[derive(Clone)]
pub struct PostgresBlackboard {
    sender: mpsc::Sender<BlackboardCommand>,
}

impl PostgresBlackboard {
    pub async fn new(database_url: &str) -> Result<Self, BlackboardError> {
        let max_conn = std::env::var("DATABASE_MAX_CONN")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(20);

        let pool = PgPoolOptions::new()
            .max_connections(max_conn)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect_lazy(database_url)
            .map_err(|e| BlackboardError::ConnectionError(e.to_string()))?;

        // Canal borné (Backpressure)
        let (tx, rx) = mpsc::channel(128);

        let actor = BlackboardActor::new(pool, rx);
        tokio::spawn(async move {
            actor.run().await;
        });

        Ok(Self { sender: tx })
    }

    pub async fn initialize_registry_tables(&self) -> Result<(), BlackboardError> {
        info!("🔧 Vérification des tables du Registre Sovereign (No-DDL Mode)...");
        Ok(())
    }
}

#[async_trait]
impl GlobalBlackboard for PostgresBlackboard {
    #[instrument(skip(self, guard))]
    async fn anchor_fragment(
        &self,
        guard: SecureMemGuard<Persistent>,
    ) -> Result<String, BlackboardError> {
        let persistent = guard.expose_payload();
        let jsonai: JsonAiV3 = serde_json::from_str(&persistent.payload)?;
        let embedding = Vector::from(persistent.embedding.clone());
        let payload_value = serde_json::to_value(&jsonai)?;

        let proof = persistent.proof_of_inference.clone();

        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::AnchorFragment {
                req: AnchorRequest {
                    id: jsonai.id.clone(),
                    source: format!("{:?}", jsonai.source),
                    timestamp: jsonai.timestamp.to_rfc3339(),
                    is_fact: jsonai.is_fact,
                    belief_state: format!("{:?}", jsonai.belief_state),
                    consensus_level: format!("{:?}", jsonai.consensus),
                    payload_value,
                    embedding,
                    proof_of_inference: proof,
                },
                reply_to,
            })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }
}

impl PostgresBlackboard {
    #[instrument(skip(self, query_embedding))]
    pub async fn recall_memory(
        &self,
        query_embedding: Vector,
        limit: i64,
    ) -> Result<Vec<String>, BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::RecallMemory {
                query_embedding,
                limit,
                reply_to,
            })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }

    #[instrument(skip(self))]
    pub async fn fetch_unconsolidated_memories(
        &self,
        limit: i64,
    ) -> Result<Vec<JsonAiV3>, BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::FetchUnconsolidated { limit, reply_to })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }

    #[instrument(skip(self))]
    pub async fn update_consensus_level(
        &self,
        id: &str,
        new_level: ConsensusLevel,
    ) -> Result<(), BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::UpdateConsensus {
                id: id.to_string(),
                new_level,
                reply_to,
            })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }

    #[instrument(skip(self, embedding))]
    pub async fn save_reflex(
        &self,
        embedding: Vector,
        action_payload: &str,
    ) -> Result<(), BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::SaveReflex {
                embedding,
                action_payload: action_payload.to_string(),
                reply_to,
            })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }

    #[instrument(skip(self, query_embedding))]
    pub async fn find_matching_reflex(
        &self,
        query_embedding: Vector,
        threshold: f32,
    ) -> Result<Option<(String, f32)>, BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::FindMatchingReflex {
                query_embedding,
                threshold,
                reply_to,
            })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }

    #[instrument(skip(self))]
    pub async fn compress_vector_index(&self) -> Result<usize, BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::CompressVectorIndex { reply_to })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }

    #[instrument(skip(self))]
    pub async fn get_all_models(&self) -> Result<Vec<ModelDbRow>, BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::GetAllModels { reply_to })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }

    #[instrument(skip(self))]
    pub async fn get_all_mcp_tools(&self) -> Result<Vec<McpToolDbRow>, BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::GetAllMcpTools { reply_to })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }

    pub async fn enable_model(&self, id: &str, enable: bool) -> Result<(), BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::EnableModel {
                id: id.to_string(),
                enable,
                reply_to,
            })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }

    pub async fn enable_mcp_tool(&self, id: &str, enable: bool) -> Result<(), BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::EnableMcpTool {
                id: id.to_string(),
                enable,
                reply_to,
            })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }

    pub async fn add_mcp_tool(
        &self,
        name: &str,
        command: &str,
        args_json: &str,
    ) -> Result<String, BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::AddMcpTool {
                name: name.to_string(),
                command: command.to_string(),
                args_json: args_json.to_string(),
                reply_to,
            })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }

    pub async fn delete_mcp_tool(&self, id: &str) -> Result<(), BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::DeleteMcpTool {
                id: id.to_string(),
                reply_to,
            })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }

    #[instrument(skip(self))]
    pub async fn enqueue_flashcard_task(
        &self,
        expert_id: &str,
        prompt: Option<String>,
        difficulty: Option<i32>,
        quantity: Option<i32>,
    ) -> Result<String, BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::EnqueueFlashcardTask {
                expert_id: expert_id.to_string(),
                prompt,
                difficulty,
                quantity,
                reply_to,
            })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }

    #[instrument(skip(self))]
    pub async fn update_flashcard_task_status(
        &self,
        id: &str,
        new_status: TaskState,
        google_task_id: Option<String>,
    ) -> Result<(), BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::UpdateFlashcardTaskStatus {
                id: id.to_string(),
                new_status: new_status.to_string(),
                google_task_id,
                reply_to,
            })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }

    #[instrument(skip(self))]
    pub async fn get_pending_flashcard_tasks(
        &self,
    ) -> Result<Vec<FlashcardTaskRow>, BlackboardError> {
        let (reply_to, rx) = oneshot::channel();
        self.sender
            .send(BlackboardCommand::GetPendingFlashcardTasks { reply_to })
            .await
            .map_err(|_| BlackboardError::ActorDead)?;

        rx.await.map_err(|_| BlackboardError::ActorDead)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blackboard_error_format() {
        let err = BlackboardError::ActorDead;
        assert_eq!(err.to_string(), "Le moteur BlackboardActor est hors ligne");
    }
}
