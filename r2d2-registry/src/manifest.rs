use crate::types::{
    BackendType, DomainRole, ModelFamily, ModelId, QuantizationLevel, TargetDevice, TaskTypology,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Le Passeport immuable d'un Modèle R2D2 (Le Manifest.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    pub identity: ModelIdentity,
    pub topology: ModelTopology,
    pub storage: StorageConfig,
    pub format: TaskTypology, // LE PIVOT DE SÉCURITÉ
    pub metrics: Option<ModelMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelIdentity {
    pub uuid: Uuid,
    pub name: ModelId,
    pub domain_role: DomainRole,
    pub version: String,
    pub family: ModelFamily,
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTopology {
    pub backend: BackendType,
    pub device: TargetDevice,
    pub architecture: String,
    pub quantization: QuantizationLevel,
    pub vector_dimension: Option<usize>,
    pub parameters: Option<u64>,
    pub context_window: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub weights_path: Option<String>,
    pub tokenizer_path: Option<String>,
    pub config_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    pub optimal_tasks: Vec<String>,
    pub training_loss: Option<f32>,
    pub bench_tok_sec: Option<f32>,
}

impl Default for ModelManifest {
    fn default() -> Self {
        Self {
            identity: ModelIdentity {
                uuid: Uuid::new_v4(),
                name: ModelId("Unknown-Model".to_string()),
                domain_role: DomainRole::Generator,
                version: "0.1.0".to_string(),
                family: ModelFamily::Custom("Legacy".to_string()),
                author: None,
            },
            topology: ModelTopology {
                backend: BackendType::Mock,
                device: TargetDevice::Cpu,
                architecture: "Unknown".to_string(),
                quantization: QuantizationLevel::Fp32,
                vector_dimension: None,
                parameters: None,
                context_window: None,
            },
            storage: StorageConfig {
                weights_path: None,
                tokenizer_path: None,
                config_path: None,
            },
            format: TaskTypology::CausalLm,
            metrics: None,
        }
    }
}
