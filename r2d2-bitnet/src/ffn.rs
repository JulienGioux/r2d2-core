use crate::bitlinear::BitLinear;
use crate::weights::WeightProvider;
use candle_core::{Module, Result, Tensor};
use tracing::instrument;

/// 🧠 `BitFFN` (Feed-Forward Network)
///
/// Topologie SwiGLU (type Llama 2/3) propulsée intégralement par la
/// couche `BitLinear` ternaire `{-1, 0, 1}`.
pub struct BitFFN<W: WeightProvider> {
    gate_proj: BitLinear<W>,
    up_proj: BitLinear<W>,
    down_proj: BitLinear<W>,
}

impl<W: WeightProvider> BitFFN<W> {
    pub fn new(gate_proj: BitLinear<W>, up_proj: BitLinear<W>, down_proj: BitLinear<W>) -> Self {
        Self {
            gate_proj,
            up_proj,
            down_proj,
        }
    }
}

impl<W> Module for BitFFN<W>
where
    W: WeightProvider,
    BitLinear<W>: Module,
{
    /// Pipelining de la propagation Llama SwiGLU
    #[instrument(skip_all, name = "BitFFN::forward")]
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        // Projection Gate & Up en parallèle via le Math-free Logic
        let gate = self.gate_proj.forward(xs)?;
        let up = self.up_proj.forward(xs)?;

        // Activation SiLU non-linéaire sur le Gate : x * sigmoid(x)
        // La fonction `Tensor::silu` utilise les capacités natives de Candle
        let silu_gate = gate.silu()?;

        // Produit d'Hadamard (Multiplication élément par élément)
        let inter = silu_gate.broadcast_mul(&up)?;

        // Projection finale (Down)
        self.down_proj.forward(&inter)
    }
}
