use candle_core::{DType, Device, Tensor};
use candle_nn::{loss, AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use r2d2_bitnet::chimera::{ChimeraConfig, ChimeraModel};
use r2d2_registry::{
    ModelFamily, ModelId, ModelIdentity, ModelManifest, ModelMetrics, ModelTopology,
    QuantizationLevel,
};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use uuid::Uuid;

/// 🧠 R2D2 - Assimilation Mathématique SFT (Type : Byte-Level LLM)
/// Compile et absorbe instantanément un fichier JSONL.
/// Mode CUDA Prioritaire.
fn main() -> anyhow::Result<()> {
    println!("============================================================");
    println!("🚀 PHASE 7 : ASSIMILATION SFT (Supervised Fine-Tuning)");
    println!("============================================================\n");

    // 1. Détection Souveraine du Hardware
    let device = if candle_core::utils::cuda_is_available() {
        println!(
            "⚡ [HARDWARE] Noyau CUDA détecté. Activation de l'accélération matérielle (GPU 0)."
        );
        Device::new_cuda(0)?
    } else {
        println!("⚠️ [HARDWARE] CUDA non disponible. Fallback sur le CPU (Lent).");
        Device::Cpu
    };

    // 2. Chargement du Dataset (Doctrine Air-Gapped = Byte-Level Tokenizer natif)
    let dataset_path = "/home/jgx/source/R2D2/datasets/genesis_sft.jsonl";
    println!("📂 Chargement du Dataset Souverain : {}", dataset_path);

    let file = File::open(dataset_path)?;
    let reader = BufReader::new(file);
    let mut sequences: Vec<Vec<u32>> = Vec::new();
    let mut max_seq_len = 0;

    for line in reader.lines() {
        let line = line?;
        // Encodage strict en Bytes (Zéro dépendance réseau BPE)
        let mut seq: Vec<u32> = line.bytes().map(|b| b as u32).collect();
        // Ajout du End-Of-Sequence Byte magique (ex: 0)
        seq.push(0);
        if seq.len() > max_seq_len {
            max_seq_len = seq.len();
        }
        sequences.push(seq);
    }
    println!(
        "   -> {} séquences JSONAI ingérées (Max Seq Len: {} tokens octets)",
        sequences.len(),
        max_seq_len
    );

    // 3. Modélisation de l'Agent (Byte-Level Engine)
    let agent_name = "Genesis-Byte-Agent";
    let mut config = ChimeraConfig::reduced();
    config.vocab_size = 256; // ASCII pur

    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let model = ChimeraModel::new_qat(&config, vb)?;

    // 4. Moteur d'Optimisation
    let params = ParamsAdamW {
        lr: 0.005,
        weight_decay: 0.01,
        ..Default::default()
    };
    let mut opt = AdamW::new(varmap.all_vars(), params)?;

    let epochs = 20;
    println!("\n🔥 Démarrage du Gradient Descent (Epochs: {})...", epochs);

    let mut final_loss = 0.0;

    for epoch in 1..=epochs {
        let mut total_loss = 0.0;
        let mut batch_count = 0;

        for seq in &sequences {
            if seq.len() < 2 {
                continue;
            }

            // Auto-Regressive Offset: x = seq[:-1], y = seq[1:]
            let x_arr = &seq[0..seq.len() - 1];
            let y_arr = &seq[1..];

            let x_data = Tensor::new(x_arr, &device)?;
            let y_data = Tensor::new(y_arr, &device)?;

            // Forward
            let (logits, _) = model.forward(&x_data, None)?;

            // Le Logits sort [SeqLen, 256]. Y attend [SeqLen].
            // On calcule l'entropie sur l'ensemble de la phrase simultanément (Teacher Forcing intégral)
            let loss = loss::cross_entropy(&logits, &y_data)?;
            opt.backward_step(&loss)?;

            total_loss += loss.to_scalar::<f32>()?;
            batch_count += 1;
        }

        let avg_loss = total_loss / batch_count as f32;
        final_loss = avg_loss;

        if epoch % 5 == 0 || epoch == 1 {
            println!(
                "   -> Epoch {:03}/{} | Loss: {:.6}",
                epoch, epochs, avg_loss
            );
        }
    }

    println!("✅ Apprentissage JSONAI Terminé.");

    // 5. Sauvegarde RMP
    let uuid = Uuid::new_v4();
    let base_model_path = PathBuf::from("/home/jgx/source/R2D2/workspace/models");
    let model_folder = base_model_path
        .join("bitmamba")
        .join(format!("{}_v1.0.0", agent_name));
    std::fs::create_dir_all(&model_folder)?;

    let weights_path = model_folder.join("weights.safetensors");
    varmap.save(&weights_path)?;

    let ops_manifest = ModelManifest {
        identity: ModelIdentity {
            uuid,
            name: ModelId(agent_name.to_string()),
            version: "1.0.0".to_string(),
            family: ModelFamily::Bitmamba,
            author: Some("Architecte R2D2 SFT".to_string()),
        },
        topology: ModelTopology {
            architecture: "Chimera-SSM-MoE (Byte-Level)".to_string(),
            quantization: QuantizationLevel::Bit1_58,
            parameters: Some(130_000_000),
            context_window: Some(4096),
        },
        metrics: Some(ModelMetrics {
            optimal_tasks: vec!["system_ontology".to_string(), "jsonai_v3".to_string()],
            training_loss: Some(final_loss),
            bench_tok_sec: None,
        }),
    };

    let manifest_toml = toml::to_string_pretty(&ops_manifest)?;
    std::fs::write(model_folder.join("manifest.toml"), manifest_toml)?;

    println!("📦 Agent SFT packagé avec succès : {:?}", model_folder);

    Ok(())
}
