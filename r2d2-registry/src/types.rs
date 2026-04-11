use serde::{Deserialize, Serialize};
use std::fmt;

/// Représente formellement un identifiant de modèle (Pattern NewType)
/// Empêche la confusion avec des String aléatoires.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelId(pub String);

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Énumération rigide des Familles de Modèles gérées par la Forge R2D2
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFamily {
    /// Les modèles LLM purs (ex: Llama3, Mistral)
    Llama,
    /// L'architecture hybride maison (BitMamba)
    Bitmamba,
    /// Encodeurs textuels pour les RAG Memory
    Embedding,
    /// Outils de traitement Audio/Vision
    Sensory,
    /// Autres modèles non-standards
    Custom(String),
}

/// Niveau de quantification architectural
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuantizationLevel {
    Fp32,
    Fp16,
    Bf16,
    Int8,
    #[serde(rename = "w1a8")]
    Weight1Activation8,
    Int4,
    #[serde(rename = "1.58b")]
    Bit1_58,
}

/// Pivot de Sécurité : Lie mathématiquement un corpus de données à une Architektur de Tenseurs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskTypology {
    CausalLm,
    ContrastiveEmbedding,
}

/// Typestate Pattern (Interdiction à la compilation des couplages maudits)
pub trait EngineMode: Send + Sync + 'static {
    type Payload: Send + Sync;
}

/// Rôle du modèle dans l'architecture (DDD)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainRole {
    Embedder,
    Generator,
    Agent,
}

/// Dispositif d'exécution physique
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetDevice {
    Cpu,
    Gpu(usize),
}

/// Backend d'exécution physique
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendType {
    LocalCandle,
    LocalBitNet,
    RemoteMCP,
    Mock,
}

/// État représentant l'inférence causale de type "Next Token Prediction"
#[derive(Debug, Clone)]
pub struct StateCausal;
impl EngineMode for StateCausal {
    type Payload = Vec<u32>;
}

/// État représentant la structuration des vecteurs spatiaux latents
#[derive(Debug, Clone)]
pub struct StateContrastive;
impl EngineMode for StateContrastive {
    type Payload = (Vec<u32>, Vec<u32>);
}
