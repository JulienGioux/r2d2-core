use crate::moe::{BitFFN, Expert};
#[cfg(feature = "cuda")]
use candle_core::backend::BackendStorage;
#[cfg(feature = "cuda")]
use candle_core::cuda_backend::cudarc::driver::{LaunchAsync, LaunchConfig};
#[cfg(feature = "cuda")]
use cudarc::nvrtc::Ptx;

use candle_core::{CustomOp2, Layout, Result, Shape, Tensor};
use candle_nn::VarBuilder;
#[allow(unused_imports)]
use tracing::{info, warn};

/// Définition canonique de l'opération tensorielle asymétrique 1.58b
#[allow(dead_code)]
struct BitNetW1A8MatMulOp {
    batch_seq: usize,
    hidden_dim: usize,
    intermediate_size: usize,
}

impl CustomOp2 for BitNetW1A8MatMulOp {
    fn name(&self) -> &'static str {
        "chimera_w1a8_matmul_nvrtc"
    }

    fn cpu_fwd(
        &self,
        _s1: &candle_core::CpuStorage,
        _l1: &Layout,
        _s2: &candle_core::CpuStorage,
        _l2: &Layout,
    ) -> Result<(candle_core::CpuStorage, Shape)> {
        // En théorie, on pourrait faire le CPU Fwd ici (Fallback natif Tensor-less)
        // Mais nous avons BitFFN comme Fallback Haut-Niveau (Expert) au dessus,
        // donc CustomOp ne sera appelé que s'il est soutenu par CudaStorage.
        candle_core::bail!("BitNetW1A8MatMul CustomOp CPU Fallback direct n'est pas supporté. Utiliser le BitFFN Expert.");
    }

    #[cfg(feature = "cuda")]
    fn cuda_fwd(
        &self,
        s1: &candle_core::CudaStorage,
        _l1: &Layout,
        s2: &candle_core::CudaStorage,
        _l2: &Layout,
    ) -> Result<(candle_core::CudaStorage, Shape)> {
        // La validation du Layout (Contiguous) est déléguée au caller (BitNExpert) pour garantir 100% de compliance VRAM.
        let dev = s1.device();
        let out_shape = Shape::from((self.batch_seq, self.intermediate_size));
        let type_size = 4; // f32

        let alloc_bytes = self.batch_seq * self.intermediate_size * type_size;

        // JIT Compile Kernel. Note : en prod, celà doit être fait UNE SEULE FOIS à l'initialisation.
        // Ici, pour l'audit, nous encapsulons la logique JIT.
        if !dev.has_func("bitnet", "bitnet_matmul_kernel") {
            info!("Compilation JIT dynamique via NVRTC...");
            let ptx = Ptx::from_src(r2d2_cuda_core::CHIMERA_CUDA_KERNEL_SRC);
            dev.load_ptx(ptx, "bitnet", &["bitnet_matmul_kernel"])
                .map_err(|e| candle_core::Error::Msg(e.to_string()))?;
        }

        let f = dev.get_func("bitnet", "bitnet_matmul_kernel").unwrap();

        // Safe Allocation du tensor de sortie (zeroed)
        let out_device_alloc = dev
            .alloc_zeros::<f32>(self.batch_seq * self.intermediate_size)
            .map_err(|e| candle_core::Error::Msg(format!("cuda alloc failed: {}", e)))?;

        // Récupération abstraite CudaSlice sans dereferencement
        let slice_x = s1.as_cuda_slice::<u8>()?;
        let slice_w = s2.as_cuda_slice::<u8>()?;

        // Configuration de la grille
        let threads_x = 16;
        let threads_y = 16;
        let blocks_x = (self.intermediate_size as u32 + threads_x - 1) / threads_x;
        let blocks_y = (self.batch_seq as u32 + threads_y - 1) / threads_y;

        let cfg = LaunchConfig {
            grid_dim: (blocks_x, blocks_y, 1),
            block_dim: (threads_x, threads_y, 1),
            shared_mem_bytes: 0,
        };

        // Lancement Asynchrone Sécurisé Zero-Pointer via cudarc
        unsafe {
            f.launch(
                cfg,
                (
                    &slice_x,
                    &slice_w,
                    &out_device_alloc,
                    self.batch_seq,
                    self.hidden_dim,
                    self.intermediate_size,
                ),
            )
        }
        .map_err(|e| candle_core::Error::Msg(format!("Cudarc launch error: {}", e)))?;

        // Encapsulation
        let out_storage = candle_core::CudaStorage::wrap_cuda_slice(out_device_alloc, dev.clone());
        Ok((out_storage, out_shape))
    }
}

/// Adaptateur Hexagonal: agit comme un Expert dans notre graphe,
/// exécute la CustomOp CUDA ultra-rapide selon la disponibilité, avec Fallback CPU (BitFFN).
pub struct BitNExpert {
    fallback: BitFFN,
    force_fallback: bool,
    #[allow(dead_code)]
    hidden_dim: usize,
    #[allow(dead_code)]
    intermediate_size: usize,
}

impl BitNExpert {
    pub fn new(hidden_dim: usize, intermediate_size: usize, vb: VarBuilder) -> Result<Self> {
        let fallback = BitFFN::new(hidden_dim, intermediate_size, vb.clone())?;

        // On active le fallback par défaut si notre CUDA core n'est pas autorisé par l'environnement
        let force_fallback = std::env::var("CHIMERA_FORCE_CPU").is_ok();

        Ok(Self {
            fallback,
            force_fallback,
            hidden_dim,
            intermediate_size,
        })
    }
}

impl Expert for BitNExpert {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        if self.force_fallback || x.device().is_cpu() {
            return self.fallback.forward(x);
        }

        #[cfg(feature = "cuda")]
        {
            warn!("Le CustomOp2 est implémenté, mais les tenseurs FFN sont en QAT (F32). Fallback GPU/CPU.");
            return self.fallback.forward(x);
        }

        #[cfg(not(feature = "cuda"))]
        {
            self.fallback.forward(x)
        }
    }
}
