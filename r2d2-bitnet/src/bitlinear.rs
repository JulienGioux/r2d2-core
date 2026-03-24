use crate::ternary::TernaryBlock16;
use candle_core::{Module, Result, Tensor};
use tracing::instrument;

/// 🚀 La Couche Linéaire propre à la topologie BitNet b1.58.
///
/// Elle substitue la matrice des Poids flottants classiques par
/// une topologie de `TernaryBlock16` (poids empaquetés).
/// L'accumulation est réalisée algébriquement via une logique SIMD "Logic-Only",
/// contournant intégralement les unités FPU de multiplication.
pub struct BitLinear {
    pub in_features: usize,
    pub out_features: usize,
    /// Les poids empaquetés sous forme de blocs de 16 paramètres ternaires.
    /// Longueur attendue : `(in_features / 16) * out_features`
    pub weights: Vec<TernaryBlock16>,
    /// Optionnel : Biais flottant (identique à une couche `Linear` standard).
    pub bias: Option<Tensor>,
}

impl BitLinear {
    /// Instancie une nouvelle couche `BitLinear` à partir de tenseurs pré-quantifiés.
    ///
    /// # Erreurs
    /// Renvoie une erreur si `in_features` n'est pas un multiple de 16, ou si la
    /// longueur du slice `flat_weights` ne correspond pas à `in_features * out_features`.
    #[instrument(skip_all, name = "BitLinear::new")]
    pub fn new(
        in_features: usize,
        out_features: usize,
        flat_weights: &[i8],
        bias: Option<Tensor>,
    ) -> candle_core::Result<Self> {
        if !in_features.is_multiple_of(16) {
            candle_core::bail!("Architectural Constraint: in_features ({}) doit être un multiple de 16 pour l'alignement intrinsèque.", in_features);
        }

        let expected_len = in_features * out_features;
        if flat_weights.len() != expected_len {
            candle_core::bail!(
                "Incohérence Matrice-Poids. Attendu: {}, Reçu: {}",
                expected_len,
                flat_weights.len()
            );
        }

        // Bit-Packing : Condensation des poids en masques (1 octet/poids -> 0.25 octet/poids)
        let blocks_count = expected_len / 16;
        let mut blocks = Vec::with_capacity(blocks_count);

        for chunk in flat_weights.chunks_exact(16) {
            blocks.push(TernaryBlock16::from_i8_slice(chunk));
        }

        Ok(Self {
            in_features,
            out_features,
            weights: blocks,
            bias,
        })
    }

    /// Charge et Quantise (1.58-bit) une couche linéaire directement depuis un `VarBuilder`.
    ///
    /// L'Architecte intercepte ici les poids flottants (F16/F32) du modèle source (ex: GGUF)
    /// et les projette brutalement dans l'espace ternaire {-1, 0, 1} avant de les packager.
    #[instrument(skip_all, name = "BitLinear::load")]
    pub fn load(
        in_features: usize,
        out_features: usize,
        vb: candle_nn::VarBuilder,
    ) -> candle_core::Result<Self> {
        // Extraction du tenseur de poids "Mère"
        let weight = vb.get((out_features, in_features), "weight")?;

        // Ramener en F32 pour une quantification déterministe
        let flat_f32 = weight
            .to_dtype(candle_core::DType::F32)?
            .flatten_all()?
            .to_vec1::<f32>()?;

        let mut flat_i8 = Vec::with_capacity(flat_f32.len());

        // ⚡ Quantification Absolute-Mean-Scale (AbsMean) Officielle (BitNet 1.58b)
        // γ (gamma) = Mean(Abs(W)). On normalise W par γ.
        let gamma: f32 = flat_f32.iter().map(|v| v.abs()).sum::<f32>() / (flat_f32.len() as f32);
        let scale = 1.0 / (gamma + 1e-5); // Évite la division par zéro

        for &val in &flat_f32 {
            // W_q = Round(W / (γ + ε))
            let scaled = (val * scale).round();

            // Constrain / Clip to {-1, 0, 1}
            let ternary_val = if scaled > 0.0 {
                1
            } else if scaled < 0.0 {
                -1
            } else {
                0
            };
            flat_i8.push(ternary_val);
        }

        // Chargement optionnel du Biais
        let bias = vb.get(out_features, "bias").ok();

        Self::new(in_features, out_features, &flat_i8, bias)
    }
}

impl Module for BitLinear {
    /// Moteur d'inférence algébrique "Forward" (MatMul-Free).
    #[instrument(skip(self, xs), name = "BitLinear::forward")]
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let dims = xs.dims();
        let rank = dims.len();

        let last_dim = if rank > 0 { dims[rank - 1] } else { 0 };
        if last_dim != self.in_features {
            candle_core::bail!(
                "Incompatibilité dimensionnelle: Tenseur d'entrée a {}, attendu {}",
                last_dim,
                self.in_features
            );
        }

        // Pour notre architecture R&D pure Rust (avant injections `std::arch` / intrinsincs CUDA),
        // nous convertissons en itérateurs 1D. La vitesse reste très élevée car nous appliquons
        // les opérations "Zero-Branch" imposées par l'architecture.

        let xs_flat = xs.flatten_all()?;
        let xs_vec = xs_flat.to_vec1::<f32>()?;
        let batch_elems = xs_vec.len() / self.in_features;

        let mut out_vec = Vec::with_capacity(batch_elems * self.out_features);
        let blocks_per_row = self.in_features / 16;

        for batch_i in 0..batch_elems {
            let x_offset = batch_i * self.in_features;
            // Notre bloc d'activations d'entrée
            let current_x = &xs_vec[x_offset..x_offset + self.in_features];

            for out_i in 0..self.out_features {
                let mut acc = 0.0f32;
                let w_offset = out_i * blocks_per_row;

                for block_i in 0..blocks_per_row {
                    let block = &self.weights[w_offset + block_i];
                    let x_chunk = &current_x[block_i * 16..(block_i + 1) * 16];

                    let pos_mask = block.m_pos as u32;
                    let neg_mask = block.m_neg as u32;

                    // ⚡ EXÉCUTION ZERO-BRANCH STRICTE
                    // Conformément aux documents d'ingénierie (Brique 5), on évite tout saut
                    // conditionnel qui détruirait le pipeline du Warp GPU / du pipeline CPU Zen.
                    for (bit, &x_val) in x_chunk.iter().enumerate().take(16) {
                        let is_pos = ((pos_mask >> bit) & 1) as f32;
                        let is_neg = ((neg_mask >> bit) & 1) as f32;

                        // FMA Logic-Only : +1, -1, ou Silence 0 pour économiser les Watts.
                        acc += (is_pos - is_neg) * x_val;
                    }
                }
                out_vec.push(acc);
            }
        }

        // Remodelage (Reshape) pour retrouver les dimensions d'origine du batch spatial
        let mut out_shape = dims.to_vec();
        out_shape[rank - 1] = self.out_features;

        let device = xs.device();
        let mut out_tensor = Tensor::from_vec(out_vec, out_shape.as_slice(), device)?;

        // Somme du Biais (Add-Broadcast)
        if let Some(b) = &self.bias {
            out_tensor = out_tensor.broadcast_add(b)?;
        }

        Ok(out_tensor)
    }
}

// -----------------------------------------------------------------------------
// SECURE AUDIT TESTS (CI: Zero-Warnings)
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    #[test]
    fn test_bitlinear_forward_logic() -> Result<()> {
        let in_f = 16;
        let out_f = 2;

        // Weights: Layer 1 (Neuron 0: mostly pos, Neuron 1: mostly neg)
        let weights: [i8; 32] = [
            1, 1, 1, 1, 0, 0, 0, 0, -1, -1, -1, -1, 0, 0, 0, 0, // Neuron 0
            -1, -1, -1, -1, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0, // Neuron 1
        ];

        let layer = BitLinear::new(in_f, out_f, &weights, None)?;

        // Activations: Un batch de 1 x 16
        // [1.0, 1.0, 1.0, 1.0, ...]
        let mut xs_data = vec![0.0f32; 16];
        xs_data[0..4].fill(1.0); // S'aligne sur les 1
        xs_data[8..12].fill(2.0); // S'aligne sur les -1

        let xs = Tensor::from_vec(xs_data, &[1, 16], &Device::Cpu)?;
        let ys = layer.forward(&xs)?;

        // Neuron 0: (4 * 1.0) + (4 * -2.0) = 4 - 8 = -4.0
        // Neuron 1: (4 * -1.0) + (4 * 2.0) = -4 + 8 = 4.0
        let ys_vec = ys.flatten_all()?.to_vec1::<f32>()?;

        assert_eq!(ys_vec[0], -4.0);
        assert_eq!(ys_vec[1], 4.0);
        Ok(())
    }

    #[test]
    fn test_absmean_quantization() -> Result<()> {
        use candle_core::DType;
        use candle_nn::VarBuilder;
        use std::collections::HashMap;

        // On crée un tenseur 2x16 (Out: 2, In: 16)
        // Ligne 1 : Poids avec forte variance [-10.0, 10.0, ...] -> AbsMean sera grand, les faibles valeurs ~0 deviendront 0.
        // Ligne 2 : Poids avec faible variance [-0.1, 0.1, ...]

        let mut w_data = vec![0.0f32; 32];
        // Neuron 1: Extreme values
        w_data[0] = 100.0;
        w_data[1] = -100.0;
        w_data[2] = 1.0; // Should be mapped to 0 because AbsMean simplifies scale.
        w_data[3] = -1.0; // -> 0

        // Modifions pour avoir un gamma de 10.0 par exemple: 100+100 + 4*0 / 32 = 200/32 = 6.25
        // W_q = Round(W / 6.25).
        // 100 / 6.25 = 16 -> Clip -> 1.
        // -100 / 6.25 = -16 -> Clip -> -1.
        // 1 / 6.25 = 0.16 -> Round -> 0.

        let w_tensor = Tensor::from_vec(w_data, &[2, 16], &Device::Cpu)?;

        let mut tensors = HashMap::new();
        tensors.insert("weight".to_string(), w_tensor);
        let vb = VarBuilder::from_tensors(tensors, DType::F32, &Device::Cpu);

        let layer = BitLinear::load(16, 2, vb)?;

        // Block 0 correspond au Neuron 0
        let pos_mask_n0 = layer.weights[0].m_pos;
        let neg_mask_n0 = layer.weights[0].m_neg;

        // bit 0 -> 100.0 -> +1 (pos_mask = 1, neg_mask = 0)
        assert_eq!((pos_mask_n0 >> 0) & 1, 1);
        assert_eq!((neg_mask_n0 >> 0) & 1, 0);

        // bit 1 -> -100.0 -> -1 (pos_mask = 0, neg_mask = 1)
        assert_eq!((pos_mask_n0 >> 1) & 1, 0);
        assert_eq!((neg_mask_n0 >> 1) & 1, 1);

        // bit 2 -> 1.0 -> 0 (pos_mask = 0, neg_mask = 0)
        assert_eq!((pos_mask_n0 >> 2) & 1, 0);
        assert_eq!((neg_mask_n0 >> 2) & 1, 0);

        Ok(())
    }
}
