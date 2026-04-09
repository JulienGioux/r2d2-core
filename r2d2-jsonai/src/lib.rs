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

/// Lien causal hypermédia entre l'entité actuelle et une Target ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub rel: OntologyRel,
    pub target_id: String,
}

impl Zeroize for Edge {
    fn zeroize(&mut self) {
        self.target_id.zeroize();
    }
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

    pub edges: Vec<Edge>,
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
            edges: Vec::new(),
        }
    }

    /// Génère un texte dense ultra-compressé, optimisé pour les FLOPs et le Vector Store.
    /// Ex : `<[Fact]> Le ciel est bleu <Requires:4fr2><Entails:18xb>`
    pub fn to_dense_embedding(&self) -> String {
        let belief_tag = match self.belief_state {
            BeliefState::Fact => "<[Fact]>",
            BeliefState::Perspective => "<[Perspective]>",
        };

        let consensus_tag = match self.consensus {
            ConsensusLevel::Raw => "<[Raw]>",
            ConsensusLevel::DebatedSynthesis => "<[Debated]>",
            ConsensusLevel::ConsensusReached => "<[Consensus]>",
        };

        let mut dense = format!("{} {} {}", belief_tag, consensus_tag, self.content);

        for edge in &self.edges {
            let rel_str = match edge.rel {
                OntologyRel::Requires => "Requires",
                OntologyRel::Entails => "Entails",
                OntologyRel::IsStepOf => "IsStepOf",
                OntologyRel::Contradicts => "Contradicts",
            };
            dense.push_str(&format!(" <{}:{}>", rel_str, edge.target_id));
        }

        dense
    }
}

// Implémentation manuelle de Zeroize pour permettre l'encapsulation dans le typestate
impl Zeroize for JsonAiV3 {
    fn zeroize(&mut self) {
        self.id.zeroize();
        self.content.zeroize();
        for edge in &mut self.edges {
            edge.zeroize();
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
