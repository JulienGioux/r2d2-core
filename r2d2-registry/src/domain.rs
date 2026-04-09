use std::borrow::Cow;
use std::marker::PhantomData;

/// Représente une donnée brute moissonnée par le Vampire (r2d2-mcp).
/// Zero-Copy: Utilise une structure Cow pour minimiser les allocations
/// lors de l'acheminement réseau -> métier.
#[derive(Debug, Clone)]
pub struct RawDataArtifact {
    pub payload: Cow<'static, str>,
    pub source_hash: String,
}

use crate::types::EngineMode;

/// Échantillon unitaire validé, naviguant de la Forge vers le Cortex
/// Remplace `synthetic_dataset.jsonl` pour un flux pur "Zéro-Fichier".
#[derive(Debug, Clone)]
pub struct ValidatedSample<M: EngineMode> {
    pub _state: PhantomData<M>,
    pub payload: M::Payload,
}
