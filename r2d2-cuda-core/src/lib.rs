//! Cuda Core - Sovereign Shield Architecture
//!
//! Cette crate est désormais un "Provider" de noyau CUDA (PTX String)
//! pour déléguer la compilation JIT (Just-In-Time) à la crate `r2d2-bitnet` via `cudarc::nvrtc`.

#[cfg(feature = "cuda")]
pub const CHIMERA_CUDA_KERNEL_PTX: &str =
    include_str!(concat!(env!("OUT_DIR"), "/chimera_cuda.ptx"));

pub fn init() {
    tracing::info!("R2D2-CUDA-CORE Initialisé : Mode Pre-Compiled PTX JIT.");
}
