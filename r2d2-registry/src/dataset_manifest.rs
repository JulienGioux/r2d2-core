use crate::types::EngineMode;
use crate::types::TaskTypology;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::path::PathBuf;
use uuid::Uuid;

/// Le Passeport immuable d'un Dataset R2D2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetManifest {
    pub identity: DatasetIdentity,
    pub format: TaskTypology,
    pub meta: DatasetMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetIdentity {
    pub uuid: Uuid,
    pub name: String,
    pub version: String,
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetMeta {
    pub size_bytes: u64,
    pub samples_count: usize,
    pub source_corpus: String,
}

/// Enveloppe Typestate pour sécuriser le pipeline de chargement des données.
/// Le `PhantomData` garantit que ValidatedDataset ne peut être consommé que par
/// un système s'exécutant dans le même `EngineMode` (Causal vs Contrastif).
#[derive(Debug, Clone)]
pub struct ValidatedDataset<M: EngineMode> {
    pub filepath: PathBuf,
    pub manifest: DatasetManifest,
    _marker: PhantomData<M>,
}

impl<M: EngineMode> ValidatedDataset<M> {
    /// Consomme le Manifest et garantit mathématiquement le couplage
    pub fn new(filepath: PathBuf, manifest: DatasetManifest) -> Self {
        Self {
            filepath,
            manifest,
            _marker: PhantomData,
        }
    }
}
