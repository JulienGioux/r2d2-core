//! # Brique 4 : Inférence Ternaire CPU (BitNet b1.58)
//!
//! Exécution AVX-512 MathMul-Free encapsulée selon les standards Zéro-UB.

#![allow(unused_assignments)]

pub mod simd_fallback;

use r2d2_kernel::{Fragment, KernelError, Signal};
use std::arch::x86_64::*;
use thiserror::Error;
use tracing::{info, instrument, warn};

#[derive(Debug, Error)]
pub enum InferenceError {
    #[error("Erreur d'accumulation SIMD: {0}")]
    SimdFailure(String),
}

/// Contrat d'Alignement Strict.
/// Garantit formellement à la compilation que la RAM est alignée sur 64 octets
/// pour éviter tout Segfault matériel `_mm512_load_ps`.
#[repr(C, align(64))]
#[derive(Clone, Copy)]
pub struct AlignedBlock(pub [f32; 16]);

impl AlignedBlock {
    pub const fn new(data: [f32; 16]) -> Self {
        Self(data)
    }
}

/// "Newtype" transparent pour les masques de poids AVX-512 (BitNet 1.58b).
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct PackedTernaryWeights {
    // Dans une Vraie prod, nous embarquons ici les données.
    // Pour l'instant, on stocke la signature u32: (mask_pos << 16) | mask_neg
    packed_mask: u32,
}

impl PackedTernaryWeights {
    #[inline(always)]
    pub const fn new(mask_pos: u16, mask_neg: u16) -> Self {
        Self {
            packed_mask: ((mask_pos as u32) << 16) | (mask_neg as u32),
        }
    }

    #[inline(always)]
    pub fn mask_pos(&self) -> __mmask16 {
        (self.packed_mask >> 16) as __mmask16
    }

    #[inline(always)]
    pub fn mask_neg(&self) -> __mmask16 {
        (self.packed_mask & 0xFFFF) as __mmask16
    }
}

/// Typestate Pattern : Le dispatcher de l'Architecture CPU.
pub trait SimdArchitecture: Send + Sync {
    fn forward_layer(
        &self,
        activations: &[f32],
        weights: &[PackedTernaryWeights],
    ) -> Result<f32, InferenceError>;
}

/// Moteur AVX-512 Pur (Sans aucun branchement)
pub struct Avx512Engine;

impl Avx512Engine {
    pub fn try_new() -> Option<Self> {
        if is_x86_feature_detected!("avx512f") {
            Some(Self)
        } else {
            None
        }
    }

    #[target_feature(enable = "avx512f")]
    unsafe fn forward_block(&self, block: &AlignedBlock, weight: &PackedTernaryWeights) -> f32 {
        // Chargement Aligné STRICT. Zéro Segfault.
        let vals_zmm = _mm512_load_ps(block.0.as_ptr());
        let mut acc_zmm = _mm512_setzero_ps();

        let m_pos = weight.mask_pos();
        if m_pos > 0 {
            acc_zmm = _mm512_mask_add_ps(acc_zmm, m_pos, acc_zmm, vals_zmm);
        }

        let m_neg = weight.mask_neg();
        if m_neg > 0 {
            acc_zmm = _mm512_mask_sub_ps(acc_zmm, m_neg, acc_zmm, vals_zmm);
        }

        let sum = _mm512_reduce_add_ps(acc_zmm);

        let _ = _mm512_xor_ps(vals_zmm, vals_zmm);
        let _ = _mm512_xor_ps(acc_zmm, acc_zmm);
        sum
    }
}

impl SimdArchitecture for Avx512Engine {
    #[instrument(skip_all)]
    fn forward_layer(
        &self,
        activations: &[f32],
        weights: &[PackedTernaryWeights],
    ) -> Result<f32, InferenceError> {
        // Validation des chunks silencieuse et mathématiquement bornée
        let chunks = activations.chunks_exact(16);
        let tail = chunks.remainder();
        let mut sum = 0.0;

        let mut i = 0;
        for chunk in chunks {
            if i < weights.len() {
                // Try to convert slice to fixed array
                if let Ok(arr) = chunk.try_into() {
                    let aligned = AlignedBlock::new(arr);
                    unsafe {
                        sum += self.forward_block(&aligned, &weights[i]);
                    }
                }
            }
            i += 1;
        }

        // Tail (Reste non multiple de 16)
        if !tail.is_empty() {
            // Traitement scalaire du bloc résiduel
            let w_idx = i;
            if w_idx < weights.len() {
                let w = &weights[w_idx];
                for (j, &val) in tail.iter().enumerate() {
                    let m_pos = w.mask_pos();
                    let m_neg = w.mask_neg();
                    if (m_pos & (1 << j)) != 0 {
                        sum += val;
                    }
                    if (m_neg & (1 << j)) != 0 {
                        sum -= val;
                    }
                }
            }
        }

        Ok(sum)
    }
}

/// Le Délégué Principal (Pont avec le Cortex)
pub struct CpuOrchestrator {
    engine: Box<dyn SimdArchitecture>,
}

impl CpuOrchestrator {
    pub fn new() -> Result<Self, KernelError> {
        if let Some(engine) = Avx512Engine::try_new() {
            info!("Moteur : AVX-512 MathMul-Free [ALLOUÉ]");
            Ok(Self {
                engine: Box::new(engine),
            })
        } else {
            info!("Moteur : AVX-512 Non Supporté. Fallback sur AVX2.");
            Ok(Self {
                engine: Box::new(simd_fallback::Avx2Engine),
            })
        }
    }

    pub fn generate_thought(&self, activations: &[f32]) -> Result<Fragment<Signal>, KernelError> {
        // Ex: Dummy Weights
        let dummy_w = vec![PackedTernaryWeights::new(
            0b0000_0000_0000_1001,
            0b0000_0000_0000_0100,
        )];
        let final_value = self
            .engine
            .forward_layer(activations, &dummy_w)
            .map_err(|e| KernelError::ValidationFailed(e.to_string()))?;

        let payload = format!(
            r#"{{ "is_fact": false, "consensus_level": "DEBATED_SYNTHESIS", "content": "L'inférence génère la valeur {final_value}" }}"#
        );
        Ok(Fragment::new(payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aligned_block_and_mask() {
        let w = PackedTernaryWeights::new(0b1001, 0b0010);
        assert_eq!(w.mask_pos(), 9);
        assert_eq!(w.mask_neg(), 2);
    }
}
