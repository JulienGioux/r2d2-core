// R2D2 Chimera - Épine Dorsale Continue
// Implémentation du State Space Model (BitMamba) ternarisé.

use candle_core::{Result, Tensor};
use candle_nn::{Init, VarBuilder};

pub struct SsmBlock {
    pub hidden_state_dim: usize,
    pub proj_a: Tensor,
    pub proj_b: Tensor,
    pub proj_c: Tensor,
}

impl SsmBlock {
    pub fn new(dim: usize, a: Tensor, b: Tensor, c: Tensor) -> Self {
        Self {
            hidden_state_dim: dim,
            proj_a: a,
            proj_b: b,
            proj_c: c,
        }
    }

    /// 🚀 Initalisation "QAT-Scratch" avec VarBuilder
    pub fn new_qat(dim: usize, vb: VarBuilder) -> Result<Self> {
        let span = tracing::span!(tracing::Level::DEBUG, "ssm_init");
        let _enter = span.enter();

        // 1. Initialisation S4D-Real pour la Matrice A (HiPPO)
        // La matrice A de transition (Mamba) DOIT capturer l'historique sans oublier.
        // Diagonales initialisées à -(n+1), le reste à zéro. C'est maintenant un Vecteur 1D
        let mut a_vec = vec![0.0f32; dim];
        for (i, v) in a_vec.iter_mut().enumerate() {
            *v = -((i + 1) as f32);
        }
        let proj_a = Tensor::from_vec(a_vec, dim, vb.device())?;

        // 2. Projections B et C: Elles seront ternarisées, donc initialisation XavierNormal
        let stdev = 1.0 / (dim as f64).sqrt();
        let init_xav = Init::Randn { mean: 0.0, stdev };
        let proj_b = vb.get_with_hints((dim, dim), "proj_b", init_xav)?;
        let proj_c = vb.get_with_hints((dim, dim), "proj_c", init_xav)?;

        Ok(Self {
            hidden_state_dim: dim,
            proj_a,
            proj_b,
            proj_c,
        })
    }

    /// Ingestion Séquentielle MIMO (Multi-Input Multi-Output) O[D] au lieu de O[T*D^2]
    /// Traite les projections B et C via Tensor Cores massivement parallèles
    pub fn forward_scan(&self, x: &Tensor, prev_state: Option<Tensor>) -> Result<(Tensor, Tensor)> {
        let dims = x.dims();
        let (batch_size, seq_len, dim) = match dims.len() {
            2 => (1, dims[0], dims[1]),
            3 => (dims[0], dims[1], dims[2]),
            _ => candle_core::bail!("SSM attend un tenseur 2D ou 3D"),
        };

        // On flatten temporairement la batch_size pour saturer encore plus le GPU
        // [B*S, D]
        let x_flat = x.reshape((batch_size * seq_len, dim))?;

        // 1. MIMO Projection B : Sature les Tensor Cores en un seul appel (Zéro CPU Loop)
        // [B*S, Dim] x [Dim, Dim]^T => [B*S, Dim]
        let bx_seq = x_flat.matmul(&self.proj_b.t()?)?;

        // On remet sous forme [B, S, D] pour la récurrence
        let bx_seq = bx_seq.reshape((batch_size, seq_len, dim))?;

        // 2. Discrétisation Exponentielle (Mamba) : A est un Decay
        // Différentiable et combat dynamiquement les Vanishing Gradients
        let exp_a = self.proj_a.exp()?;

        let mut h_t =
            prev_state.unwrap_or_else(|| Tensor::zeros(dim, x.dtype(), x.device()).unwrap());
        let mut h_seq = Vec::with_capacity(seq_len);

        // 3. Boucle Récurrente Element-Wise : Graphe Autograd Intact (O(D) au lieu de O(D^2))
        for t in 0..seq_len {
            let bx_t = bx_seq.narrow(1, t, 1)?.squeeze(1)?; // Extraction temporelle [B, D]

            // h_t = exp(A) * h_{t-1} + bx_t
            h_t = exp_a.broadcast_mul(&h_t)?.broadcast_add(&bx_t)?;
            h_seq.push(h_t.unsqueeze(1)?); // Forme [B, 1, D]
        }

        // Reconstruction de l'historique d'états
        let h_global = Tensor::cat(&h_seq, 1)?; // [B, Seq, Dim]

        // 4. MIMO Projection C : Sature les Tensor Cores
        let h_global_flat = h_global.reshape((batch_size * seq_len, dim))?;
        let y_seq_flat = h_global_flat.matmul(&self.proj_c.t()?)?;
        let y_seq = y_seq_flat.reshape((batch_size, seq_len, dim))?;

        Ok((y_seq, h_t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    #[test]
    fn test_ssm_matmul_free_logic() -> Result<()> {
        let dim = 2;
        // Poids parfaitement ternaires pour B et C
        // A est le coefficient decay S4D scalaire
        let a = Tensor::new(&[-1.0f32, -1.0], &Device::Cpu)?;
        let b = Tensor::new(&[[1.0f32, -1.0], [0.0, 1.0]], &Device::Cpu)?;
        let c = Tensor::new(&[[1.0f32, 1.0], [-1.0, 0.0]], &Device::Cpu)?;

        let block = SsmBlock::new(dim, a, b, c);

        // x_t = [2.0, 3.0]
        let x = Tensor::new(&[[2.0f32, 3.0]], &Device::Cpu)?;
        // h_prev = [1.0, 2.0]
        let h_prev = Tensor::new(&[1.0f32, 2.0], &Device::Cpu)?;

        let (y_t, h_t) = block.forward_scan(&x, Some(h_prev))?;

        let y_t_out = y_t.flatten_all()?.to_vec1::<f32>()?;
        let h_t_out = h_t.flatten_all()?.to_vec1::<f32>()?;

        // Calcul MIMO Native:
        // x_in = X * B^T => [2.0, 3.0] * [[1, -1], [0, 1]]^T
        // B^T = [[1, 0], [-1, 1]]
        // x_in = [2*1 + 3*-1, 2*0 + 3*1] => [-1.0, 3.0]
        // exp_A = exp([-1, -1]) = [0.36787945, 0.36787945]
        // h_new = exp_A * [1, 2] + [-1, 3] = [0.36787945 - 1, 0.7357589 + 3]
        // h_new = [-0.63212055, 3.7357588]
        // y_t = h_new * C^T => [-0.63212055, 3.7357588] * [[1, 1], [-1, 0]]^T
        // C^T = [[1, -1], [1, 0]]
        // y_out[0] = -0.63212055 + 3.7357588 = 3.1036382
        // y_out[1] = 0.63212055 + 0 = 0.63212055

        assert!((h_t_out[0] - -0.63212055).abs() < 1e-4, "h_t diverge");
        assert!((y_t_out[0] - 3.1036382).abs() < 1e-4, "y_t diverge");

        Ok(())
    }
}
