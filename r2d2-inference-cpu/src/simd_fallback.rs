//! SIMD Fallback (AVX2 / Scala) pour processeurs ne supportant pas AVX-512 nativement.

use crate::{InferenceError, PackedTernaryWeights, SimdArchitecture};

pub struct Avx2Engine;

impl SimdArchitecture for Avx2Engine {
    fn forward_layer(
        &self,
        activations: &[f32],
        weights: &[PackedTernaryWeights],
    ) -> Result<f32, InferenceError> {
        let mut sum = 0.0;

        // Traitement simple scalaire (ou AVX2 intrinsics si compilé)
        // avec padding garanti à la dimension 16 par la loop appelante
        let chunks = activations.chunks_exact(16);
        let tail = chunks.remainder();

        let mut i = 0;
        for chunk in chunks {
            if i < weights.len() {
                let w = &weights[i];
                let m_pos = w.mask_pos();
                let m_neg = w.mask_neg();

                for (j, &val) in chunk.iter().enumerate() {
                    if (m_pos & (1 << j)) != 0 {
                        sum += val;
                    }
                    if (m_neg & (1 << j)) != 0 {
                        sum -= val;
                    }
                }
            }
            i += 1;
        }

        // Tail loop
        if !tail.is_empty() {
            let w_idx = i;
            if w_idx < weights.len() {
                let w = &weights[w_idx];
                for (j, &val) in tail.iter().enumerate() {
                    if (w.mask_pos() & (1 << j)) != 0 {
                        sum += val;
                    }
                    if (w.mask_neg() & (1 << j)) != 0 {
                        sum -= val;
                    }
                }
            }
        }

        Ok(sum)
    }
}
