//! Domain Types pour NotebookLM "Zero-Trust"
use serde::{Deserialize, Serialize};
use std::fmt;

/// Identifiant d'une requête RPC Google (obfuscated).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RpcCode(pub &'static str);

pub const CREATE_ARTIFACT: RpcCode = RpcCode("R7cb6c");
pub const LIST_ARTIFACTS: RpcCode = RpcCode("gArtLc");
pub const GET_INTERACTIVE_HTML: RpcCode = RpcCode("v9rmvd");

/// Type d'Artefact généré par le Studio de Google.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactType {
    Audio = 1,
    Report = 2,
    Video = 3,
    QuizFlashcard = 4,
    MindMap = 5,
    Infographic = 7,
    SlideDeck = 8,
    DataTable = 9,
}

impl ArtifactType {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

/// Quantité pour les Flashcards/Quiz.
/// L'API Google ne fait pas de différence entre Standard et More.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuizQuantity {
    Fewer = 1,
    Standard = 2,
}

impl QuizQuantity {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

/// Difficulté pour les Flashcards/Quiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuizDifficulty {
    Easy = 1,
    Medium = 2,
    Hard = 3,
}

impl QuizDifficulty {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

/// Statut de génération d'un Artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ArtifactStatus {
    Processing = 1,
    Pending = 2,
    Completed = 3,
    Failed = 4,
    Unknown = 99,
}

impl From<u64> for ArtifactStatus {
    fn from(val: u64) -> Self {
        match val {
            1 => ArtifactStatus::Processing,
            2 => ArtifactStatus::Pending,
            3 => ArtifactStatus::Completed,
            4 => ArtifactStatus::Failed,
            _ => ArtifactStatus::Unknown,
        }
    }
}

impl fmt::Display for ArtifactStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ArtifactStatus::Processing => "Processing",
            ArtifactStatus::Pending => "Pending",
            ArtifactStatus::Completed => "Completed",
            ArtifactStatus::Failed => "Failed",
            ArtifactStatus::Unknown => "Unknown",
        };
        write!(f, "{}", s)
    }
}

/// Structure Domain purifiée représentant l'état d'un Artefact asynchrone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactInfo {
    pub id: String,
    pub title: Option<String>,
    pub status: ArtifactStatus,
}
