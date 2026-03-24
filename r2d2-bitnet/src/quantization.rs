use candle_core::{Result, Tensor};

/// ⚖️ Quantification des Activations (A8W1.58)
///
/// Implémente la quantification mathématique `Absmax` qui projette
/// le spectre flottant continu vers un espace discret 8-bit `[-128, 127]`.
pub fn absmax_quantize_activations(xs: &Tensor) -> Result<(Tensor, Tensor)> {
    let epsilon = 1e-5f64;

    // 1. Extraction de la magnitude spatiale (gamma)
    let gamma = xs.abs()?.max_keepdim(candle_core::D::Minus1)?;

    // 2. Facteur d'échelle (Target space : 127.0 pour l'Int8)
    let limit = Tensor::new(127.0f32, xs.device())?;
    // Formule : scale = gamma / 127.0 (avec précaution epsilon)
    let scale = (gamma + epsilon)?.broadcast_div(&limit)?;

    // 3. Mise à l'échelle des Activations
    let x_scaled = xs.broadcast_div(&scale)?;

    // 4. Projection Non-Linéaire : Clamp sur l'espace d'entiers puis Arrondi
    // Note : On maintient le conteneur en float32 (Simulated Quantization)
    // pour bénéficier de l'alignement CPU, mais les valeurs sont discrétisées exactes.
    let x_quant = x_scaled.clamp(-128.0f32, 127.0f32)?.round()?;

    Ok((x_quant, scale))
}

/// 📉 Quantification des Poids -> Topologie {-1, 0, 1}
///
/// Implémente la méthode `Absmean` pour forger la compression Llama classique
/// vers le format restreint Dual-Mask `TernaryBlock16`.
pub fn absmean_quantize_weights(weights: &Tensor) -> Result<Tensor> {
    let epsilon = 1e-5f64;

    // 1. Extraction de la moyenne absolue des énergies synaptiques (beta)
    let abs_w = weights.abs()?;
    let beta = abs_w.mean_all()?; // Moyenne globale du tenseur

    // 2. Échelle (Scale par rapport à la moyenne unifiée)
    let w_scaled = weights.broadcast_div(&(beta + epsilon)?)?;

    // 3. Compression Ternaire : Clamp(Round(W / beta), -1, 1)
    let w_quant = w_scaled.clamp(-1.0f32, 1.0f32)?.round()?;

    Ok(w_quant)
}
