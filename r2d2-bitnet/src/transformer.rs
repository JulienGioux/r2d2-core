use crate::attention::BitSelfAttention;
use crate::bitlinear::BitLinear;
use crate::ffn::BitFFN;
use crate::rmsnorm::RmsNorm;
use crate::weights::WeightProvider;
use candle_core::{Module, Result, Tensor};
use tracing::instrument;

/// 🏛️ `BitTransformerBlock`
///
/// Le bloc de construction unitaire du modèle R2D2-BitNet.
/// Il orchestre Pre-Norm -> BitAttention -> Pre-Norm -> BitFFN
/// avec connexions résiduelles (Residual Connections).
pub struct BitTransformerBlock<W: WeightProvider> {
    pub attention_norm: RmsNorm,
    pub attention: BitSelfAttention<W>,
    pub ffn_norm: RmsNorm,
    pub ffn: BitFFN<W>,
}

impl<W: WeightProvider> BitTransformerBlock<W> {
    pub fn new(
        attention_norm: RmsNorm,
        attention: BitSelfAttention<W>,
        ffn_norm: RmsNorm,
        ffn: BitFFN<W>,
    ) -> Self {
        Self {
            attention_norm,
            attention,
            ffn_norm,
            ffn,
        }
    }
}

impl<W> Module for BitTransformerBlock<W>
where
    W: WeightProvider,
    BitLinear<W>: Module,
{
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
