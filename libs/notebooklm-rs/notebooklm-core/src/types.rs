use serde::{Deserialize, Serialize};

/// Type fortement typé pour l'ID d'un carnet
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotebookId(pub String);

/// Type fortement typé pour l'ID d'une source documentaire
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(pub String);

/// Type fortement typé pour l'ID d'un artefact généré
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtifactId(pub String);

/// Les différents types d'artefacts que NotebookLM peut générer (Code RPC)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactType {
    Flashcards = 4,
    BriefingDocument = 2,
    FAQ = 1,
    Timeline = 3,
    MindMap = 5,
}

/// Représentation de l'état d'un artefact au sein de Google
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactStatus {
    Pending,
    Running,
    Completed,
    Suggested, // Google suggère la création mais elle n'est pas lancée
    Failed(String),
}

/// Les données pures retournées pour les flashcards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flashcard {
    #[serde(rename = "f")]
    pub front: String,
    #[serde(rename = "b")]
    pub back: String,
    #[serde(rename = "c")]
    pub confidence: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashcardDeck {
    pub flashcards: Vec<Flashcard>, 
}
