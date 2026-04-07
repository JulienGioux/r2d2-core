use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CortexError {
    // ---- Erreurs Métier Pures (Domain Logic) ----
    #[error("Daemon MCP distant indisponible : {0}")]
    McpDaemonFault(String),

    #[error("Erreur d'initialisation MCP : {0}")]
    McpInitializationError(String),

    #[error("Erreur de Base de Données (Blackboard/Paradox) : {0}")]
    Database(String),

    #[error("Le modèle '{0}' est introuvable ou absent du store local.")]
    ModelNotFound(String),

    #[error("Modèle déjà déchargé ou inactif.")]
    NotActive,

    #[error("Erreur d'inférence (Moteur Tensoriel local) : {0}")]
    InferenceError(String),

    #[error("Panique Mathématique Interne (Candle Tensor) : L'exécution a été mortellement stoppée aux bornes C-Binding. Intercepté via Bulkhead. Détails: {0}")]
    InferencePanic(String),

    #[error("Erreur de chargement des poids du tenseur en mémoire : {0}")]
    LoadError(String),

    #[error("Fichier de poids introuvable au chemin : {0}")]
    WeightsNotFound(PathBuf),

    #[error("Erreur liée au Tokenizer : {0}")]
    TokenizerError(String),

    #[error("Erreur spécifique liée au sous-type ou au décodage : {0}")]
    ComponentDecouplingError(String),

    // ---- Encapsulation Typée des Erreurs d'Infrastructure Internes ----
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error(transparent)]
    Serialization(#[from] serde_json::Error),

    #[error(transparent)]
    Network(#[from] reqwest::Error),

    #[error("Erreur Bytemuck (Manipulation de pointeurs GPU/Tenseurs) : {0}")]
    TensorCastFail(String),
}
