use candle_core::{Device, Result};
use tracing::{info, warn};

/// Obtains the optimal computing device according to compile-time features.
/// This method gracefully falls back to CPU if no GPU hardware or driver is detected,
/// preventing strict panic conditions.
pub fn get_best_device() -> Result<Device> {
    #[cfg(feature = "cuda")]
    {
        if candle_core::utils::cuda_is_available() {
            info!("🚀 [HARDWARE] CUDA Toolkit détecté et Feature activée. Allocation Tensorielle sur GPU (NVidia).");
            match Device::new_cuda(0) {
                Ok(device) => return Ok(device),
                Err(e) => warn!("⚠️ [HARDWARE] Erreur d'initialisation CUDA: {:?}. Fallback CPU.", e),
            }
        } else {
            warn!("⚠️ [HARDWARE] Feature CUDA compilée mais driver non disponible sur l'hôte. Fallback sur CPU.");
        }
    }

    #[cfg(feature = "metal")]
    {
        if candle_core::utils::metal_is_available() {
            info!("🚀 [HARDWARE] Metal Framework détecté et Feature activée. Allocation Tensorielle sur Apple Silicon.");
            match Device::new_metal(0) {
                Ok(device) => return Ok(device),
                Err(e) => warn!("⚠️ [HARDWARE] Erreur d'initialisation Metal: {:?}. Fallback CPU.", e),
            }
        } else {
            warn!("⚠️ [HARDWARE] Feature Metal compilée mais framework non disponible. Fallback sur CPU.");
        }
    }

    // Default Fallback
    info!("🐢 [HARDWARE] Target: CPU pur (aucun accélérateur ne sera réquisitionné).");
    Ok(Device::Cpu)
}
