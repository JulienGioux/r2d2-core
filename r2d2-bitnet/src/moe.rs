// R2D2 Chimera - L'Adaptation (Mixture of Experts)
// Routage neuro-dynamique et Pruning.
// Doctrine: Sélection Top-K des experts pour zéro surcharge RAM.

use crate::custom_op_cuda::BitNExpert;
use candle_core::{Result, Tensor};
use candle_nn::{Init, Linear, Module, VarBuilder};
use rayon::prelude::*;

/// Trait représentant un "Expert" asymétrique (Ex: Un bloc BitFFN ou BitLinear)
pub trait Expert {
    fn forward(&self, x: &Tensor) -> Result<Tensor>;
}

/// 🧠 BitFFN (Feed-Forward Réel)
/// Topologie 1.58-bit (Squared ReLU) validée par RustyMaster.
pub struct BitFFN {
    pub w1: Linear,
    pub w2: Linear,
}

impl BitFFN {
    pub fn new(hidden_dim: usize, intermediate_size: usize, vb: VarBuilder) -> Result<Self> {
        let span = tracing::span!(tracing::Level::DEBUG, "bitffn_init");
        let _enter = span.enter();

        let stdev = (2.0 / (hidden_dim + intermediate_size) as f64).sqrt();
        let init_xav = Init::Randn { mean: 0.0, stdev };

        // L'activation est un Squared ReLU (ReLU^2), nous supprimons donc purement W3
        // Extraction explicite statique pour QAT
        let w1_w = vb.get_with_hints((intermediate_size, hidden_dim), "w1.weight", init_xav)?;
        let w2_w = vb.get_with_hints((hidden_dim, intermediate_size), "w2.weight", init_xav)?;

        // Zero-Bias par doctrine "MatMul-Free"
        let w1 = Linear::new(w1_w, None);
        let w2 = Linear::new(w2_w, None);

        Ok(Self { w1, w2 })
    }
}

impl Expert for BitFFN {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        // Squared ReLU : max(0, x * W1)^2 -> W2
        // Optimisation Zéro-Bloat sans float "SiLU" et sans Matrice de Porte.
        let hidden = self.w1.forward(x)?;
        let relu_sqr = hidden.relu()?.sqr()?;
        self.w2.forward(&relu_sqr)
    }
}

pub struct SparseMoe {
    pub num_experts: usize,
    pub top_k: usize,
    // Le routeur est une matrice contenant les "Poids de la porte"
    pub gate_weights: Tensor,
    // Liste des dimensions internes pour le dispatch
    pub experts: Vec<Box<dyn Expert + Send + Sync>>,
}

impl SparseMoe {
    pub fn new(top_k: usize, gate_w: Tensor, experts: Vec<Box<dyn Expert + Send + Sync>>) -> Self {
        Self {
            num_experts: experts.len(),
            top_k,
            gate_weights: gate_w,
            experts,
        }
    }

    /// 🚀 Initalisation "QAT-Scratch" avec VarBuilder
    pub fn new_qat(
        hidden_dim: usize,
        num_experts: usize,
        top_k: usize,
        vb: VarBuilder,
    ) -> Result<Self> {
        // Le routeur
        let gate_w = vb.get_with_hints(
            (num_experts, hidden_dim),
            "gate_weights",
            Init::Randn {
                mean: 0.0,
                stdev: 1.0 / (hidden_dim as f64).sqrt(),
            },
        )?;

        let mut experts: Vec<Box<dyn Expert + Send + Sync>> = Vec::with_capacity(num_experts);
        let intermediate_size = hidden_dim * 4; // Ratio FFN classique

        for i in 0..num_experts {
            let expert_vb = vb.pp(format!("expert_{i}"));
            experts.push(Box::new(BitNExpert::new(
                hidden_dim,
                intermediate_size,
                expert_vb,
            )?));
        }

        Ok(Self {
            num_experts,
            top_k,
            gate_weights: gate_w,
            experts,
        })
    }

    /// Routeur Top-1 par jeton : Calcule les probabilités MatMul-Free, groupe les jetons
    /// par expert, n'évalue que les experts actifs, et recombine la sortie.
    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let shape = x.shape();
        let dims = shape.dims();
        let hidden_dim = *dims.last().unwrap_or(&0);
        let num_tokens: usize = dims[..dims.len() - 1].iter().product();

        let mut expert_assignments: Vec<Vec<usize>> = vec![Vec::new(); self.num_experts];
        let gpu_routing = x.device().is_cuda() && cfg!(feature = "cuda");

        if gpu_routing {
            #[cfg(feature = "cuda")]
            {
                use crate::custom_op_cuda::SparseMoeRoutingOp;
                let routing_op = SparseMoeRoutingOp {
                    num_experts: self.num_experts,
                    hidden_dim,
                    num_tokens,
                };

                // Appel Asynchrone VRAM au Routeur Top-K GPU
                let assignments_t = x.apply_op2_no_bwd(&self.gate_weights, &routing_op)?;
                // Seul ce tout petit Array (2 Ko pour 512 tokens) redescend du VRAM au CPU
                let assignments_vec = assignments_t.to_vec1::<u32>()?;

                for (t, &e) in assignments_vec.iter().enumerate() {
                    expert_assignments[e as usize].push(t);
                }
            }
        } else {
            let x_vec = x.flatten_all()?.to_vec1::<f32>()?;
            let gate_vec = self.gate_weights.flatten_all()?.to_vec1::<f32>()?;

            let assignments: Vec<(usize, usize)> = (0..num_tokens)
                .into_par_iter()
                .map(|t| {
                    let token_offset = t * hidden_dim;
                    let mut best_expert = 0;
                    let mut best_score = f32::NEG_INFINITY;

                    for e in 0..self.num_experts {
                        let mut score = 0.0;
                        let gate_offset = e * hidden_dim;

                        for i in 0..hidden_dim {
                            let weight = gate_vec[gate_offset + i];
                            let val = x_vec[token_offset + i];
                            if weight > 0.5 {
                                score += val;
                            } else if weight < -0.5 {
                                score -= val;
                            }
                        }

                        if score > best_score {
                            best_score = score;
                            best_expert = e;
                        }
                    }
                    (t, best_expert)
                })
                .collect();

            for (t, e) in assignments {
                expert_assignments[e].push(t);
            }
        }

        // --- 2. DISPATCH & RECOMBINAISON 100% DEVICE ---
        let x_2d = x.reshape((num_tokens, hidden_dim))?;

        // Initialisation de la sortie VRAM "Zeroed"
        let mut output_tensor = candle_core::Tensor::zeros(
            (num_tokens, hidden_dim),
            candle_core::DType::F32,
            x.device(),
        )?;

        for (e, assigned_tokens) in expert_assignments.iter().enumerate().take(self.num_experts) {
            if assigned_tokens.is_empty() {
                continue; // Zéro-Bloat absolu: Si l'expert n'est pas requis, on le saute totalement.
            }

            // [CUDA-NATIVE] Conversion des index en Tensor CPU puis déplacement asynchrone VRAM
            let idx_u32: Vec<u32> = assigned_tokens.iter().map(|&id| id as u32).collect();
            let idx = candle_core::Tensor::new(idx_u32.as_slice(), x.device())?;

            // Gather des activations via Slicing Tensoriel interne (Zéro extraction Host)
            let batch_tensor = x_2d.index_select(&idx, 0)?;

            // Exécution du Forward sur le sous-réseau
            let batch_output = self.experts[e].forward(&batch_tensor)?;

            // Scatter exact via addition sur le Tenseur F32 zeros (Propriété mathématique garantie pour Top-1/Top-K)
            output_tensor = output_tensor.index_add(&idx, &batch_output, 0)?;
        }

        // 4. Reconstitution de la Forme d'origine
        output_tensor.reshape(shape)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    struct MockExpert {
        pub id: f32,
    }

    impl Expert for MockExpert {
        fn forward(&self, x: &Tensor) -> Result<Tensor> {
            // Multiplie le tenseur par l'ID de l'expert
            // C'est notre signature radioactive pour tracer le moissonnage.
            let scalar = Tensor::new(&[self.id], x.device())?.broadcast_as(x.shape())?;
            x.broadcast_mul(&scalar)
        }
    }

    #[test]
    fn test_zero_bloat_routing() -> Result<()> {
        let experts: Vec<Box<dyn Expert + Send + Sync>> = vec![
            Box::new(MockExpert { id: 10.0 }),   // Expert 0 -> Signature x10
            Box::new(MockExpert { id: 100.0 }),  // Expert 1 -> Signature x100
            Box::new(MockExpert { id: 1000.0 }), // Expert 2 -> Signature x1000
        ];

        // Gate Weights: shape [num_experts, hidden_dim]
        let gate_w = Tensor::new(
            &[
                [1.0f32, 1.0], // Expert 0
                [-1.0, 1.0],   // Expert 1
                [-1.0, -1.0],  // Expert 2 (Ceci force un score constamment faible)
            ],
            &Device::Cpu,
        )?;

        let moe = SparseMoe::new(1, gate_w, experts);

        // Sequence de 3 jetons
        let x = Tensor::new(
            &[
                [1.0f32, 1.0], // Token 0 -> Score E0: 2, E1: 0, E2: -2.  Gagnant: Expert 0
                [-1.0, 1.0],   // Token 1 -> Score E0: 0, E1: 2, E2: 0.   Gagnant: Expert 1
                [-2.0, 2.0],   // Token 2 -> Score E0:0, E1:4, E2:0. Gagnant: Expert 1
            ],
            &Device::Cpu,
        )?;

        let out = moe.forward(&x)?;
        let out_vec = out.flatten_all()?.to_vec1::<f32>()?;

        // On vérifie le Scatter/Gather :
        // Le Jeton 0 a été multiplié par 10.
        // Les jetons 1 et 2 ont été multipliés par 100 parce qu'ils ont été "Paddés" ensemble.
        // L'expert 2 n'a jamais été chargé (Zéro Bloat prouvé).
        assert_eq!(out_vec, vec![10.0, 10.0, -100.0, 100.0, -200.0, 200.0]);
        Ok(())
    }

    #[test]
    #[cfg(feature = "cuda")]
    fn test_zero_bloat_routing_cuda() -> Result<()> {
        if !candle_core::utils::cuda_is_available() {
            return Ok(()); // Ignorer si le runtime n'a pas de GPU disponible
        }

        let device = candle_core::Device::new_cuda(0)?;

        let experts: Vec<Box<dyn Expert + Send + Sync>> = vec![
            Box::new(MockExpert { id: 10.0 }),
            Box::new(MockExpert { id: 100.0 }),
            Box::new(MockExpert { id: 1000.0 }),
        ];

        let gate_w = Tensor::new(&[[1.0f32, 1.0], [-1.0, 1.0], [-1.0, -1.0]], &device)?;

        let moe = SparseMoe::new(1, gate_w, experts);

        let x = Tensor::new(&[[1.0f32, 1.0], [-1.0, 1.0], [-2.0, 2.0]], &device)?;

        // Exécution Pure Asynchrone VRAM (Kernel OPs + Slicing natif)
        let out = moe.forward(&x)?;
        // On rapatrie à la toute fin pour le test uniquement
        let out_vec = out.flatten_all()?.to_vec1::<f32>()?;

        assert_eq!(out_vec, vec![10.0, 10.0, -100.0, 100.0, -200.0, 200.0]);
        Ok(())
    }
}
