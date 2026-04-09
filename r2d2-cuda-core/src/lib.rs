//! Cuda Core - Sovereign Shield Architecture
//!
//! Cette crate est désormais un "Provider" de noyau CUDA (PTX String)
//! pour déléguer la compilation JIT (Just-In-Time) à la crate `r2d2-bitnet` via `cudarc::nvrtc`.

/// Noyau CUDA natif au format Source (String) prêt pour la compilation asynchrone (nvrtc).
pub const CHIMERA_CUDA_KERNEL_SRC: &str = include_str!("kernels/chimera_cuda.cu");

pub fn init() {
    tracing::info!("R2D2-CUDA-CORE Initialisé : Mode Dynamic PTX JIT.");
}
