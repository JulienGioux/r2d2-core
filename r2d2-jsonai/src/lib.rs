//! # Brique 1: JSONAI v3.1
//! Standard de représentation sémantique stricte.
//!
//! Implémente le typage de la vérité et l'état de croyance selon le Livre Blanc R2D2.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// Identité de l'agent ou du module ayant produit le fragment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentSource {
    Vision(String), // ex: "Gemini 2.5 Pro"
    Audio(String),  // ex: "Whisper C++"
    OCR(String),    // ex: "Tesseract"
    ParadoxEngine,  // Le solveur interne
    System,
}

/// État de croyance d'un fragment d'information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BeliefState {
    /// Fait mathématique, physique ou numérique avéré
    Fact,
    /// Synthèse probabiliste, opinion ou hypothèse
    Perspective,
}

/// Niveau de résolution du consensus épistémologique.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConsensusLevel {
    /// Jamais débattu
    Raw,
    /// Débattu, avec des contradictions résiduelles
    DebatedSynthesis,
    /// Accord unanime et non-contradictoire
    ConsensusReached,
}

/// Relation sémantique forte entre deux fragments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OntologyRel {
    Requires,
    Entails,
    IsStepOf,
    Contradicts,
}

/// Traçabilité absolue du JSONAI v3.1.
///
/// Ne contient PAS de `ZeroizeOnDrop` car cette structure est conçue pour
/// être stockée dans la Rust mémoire standard avant d'être validée et convertie.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonAiV3 {
    pub id: String,
    pub source: AgentSource,
    pub timestamp: DateTime<Utc>,

    pub is_fact: bool,
    pub belief_state: BeliefState,
    pub consensus: ConsensusLevel,

    pub content: String,

    pub ontological_tags: Vec<OntologyRel>,
    pub dependencies: Vec<String>,
}

impl JsonAiV3 {
    pub fn new(
        id: String,
        source: AgentSource,
        content: String,
        belief_state: BeliefState,
    ) -> Self {
        Self {
            id,
            source,
            timestamp: Utc::now(),
            is_fact: belief_state == BeliefState::Fact,
            belief_state,
            consensus: ConsensusLevel::Raw,
            content,
            ontological_tags: Vec::new(),
            dependencies: Vec::new(),
        }
    }
}

// Implémentation manuelle de Zeroize pour permettre l'encapsulation dans le typestate
impl Zeroize for JsonAiV3 {
    fn zeroize(&mut self) {
        self.id.zeroize();
        self.content.zeroize();
        for dep in &mut self.dependencies {
            dep.zeroize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonai_serialization() {
        let jsonai = JsonAiV3::new(
            "test_123".to_string(),
            AgentSource::System,
            "Ceci est un test".to_string(),
            BeliefState::Fact,
        );

        let serialized = serde_json::to_string(&jsonai).unwrap();
        assert!(serialized.contains("test_123"));
        assert!(serialized.contains("Ceci est un test"));
    }
}
