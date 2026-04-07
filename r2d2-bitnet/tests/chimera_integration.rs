use candle_core::{Device, Result, Tensor};
use r2d2_bitnet::hadamard::HadamardLayer;
use r2d2_bitnet::moe::{Expert, SparseMoe};
use r2d2_bitnet::ssm::SsmBlock;

/// Adaptateur pour transformer un bloc Mamba en un "Expert" pour le routeur.
struct SsmExpertAdapter {
    block: SsmBlock,
}

impl Expert for SsmExpertAdapter {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        // Mode Inférence Directe:
        // En vrai Mamba multi-tours, on garde l'état caché (KV Cache équivalent).
        // Ici, on valide la traversée Mathématique sans état initial (h_{t-1} = None).
        let (y_t, _h_t) = self.block.forward_scan(x, None)?;
        Ok(y_t)
    }
}

#[test]
fn test_chimera_full_pipeline_integration() -> Result<()> {
    let dim = 4;

    // ==========================================
    // 1. Instanciation de la Chaîne "Chimera V2"
    // ==========================================

    // A. La Couche de Lissage (Hadamard Transform - Zéro Float Mult)
    let hadamard = HadamardLayer::new(dim);

    // B. Création des Experts Mamba (MatMul-Free)
    let a_matrix = Tensor::new(&[1.0f32, 1.0, -1.0, -1.0], &Device::Cpu)?;

    let b_matrix = Tensor::ones((4, 4), candle_core::DType::F32, &Device::Cpu)?;
    let c_matrix = Tensor::ones((4, 4), candle_core::DType::F32, &Device::Cpu)?;

    // Expert Alpha & Beta
    let expert_alpha = SsmExpertAdapter {
        block: SsmBlock::new(dim, a_matrix.clone(), b_matrix.clone(), c_matrix.clone()),
    };
    let expert_beta = SsmExpertAdapter {
        block: SsmBlock::new(dim, a_matrix, b_matrix, c_matrix),
    };

    let experts: Vec<Box<dyn Expert + Send + Sync>> =
        vec![Box::new(expert_alpha), Box::new(expert_beta)];

    // C. Initialisation du Routeur MoE (Top-1)
    let gate_w = Tensor::new(
        &[
            [1.0f32, 1.0, -1.0, -1.0], // Matrice de signature pour attirer le Jeton 1
            [-1.0, -1.0, 1.0, 1.0],    // Matrice de signature pour attirer le Jeton 2
        ],
        &Device::Cpu,
    )?;

    let chimera_engine = SparseMoe::new(1, gate_w, experts);

    // ==========================================
    // 2. Exécution du Pipeline
    // ==========================================

    // Un prompt de 2 mots (jetons)
    let user_prompt = Tensor::new(
        &[
            [2.0f32, 2.0, 0.0, 0.0], // Ce mot est fort sur la gauche (Expert Alpha)
            [0.0, 0.0, 2.0, 2.0],    // Ce mot est fort sur la droite (Expert Beta)
        ],
        &Device::Cpu,
    )?;

    // [Step 1] : Lissage Anti-Outliers par Transformée de Hadamard Vectorisée
    let smoothed_tokens = hadamard.forward(&user_prompt)?;

    // [Step 2] : Routage Mamba par Gating Temps Réel
    let final_output = chimera_engine.forward(&smoothed_tokens)?;

    // ==========================================
    // 3. Validation Dimensionnelle et d'Intégrité
    // ==========================================

    let shape = final_output.shape().dims();
    // Le shape doit rester [2 (jetons), 4 (dim)]
    assert_eq!(
        shape,
        &[2, 4],
        "La dimension terminale du Moteur Chimera a été corrompue."
    );

    Ok(())
}
