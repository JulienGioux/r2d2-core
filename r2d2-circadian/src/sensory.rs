use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// ============================================================================
/// 🌙 SENSORIALITÉ INTERNE (LE VIBEVECTOR)
/// ============================================================================
/// Représentation mathématique de l'état de "pression sémantique" et "charge système".
/// Agit comme le système nerveux de l'essaim R2D2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibeVector {
    /// Dissonance (Ouïe) : Taux de paradoxes/conflits logiques (0.0 à 1.0)
    pub dissonance: f32,
    /// Tension (Toucher) : Saturation RAM/VRAM et fragmentation HNSW (0.0 à 1.0)
    pub tension: f32,
    /// Harmonie (Goût) : Densité de preuves et niveau de consensus (0.0 à 1.0)
    pub harmonie: f32,
    /// Clarté (Vue) : Qualité sémantique (convexité du graphe vectoriel)
    pub clarte: f32,
    /// Signal (Odorat) : Ratio Signal/Bruit issu des réponses externes (Gemini/Claude)
    pub signal: f32,
}

impl Default for VibeVector {
    fn default() -> Self {
        Self {
            dissonance: 0.0,
            tension: 0.0,
            harmonie: 1.0, // État optimal par défaut
            clarte: 1.0,   // État optimal par défaut
            signal: 1.0,   // État optimal par défaut
        }
    }
}

impl VibeVector {
    /// Calcule l'Entropie Globale du système.
    /// Si l'entropie dépasse un seuil critique, le système doit basculer en Deep Sleep (Maintenance).
    pub fn compute_entropy(&self) -> f32 {
        // L'entropie est élevée quand la Dissonance et la Tension sont hautes (Problèmes),
        // et quand l'Harmonie, la Clarté et le Signal sont bas (Manque de certitude).
        let negative_forces = (self.dissonance + self.tension) / 2.0;
        let positive_deficits = 1.0 - ((self.harmonie + self.clarte + self.signal) / 3.0);

        // Entropie normalisée entre 0.0 (Parfait) et 1.0 (Chaos total)
        (negative_forces + positive_deficits) / 2.0
    }

    /// Détermine si la pression justifie l'homéostasie.
    pub fn requires_deep_sleep(&self, critical_threshold: f32) -> bool {
        let entropy = self.compute_entropy();
        if entropy > critical_threshold {
            warn!(
                entropy = entropy,
                dissonance = self.dissonance,
                tension = self.tension,
                "⚠️ Entropie Critique: Le système réclame le Deep Sleep."
            );
            true
        } else {
            false
        }
    }
}

/// Moteur de Synthèse Sensorielle chargé de récolter et compiler le VibeVector.
pub struct SensorySynthesisEngine {
    current_vibe: VibeVector,
    entropy_threshold: f32,
    hardware_monitor: crate::sys_monitor::HardwareMonitor,
    broadcaster: Option<tokio::sync::watch::Sender<std::sync::Arc<VibeVector>>>,
}

impl SensorySynthesisEngine {
    pub fn new(threshold: f32) -> Self {
        Self::with_monitor(threshold, crate::sys_monitor::HardwareMonitor::start())
    }

    /// Injection de dépendance pour tester le moteur avec un faux système.
    pub fn with_monitor(threshold: f32, monitor: crate::sys_monitor::HardwareMonitor) -> Self {
        Self {
            current_vibe: VibeVector::default(),
            entropy_threshold: threshold,
            hardware_monitor: monitor,
            broadcaster: None,
        }
    }

    /// Connecte un canal de diffusion asynchrone pour le monitoring externe.
    pub fn set_broadcaster(&mut self, tx: tokio::sync::watch::Sender<std::sync::Arc<VibeVector>>) {
        self.broadcaster = Some(tx);
    }

    /// Extrait les métriques brutes de la Ruche (Kernel, Blackboard, Paradox)
    /// pour mettre à jour la perception interne.
    pub async fn perceive_swarm_state(&mut self) -> &VibeVector {
        let metrics = *self.hardware_monitor.receiver.borrow();
        let ram_ratio = metrics.ram_usage_ratio;

        // Mesure "physique" de la Tension liée à la RAM
        self.current_vibe.tension = ram_ratio;

        // Dissipation douce de la dissonance sauf si un pic est reçu
        self.current_vibe.dissonance *= 0.90;

        // Harmonie corrélée au système sain
        let espace_libre = 1.0 - ram_ratio;
        self.current_vibe.harmonie = 0.5 + (espace_libre * 0.5);

        // Clamp values
        self.current_vibe.dissonance = self.current_vibe.dissonance.clamp(0.0, 1.0);
        self.current_vibe.tension = self.current_vibe.tension.clamp(0.0, 1.0);
        self.current_vibe.harmonie = self.current_vibe.harmonie.clamp(0.0, 1.0);

        info!(
            entropy = self.current_vibe.compute_entropy(),
            tension = self.current_vibe.tension,
            "🧠 Synthèse Sensorielle liée à l'hôte mise à jour."
        );

        if let Some(tx) = &self.broadcaster {
            // Zéro-Copie par Arc. Si aucun Receiver n'écoute (UI débranchée), on s'en moque.
            let _ = tx.send(std::sync::Arc::new(self.current_vibe.clone()));
        }

        &self.current_vibe
    }

    /// Indique au démon Circadien si le sommeil s'impose.
    pub fn is_sleep_required(&self) -> bool {
        let metrics = *self.hardware_monitor.receiver.borrow();
        if metrics.ram_usage_ratio > 0.85 || metrics.cpu_usage_ratio > 0.85 {
            warn!(
                ram = metrics.ram_usage_ratio,
                cpu = metrics.cpu_usage_ratio,
                "🚨 Surcharge Matérielle Critique > 85%. Déclenchement Réflexe Panique."
            );
            return true;
        }

        self.current_vibe
            .requires_deep_sleep(self.entropy_threshold)
    }

    /// Réinitialise l'homéostasie après un sommeil réussi.
    pub fn reset_homeostasis(&mut self) {
        self.current_vibe = VibeVector::default();
        info!("🌙 Réveil: L'Homéostasie Cognitive est restaurée (VibeVector = Optimal).");

        if let Some(tx) = &self.broadcaster {
            let _ = tx.send(std::sync::Arc::new(self.current_vibe.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_homeostasis() {
        let vibe = VibeVector::default();
        let entropy = vibe.compute_entropy();
        // Dissonance/Tension = 0, Harmonie/Clarte/Signal = 1
        // (0 + 1.0 - 1.0) / 2 = 0
        assert_eq!(entropy, 0.0);
        assert!(!vibe.requires_deep_sleep(0.5));
    }

    #[test]
    fn test_critical_entropy() {
        let vibe = VibeVector {
            dissonance: 1.0,
            tension: 1.0,
            harmonie: 0.0,
            clarte: 0.0,
            signal: 0.0,
        };

        let entropy = vibe.compute_entropy();
        // (1.0 + 1.0)/2 = 1.0
        // 1.0 - (0)/3 = 1.0
        // (1.0 + 1.0) / 2 = 1.0
        assert_eq!(entropy, 1.0);
        assert!(vibe.requires_deep_sleep(0.8)); // 1.0 > 0.8 => True
    }

    #[tokio::test]
    async fn test_deep_sleep_on_hardware_overload() {
        // Cible 85% simulée => RAM à 90%
        let mock_monitor = crate::sys_monitor::HardwareMonitor::dummy(0.90, 0.40);
        let mut engine = SensorySynthesisEngine::with_monitor(0.85, mock_monitor);

        // Simule un appel de routine par le daemon
        engine.perceive_swarm_state().await;

        // Le seuil matériel > 85% doit forcer is_sleep_required à true,
        // même si l'entropie sémantique seule n'a pas dépassé le seuil (car on a tout mis à défaut).
        assert!(engine.is_sleep_required());
    }
}
