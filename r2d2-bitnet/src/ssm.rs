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

    /// Ingestion séquentielle MLGRU / Mamba [O(T * D)]
    /// Recycle prev_state (Zéro-Aliasing) pour annuler le KV-Cache
    pub fn forward_scan(
        &self,
        x: &Tensor,
        prev_state: Option<Vec<f32>>,
    ) -> Result<(Tensor, Vec<f32>)> {
        let (seq_len, dim) = x.dims2()?;
        let x_vec = x.flatten_all()?.to_vec1::<f32>()?;

        // L'Extraction (La Matrice A est désormais juste un Decay scalaire 1D)
        let a_vec = self.proj_a.flatten_all()?.to_vec1::<f32>()?;
        let b_vec = self.proj_b.flatten_all()?.to_vec1::<f32>()?;
        let c_vec = self.proj_c.flatten_all()?.to_vec1::<f32>()?;

        // 1. Consommation et Recyclage du KV-Cache !
        let mut state = prev_state.unwrap_or_else(|| vec![0.0f32; dim]);
        let mut y_seq = vec![0.0f32; seq_len * dim];

        // 2. Scan temporel séquentiel SIMD Zéro-Aliasing (Pas de Rayon)
        for t in 0..seq_len {
            let offset = t * dim;
            let x_t = &x_vec[offset..offset + dim];
            let y_t = &mut y_seq[offset..offset + dim];

            // A) Vecteur B * x_t (MatMul-Free via Zip/Fold)
            let mut bx_t = vec![0.0f32; dim];
            bx_t.iter_mut().enumerate().for_each(|(i, bx_i)| {
                let b_row = &b_vec[i * dim..(i + 1) * dim];
                *bx_i = b_row.iter().zip(x_t.iter()).fold(0.0, |acc, (&w, &val)| {
                    if w > 0.5 {
                        acc + val
                    } else if w < -0.5 {
                        acc - val
                    } else {
                        acc
                    }
                });
            });

            // B) Mamba SSM Recurrence (SIMD In-Place) -> h_t = A * h_{t-1} + B * x_t
            state
                .iter_mut()
                .zip(a_vec.iter())
                .zip(bx_t.iter())
                .for_each(|((h_i, &a_i), &bx_i)| {
                    *h_i = (a_i * *h_i) + bx_i;
                });

            // C) Vecteur y_t = C * h_t (MatMul-Free via Zip/Fold)
            y_t.iter_mut().enumerate().for_each(|(i, y_i)| {
                let c_row = &c_vec[i * dim..(i + 1) * dim];
                *y_i = c_row.iter().zip(state.iter()).fold(0.0, |acc, (&w, &s)| {
                    if w > 0.5 {
                        acc + s
                    } else if w < -0.5 {
                        acc - s
                    } else {
                        acc
                    }
                });
            });
        }

        let out_tensor = Tensor::from_vec(y_seq, (seq_len, dim), x.device())?;
        Ok((out_tensor, state))
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
        let h_prev = vec![1.0f32, 2.0];

        let (y_t, h_t) = block.forward_scan(&x, Some(h_prev))?;

        let y_t_out = y_t.flatten_all()?.to_vec1::<f32>()?;

        // Calcul Manuel:
        // x_in[0] = 1*2 - 1*3 = -1
        // h_new[0] = -1*1 + -1 = -2
        // x_in[1] = 0*2 + 1*3 = 3
        // h_new[1] = -1*2 + 3 = 1
        // => h_t state = [-2, 1]
        assert_eq!(h_t, vec![-2.0, 1.0]);

        // y_t[0] = 1*-2 + 1*1 = -1
        // y_t[1] = -1*-2 + 0*1 = 2
        // => y_t = [-1, 2]
        assert_eq!(y_t_out, vec![-1.0, 2.0]);

        Ok(())
    }
}
