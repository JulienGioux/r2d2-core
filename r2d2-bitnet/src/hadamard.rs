// R2D2 Chimera - Stabilisateur Quantique V2
// Implémentation Native de la Fast Walsh-Hadamard Transform (FWHT)
// Doctrine: Élimination de l'aliasing, Vectorisation SIMD implicite, Zéro Float Multiplication.

use candle_core::{Result, Tensor};

pub struct HadamardLayer {
    pub dimension: usize,
}

impl HadamardLayer {
    pub fn new(dim: usize) -> Self {
        assert!(
            dim.is_power_of_two(),
            "La dimension de la Transformée de Hadamard doit être une puissance de 2"
        );
        Self { dimension: dim }
    }

    /// Applique la Transformée de Hadamard Rapide (FWHT) de manière 100% Différentiable
    /// Exécution en O(N log N) via des "Bifurcations Tensorielles" (Saturate Tensor Cores)
    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let shape = x.shape();
        let dims = shape.dims().to_vec();
        let num_dims = dims.len();
        let d = *dims.last().unwrap_or(&0);

        assert_eq!(
            d, self.dimension,
            "La dernière dimension du tenseur doit correspondre à la dimension de HadamardLayer"
        );

        let mut current = x.clone();
        let mut h = 1;
        while h < d {
            // On manipule l'architecture tensorielle pour créer le pattern "Papillon" (Butterfly) :
            // Séparation des tranches en [..., Batch, 2, h]
            let mut reshaped_dims = dims[0..num_dims - 1].to_vec();
            reshaped_dims.push(d / (2 * h));
            reshaped_dims.push(2);
            reshaped_dims.push(h);

            let reshaped = current.reshape(reshaped_dims.as_slice())?;
            let split_dim = reshaped_dims.len() - 2;

            // Extractions Zéro-Copie des vues
            let left = reshaped.narrow(split_dim, 0, 1)?;
            let right = reshaped.narrow(split_dim, 1, 1)?;

            // Évaluation Mathématique Parallèle Différentiable (GPU/CPU)
            let new_left = left.broadcast_add(&right)?;
            let new_right = left.broadcast_sub(&right)?;

            // Reconstruction du Papillon
            current = Tensor::cat(&[&new_left, &new_right], split_dim)?;
            h *= 2;
        }

        // On restaure la dimension originelle (Zéro-Copie Layout)
        current.reshape(shape)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    #[test]
    fn test_fwht_correctness() -> Result<()> {
        let layer = HadamardLayer::new(4);
        // Tenseur de 1 batch, 1 seq_len, 4 hidden_dim
        let input = Tensor::new(&[[[1.0f32, 0.0, 1.0, 0.0]]], &Device::Cpu)?;
        let output = layer.forward(&input)?;

        let out_vec = output.flatten_all()?.to_vec1::<f32>()?;
        // FWH de [1, 0, 1, 0] attendu: [2, 0, 2, 0] car:
        // pass 1 (h=1): [1+0, 1-0, 1+0, 1-0] -> [1, 1, 1, 1]
        // pass 2 (h=2): [1+1, 1+1, 1-1, 1-1] -> [2, 2, 0, 0]
        assert_eq!(out_vec, vec![2.0, 2.0, 0.0, 0.0]);
        Ok(())
    }
}
