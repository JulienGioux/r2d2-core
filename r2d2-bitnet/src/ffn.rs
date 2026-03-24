use candle_core::{Module, Result, Tensor};
use crate::bitlinear::BitLinear;
use tracing::instrument;

/// 🧠 `BitFFN` (Feed-Forward Network)
///
/// Topologie SwiGLU (type Llama 2/3) propulsée intégralement par la 
/// couche `BitLinear` ternaire `{-1, 0, 1}`.
pub struct BitFFN {
    gate_proj: BitLinear,
    up_proj: BitLinear,
    down_proj: BitLinear,
}

impl BitFFN {
    pub fn new(gate_proj: BitLinear, up_proj: BitLinear, down_proj: BitLinear) -> Self {
        Self {
            gate_proj,
            up_proj,
            down_proj,
        }
    }
}

impl Module for BitFFN {
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
