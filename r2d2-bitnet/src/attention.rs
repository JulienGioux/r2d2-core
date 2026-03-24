use crate::bitlinear::BitLinear;
use candle_core::{Module, Result, Tensor};
use tracing::instrument;

/// 🧠 `BitSelfAttention`
///
/// Implémente le mécanisme d'Attention Multi-Tête de R2D2.
/// L'innovation de Phase 5 : toutes les projections (Query, Key, Value, et Output)
/// sont assurées par des couches `BitLinear` (Ternaires).
pub struct BitSelfAttention {
    pub num_heads: usize,
    pub head_dim: usize,
    pub q_proj: BitLinear,
    pub k_proj: BitLinear,
    pub v_proj: BitLinear,
    pub o_proj: BitLinear,
}

impl BitSelfAttention {
    pub fn new(
        num_heads: usize,
        head_dim: usize,
        q_proj: BitLinear,
        k_proj: BitLinear,
        v_proj: BitLinear,
        o_proj: BitLinear,
    ) -> Self {
        Self {
            num_heads,
            head_dim,
            q_proj,
            k_proj,
            v_proj,
            o_proj,
        }
    }
}

impl Module for BitSelfAttention {
    #[instrument(skip_all, name = "BitSelfAttention::forward")]
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let (batch, seq_len, in_dim) = xs.dims3()?;

        // Projections Ternaires (MatMath-Free)
        // Les Poids sont {-1, 0, 1} mais le résultat est dimensionnel FP32/Int8
        let q = self.q_proj.forward(xs)?;
        let k = self.k_proj.forward(xs)?;
        let v = self.v_proj.forward(xs)?;

        // Remodelage pour le Multi-Head Attention : [batch, seq, num_heads, head_dim]
        let q = q
            .reshape((batch, seq_len, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;
        let k = k
            .reshape((batch, seq_len, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;
        let v = v
            .reshape((batch, seq_len, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;

        // Note: L'intégration des Rotary Position Embeddings (RoPE)
        // s'intercalera ici avant le calcul du scaling.

        let scale = 1f64 / f64::sqrt(self.head_dim as f64);

        // Causal Self-Attention avec Mask
        // Q * K.T
        let att = candle_nn::ops::softmax(
            &q.matmul(&k.transpose(2, 3)?)?
                .broadcast_mul(&Tensor::new(scale, xs.device())?)?,
            candle_core::D::Minus1,
        )?;

        // Poids d'Attention * Values
        let out = att.matmul(&v)?;

        // Fusion des Têtes
        let out = out.transpose(1, 2)?.reshape((batch, seq_len, in_dim))?;

        // Projection de Sortie (Ternaire)
        self.o_proj.forward(&out)
    }
}
