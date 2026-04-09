use crate::moe::{BitFFN, Expert};
#[cfg(feature = "cuda")]
use candle_core::backend::BackendStorage;

use candle_core::{CustomOp2, Layout, Result, Shape, Tensor};
use candle_nn::VarBuilder;
#[allow(unused_imports)]
use tracing::{info, warn};

/// Définition canonique de l'opération tensorielle asymétrique 1.58b
#[allow(dead_code)]
struct BitNetW1A8MatMulOp {
    m: usize,
    n: usize,
    k: usize, // Dimension interne K (avant l'emballage uint8_t)
}

impl CustomOp2 for BitNetW1A8MatMulOp {
    fn name(&self) -> &'static str {
        "bitnet_w1a8_matmul_nvrtc"
    }

    fn cpu_fwd(
        &self,
        _s1: &candle_core::CpuStorage,
        _l1: &Layout,
        _s2: &candle_core::CpuStorage,
        _l2: &Layout,
    ) -> Result<(candle_core::CpuStorage, Shape)> {
        candle_core::bail!("BitNetW1A8MatMul CustomOp CPU Fallback direct n'est pas supporté. Utiliser le BitFFN Expert.");
    }

    #[cfg(feature = "cuda")]
    fn cuda_fwd(
        &self,
        act: &candle_core::CudaStorage,
        act_layout: &Layout,
        w: &candle_core::CudaStorage,
        w_layout: &Layout,
    ) -> Result<(candle_core::CudaStorage, Shape)> {
        let dev = act.device();
        let out_shape = Shape::from((self.m, self.n));

        // Zero-Alloc Allocation
        let out_slice = dev
            .alloc_zeros::<f32>(self.m * self.n)
            .map_err(|e| candle_core::Error::Msg(format!("cuda alloc failed: {}", e)))?;

        let ptx_str = r2d2_cuda_core::CHIMERA_CUDA_KERNEL_PTX;

        let func =
            dev.get_or_load_custom_func("bitnet_f32_u8_matmul", "chimera_module", ptx_str)?;

        let act_slice = act.as_cuda_slice::<f32>()?;
        let w_slice = w.as_cuda_slice::<u32>()?;

        // Configuration de la grille (Heuristique CUDA Expert Ampere/Ada)
        let threads_x = 32;
        let threads_y = 32;
        let blocks_x = (self.n as u32 + threads_x - 1) / threads_x;
        let blocks_y = (self.m as u32 + threads_y - 1) / threads_y;

        let cfg = cudarc::driver::LaunchConfig {
            grid_dim: (blocks_x, blocks_y, 1),
            block_dim: (threads_x, threads_y, 1),
            shared_mem_bytes: 0,
        };

        // Lancement Asynchrone Sécurisé Zero-Pointer
        let mut builder = func.builder();
        use candle_core::cuda_backend::cudarc::driver::PushKernelArg;

        let m_i32 = self.m as i32;
        let n_i32 = self.n as i32;
        let k_i32 = self.k as i32;

        let act_sub = act_slice.slice(act_layout.start_offset()..);
        let w_sub = w_slice.slice(w_layout.start_offset()..);

        builder.arg(&act_sub);
        builder.arg(&w_sub);
        builder.arg(&out_slice);
        builder.arg(&m_i32);
        builder.arg(&n_i32);
        builder.arg(&k_i32);

        unsafe { builder.launch(cfg) }
            .map_err(|e| candle_core::Error::Msg(format!("Cudarc launch error: {}", e)))?;

        Ok((
            candle_core::CudaStorage::wrap_cuda_slice(out_slice, dev.clone()),
            out_shape,
        ))
    }
}

/// Adaptateur Hexagonal: agit comme un Expert dans notre graphe,
/// exécute la CustomOp CUDA ultra-rapide selon la disponibilité, avec Fallback CPU (BitFFN).
pub struct BitNExpert {
    fallback: BitFFN,
    force_fallback: bool,
    #[allow(dead_code)]
    w1_quant: Option<Tensor>,
    #[allow(dead_code)]
    w2_quant: Option<Tensor>,
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

        #[cfg(feature = "cuda")]
        let (w1_quant, w2_quant) = {
            if !force_fallback && vb.device().is_cuda() {
                // Construction formelle ("Zero-Allocation") du Tenseur Dual-Mask U32
                let w1_t = Self::quantize_to_u32_tensor(fallback.w1.weight())?;
                let w2_t = Self::quantize_to_u32_tensor(fallback.w2.weight())?;
                (Some(w1_t), Some(w2_t))
            } else {
                (None, None)
            }
        };

        #[cfg(not(feature = "cuda"))]
        let (w1_quant, w2_quant) = (None, None);

        Ok(Self {
            fallback,
            force_fallback,
            w1_quant,
            w2_quant,
            hidden_dim,
            intermediate_size,
        })
    }

    #[cfg(feature = "cuda")]
    fn quantize_to_u32_tensor(weight: &Tensor) -> Result<Tensor> {
        let flat_f32 = weight
            .to_dtype(candle_core::DType::F32)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        let mut u32_vec = Vec::with_capacity(flat_f32.len() / 16);

        let gamma: f32 = flat_f32.iter().map(|v| v.abs()).sum::<f32>() / (flat_f32.len() as f32);
        let scale = 1.0 / (gamma + 1e-5);

        for chunk in flat_f32.chunks_exact(16) {
            let mut m_pos = 0u16;
            let mut m_neg = 0u16;
            for (i, &v) in chunk.iter().enumerate() {
                let scaled = (v * scale).round();
                if scaled > 0.0 {
                    m_pos |= 1 << i;
                } else if scaled < 0.0 {
                    m_neg |= 1 << i;
                }
            }
            // Packing little-endian u32: inférieur=m_pos, supérieur=m_neg
            let packed = (m_pos as u32) | ((m_neg as u32) << 16);
            u32_vec.push(packed);
        }

        let shape = weight.shape().dims();
        let mut out_shape = shape.to_vec();
        let last_idx = out_shape.len() - 1;
        out_shape[last_idx] /= 16;

        Tensor::from_vec(u32_vec, out_shape, weight.device())
    }
}

impl Expert for BitNExpert {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        if self.force_fallback || x.device().is_cpu() {
            return self.fallback.forward(x);
        }

        #[cfg(feature = "cuda")]
        {
            if self.w1_quant.is_none() || self.w2_quant.is_none() {
                return self.fallback.forward(x);
            }

            let dims = x.dims();
            let rank = dims.len();
            let mut seq_batch = 1;
            for i in 0..rank - 1 {
                seq_batch *= dims[i];
            }

            // Garantie architecturale de Continuité Mémoire (Zéro-Segfault sur Tenseur Strided)
            let x_cont = x.contiguous()?;

            // Passe W1
            let w1 = self.w1_quant.as_ref().unwrap();
            let op1 = BitNetW1A8MatMulOp {
                m: seq_batch,
                n: self.intermediate_size,
                k: self.hidden_dim,
            };
            let hidden = x_cont.apply_op2_no_bwd(w1, &op1)?;

            // Fonction d'Activation (Squared ReLU sur Float32)
            let relu_sqr = hidden.relu()?.sqr()?;

            // Passe W2
            let w2 = self.w2_quant.as_ref().unwrap();
            let op2 = BitNetW1A8MatMulOp {
                m: seq_batch,
                n: self.hidden_dim,
                k: self.intermediate_size,
            };
            let out = relu_sqr.apply_op2_no_bwd(w2, &op2)?;

            // Reconstitution des dimensions originales
            let mut out_shape = dims.to_vec();
            out_shape[rank - 1] = self.hidden_dim;
            out.reshape(out_shape)
        }

        #[cfg(not(feature = "cuda"))]
        {
            self.fallback.forward(x)
        }
    }
}
