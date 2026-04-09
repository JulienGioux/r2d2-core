use candle_core::{DType, Device, Tensor};
use candle_nn::{loss, AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use clap::Parser;
use r2d2_bitnet::chimera::{ChimeraConfig, ChimeraModel};
use r2d2_registry::{
    DatasetManifest, ModelFamily, ModelId, ModelIdentity, ModelManifest, ModelMetrics,
    ModelTopology, QuantizationLevel, StateCausal, StateContrastive, TaskTypology,
    ValidatedDataset,
};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "R2D2 - Assimilation Mathématique SFT (Registry Secure)"
)]
struct Args {
    #[arg(short, long)]
    dataset_name: String,
}

/// 🧠 R2D2 - Assimilation Mathématique SFT
/// Doctrine Zéro-Erreurs: Charge dynamiquement via les Typestates Rust.
fn main() -> anyhow::Result<()> {
    println!("============================================================");
    println!("🚀 PHASE 11 : ASSIMILATION SFT SÉCURISÉE (TYPESTATE)");
    println!("============================================================\n");

    let args = Args::parse();
    println!("⚙️ Chargement du Dataset cible : '{}'", args.dataset_name);

    let device = if candle_core::utils::cuda_is_available() {
        println!("⚡ [HARDWARE] Noyau CUDA détecté.");
        Device::new_cuda(0)?
    } else {
        println!("⚠️ [HARDWARE] CUDA non disponible. Fallback CPU.");
        Device::Cpu
    };

    // 1. Validation Registry (Air-Gapped)
    let workspace_path =
        PathBuf::from("/home/jgx/source/R2D2/workspace/datasets").join(&args.dataset_name);
    let manifest_path = workspace_path.join("manifest.toml");
    let data_path = workspace_path.join("data.jsonl");

    if !manifest_path.exists() || !data_path.exists() {
        anyhow::bail!(
            "❌ ERREUR REGISTRY: Dataset '{}' introuvable dans la forge.",
            args.dataset_name
        );
    }

    let manifest_str = std::fs::read_to_string(&manifest_path)?;
    let dataset_manifest: DatasetManifest = toml::from_str(&manifest_str)?;

    println!(
        "📜 Manifeste Dataset vérifié: Mode {:?}",
        dataset_manifest.format
    );

    // 2. Modélisation de l'Agent (Byte-Level Engine)
    let agent_name = "Genesis-Byte-Agent";
    let mut config = ChimeraConfig::reduced();
    config.vocab_size = 256;

    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let model = ChimeraModel::new_qat(&config, vb)?;

    let params = ParamsAdamW {
        lr: 0.005,
        weight_decay: 0.01,
        ..Default::default()
    };
    let mut opt = AdamW::new(varmap.all_vars(), params)?;
    let epochs = 20;
    println!("\n🔥 Démarrage du Gradient Descent (Epochs: {})...", epochs);

    // 3. Typestate Dispatch Guard
    // C'est ici que l'incompatiblité matérielle est prévenue.
    let final_loss = match dataset_manifest.format {
        TaskTypology::CausalLm => {
            let validated =
                ValidatedDataset::<StateCausal>::new(data_path, dataset_manifest.clone());
            run_causal_training(&validated, &model, &mut opt, &device, epochs)?
        }
        TaskTypology::ContrastiveEmbedding => {
            let validated =
                ValidatedDataset::<StateContrastive>::new(data_path, dataset_manifest.clone());
            run_contrastive_training(&validated, &model, &mut opt, &device, epochs)?
        }
    };

    println!("✅ Apprentissage Terminé. Loss: {:.6}", final_loss);

    // 4. Sauvegarde RMP avec Traçabilité Totale
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
            architecture: "Chimera-SSM-MoE".to_string(),
            quantization: QuantizationLevel::Bit1_58,
            parameters: Some(130_000_000),
            context_window: Some(4096),
        },
        format: dataset_manifest.format, // Le Modèle hérite formellement du State
        metrics: Some(ModelMetrics {
            optimal_tasks: vec![format!("task_{:?}", dataset_manifest.format)],
            training_loss: Some(final_loss),
            bench_tok_sec: None,
        }),
    };

    let manifest_toml = toml::to_string_pretty(&ops_manifest)?;
    std::fs::write(model_folder.join("manifest.toml"), manifest_toml)?;

    println!(
        "📦 Agent SFT packagé et lié au dataset '{}' : {:?}",
        args.dataset_name, model_folder
    );

    Ok(())
}

/// Boucle stricte Causal (N'accepte qu'un Dataset <StateCausal>)
fn run_causal_training(
    dataset: &ValidatedDataset<StateCausal>,
    model: &ChimeraModel,
    opt: &mut AdamW,
    device: &Device,
    epochs: usize,
) -> anyhow::Result<f32> {
    let file = File::open(&dataset.filepath)?;
    let reader = BufReader::new(file);

    let mut causal_sequences: Vec<Vec<u32>> = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&line) {
            if let Some(text) = json_val.get("text").and_then(|t| t.as_str()) {
                let mut seq: Vec<u32> = text.bytes().map(|b| b as u32).collect();
                seq.push(0);
                causal_sequences.push(seq);
            }
        }
    }

    println!(
        "   -> {} séquences causales à l'entraînement.",
        causal_sequences.len()
    );

    let mut final_loss = 0.0;
    for epoch in 1..=epochs {
        let mut total_loss = 0.0;
        let mut batch_count = 0;

        for seq in &causal_sequences {
            if seq.len() < 2 {
                continue;
            }
            let x_data = Tensor::new(&seq[0..seq.len() - 1], device)?;
            let y_data = Tensor::new(&seq[1..], device)?;
            let (logits, _) = model.forward(&x_data, None)?;
            let loss = loss::cross_entropy(&logits, &y_data)?;
            opt.backward_step(&loss)?;
            total_loss += loss.to_scalar::<f32>()?;
            batch_count += 1;
        }

        let avg_loss = if batch_count > 0 {
            total_loss / batch_count as f32
        } else {
            0.0
        };
        final_loss = avg_loss;
        if epoch % 5 == 0 || epoch == 1 {
            println!(
                "   [CAUSAL] Epoch {:03}/{} | MSE Loss: {:.6}",
                epoch, epochs, avg_loss
            );
        }
    }
    Ok(final_loss)
}

/// Boucle stricte Contrastive (N'accepte qu'un Dataset <StateContrastive>)
fn run_contrastive_training(
    dataset: &ValidatedDataset<StateContrastive>,
    model: &ChimeraModel,
    opt: &mut AdamW,
    device: &Device,
    epochs: usize,
) -> anyhow::Result<f32> {
    let file = File::open(&dataset.filepath)?;
    let reader = BufReader::new(file);

    let mut paired_sequences: Vec<(Vec<u32>, Vec<u32>)> = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&line) {
            if let Some(text) = json_val.get("text").and_then(|t| t.as_str()) {
                if let Some(idx) = text.find(" [/INST] ") {
                    let mut p_seq: Vec<u32> = text[..idx].bytes().map(|b| b as u32).collect();
                    p_seq.push(0);
                    let mut j_seq: Vec<u32> = text[idx + 9..].bytes().map(|b| b as u32).collect();
                    j_seq.push(0);
                    paired_sequences.push((p_seq, j_seq));
                }
            }
        }
    }

    println!(
        "   -> {} paires (Prompt/JSONAI) à l'alignement.",
        paired_sequences.len()
    );

    let mut final_loss = 0.0;
    for epoch in 1..=epochs {
        let mut total_loss = 0.0;
        let mut batch_count = 0;

        for (p_seq, j_seq) in &paired_sequences {
            if p_seq.is_empty() || j_seq.is_empty() {
                continue;
            }
            let p_data = Tensor::new(p_seq.as_slice(), device)?;
            let j_data = Tensor::new(j_seq.as_slice(), device)?;

            let (p_hidden, _) = model.forward_hidden(&p_data, None)?;
            let (j_hidden, _) = model.forward_hidden(&j_data, None)?;

            let p_emb = p_hidden.mean(0)?;
            let j_emb = j_hidden.mean(0)?;

            let p_norm = p_emb.sqr()?.sum_keepdim(candle_core::D::Minus1)?.sqrt()?;
            let p_norm = p_norm.broadcast_add(&Tensor::new(&[1e-6_f32], device)?)?;
            let p_normed = p_emb.broadcast_div(&p_norm)?;

            let j_norm = j_emb.sqr()?.sum_keepdim(candle_core::D::Minus1)?.sqrt()?;
            let j_norm = j_norm.broadcast_add(&Tensor::new(&[1e-6_f32], device)?)?;
            let j_normed = j_emb.broadcast_div(&j_norm)?;

            let loss = loss::mse(&p_normed, &j_normed)?;
            opt.backward_step(&loss)?;
            total_loss += loss.to_scalar::<f32>()?;
            batch_count += 1;
        }

        let avg_loss = if batch_count > 0 {
            total_loss / batch_count as f32
        } else {
            0.0
        };
        final_loss = avg_loss;
        if epoch % 5 == 0 || epoch == 1 {
            println!(
                "   [CONTRASTIVE] Epoch {:03}/{} | MSE Loss: {:.6}",
                epoch, epochs, avg_loss
            );
        }
    }
    Ok(final_loss)
}
