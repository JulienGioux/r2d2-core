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
                Ok(device) => {
                    // Verrouillage Anti-Fragmentation (SparseMoE Async Malloc)
                    unsafe {
                        use candle_core::cuda_backend::cudarc::driver::sys;
                        let mut pool: sys::CUmemoryPool = std::ptr::null_mut();
                        let dev_id: sys::CUdevice = 0;
                        if sys::cuDeviceGetDefaultMemPool(&mut pool, dev_id) == sys::CUresult::CUDA_SUCCESS {
                            let threshold: u64 = u64::MAX;
                            let res = sys::cuMemPoolSetAttribute(
                                pool,
                                sys::CUmemPool_attribute_enum::CU_MEMPOOL_ATTR_RELEASE_THRESHOLD,
                                &threshold as *const _ as *mut _,
                            );
                            if res == sys::CUresult::CUDA_SUCCESS {
                                info!("🛡️ [VRAM] Pool Mémoire CUDA verrouillé (ReleaseThreshold = UINT64_MAX).");
                            } else {
                                warn!("⚠️ [VRAM] Échec de la configuration du Pool Mémoire asynchrone.");
                            }
                        }
                    }
                    return Ok(device);
                }
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
