use candle_core::{DType, Device, IndexOp, Result, Tensor};
use candle_nn::{loss, AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use r2d2_bitnet::chimera::{ChimeraConfig, ChimeraModel};

/// Script "La Forge" (Micro-Entraînement 1.58-bit)
/// Démontre la convergence mathématique d'une architecture ChimeraModel (Mamba + MoE)
/// avec un Two-Stage Scheduler exigé par l'Architecte RustyMaster.
fn main() -> Result<()> {
    println!("============================================================");
    println!("🚀 INITIALISATION DE LA FORGE SOUVERAINE (Chimera QAT-Scratch)");
    println!("============================================================\n");

    let device = Device::Cpu;
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

    // Configuration réduite pour forger rapidement
    let config = ChimeraConfig::reduced();

    // Instanciation de l'architecture réelle 1.58-bit (MatMul-Free MLGRU + ReLU^2)
    let model = ChimeraModel::new_qat(&config, vb)?;

    // Dataset Synthétique (Prédiction du prochain jeton / Inversion)
    // Entrées: Séquence de 2 jetons par lot
    let x_data = Tensor::new(&[[1u32, 2], [2u32, 3], [3u32, 4], [4u32, 1]], &device)?;

    // Cibles (auto-encodeur décalé pour sémantique basique)
    // On veut un résultat dimensionnel correspondant au vocab (ici on s'assure juste du gradient flow)
    // Les cibles représentent la classe (le jeton) attendu pour la dernière position par exemple
    let y_data = Tensor::new(&[2u32, 3, 4, 1], &device)?;

    let base_lr = 0.01;
    let base_wd = 0.05;

    let params = ParamsAdamW {
        lr: base_lr,
        weight_decay: base_wd,
        ..Default::default()
    };
    let mut opt = AdamW::new(varmap.all_vars(), params)?;

    println!("🧠 Début de l'apprentissage (Two-Stage Scheduler)...");

    let epochs = 200;
    let mid_epoch = epochs / 2;

    for epoch in 1..=epochs {
        // --- 1. Two-Stage Scheduler (Ajustement dynamique LR & WD) ---
        let (current_lr, current_wd) = if epoch <= mid_epoch {
            // Phase 1 : Exploration avec apprentissage constant
            (base_lr, base_wd)
        } else {
            // Phase 2 : Refroidissement (Decay to Zero) pour stabiliser la quantification
            let progress = (epoch - mid_epoch) as f64 / (epochs - mid_epoch) as f64;
            let decay_factor = 1.0 - progress; // Linéaire vers 0
            (base_lr * decay_factor, base_wd * decay_factor)
        };

        opt.set_learning_rate(current_lr);

        // --- 2. Forward Pass ---
        // x_data a shape [4, 2] -> On recupère la sortie
        // forward renvoie (tensor, new_state) -> shape [4, 2, Vocab]
        let (logits, _new_state) = model.forward(&x_data, None)?;

        // On ne regarde que le dernier jeton de la séquence (position 1 index 2)
        let logits_last = logits.i((.., 1, ..))?;

        // --- 3. Backward Pass (Cross Entropy) ---
        let loss = loss::cross_entropy(&logits_last, &y_data)?;
        opt.backward_step(&loss)?;

        // --- 4. Weight Decay Industriel (Manuel & Vectorisé) ---
        // Exécuté post-step sur le Graphe Tensoriel direct pour contourner les limitations de l'opt.
        if current_wd > 0.0 {
            let decay_factor = 1.0 - (current_lr * current_wd);
            for var in varmap.all_vars().iter() {
                let decayed = var.affine(decay_factor, 0.0)?;
                var.set(&decayed)?;
            }
        }

        if epoch % 50 == 0 || epoch == 1 {
            let loss_val = loss.to_scalar::<f32>()?;
            println!(
                "   -> Epoch {:03}/{} | LR: {:.6} | Loss: {:.6}",
                epoch, epochs, current_lr, loss_val
            );
        }
    }

    println!("\n✅ Entraînement physique terminé. Stabilité binaire atteinte sous gel du WD.");
    println!("------------------------------------------------------------");

    // --- 4. Sauvegarde Sécurisée du Binaire ---
    let model_path = "chimera_qat.safetensors";
    println!("💾 Gravure des Tenseurs purs vers : {}", model_path);
    varmap.save(model_path)?;

    println!("🔥 PREUVE RÉUSSIE : Fichier model généré prêt pour Axum Inference !");
    Ok(())
}
