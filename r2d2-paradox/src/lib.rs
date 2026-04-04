//! # Brique 3: Paradox Engine
//! Moteur de validation logique et de consensus.
//!
//! Reçoit un fragment `Unverified` depuis le Kernel, valide sa cohérence
//! interne et externe, et retourne une preuve d'inférence autorisant le passage
//! au typestate `Validated`.

pub mod reflex_judge;

use r2d2_jsonai::{ConsensusLevel, JsonAiV3};
use r2d2_kernel::{KernelError, TruthValidator};
use thiserror::Error;
use tokio::sync::Mutex;
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
    pub reflex: Option<std::sync::Arc<Mutex<reflex_judge::ReflexJudge>>>,
}

impl ParadoxSolver {
    /// Crée un solveur sans juge externe (Fallback sur résolution symbolique pure).
    pub fn new() -> Self {
        Self {
            judge: None,
            reflex: None,
        }
    }

    /// Crée un solveur couplé à un juge sémantique (Slow-Path activé pour les MCTS complexes).
    pub fn with_judge(judge: std::sync::Arc<dyn SemanticJudge>) -> Self {
        Self {
            judge: Some(judge),
            reflex: None,
        }
    }

    /// Ajoute le Système 1 (Réflexe)
    pub fn with_reflex(mut self, reflex: std::sync::Arc<Mutex<reflex_judge::ReflexJudge>>) -> Self {
        self.reflex = Some(reflex);
        self
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

        // FAST PATH: Résolution Symbolique (Réflexe) - SYSTEM 1
        let mut reflex_approved = false;
        let mut reflex_action_payload = None;

        if let Some(reflex_mutex) = &self.reflex {
            let mut reflex = reflex_mutex.lock().await;

            if let Ok(routing_action) = reflex.hybrid_evaluate(&jsonai.content).await {
                match routing_action {
                    reflex_judge::RoutingAction::Reflex(action) => {
                        reflex_approved = true;
                        reflex_action_payload = Some(action);
                    }
                    reflex_judge::RoutingAction::Cognitive(_) => {
                        // Escalade requise vers le Système 2 (LLM).
                        reflex_approved = false;
                    }
                    reflex_judge::RoutingAction::Ignore => {
                        reflex_approved = true;
                    }
                }
            } else {
                warn!("Échec de l'évaluation hybride. Fallback vers le Slow Path (Système 2).");
            }
        } else {
            // Comportement de fallback : Les faits élémentaires sans ramifications ontologiques ne nécessitent pas de Slow Path.
            if jsonai.is_fact && jsonai.ontological_tags.is_empty() {
                reflex_approved = true;
            }
        }

        if reflex_approved {
            jsonai.consensus = ConsensusLevel::ConsensusReached;

            if let Some(action) = reflex_action_payload {
                jsonai.content = format!("{} [REFLEX_ACTION: {}]", jsonai.content, action);
                info!("Action Réflexe injectée dans le payload: {}", action);
            }

            let verified_payload = serde_json::to_string(&jsonai).map_err(|e| {
                ParadoxError::ContradictionDetected(format!("Erreur SérialiZation: {}", e))
            })?;
            return Ok((verified_payload, format!("POI_FAST_SAT_{}", jsonai.id)));
        }

        // SLOW PATH: Résolution Sémantique et Cognitive (LLM) - SYSTEM 2
        if let Some(judge) = &self.judge {
            info!("Axiome complexe : Délégation au Juge Sémantique (Slow-Path)...");
            let is_valid = judge
                .evaluate(&jsonai)
                .await
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
    use std::sync::Arc;

    /// Mock Strict du Super-Juge Sémantique pour isoler la CI.
    struct MockStrictSemanticJudge {
        pub should_approve: bool,
    }

    impl SemanticJudge for MockStrictSemanticJudge {
        fn evaluate<'a>(
            &'a self,
            _jsonai: &'a JsonAiV3,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<bool, ParadoxError>> + Send + 'a>,
        > {
            let result = if self.should_approve {
                Ok(true)
            } else {
                Ok(false)
            };
            Box::pin(async move { result })
        }
    }

    #[tokio::test]
    async fn test_paradox_solver_fast_path_fallback() {
        let jsonai = JsonAiV3::new(
            "test_fast_path".to_string(),
            AgentSource::System,
            "Vérité axiomatique triviale".to_string(),
            BeliefState::Fact,
        );
        let payload = serde_json::to_string(&jsonai).unwrap();

        // Solveur sans System 2 (Juge). Devrait statuer en Fallback/Fast-path local
        let solver = ParadoxSolver::new();
        let (verified, poi) = solver
            .validate_payload(&payload)
            .await
            .expect("Doit valider localement sans erreur");

        let verified_json: JsonAiV3 = serde_json::from_str(&verified).unwrap();
        assert_eq!(verified_json.consensus, ConsensusLevel::ConsensusReached);
        assert!(poi.starts_with("POI_FAST_SAT") || poi.starts_with("POI_FALLBACK_SAT"));
    }

    #[tokio::test]
    async fn test_paradox_solver_slow_path_approval() {
        let mut jsonai = JsonAiV3::new(
            "test_ext_approve".to_string(),
            AgentSource::System,
            "Proposition complexe MCTS".to_string(),
            BeliefState::Perspective, // Force potentiellement le Slow-Path (hors facto)
        );
        jsonai
            .ontological_tags
            .push(r2d2_jsonai::OntologyRel::Requires);

        let payload = serde_json::to_string(&jsonai).unwrap();

        let mock_judge = Arc::new(MockStrictSemanticJudge {
            should_approve: true,
        });
        let solver = ParadoxSolver::with_judge(mock_judge);

        let (verified, poi) = solver
            .validate_payload(&payload)
            .await
            .expect("Le juge mocké valide");
        assert!(poi.starts_with("POI_SLOW_SAT"));
        let verified_json: JsonAiV3 = serde_json::from_str(&verified).unwrap();
        assert_eq!(verified_json.consensus, ConsensusLevel::ConsensusReached);
    }

    #[tokio::test]
    async fn test_paradox_solver_slow_path_rejection() {
        let mut jsonai = JsonAiV3::new(
            "test_ext_reject".to_string(),
            AgentSource::System,
            "Proposition absurde".to_string(),
            BeliefState::Perspective,
        );
        jsonai
            .ontological_tags
            .push(r2d2_jsonai::OntologyRel::Requires);
        let payload = serde_json::to_string(&jsonai).unwrap();

        let mock_judge = Arc::new(MockStrictSemanticJudge {
            should_approve: false,
        });
        let solver = ParadoxSolver::with_judge(mock_judge);

        let result = solver.validate_payload(&payload).await;
        assert!(result.is_err());
        let KernelError::ValidationFailed(msg) = result.unwrap_err();
        assert!(msg.contains("contradictoire ou absurde"));
    }
}
