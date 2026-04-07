// R2D2 Chimera - Épine Dorsale Continue
// Implémentation du State Space Model (BitMamba) ternarisé.

use candle_core::{Result, Tensor};

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

    /// Ingestion séquentielle linéaire [O(N)] via Scan Cumulatif
    /// prev_state représente l'état compressé précédent (h_{t-1})
    pub fn forward(&self, x: &Tensor, prev_state: Option<&Tensor>) -> Result<(Tensor, Tensor)> {
        // Équation fondamentale d'un SSM discret:
        // h_t = A * h_{t-1} + B * x_t
        // y_t = C * h_t

        let shape = x.shape();
        let dims = shape.dims();
        let dim = self.hidden_state_dim;
        let num_tokens: usize = dims[..dims.len() - 1].iter().product(); // Support du Batch Size

        let x_vec = x.flatten_all()?.to_vec1::<f32>()?;

        // Extraction des poids ternaires
        let a_vec = self.proj_a.flatten_all()?.to_vec1::<f32>()?;
        let b_vec = self.proj_b.flatten_all()?.to_vec1::<f32>()?;
        let c_vec = self.proj_c.flatten_all()?.to_vec1::<f32>()?;

        // Allocation de l'état t (Dimensionné pour l'intégralité du batch)
        let mut h_t_vec = vec![0.0f32; num_tokens * dim];
        let h_prev_vec = prev_state.map(|t| t.flatten_all().unwrap().to_vec1::<f32>().unwrap());

        use rayon::prelude::*;

        // 1. Calcul de h_t (Parallélisé par jeton)
        h_t_vec
            .par_chunks_exact_mut(dim)
            .enumerate()
            .for_each(|(t, h_t_token)| {
                let token_offset = t * dim;
                let x_token = &x_vec[token_offset..token_offset + dim];
                let prev_token = h_prev_vec
                    .as_ref()
                    .map(|p| &p[token_offset..token_offset + dim]);

                for i in 0..dim {
                    let mut sum = 0.0;

                    // Partie A * h_{t-1}
                    if let Some(p) = prev_token {
                        let a_row = &a_vec[i * dim..(i + 1) * dim];
                        sum += p.iter().zip(a_row.iter()).fold(0.0, |acc, (&p_v, &w)| {
                            acc + if w > 0.5 {
                                p_v
                            } else if w < -0.5 {
                                -p_v
                            } else {
                                0.0
                            }
                        });
                    }

                    // Partie B * x_t
                    let b_row = &b_vec[i * dim..(i + 1) * dim];
                    sum += x_token
                        .iter()
                        .zip(b_row.iter())
                        .fold(0.0, |acc, (&x_v, &w)| {
                            acc + if w > 0.5 {
                                x_v
                            } else if w < -0.5 {
                                -x_v
                            } else {
                                0.0
                            }
                        });

                    h_t_token[i] = sum;
                }
            });

        // 2. Calcul de y_t = C * h_t (Parallélisé par jeton)
        let mut y_t_vec = vec![0.0f32; num_tokens * dim];

        y_t_vec
            .par_chunks_exact_mut(dim)
            .enumerate()
            .for_each(|(t, y_t_token)| {
                let token_offset = t * dim;
                let h_token = &h_t_vec[token_offset..token_offset + dim];

                for i in 0..dim {
                    let c_row = &c_vec[i * dim..(i + 1) * dim];
                    y_t_token[i] = h_token
                        .iter()
                        .zip(c_row.iter())
                        .fold(0.0, |acc, (&h_v, &w)| {
                            acc + if w > 0.5 {
                                h_v
                            } else if w < -0.5 {
                                -h_v
                            } else {
                                0.0
                            }
                        });
                }
            });

        let h_t = Tensor::from_vec(h_t_vec, shape, x.device())?;
        let y_t = Tensor::from_vec(y_t_vec, shape, x.device())?;

        Ok((y_t, h_t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    #[test]
    fn test_ssm_matmul_free_logic() -> Result<()> {
        let dim = 2;
        // Poids parfaitement ternaires
        let a = Tensor::new(&[[-1.0f32, 0.0], [1.0, -1.0]], &Device::Cpu)?;
        let b = Tensor::new(&[[1.0f32, -1.0], [0.0, 1.0]], &Device::Cpu)?;
        let c = Tensor::new(&[[1.0f32, 1.0], [-1.0, 0.0]], &Device::Cpu)?;

        let block = SsmBlock::new(dim, a, b, c);

        // x_t = [2.0, 3.0]
        let x = Tensor::new(&[[2.0f32, 3.0]], &Device::Cpu)?;
        // h_prev = [1.0, 2.0]
        let h_prev = Tensor::new(&[[1.0f32, 2.0]], &Device::Cpu)?;

        let (y_t, h_t) = block.forward(&x, Some(&h_prev))?;

        let h_t_out = h_t.flatten_all()?.to_vec1::<f32>()?;
        let y_t_out = y_t.flatten_all()?.to_vec1::<f32>()?;

        // Calcul Mathématique Manuel h_t = A * h_{t-1} + B * x_t:
        // A * h_prev = [-1*1 + 0*2, 1*1 + -1*2] = [-1.0, -1.0]
        // B * x_t = [1*2 + -1*3, 0*2 + 1*3] = [-1.0, 3.0]
        // h_t = [-2.0, 2.0]
        assert_eq!(
            h_t_out,
            vec![-2.0, 2.0],
            "Le calcul de h_t matmul-free a échoué."
        );

        // Calcul y_t = C * h_t :
        // C * h_t = [1*-2 + 1*2, -1*-2 + 0*2] = [0.0, 2.0]
        assert_eq!(
            y_t_out,
            vec![0.0, 2.0],
            "Le calcul de y_t matmul-free a échoué."
        );

        Ok(())
    }
}
