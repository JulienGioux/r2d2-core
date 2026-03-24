use crate::attention::BitSelfAttention;
use crate::ffn::BitFFN;
use crate::rmsnorm::RmsNorm;
use candle_core::{Module, Result, Tensor};
use tracing::instrument;

/// 🏛️ `BitTransformerBlock`
///
/// Le bloc de construction unitaire du modèle R2D2-BitNet.
/// Il orchestre Pre-Norm -> BitAttention -> Pre-Norm -> BitFFN
/// avec connexions résiduelles (Residual Connections).
pub struct BitTransformerBlock {
    pub attention_norm: RmsNorm,
    pub attention: BitSelfAttention,
    pub ffn_norm: RmsNorm,
    pub ffn: BitFFN,
}

impl BitTransformerBlock {
    pub fn new(
        attention_norm: RmsNorm,
        attention: BitSelfAttention,
        ffn_norm: RmsNorm,
        ffn: BitFFN,
    ) -> Self {
        Self {
            attention_norm,
            attention,
            ffn_norm,
            ffn,
        }
    }
}

impl Module for BitTransformerBlock {
    #[instrument(skip_all, name = "BitTransformerBlock::forward")]
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        // Chemin 1: Self-Attention
        let residual = xs.clone();
        let xs = self.attention_norm.forward(xs)?;
        let xs = self.attention.forward(&xs)?;
        let xs = (xs + residual)?;

        // Chemin 2: Feed Forward
        let residual = xs.clone();
        let xs = self.ffn_norm.forward(&xs)?;
        let xs = self.ffn.forward(&xs)?;
        let xs = (xs + residual)?;

        Ok(xs)
    }
}
