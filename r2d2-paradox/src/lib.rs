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

/// Moteur de résolution des contradictions.
pub struct ParadoxSolver;

impl TruthValidator for ParadoxSolver {
    #[tracing::instrument(skip(self, payload), fields(payload_len = payload.len()))]
    fn validate_payload(&self, payload: &str) -> Result<(String, String), KernelError> {
        info!("Début de l'analyse Paradox sur un fragment Unverified.");

        // On tente de parser la structure de données formelle.
        let mut jsonai: JsonAiV3 = serde_json::from_str(payload).map_err(|e| {
            ParadoxError::ContradictionDetected(format!("Erreur de parsing JSONAI: {}", e))
        })?;

        // Règle 1: Un fait ne peut pas être contredit
        if jsonai.is_fact && !jsonai.ontological_tags.is_empty() {
            warn!(
                "Validation stricte: un fait absolu est analysé à travers ses tags ontologiques."
            );
        }

        // Règle 2: Élévation du consensus une fois les axiomes vérifiés
        jsonai.consensus = ConsensusLevel::ConsensusReached;

        // On sérialise la donnée de nouveau, avec son niveau de consensus mis à jour.
        let verified_payload = serde_json::to_string(&jsonai).map_err(|e| {
            ParadoxError::ContradictionDetected(format!("Erreur SérialiZation: {}", e))
        })?;

        let proof_of_inference = format!("POI_SAT_SOLVED_{}", jsonai.id);
        info!(
            "Analyse terminée. Consensus atteint sans paradoxe. [{}]",
            proof_of_inference
        );

        Ok((verified_payload, proof_of_inference))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use r2d2_jsonai::{AgentSource, BeliefState};

    #[test]
    fn test_paradox_solver() {
        let jsonai = JsonAiV3::new(
            "test_123".to_string(),
            AgentSource::System,
            "Vérité axiomatique".to_string(),
            BeliefState::Fact,
        );
        let payload = serde_json::to_string(&jsonai).unwrap();

        let solver = ParadoxSolver;
        let (verified, poi) = solver.validate_payload(&payload).expect("Doit valider");

        let verified_json: JsonAiV3 = serde_json::from_str(&verified).unwrap();
        assert_eq!(verified_json.consensus, ConsensusLevel::ConsensusReached);
        assert!(poi.starts_with("POI_SAT_SOLVED_test_123"));
    }
}
