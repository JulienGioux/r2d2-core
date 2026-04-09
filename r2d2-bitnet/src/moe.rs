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

        // 1. Extraction (Zéro Float Multiplication)
        let x_vec = x.flatten_all()?.to_vec1::<f32>()?;
        let gate_vec = self.gate_weights.flatten_all()?.to_vec1::<f32>()?; // Attendu: [num_experts, hidden_dim]

        // 2. Gating MatMul-Free en Parallèle (CPU Multithreading)
        // Utilisation de Rayon (L'optimisation "Ryzen/Rayon" !) pour distribuer
        // le calcul d'affinité des jetons sur tous les cœurs du CPU.
        let mut expert_assignments: Vec<Vec<usize>> = vec![Vec::new(); self.num_experts];

        let assignments: Vec<(usize, usize)> = (0..num_tokens)
            .into_par_iter()
            .map(|t| {
                let token_offset = t * hidden_dim;
                let mut best_expert = 0;
                let mut best_score = f32::NEG_INFINITY;

                for e in 0..self.num_experts {
                    let mut score = 0.0;
                    let gate_offset = e * hidden_dim;

                    // Opération mathématique sans Float Mult
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

        // Répartition séquentielle des résultats du multithreading
        for (t, e) in assignments {
            expert_assignments[e].push(t);
        }

        // 3. Dispatch & Récolte groupée (Batching par expert pour performance)
        let mut output_vec = vec![0.0f32; num_tokens * hidden_dim];

        for (e, assigned_tokens) in expert_assignments.iter().enumerate().take(self.num_experts) {
            if assigned_tokens.is_empty() {
                continue; // Zéro-Bloat absolu: Si l'expert n'est pas requis, on le saute totalement.
            }

            // Préparer le tenseur de batch pour cet expert
            let mut expert_input = Vec::with_capacity(assigned_tokens.len() * hidden_dim);
            for &t_idx in assigned_tokens {
                let offset = t_idx * hidden_dim;
                expert_input.extend_from_slice(&x_vec[offset..offset + hidden_dim]);
            }

            let batch_tensor = Tensor::from_vec(
                expert_input,
                (assigned_tokens.len(), hidden_dim),
                x.device(),
            )?;

            // Exécution du Forward sur le sous-réseau
            let batch_output = self.experts[e].forward(&batch_tensor)?;
            let batch_out_vec = batch_output.flatten_all()?.to_vec1::<f32>()?;

            // Dispersion (Scatter) des résultats à leur place originale
            for (batch_idx, &t_idx) in assigned_tokens.iter().enumerate() {
                let global_offset = t_idx * hidden_dim;
                let batch_offset = batch_idx * hidden_dim;
                output_vec[global_offset..global_offset + hidden_dim]
                    .copy_from_slice(&batch_out_vec[batch_offset..batch_offset + hidden_dim]);
            }
        }

        // 4. Reconstitution du Tenseur final
        Tensor::from_vec(output_vec, shape, x.device())
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
}
