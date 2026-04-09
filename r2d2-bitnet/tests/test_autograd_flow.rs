use candle_core::{DType, Device, Result, Tensor};
use candle_nn::{VarBuilder, VarMap};
use r2d2_bitnet::chimera::{ChimeraConfig, ChimeraModel};

#[test]
fn test_autograd_graph_flow_preservation() -> Result<()> {
    // 1. Initialisation de l'environnement de Test (Device CPU ou Cuda)
    let device = if candle_core::utils::cuda_is_available() {
        Device::new_cuda(0)?
    } else {
        Device::Cpu
    };

    // 2. Création du modèle Chimera avec de petits paramètres
    let config = ChimeraConfig {
        vocab_size: 100,
        hidden_size: 16,
        num_hidden_layers: 2,
        num_experts: 4,
        top_k: 2,
    };

    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

    let model = ChimeraModel::new_qat(&config, vb)?;

    // 3. Forger un tenseur Token factice et extraire la passe-avant (autograd tracked!)
    // Le VarMap assure que toutes les variables sont trackées
    let tokens = Tensor::new(&[[1u32, 2, 3, 4]], &device)?;

    // On passe à travers la "Tête" complète du Chimera
    let (logits, _states) = model.forward(&tokens, None)?;

    // 4. Calculer une perte arbitraire L1 pour déclencher l'autodifférenciation
    // Target : tout à zéro. Logits doit s'annuler
    let target = Tensor::zeros_like(&logits)?;
    let loss = (logits.sub(&target)?).abs()?.mean_all()?;

    // 5. Exécution du Sceau : Backward de Candle
    // S'il y a la moindre rupture du graphe (ex: flatten_all()?.to_vec1()),
    // Candle paniquera ou l'accumulateur de gradients renverra Vide.
    let grads = loss.backward()?;

    // 6. Validation de l'Intégrité du Vaisseau Sanguin de l'Apprentissage
    // a) Vérification des Poids Embedding
    let embed_grad = grads.get(model.embed.embeddings());
    assert!(
        embed_grad.is_some(),
        "FATAL: L'Autograd n'atteint plus les embeddings ! Graphe Graphique Brisé !"
    );

    // On vérifie que les gradients de l'Embedding (première couche) sont non-nuls
    // Preuve que la "Pression Sanguine" atteint la base du modèle
    let embed_diff = embed_grad.unwrap().abs()?.sum_all()?.to_scalar::<f32>()?;
    println!("Embedding Grad Energy: {}", embed_diff);
    assert!(
        embed_diff > 1e-6,
        "FATAL: Vanishing Gradient à la base du réseau ! Le signal est mort."
    );

    // b) Vérification d'un bloc SSM profond
    let ssm_proj_b_grad = grads.get(&model.layers[0].ssm.proj_b);
    assert!(
        ssm_proj_b_grad.is_some(),
        "FATAL: L'Autograd a échoué à traverser le SSM MIMO !"
    );
    let ssm_diff = ssm_proj_b_grad
        .unwrap()
        .abs()?
        .sum_all()?
        .to_scalar::<f32>()?;
    println!("SSM Proj_B Layer0 Grad Energy: {}", ssm_diff);
    assert!(
        ssm_diff > 1e-6,
        "FATAL: Les convolutions différentiables MIMOs ne font pas propager l'erreur."
    );

    println!(
        "✅ Le Mur des Gradients est Fracasé ! 100% de la Chimera est Rétro-Propagée Zéro-Copie !"
    );
    Ok(())
}
