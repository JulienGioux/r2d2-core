use std::future::Future;
use std::pin::Pin;

/// Vecteur Mathématique Sémantique
#[derive(Debug, Clone)]
pub struct EmbeddingVector {
    pub data: Vec<f32>,
    pub dimension: usize,
}

/// Erreur standardisée de domaine pour les agents
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Erreur d'inférence du modèle : {0}")]
    InferenceError(String),
    #[error("Ressource indisponible ou déconnectée : {0}")]
    ResourceUnavailable(String),
    #[error("Erreur d'encodage/décodage sémantique : {0}")]
    EncodingError(String),
}

/// Port d'Extraction Sémantique (Embedder)
pub trait TextEmbedder: Send + Sync {
    fn embed_text<'a>(
        &'a self,
        text: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<EmbeddingVector, DomainError>> + Send + 'a>>;

    fn expected_dimension(&self) -> usize;
}

/// Port de Raisonnement Cognitif (LLM local ou distant)
pub trait TextGenerator: Send + Sync {
    fn generate<'a>(
        &'a self,
        prompt: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, DomainError>> + Send + 'a>>;
}
