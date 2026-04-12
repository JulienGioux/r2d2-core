use thiserror::Error;

/// Ontologie souveraine des erreurs du Domaine NotebookLM
#[derive(Debug, Error)]
pub enum NotebookError {
    #[error("Erreur de communication avec l'infrastructure NotebookLM: {0}")]
    InfrastructureError(String),

    #[error("Sécurité de session compromise: {0}")]
    SessionCompromised(String),

    #[error("Carnet introuvable: {0}")]
    NotebookNotFound(String),

    #[error("Source introuvable ou inaccessible: {0}")]
    SourceNotFound(String),

    #[error("Artefact introuvable ou invalide: {0}")]
    ArtifactNotFound(String),

    #[error("Erreur de Parsing du Payload RPC: {0}")]
    PayloadParsingError(String),

    #[error("Génération échouée ou annulée (Timeout/Status: {0})")]
    GenerationFailed(String),

    #[error("Opération non supportée ou quota dépassé: {0}")]
    OperationRejected(String),
}

/// Type Result standardisé pour le Domaine
pub type Result<T> = std::result::Result<T, NotebookError>;
