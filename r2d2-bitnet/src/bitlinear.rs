use crate::ternary::TernaryBlock16;
use crate::weights::{InferenceWeights, TrainingWeights, WeightProvider};
use candle_core::{Module, Result, Tensor};
use tracing::instrument;

/// 🚀 La Couche Linéaire propre à la topologie BitNet b1.58.
///
/// Implémentation via Type-State pour garantir une isolation Compile-Time parfaite entre:
/// - `InferenceWeights` (0.25 bytes/poids, Accumulation Logic-Only CPU)
/// - `TrainingWeights` (f32 Latent Weights, STE Quantization, GPU/MPS Autograd)
pub struct BitLinear<W: WeightProvider> {
    pub in_features: usize,
    pub out_features: usize,
    pub weights: W,
    pub bias: Option<Tensor>,
}

impl BitLinear<InferenceWeights> {
    /// Instancie une nouvelle couche `BitLinear` d'inférence à partir de tenseurs pré-quantifiés i8.
    #[instrument(skip_all, name = "BitLinear::new")]
    pub fn new(
        in_features: usize,
        out_features: usize,
        flat_weights: &[i8],
        bias: Option<Tensor>,
    ) -> candle_core::Result<Self> {
        if !in_features.is_multiple_of(16) {
            candle_core::bail!(
                "Architectural Constraint: in_features ({}) doit être un multiple de 16.",
                in_features
            );
        }

        let expected_len = in_features * out_features;
        if flat_weights.len() != expected_len {
            candle_core::bail!(
                "Incohérence Matrice-Poids. Attendu: {}, Reçu: {}",
                expected_len,
                flat_weights.len()
            );
        }

        let blocks_count = expected_len / 16;
        let mut blocks = Vec::with_capacity(blocks_count);

        for chunk in flat_weights.chunks_exact(16) {
            blocks.push(TernaryBlock16::from_i8_slice(chunk));
        }

        let weights = InferenceWeights { blocks };

        Ok(Self {
            in_features,
            out_features,
            weights,
            bias,
        })
    }

    /// Charge et Quantise (1.58-bit) une couche linéaire directement depuis un `VarBuilder`.
    #[instrument(skip_all, name = "BitLinear::load")]
    pub fn load(
        in_features: usize,
        out_features: usize,
        vb: candle_nn::VarBuilder,
    ) -> candle_core::Result<Self> {
        let weight = vb.get((out_features, in_features), "weight")?;

        let flat_f32 = weight
            .to_dtype(candle_core::DType::F32)?
            .flatten_all()?
            .to_vec1::<f32>()?;

        let mut flat_i8 = Vec::with_capacity(flat_f32.len());

        let gamma: f32 = flat_f32.iter().map(|v| v.abs()).sum::<f32>() / (flat_f32.len() as f32);
        let scale = 1.0 / (gamma + 1e-5);

        for &val in &flat_f32 {
            let scaled = (val * scale).round();
            let ternary_val = if scaled > 0.0 {
                1
            } else if scaled < 0.0 {
                -1
            } else {
                0
            };
            flat_i8.push(ternary_val);
        }

        let bias = vb.get(out_features, "bias").ok();

        Self::new(in_features, out_features, &flat_i8, bias)
    }
}

impl BitLinear<TrainingWeights> {
    /// Initialise une couche `BitLinear` pour l'ENTRAÎNEMENT en générant une variable f32 (`Var`)
    #[instrument(skip_all, name = "BitLinear::load_train")]
    pub fn load_train(
        in_features: usize,
        out_features: usize,
        vb: candle_nn::VarBuilder,
    ) -> candle_core::Result<Self> {
        let weight_tensor = vb.get((out_features, in_features), "weight")?;

        // Convert to Var to allow Autograd
        let var = candle_core::Var::from_tensor(&weight_tensor)?;
        let bias = vb.get(out_features, "bias").ok();

        let weights = TrainingWeights { var };

        Ok(Self {
            in_features,
            out_features,
            weights,
            bias,
        })
    }
}

// ==============================================================================
// 🧠 FORWARD INFERENCE (Zero-Branch CPU Logic-Only)
// ==============================================================================
impl Module for BitLinear<InferenceWeights> {
    #[instrument(skip(self, xs), name = "BitLinear::forward_inference")]
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

        let xs_flat = xs.flatten_all()?;
        let xs_vec = xs_flat.to_vec1::<f32>()?;
        let batch_elems = xs_vec.len() / self.in_features;

        let mut out_vec = Vec::with_capacity(batch_elems * self.out_features);
        let blocks_per_row = self.in_features / 16;

        for batch_i in 0..batch_elems {
            let x_offset = batch_i * self.in_features;
            let current_x = &xs_vec[x_offset..x_offset + self.in_features];

            for out_i in 0..self.out_features {
                let mut acc = 0.0f32;
                let w_offset = out_i * blocks_per_row;

                for block_i in 0..blocks_per_row {
                    let block = &self.weights.blocks[w_offset + block_i];
                    let x_chunk = &current_x[block_i * 16..(block_i + 1) * 16];

                    let pos_mask = block.m_pos as u32;
                    let neg_mask = block.m_neg as u32;

                    for (bit, &x_val) in x_chunk.iter().enumerate().take(16) {
                        let is_pos = ((pos_mask >> bit) & 1) as f32;
                        let is_neg = ((neg_mask >> bit) & 1) as f32;
                        acc += (is_pos - is_neg) * x_val;
                    }
                }
                out_vec.push(acc);
            }
        }

        let mut out_shape = dims.to_vec();
        out_shape[rank - 1] = self.out_features;

        let device = xs.device();
        let mut out_tensor = Tensor::from_vec(out_vec, out_shape.as_slice(), device)?;

        if let Some(b) = &self.bias {
            out_tensor = out_tensor.broadcast_add(b)?;
        }

        Ok(out_tensor)
    }
}

// ==============================================================================
// 🚀 FORWARD ENTRAINEMENT (STE Quantization + Autograd)
// ==============================================================================
impl Module for BitLinear<TrainingWeights> {
    #[instrument(skip(self, xs), name = "BitLinear::forward_train")]
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        // En apprentissage, on applique la quantification STE sur les poids latents
        let w_q = crate::training::ste::ste_quantize(self.weights.var.as_tensor())?;

        let mut out_tensor = xs.broadcast_matmul(&w_q.t()?)?;

        if let Some(b) = &self.bias {
            out_tensor = out_tensor.broadcast_add(b)?;
        }

        Ok(out_tensor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    #[test]
    fn test_bitlinear_forward_logic() -> Result<()> {
        let in_f = 16;
        let out_f = 2;

        let weights: [i8; 32] = [
            1, 1, 1, 1, 0, 0, 0, 0, -1, -1, -1, -1, 0, 0, 0, 0, -1, -1, -1, -1, 0, 0, 0, 0, 1, 1,
            1, 1, 0, 0, 0, 0,
        ];

        let layer = BitLinear::<InferenceWeights>::new(in_f, out_f, &weights, None)?;

        let mut xs_data = vec![0.0f32; 16];
        xs_data[0..4].fill(1.0);
        xs_data[8..12].fill(2.0);

        let xs = Tensor::from_vec(xs_data, &[1, 16], &Device::Cpu)?;
        let ys = layer.forward(&xs)?;

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

        let mut w_data = vec![0.0f32; 32];
        w_data[0] = 100.0;
        w_data[1] = -100.0;
        w_data[2] = 1.0;
        w_data[3] = -1.0;

        let w_tensor = Tensor::from_vec(w_data, &[2, 16], &Device::Cpu)?;

        let mut tensors = HashMap::new();
        tensors.insert("weight".to_string(), w_tensor);
        let vb = VarBuilder::from_tensors(tensors, DType::F32, &Device::Cpu);

        let layer = BitLinear::<InferenceWeights>::load(16, 2, vb)?;

        let pos_mask_n0 = layer.weights.blocks[0].m_pos;
        let neg_mask_n0 = layer.weights.blocks[0].m_neg;

        assert_eq!(pos_mask_n0 & 1, 1);
        assert_eq!(neg_mask_n0 & 1, 0);
        assert_eq!((pos_mask_n0 >> 1) & 1, 0);
        assert_eq!((neg_mask_n0 >> 1) & 1, 1);
        assert_eq!((pos_mask_n0 >> 2) & 1, 0);
        assert_eq!((neg_mask_n0 >> 2) & 1, 0);

        Ok(())
    }
}
