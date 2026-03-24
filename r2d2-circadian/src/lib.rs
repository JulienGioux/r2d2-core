pub mod dream;
pub mod firewall;
pub mod folding;
pub mod sensory;

use dream::DreamSimulator;
use firewall::AxiomaticFirewall;
use folding::FoldingEngine;
use r2d2_blackboard::PostgresBlackboard;
use r2d2_cortex::CortexRegistry;
use r2d2_paradox::ParadoxSolver;
use sensory::SensorySynthesisEngine;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

/// ============================================================================
/// ⚙️ MOTEUR CIRCADIEN (R2D2 BIOLOGICAL CYCLE)
/// ============================================================================
/// Le Démon principal qui observe le VibeVector et décide de la phase du système.
pub struct CircadianDaemon {
    sensory_engine: SensorySynthesisEngine,
    // Intervalle entre les scans métaboliques (polling)
    polling_interval: Duration,
    blackboard: Arc<PostgresBlackboard>,
    cortex: Arc<CortexRegistry>,
    solver: Arc<ParadoxSolver>,
}

impl CircadianDaemon {
    /// Initialise le moteur Circadien avec une tolérance d'entropie critique.
    pub fn new(
        critical_entropy_threshold: f32,
        interval_sec: u64,
        blackboard: Arc<PostgresBlackboard>,
        cortex: Arc<CortexRegistry>,
        solver: Arc<ParadoxSolver>,
    ) -> Self {
        Self {
            sensory_engine: SensorySynthesisEngine::new(critical_entropy_threshold),
            polling_interval: Duration::from_secs(interval_sec),
            blackboard,
            cortex,
            solver,
        }
    }

    /// Lance la boucle infinie d'homéostasie.
    /// Tourne en asynchrone pour ne pas bloquer le Kernel.
    pub async fn start_homeostasis_loop(&mut self) -> anyhow::Result<()> {
        info!("🌙 Démarrage du Démon Circadien R2D2. Surveillance du métabolisme...");

        loop {
            // Repos avant la prochaine analyse
            sleep(self.polling_interval).await;

            // 1. Perception de l'état (Dissonance, Tension, etc.)
            self.sensory_engine.perceive_swarm_state().await;

            // 2. Décision biologique (Faut-il déclencher le Deep Sleep ?)
            if self.sensory_engine.is_sleep_required() {
                warn!("⚠️ Le VibeVector franchit le seuil critique d'Entropie.");
                self.trigger_deep_sleep().await?;
            }
        }
    }

    /// Déclenche la Phase de Maintenance Lourde (Folding, Dreams, Axiomatic check).
    async fn trigger_deep_sleep(&mut self) -> anyhow::Result<()> {
        info!("💤 === DEEP SLEEP INITIÉ ===");
        info!("Blocage réseau externe. L'Hyperviseur prend la main.");

        let folding = FoldingEngine::new();
        let dream = DreamSimulator::new(
            self.blackboard.clone(),
            self.cortex.clone(),
            self.solver.clone(),
        );
        let firewall = AxiomaticFirewall::new();

        // 1. Dédoublonnement Sémantique
        folding.compress_memory().await?;

        // 2. Inférence Paradoxale
        dream.play_stochastic_variations().await?;

        // 3. Vérification Finale d'Immunité
        firewall.verify_core_integrity().await?;

        // Fin de la nuit.
        info!("☀️ === RÉVEIL DU SYSTÈME ===");
        self.sensory_engine.reset_homeostasis();

        Ok(())
    }
}