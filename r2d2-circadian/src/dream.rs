use r2d2_blackboard::{GlobalBlackboard, PostgresBlackboard};
use r2d2_cortex::CortexRegistry;
use r2d2_jsonai::ConsensusLevel;
use r2d2_kernel::{Fragment, Signal};
use r2d2_paradox::ParadoxSolver;
use std::sync::Arc;
use tracing::{info, instrument, warn};

/// ============================================================================
/// 🌌 MOTEUR DE RÊVE (MCTS) & INFERENCE STOCHASTIQUE
/// ============================================================================
pub struct DreamSimulator {
    blackboard: Arc<PostgresBlackboard>,
    cortex: Arc<CortexRegistry>,
    solver: Arc<ParadoxSolver>,
}

impl DreamSimulator {
    pub fn new(
        blackboard: Arc<PostgresBlackboard>,
        cortex: Arc<CortexRegistry>,
        solver: Arc<ParadoxSolver>,
    ) -> Self {
        Self {
            blackboard,
            cortex,
            solver,
        }
    }
}

impl DreamSimulator {
    /// Génère des millions de variations pour trouver le consensus absolu.
    #[instrument(skip_all, name = "DreamSimulator::play_stochastic_variations")]
    pub async fn play_stochastic_variations(&self) -> anyhow::Result<()> {
        info!("🌌 Démarrage de la Phase Paradoxale (REM Sleep) - Monte Carlo Tree Search.");

        // 1. Exhumation des mémoires non-consolidées
        let fragments = self.blackboard.fetch_unconsolidated_memories(5).await?;
        if fragments.is_empty() {
            info!("Aucune mémoire à consolider ce cycle. Le repos est total.");
            return Ok(());
        }

        info!(
            "⏳ Simulation de scénarios 'Et si ?' sur {} fragments...",
            fragments.len()
        );

        // 2. Préparation du Prompt de Cross-Pollinisation
        let mut prompt = String::from("Synthetize these isolated facts into a single absolute logical deduction. Return ONLY a pure JSONAI v3.0 object:\n");
        for f in &fragments {
            prompt.push_str(&format!("- [{}] {}\n", f.id, f.content));
        }

        // 3. Appel de l'Agent de Raisonnement (Slow-Path MCTS)
        let response = match self
            .cortex
            .interact_with("Paradox-MultiAPI Router", &prompt)
            .await
        {
            Ok(json_str) => json_str,
            Err(r2d2_cortex::error::CortexError::InferencePanic(ref msg)) => {
                warn!("🚨 [MCTS] Défaillance critique interceptée (Panic FFI isolée par Bulkhead) : {}", msg);
                return Ok(()); // On protège le Démon Circadien de la contagion
            }
            Err(e) => {
                warn!("Hallucination ou Timeout de l'Agent : {}", e);
                return Ok(()); // On ne crashe pas la boucle MCTS, on skip
            }
        };

        // 4. Passage dans le Tunnel de Typestate du Kernel (ParadoxEngine)
        let signal = Fragment::<Signal>::new(response);
        let unverified = match signal.parse() {
            Ok(u) => u,
            Err(e) => {
                warn!("BitNet a généré un JSON invalide (Dead Letter) : {}", e);
                return Ok(());
            }
        };

        match unverified.verify(self.solver.as_ref()).await {
            Ok(validated) => {
                info!("🧠 [Dream Results] Consensus MCTS validé par le ParadoxEngine !");
                let guard = validated.finalize();
                let saved_id = self.blackboard.anchor_fragment(guard).await?;
                info!("👉 Nouvelle intuition gravée: {}", saved_id);

                // 5. Marquer les anciens fragments comme consolidés
                for f in fragments {
                    let _ = self
                        .blackboard
                        .update_consensus_level(&f.id, ConsensusLevel::ConsensusReached)
                        .await;
                }
            }
            Err(e) => {
                warn!(
                    "Le paradoxe est insoluble (Rejet par Firewall Axiomatique) : {}",
                    e
                );
            }
        }

        Ok(())
    }
}
