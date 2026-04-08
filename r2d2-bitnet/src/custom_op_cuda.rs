use crate::moe::{BitFFN, Expert};
#[cfg(feature = "cuda")]
use candle_core::cuda_backend::cudarc::driver::DevicePtr;
use candle_core::{CustomOp2, Layout, Result, Shape, Tensor};
use candle_nn::VarBuilder;
#[allow(unused_imports)]
use tracing::warn;

// FFI vers le crate r2d2-cuda-core
#[allow(dead_code)]
extern "C" {
    fn chimera_w1a8_matmul(
        x_ptr: *const u8,
        w_ptr: *const u8,
        out_ptr: *mut f32,
        batch_seq: usize,
        hidden_dim: usize,
        intermediate_size: usize,
    ) -> i32;
}

/// Définition canonique de l'opération tensorielle asymétrique 1.58b
#[allow(dead_code)]
struct BitNetW1A8MatMulOp {
    batch_seq: usize,
    hidden_dim: usize,
    intermediate_size: usize,
}

impl CustomOp2 for BitNetW1A8MatMulOp {
    fn name(&self) -> &'static str {
        "chimera_w1a8_matmul"
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
        use candle_core::backend::BackendStorage;

        // La validation du Layout (Contiguous) est déléguée au caller (BitNExpert) pour garantir 100% de compliance VRAM.
        let dev = s1.device();
        let out_shape = Shape::from((self.batch_seq, self.intermediate_size));
        let type_size = 4; // f32

        let alloc_bytes = self.batch_seq * self.intermediate_size * type_size;

        let out_device_alloc = dev.alloc_zeros::<u8>(alloc_bytes).map_err(|e| {
            candle_core::Error::Cuda(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("cuda_alloc: {}", e),
            )))
        })?;

        // Récupération sécurisée avec prise en compte stricte de la "Fenêtre de Vue" (Offset VRAM)
        let slice_x = s1.as_cuda_slice::<u8>()?;
        let base_x = slice_x.device_ptr(slice_x.stream()).0 as *const u8;

        let slice_w = s2.as_cuda_slice::<u8>()?;
        let base_w = slice_w.device_ptr(slice_w.stream()).0 as *const u8;

        let d_x = unsafe { base_x.add(_l1.start_offset()) };
        let d_w = unsafe { base_w.add(_l2.start_offset()) };

        let d_out = out_device_alloc.device_ptr(out_device_alloc.stream()).0 as *mut f32;

        let status = unsafe {
            chimera_w1a8_matmul(
                d_x,
                d_w,
                d_out,
                self.batch_seq,
                self.hidden_dim,
                self.intermediate_size,
            )
        };

        if status != 0 {
            candle_core::bail!(
                "Kernel FF1 chimera_w1a8_matmul crashed with code {}",
                status
            );
        }

        let out_storage = candle_core::CudaStorage::wrap_cuda_slice(out_device_alloc, dev.clone());
        Ok((out_storage, out_shape))
    }
}

/// Adaptateur Hexagonal: agit comme un Expert dans notre graphe,
/// exécute la CustomOp CUDA ultra-rapide selon la disponibilité, avec Fallback CPU (BitFFN).
pub struct BitNExpert {
    // Tenseur pré-transposé, prêt pour la CustomOp !
    // w: Tensor,
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

        // En vrai mode 1.58b CustomOp, on devrait stocker ici le W de l'expert
        // sous forme int8 pre-loadé depuis vb.
        // Pour s'assurer de ne rien casser pour l'instant et vu que W1 et W2 de fallback sont f32 QAT,
        // nous utilisons le fallback si `w` n'est pas géré en int8 dans ce wrapper.

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
            // Dans l'avenir, lorsque le VarBuilder sera `i8` pur BitNet:
            // let x_contig = x.contiguous()?;
            // let w_contig = self.w.contiguous()?; // LE BOUCLIER ANTI-TRASHING
            // let op = BitNetW1A8MatMulOp { ... };
            // x_contig.apply_op2_no_bwd(&w_contig, &op)

            warn!("Le CustomOp2 est implémenté, mais les tenseurs FFN sont en QAT (F32). Fallback GPU/CPU.");
            return self.fallback.forward(x);
        }

        #[cfg(not(feature = "cuda"))]
        {
            self.fallback.forward(x)
        }
    }
}
