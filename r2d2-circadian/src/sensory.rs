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
}

impl SensorySynthesisEngine {
    pub fn new(threshold: f32) -> Self {
        Self {
            current_vibe: VibeVector::default(),
            entropy_threshold: threshold,
        }
    }

    /// Extrait les métriques brutes de la Ruche (Kernel, Blackboard, Paradox)
    /// pour mettre à jour la perception interne.
    pub async fn perceive_swarm_state(&mut self) -> &VibeVector {
        // TODO: Extraire les KPIs depuis r2d2-kernel et r2d2-blackboard.
        // Pour l'instant, posons une perception simulée (mock) du système nerveux.
        self.current_vibe.dissonance += 0.01; // S'accumule naturellement
        self.current_vibe.tension += 0.05;    // Fragmentation mémoire
        self.current_vibe.harmonie -= 0.02;   // Dégradation lente des preuves

        // Clamp values
        self.current_vibe.dissonance = self.current_vibe.dissonance.clamp(0.0, 1.0);
        self.current_vibe.tension = self.current_vibe.tension.clamp(0.0, 1.0);
        self.current_vibe.harmonie = self.current_vibe.harmonie.clamp(0.0, 1.0);

        info!(
            entropy = self.current_vibe.compute_entropy(),
            "🧠 Synthèse Sensorielle mise à jour."
        );
        &self.current_vibe
    }

    /// Indique au démon Circadien si le sommeil s'impose.
    pub fn is_sleep_required(&self) -> bool {
        self.current_vibe.requires_deep_sleep(self.entropy_threshold)
    }

    /// Réinitialise l'homéostasie après un sommeil réussi.
    pub fn reset_homeostasis(&mut self) {
        self.current_vibe = VibeVector::default();
        info!("🌙 Réveil: L'Homéostasie Cognitive est restaurée (VibeVector = Optimal).");
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
        let mut vibe = VibeVector::default();
        vibe.dissonance = 1.0;
        vibe.tension = 1.0;
        vibe.harmonie = 0.0;
        vibe.clarte = 0.0;
        vibe.signal = 0.0;

        let entropy = vibe.compute_entropy();
        // (1.0 + 1.0)/2 = 1.0
        // 1.0 - (0)/3 = 1.0
        // (1.0 + 1.0) / 2 = 1.0
        assert_eq!(entropy, 1.0);
        assert!(vibe.requires_deep_sleep(0.8)); // 1.0 > 0.8 => True
    }
}
