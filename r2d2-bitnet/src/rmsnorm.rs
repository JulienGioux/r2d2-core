use candle_core::{Module, Result, Tensor};
use tracing::instrument;

/// `RmsNorm` (Root Mean Square Normalization)
/// Standard pour les modèles modernes (Llama, BitNet), il remplace
/// le LayerNorm traditionnel car il supprime le recalcule de la moyenne
/// et ne nécessite pas de biais, ce qui s'aligne bien avec l'efficacité de BitNet.
#[derive(Debug)]
pub struct RmsNorm {
    weight: Tensor,
    eps: f64,
}

impl RmsNorm {
    pub fn new(weight: Tensor, eps: f64) -> Self {
        Self { weight, eps }
    }
}

impl Module for RmsNorm {
    #[instrument(skip_all, name = "RmsNorm::forward")]
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let x_dtype = xs.dtype();
        let internal_dtype = candle_core::DType::F32;
        
        let xs_f32 = xs.to_dtype(internal_dtype)?;
        // Normalisation stricte de la variance (mean des carrés)
        let variance = xs_f32.sqr()?.mean_keepdim(candle_core::D::Minus1)?;
        let norm_x = xs_f32.broadcast_div(&(variance + self.eps)?.sqrt()?)?;
        
        // Retour au type d'origine et application des poids de scaling
        let out = norm_x.to_dtype(x_dtype)?;
        out.broadcast_mul(&self.weight)
    }
}
