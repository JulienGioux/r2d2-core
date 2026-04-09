use crate::types::{ModelFamily, ModelId, QuantizationLevel, TaskTypology};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Le Passeport immuable d'un Modèle R2D2 (Le Manifest.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    pub identity: ModelIdentity,
    pub topology: ModelTopology,
    pub format: TaskTypology, // LE PIVOT DE SÉCURITÉ
    pub metrics: Option<ModelMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelIdentity {
    /// L'Identifiant unique universel pour lier la BDD, le Front et le MLOps
    pub uuid: Uuid,
    /// Nom humainement lisible du modèle
    pub name: ModelId,
    /// Versioning semantique (ex: "1.0.0")
    pub version: String,
    /// Famille du modèle
    pub family: ModelFamily,
    /// Auteur ou origine de l'inférence
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTopology {
    /// Description de l'architecture backend
    pub architecture: String,
    /// Niveau de quantification des poids
    pub quantization: QuantizationLevel,
    /// Nombre de paramètres globaux (ex: 3_000_000_000 pour 3B)
    pub parameters: Option<u64>,
    /// Fenêtre de contexte maximale d'inférence
    pub context_window: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    /// Tâches de prédilection (ex: "reasoning", "coding", "rag")
    pub optimal_tasks: Vec<String>,
    /// Score Loss de fin d'entraînement
    pub training_loss: Option<f32>,
    /// Débit mesuré en tokens/sec (Benchmark purement indicatif)
    pub bench_tok_sec: Option<f32>,
}

impl Default for ModelManifest {
    fn default() -> Self {
        Self {
            identity: ModelIdentity {
                uuid: Uuid::new_v4(),
                name: ModelId("Unknown-Model".to_string()),
                version: "0.1.0".to_string(),
                family: ModelFamily::Custom("Legacy".to_string()),
                author: None,
            },
            topology: ModelTopology {
                architecture: "Unknown".to_string(),
                quantization: QuantizationLevel::Fp32,
                parameters: None,
                context_window: None,
            },
            format: TaskTypology::CausalLm,
            metrics: None,
        }
    }
}
