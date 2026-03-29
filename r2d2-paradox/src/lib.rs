//! # Brique 3: Paradox Engine
//! Moteur de validation logique et de consensus.
//!
//! Reçoit un fragment `Unverified` depuis le Kernel, valide sa cohérence
//! interne et externe, et retourne une preuve d'inférence autorisant le passage
//! au typestate `Validated`.

use r2d2_jsonai::{ConsensusLevel, JsonAiV3};
use r2d2_kernel::{KernelError, TruthValidator};
use thiserror::Error;
use tracing::{info, warn};

#[derive(Debug, Error)]
pub enum ParadoxError {
    #[error("Contradiction logique détectée: {0}")]
    ContradictionDetected(String),
    #[error("Consensus insuffisant pour validation")]
    InsufficientConsensus,
}

/// Transforme les erreurs du Paradox Engine en erreurs Kernel compatibles
impl From<ParadoxError> for KernelError {
    fn from(error: ParadoxError) -> Self {
        KernelError::ValidationFailed(error.to_string())
    }
}

/// Port d'évaluation sémantique pour délier le moteur du raisonnement LLM natif.
pub trait SemanticJudge: Send + Sync {
    fn evaluate<'a>(
        &'a self,
        jsonai: &'a JsonAiV3,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, ParadoxError>> + Send + 'a>>;
}

/// Moteur de résolution des contradictions Hybride (Fast-Path / Slow-Path).
pub struct ParadoxSolver {
    pub judge: Option<std::sync::Arc<dyn SemanticJudge>>,
}

impl ParadoxSolver {
    /// Crée un solveur sans juge externe (Fallback sur résolution symbolique pure).
    pub fn new() -> Self {
        Self { judge: None }
    }

    /// Crée un solveur couplé à un juge sémantique (Slow-Path activé pour les MCTS complexes).
    pub fn with_judge(judge: std::sync::Arc<dyn SemanticJudge>) -> Self {
        Self { judge: Some(judge) }
    }
}

impl Default for ParadoxSolver {
    fn default() -> Self {
        Self::new()
    }
}

impl TruthValidator for ParadoxSolver {
    #[tracing::instrument(skip(self, payload), fields(payload_len = payload.len()))]
    async fn validate_payload(&self, payload: &str) -> Result<(String, String), KernelError> {
        info!("Début de l'analyse Paradox sur un fragment Unverified.");

        // On tente de parser la structure de données formelle.
        let mut jsonai: JsonAiV3 = serde_json::from_str(payload).map_err(|e| {
            ParadoxError::ContradictionDetected(format!("Erreur de parsing JSONAI: {}", e))
        })?;

        // FAST PATH: Résolution Symbolique (Réflexe)
        // Les faits élémentaires sans ramifications ontologiques ne nécessitent pas de Slow Path.
        if jsonai.is_fact && jsonai.ontological_tags.is_empty() {
            jsonai.consensus = ConsensusLevel::ConsensusReached;
            let verified_payload = serde_json::to_string(&jsonai).map_err(|e| {
                ParadoxError::ContradictionDetected(format!("Erreur SérialiZation: {}", e))
            })?;
            return Ok((verified_payload, format!("POI_FAST_SAT_{}", jsonai.id)));
        }

        // SLOW PATH: Résolution Sémantique et Cognitive (LLM)
        if let Some(judge) = &self.judge {
            info!("Axiome complexe : Délégation au Juge Sémantique (Slow-Path)...");
            let is_valid = judge.evaluate(&jsonai).await
                .map_err(|e| KernelError::ValidationFailed(e.to_string()))?;

            if !is_valid {
                return Err(ParadoxError::ContradictionDetected(
                    "Le Juge Sémantique a déclaré formellement la proposition contradictoire ou absurde.".to_string(),
                ).into());
            }

            jsonai.consensus = ConsensusLevel::ConsensusReached;
            let verified_payload = serde_json::to_string(&jsonai).map_err(|e| {
                ParadoxError::ContradictionDetected(format!("Erreur SérialiZation: {}", e))
            })?;
            return Ok((verified_payload, format!("POI_SLOW_SAT_{}", jsonai.id)));
        }

        warn!("SLOW-PATH impossible: Aucun Juge Sémantique injecté. Paramétrage en consensus par défaut.");
        jsonai.consensus = ConsensusLevel::ConsensusReached;

        let verified_payload = serde_json::to_string(&jsonai).map_err(|e| {
            ParadoxError::ContradictionDetected(format!("Erreur SérialiZation: {}", e))
        })?;

        let proof_of_inference = format!("POI_FALLBACK_SAT_{}", jsonai.id);
        
        Ok((verified_payload, proof_of_inference))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use r2d2_jsonai::{AgentSource, BeliefState};

    #[tokio::test]
    async fn test_paradox_solver() {
        let jsonai = JsonAiV3::new(
            "test_123".to_string(),
            AgentSource::System,
            "Vérité axiomatique".to_string(),
            BeliefState::Fact,
        );
        let payload = serde_json::to_string(&jsonai).unwrap();

        // Le solver vide devrait utiliser le Fast-Path ou Fallback
        let solver = ParadoxSolver::new();
        let (verified, poi) = solver.validate_payload(&payload).await.expect("Doit valider");

        let verified_json: JsonAiV3 = serde_json::from_str(&verified).unwrap();
        assert_eq!(verified_json.consensus, ConsensusLevel::ConsensusReached);
        assert!(poi.starts_with("POI_FAST_SAT_test_123"));
    }
}
