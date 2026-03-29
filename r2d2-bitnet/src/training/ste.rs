use candle_core::{Result, Tensor};
use tracing::instrument;

/// 🧠 Opération Custom "Straight-Through Estimator" (STE) pour BitNet 1.58-bit.
///
/// Applique la quantification AbsMean 1.58-bit avec passage de gradient STE.
#[instrument(skip_all, name = "ste_quantize")]
pub fn ste_quantize(x: &Tensor) -> Result<Tensor> {
    // 1. Calculer γ (Gamma) = Mean(Abs(W))
    let abs_x = x.abs()?;
    let gamma = abs_x.mean_all()?;

    // 2. Normalisation AbsMean : W / (γ + ε)
    let eps = Tensor::new(1e-5f32, x.device())?;
    // gamma est un scalaire 0D, il faut le broadcast_add.
    let denom = gamma.broadcast_add(&eps)?;

    // 3. W_q = Round(W / Denom)
    let scaled = x.broadcast_div(&denom)?;
    let rounded = scaled.round()?;

    // 4. Clipping {-1.0, 0.0, 1.0}
    let ones = rounded.ones_like()?;
    let neg_ones = ones.neg()?;

    let w_q = rounded.maximum(&neg_ones)?.minimum(&ones)?;

    // 5. L'astuce magique du Straight-Through Estimator via le graphe autograd "Detach"
    // Formule : x + (w_q - x).detach()
    // - En Forward : la valeur est mathématiquement `w_q` (puisque x et -x s'annulent).
    // - En Backward : `.detach()` bloque le gradient de `(w_q - x)`.
    //   Le gradient coule donc directement et librement dans le `x` initial.
    // C'est la méthode "Zero-CustomOp" qui permet au backend de tourner sur CPU, Cuda et Metal sans écrire de C++ bas niveau.

    let w_q_detached = w_q.detach();
    let x_detached = x.detach();

    // delta = w_q_detached - x_detached
    let delta = w_q_detached.broadcast_sub(&x_detached)?;

    // result = x + delta
    let result = x.broadcast_add(&delta)?;

    Ok(result)
}
