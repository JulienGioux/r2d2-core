//! r2d2-registry : Le Crate de Gestion MLOps des Modèles
//! Separe proprement la logique de parsing, catalogage et indexation
//! du noyau intensif de calcul CUDA/Candle.

pub mod dataset_manifest;
pub mod domain;
pub mod manager;
pub mod manifest;
pub mod types;

pub use dataset_manifest::{DatasetIdentity, DatasetManifest, DatasetMeta, ValidatedDataset};
pub use domain::{RawDataArtifact, ValidatedSample};
pub use manager::{ModelRegistry, RegistryError};
pub use manifest::{ModelIdentity, ModelManifest, ModelMetrics, ModelTopology};
pub use types::{
    EngineMode, ModelFamily, ModelId, QuantizationLevel, StateCausal, StateContrastive,
    TaskTypology,
};
