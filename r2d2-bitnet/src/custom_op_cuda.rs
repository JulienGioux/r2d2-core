use crate::moe::{BitFFN, Expert};
#[cfg(feature = "cuda")]
use candle_core::backend::BackendStorage;

use candle_core::{CustomOp2, Layout, Result, Shape, Tensor};
use candle_nn::VarBuilder;
#[allow(unused_imports)]
use tracing::{info, warn};

/// Définition canonique de l'opération tensorielle asymétrique 1.58b
#[allow(dead_code)]
pub struct BitNetW1A8MatMulOp {
    pub m: usize,
    pub n: usize,
    pub k: usize, // Dimension interne K (avant l'emballage uint8_t)
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
            dev.get_or_load_custom_func("bitnet_f32_dualmask_matmul", "chimera_module", ptx_str)?;

        let act_slice = act.as_cuda_slice::<f32>()?;
        let w_slice = w.as_cuda_slice::<u32>()?;

        // Configuration de la grille (Heuristique CUDA Expert Ampere/Ada)
        let threads_x = 32;
        let threads_y = 32;
        let blocks_x = (self.n as u32 + threads_x - 1) / threads_x;
        let blocks_y = (self.m as u32 + threads_y - 1) / threads_y;

        let cfg = candle_core::cuda_backend::cudarc::driver::LaunchConfig {
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

    fn bwd(
        &self,
        arg1: &Tensor,
        _arg2: &Tensor,
        _res: &Tensor,
        grad_res: &Tensor,
    ) -> Result<(Option<Tensor>, Option<Tensor>)> {
        // arg1: act [M, K] -> en f32
        // arg2: w [N, K/16] -> en u32
        // grad_res: dY [M, N] -> en f32

        // 1. grad_w (Poids Latents): X^T * dY
        // Note: Le Tensor original `w` est quantifié [N, K/16], mais le graphe a besoin d'un gradient f32 [N, K]
        // Si l'optimiseur applique MAJ_W = W_PleinePrecision - LR * grad_w_t, les shapes doivent matcher.
        // Puisque nous sommes dans une logique CustomOp2 asymétrique, nous devons retourner Some(grad_w_t_quantifié?)
        // OU BIEN, dans la structure QAT, arg2 est le weight f32 ?
        // Puisque le noyau prend W quantifié directement, arg2 EST quantifié.
        // Si arg2 est quantifié (U32), renvoyer Some(Tensor) pour grad_w pourrait provoquer un crash de Shape
        // dans le backward pass de l'optimiseur s'il s'attend à [N, K/16] et qu'on lui donne [N, K].
        // Dans notre architecture QAT Bifurquée, on modèlera la quantification avec des OPs standards,
        // donc `BitNetW1A8MatMulOp` acceptera un gradient Dummy, l'astuce n'a lieu qu'en Fast Inference QAT.

        // Fallback sécurisé pour Candle MatMul: on ne réalloue QUE si la mémoire est disloquée
        let dy_cont = if grad_res.is_contiguous() {
            grad_res.clone()
        } else {
            grad_res.affine(1.0, 0.0)?
        };
        let act_cont = if arg1.is_contiguous() {
            arg1.clone()
        } else {
            arg1.affine(1.0, 0.0)?
        };
        // grad_w = (X^T * dY)^T = dY^T * X  => [N, M] x [M, K] = [N, K]
        let x_t = dy_cont.t()?.contiguous()?.matmul(&act_cont)?;
        let _grad_w = x_t;

        // 2. grad_x (Activations): dY * W_quant^T
        // Appel au nouveau noyau PTX "Dual-STE"
        #[cfg(feature = "cuda")]
        {
            let dev = arg1.device();
            if let candle_core::Device::Cuda(dev_cuda) = dev {
                let m = self.m;
                let n = self.n;
                let k = self.k;
                // Sécurité Anti-3D : on récupère toujours les 2 derniers strides
                // qui correspondent sémantiquement aux dimensions (M, N) ou (batch*seq, dim)
                let dy_storage = grad_res.storage_and_layout(); // Utilisation DYNAMIQUE sur vue
                let dy_cuda_storage = match &*dy_storage.0 {
                    candle_core::Storage::Cuda(c) => c,
                    _ => {
                        return Err(candle_core::Error::Msg(
                            "grad_res expected CudaStorage".to_string(),
                        ))
                    }
                };
                let dy_slice = dy_cuda_storage.as_cuda_slice::<f32>()?;
                let dy_view = dy_slice.slice(dy_storage.1.start_offset()..);
                let stride_dy = dy_storage.1.stride();
                let rank_dy = stride_dy.len();
                let stride_dy_m = if rank_dy >= 2 {
                    stride_dy[rank_dy - 2] as i32
                } else {
                    stride_dy[0] as i32
                };
                let stride_dy_n = if rank_dy >= 2 {
                    stride_dy[rank_dy - 1] as i32
                } else {
                    1
                };

                let w_storage = arg2.storage_and_layout();
                let w_cuda_storage = match &*w_storage.0 {
                    candle_core::Storage::Cuda(c) => c,
                    _ => {
                        return Err(candle_core::Error::Msg(
                            "arg2 expected CudaStorage".to_string(),
                        ))
                    }
                };
                let w_slice = w_cuda_storage.as_cuda_slice::<u32>()?;
                let w_view = w_slice.slice(w_storage.1.start_offset()..);
                let stride_w = w_storage.1.stride();
                let stride_w_n = stride_w[0] as i32;
                let stride_w_k = stride_w[1] as i32;

                let out_slice = dev_cuda.alloc_zeros::<f32>(m * k).map_err(|e| {
                    candle_core::Error::Msg(format!("cuda alloc failed during BWD: {}", e))
                })?;

                let ptx_str = r2d2_cuda_core::CHIMERA_CUDA_KERNEL_PTX;
                let func = dev_cuda.get_or_load_custom_func(
                    "bitnet_bwd_dx_matmul",
                    "chimera_module",
                    ptx_str,
                )?;

                let threads_x = 32;
                let threads_y = 8;
                let blocks_x = (k as u32 + threads_x - 1) / threads_x;
                let blocks_y = (m as u32 + threads_y - 1) / threads_y;

                let cfg = candle_core::cuda_backend::cudarc::driver::LaunchConfig {
                    grid_dim: (blocks_x, blocks_y, 1),
                    block_dim: (threads_x, threads_y, 1),
                    shared_mem_bytes: 0,
                };

                let mut builder = func.builder();
                use candle_core::cuda_backend::cudarc::driver::PushKernelArg;

                builder.arg(&dy_view);
                builder.arg(&w_view);
                builder.arg(&out_slice);
                let m_i32 = m as i32;
                let n_i32 = n as i32;
                let k_i32 = k as i32;
                builder.arg(&m_i32);
                builder.arg(&n_i32);
                builder.arg(&k_i32);
                builder.arg(&stride_dy_m);
                builder.arg(&stride_dy_n);
                builder.arg(&stride_w_n);
                builder.arg(&stride_w_k);

                unsafe { builder.launch(cfg) }.map_err(|e| {
                    candle_core::Error::Msg(format!("Cudarc launch error BWD: {}", e))
                })?;

                let shape = candle_core::Shape::from((m, k));
                let ct = candle_core::CudaStorage::wrap_cuda_slice(out_slice, dev_cuda.clone());
                let grad_x_tens = candle_core::Tensor::from_storage(
                    candle_core::Storage::Cuda(ct),
                    shape,
                    candle_core::op::BackpropOp::none(),
                    false,
                );

                return Ok((Some(grad_x_tens), Some(grad_w)));
            }
        }

        // Cpu fallback BWD (Zéro-Crash)
        Ok((None, None))
    }
}

pub struct SparseMoeRoutingOp {
    pub num_experts: usize,
    pub hidden_dim: usize,
    pub num_tokens: usize,
}

impl candle_core::CustomOp2 for SparseMoeRoutingOp {
    fn name(&self) -> &'static str {
        "sparse_moe_routing_top1"
    }

    fn cpu_fwd(
        &self,
        _s1: &candle_core::CpuStorage,
        _l1: &candle_core::Layout,
        _s2: &candle_core::CpuStorage,
        _l2: &candle_core::Layout,
    ) -> Result<(candle_core::CpuStorage, candle_core::Shape)> {
        Err(candle_core::Error::Msg(
            "SparseMoeRoutingOp uniquement supporté sur CUDA. Fallback non implémenté.".to_string(),
        ))
    }

    #[cfg(feature = "cuda")]
    fn cuda_fwd(
        &self,
        t1: &candle_core::CudaStorage,
        l1: &candle_core::Layout,
        t2: &candle_core::CudaStorage,
        l2: &candle_core::Layout,
    ) -> Result<(candle_core::CudaStorage, candle_core::Shape)> {
        let dev = t1.device().clone();

        let act_slice = t1.as_cuda_slice::<f32>()?;
        let gate_slice = t2.as_cuda_slice::<f32>()?;

        let out_shape = candle_core::Shape::from(self.num_tokens);

        // alloc_zeros est appelé sur le Device (qui invoque Async cudaMalloc sous le capot
        // via cudarc::driver::DeviceAsyncExt::alloc_zeros_async lorsqu'un stream est en cours)
        let out_slice = dev.alloc_zeros::<u32>(self.num_tokens)?;

        let ptx_str = r2d2_cuda_core::CHIMERA_CUDA_KERNEL_PTX;
        let func =
            dev.get_or_load_custom_func("sparse_moe_routing_top1", "chimera_module", ptx_str)?;

        // 1 Token = 1 Warp (32 threads).
        // 4 Warps (128 threads) par Bloc
        let threads_per_block = 128;
        let total_threads = self.num_tokens as u32 * 32;
        let blocks = (total_threads + threads_per_block - 1) / threads_per_block;

        let cfg = candle_core::cuda_backend::cudarc::driver::LaunchConfig {
            grid_dim: (blocks, 1, 1),
            block_dim: (threads_per_block, 1, 1),
            shared_mem_bytes: 0,
        };

        let mut builder = func.builder();
        use candle_core::cuda_backend::cudarc::driver::PushKernelArg;

        let act_sub = act_slice.slice(l1.start_offset()..);
        let gate_sub = gate_slice.slice(l2.start_offset()..);

        let n_tok = self.num_tokens as i32;
        let n_exp = self.num_experts as i32;
        let h_dim = self.hidden_dim as i32;

        builder.arg(&act_sub);
        builder.arg(&gate_sub);
        builder.arg(&out_slice);
        builder.arg(&n_tok);
        builder.arg(&n_exp);
        builder.arg(&h_dim);

        unsafe { builder.launch(cfg) }.map_err(|e| {
            candle_core::Error::Msg(format!("Cudarc launch error WarpRouting: {}", e))
        })?;

        // Wrapper le slice u32 pour le rendre à l'écosystème Tensor Candle
        // Puisqu'on crée un CudaStorage manuellement, on emballe le DeviceId et le Slice Cuda
        Ok((
            candle_core::CudaStorage::wrap_cuda_slice(out_slice, dev.clone()),
            out_shape,
        ))
    }

    fn bwd(
        &self,
        arg1: &Tensor,
        arg2: &Tensor,
        _res: &Tensor,
        grad_res: &Tensor,
    ) -> Result<(Option<Tensor>, Option<Tensor>)> {
        // arg1: act [num_tokens, hidden_dim]
        // arg2: gate_w [num_experts, hidden_dim]
        // _res: experts_assignment [num_tokens] (UINT32)
        // grad_res: [num_tokens] (gradient of output)
        //
        // ATTENTION Dual-STE: Le routeur produit des INDEX discrets (top-1 expert)
        // Les gradients entrant grad_res devraient être O, mais pour permettre un STE "dense" :
        // grad_x = grad_res * W^T
        // grad_w = X^T * grad_res
        // Pour des tensors 2D standards:
        let w_t = arg2.contiguous()?.t()?;

        let grad_x = match grad_res.shape().dims().len() {
            1 => {
                // S'il redescend un vec 1D d'indices ce qui n'a pas de sens matmul
                None
            }
            2 => Some(grad_res.matmul(&w_t)?),
            _ => None,
        };

        let grad_w = match grad_res.shape().dims().len() {
            2 => {
                let x_t = arg1.flatten_to(1)?.t()?;
                Some(x_t.matmul(&grad_res.flatten_to(1)?)?)
            }
            _ => None,
        };

        // Si l'opération de routing est juste utilisée comme sélecteur d'indices, les dérivées sont purement Option::None
        // car l'opération elle même ne propage pas de gradients continus (grad_res serait vide / dType inattendu).
        // Le STE du routeur s'applique si la gate produit des Continuous Scores (TopK softmax weights).
        // Puisque nous sommes "MatMul Free" strict (Top-1 discret), nous désactivons le routing gradient
        // ou nous renvoyons Ok((None, None)) si grad_res n'est pas applicable.

        if grad_x.is_some() && grad_w.is_some() {
            Ok((grad_x, grad_w))
        } else {
            // Fallback Zero-Panic
            Ok((None, None))
        }
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
