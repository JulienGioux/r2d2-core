pub mod dream;
pub mod firewall;
pub mod folding;
pub mod sensory;
pub mod sys_monitor;

use dream::DreamSimulator;
use firewall::AxiomaticFirewall;
use folding::FoldingEngine;
use r2d2_blackboard::PostgresBlackboard;
use r2d2_cortex::CortexRegistry;
use r2d2_paradox::ParadoxSolver;
use sensory::{SensorySynthesisEngine, VibeVector};
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
    /// Initialise le moteur Circadien et retourne le canal pour lire les battements du système.
    pub fn new(
        critical_entropy_threshold: f32,
        interval_sec: u64,
        blackboard: Arc<PostgresBlackboard>,
        cortex: Arc<CortexRegistry>,
        solver: Arc<ParadoxSolver>,
    ) -> (Self, tokio::sync::watch::Receiver<Arc<VibeVector>>) {
        let mut sensory_engine = SensorySynthesisEngine::new(critical_entropy_threshold);
        let (tx, rx) = tokio::sync::watch::channel(Arc::new(VibeVector::default()));
        sensory_engine.set_broadcaster(tx);

        (
            Self {
                sensory_engine,
                polling_interval: Duration::from_secs(interval_sec),
                blackboard,
                cortex,
                solver,
            },
            rx,
        )
    }

    /// Lance la boucle infinie d'homéostasie.
    /// Accepte un canal `shutdown` pour un arrêt propre (Graceful Shutdown coordonné avec Axum).
    pub async fn start_homeostasis_loop(
        &mut self,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> anyhow::Result<()> {
        info!("🌙 Démarrage du Démon Circadien (Supervisé) R2D2. Surveillance...");

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("🛑 [CircadianDaemon] Signal d'arrêt reçu. Arrêt gracieux du système homéostatique...");
                        break;
                    }
                }
                _ = sleep(self.polling_interval) => {
                    // 1. Perception de l'état (Dissonance, Tension, etc.)
                    self.sensory_engine.perceive_swarm_state().await;

                    // 2. Décision biologique (Faut-il déclencher le Deep Sleep ?)
                    if self.sensory_engine.is_sleep_required() {
                        warn!("⚠️ Le VibeVector franchit le seuil critique d'Entropie.");
                        self.trigger_deep_sleep().await?;
                    }
                }
            }
        }

        info!("✅ [CircadianDaemon] Arrêté en toute sécurité.");
        Ok(())
    }

    /// Déclenche la Phase de Maintenance Lourde (Folding, Dreams, Axiomatic check).
    async fn trigger_deep_sleep(&mut self) -> anyhow::Result<()> {
        info!("💤 === DEEP SLEEP INITIÉ ===");
        info!("Blocage réseau externe. L'Hyperviseur prend la main.");

        let folding = FoldingEngine::new(self.blackboard.clone());
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
