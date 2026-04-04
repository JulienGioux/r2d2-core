use candle_core::{Result, Tensor};
use candle_nn::{AdamW, Optimizer};
use tracing::info;

/// Wrapper intelligent "Deux Phases" autour du AdamW standard de Candle.
///
/// Implémente la doctrine "Two-Stage" pour BitNet b1.58 :
/// - **Phase 1 (Cristallisation - ex: 80% du temps)** :
///   On applique le `weight_decay` (régularisation) normal pour guider les poids vers {-1, 0, 1}.
/// - **Phase 2 (Verrouillage - ex: 20% restants)** :
///   Le `weight_decay` est forcé à 0.0. Sans force de rappel vers l'origine, les poids
///   gelent leurs oscillations autour des puits de quantification (STE).
pub struct TwoStageAdamW {
    pub inner: AdamW,
    total_steps: usize,
    phase1_ratio: f64,
    phase_switched: bool,
}

impl TwoStageAdamW {
    /// Crée l'Optimiseur Two-Stage.
    /// `phase1_ratio` : proportion du temps (ex: 0.8) avec le LR nominal et le Weight Decay.
    pub fn new(inner: AdamW, total_steps: usize, phase1_ratio: f64) -> Self {
        TwoStageAdamW {
            inner,
            total_steps,
            phase1_ratio,
            phase_switched: false,
        }
    }

    /// Effectue une étape d'optimisation classique, mais monitore la phase.
    pub fn backward_step(&mut self, loss: &Tensor, current_step: usize) -> Result<()> {
        let phase1_limit = (self.total_steps as f64 * self.phase1_ratio) as usize;

        // Si nous entrons dans la Phase 2 (Verrouillage)
        if current_step >= phase1_limit && !self.phase_switched {
            info!("🔄 Two-Stage Optimizer: Transition en Phase 2 (Verrouillage Ternaire). Désactivation du Weight Decay.");
            let mut current_params = self.inner.params().clone();
            current_params.weight_decay = 0.0;
            self.inner.set_params(current_params);
            // Si Candle < 0.9 ne supporte pas set_params ou params_mut,
            // la compilation hurlera et on devra trouver un workaround manuel.
            self.phase_switched = true;
        }

        self.inner.backward_step(loss)?;

        Ok(())
    }
}
