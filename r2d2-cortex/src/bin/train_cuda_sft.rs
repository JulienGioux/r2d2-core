use candle_core::{DType, Device, Tensor};
use candle_nn::{loss, AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use clap::{Parser, ValueEnum};
use r2d2_bitnet::chimera::{ChimeraConfig, ChimeraModel};
use r2d2_registry::{
    ModelFamily, ModelId, ModelIdentity, ModelManifest, ModelMetrics, ModelTopology,
    QuantizationLevel,
};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum TrainingMode {
    CausalLm,
    ContrastiveEmbedding,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "R2D2 - Assimilation Mathématique SFT")]
struct Args {
    #[arg(short, long, value_enum, default_value_t = TrainingMode::CausalLm)]
    mode: TrainingMode,
}

/// 🧠 R2D2 - Assimilation Mathématique SFT (Type : Byte-Level LLM)
/// Compile et absorbe instantanément un fichier JSONL.
/// Mode CUDA Prioritaire. Support Hybride: CausalLM et ContrastiveEmbedding.
fn main() -> anyhow::Result<()> {
    println!("============================================================");
    println!("🚀 PHASE 7/9B : ASSIMILATION SFT HYBRIDE");
    println!("============================================================\n");

    let args = Args::parse();
    println!("⚙️ Mode SFT Actif : {:?}", args.mode);

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
    let dataset_path = match args.mode {
        TrainingMode::CausalLm => "/home/jgx/source/R2D2/datasets/genesis_causal.jsonl",
        TrainingMode::ContrastiveEmbedding => {
            "/home/jgx/source/R2D2/datasets/genesis_contrastive.jsonl"
        }
    };
    println!("📂 Chargement du Dataset Souverain : {}", dataset_path);

    let file = File::open(dataset_path)?;
    let reader = BufReader::new(file);

    let mut causal_sequences: Vec<Vec<u32>> = Vec::new();
    let mut paired_sequences: Vec<(Vec<u32>, Vec<u32>)> = Vec::new();
    let mut max_seq_len = 0;

    for line in reader.lines() {
        let line = line?;
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&line) {
            if let Some(text) = json_val.get("text").and_then(|t| t.as_str()) {
                match args.mode {
                    TrainingMode::CausalLm => {
                        let mut seq: Vec<u32> = text.bytes().map(|b| b as u32).collect();
                        seq.push(0); // EOS
                        if seq.len() > max_seq_len {
                            max_seq_len = seq.len();
                        }
                        causal_sequences.push(seq);
                    }
                    TrainingMode::ContrastiveEmbedding => {
                        if let Some(idx) = text.find(" [/INST] ") {
                            let prompt_str = &text[..idx];
                            let json_str = &text[idx + 9..];

                            let mut p_seq: Vec<u32> =
                                prompt_str.bytes().map(|b| b as u32).collect();
                            p_seq.push(0);
                            let mut j_seq: Vec<u32> = json_str.bytes().map(|b| b as u32).collect();
                            j_seq.push(0);

                            paired_sequences.push((p_seq, j_seq));
                        }
                    }
                }
            }
        }
    }

    match args.mode {
        TrainingMode::CausalLm => {
            println!(
                "   -> {} séquences ingérées (Max Seq Len: {})",
                causal_sequences.len(),
                max_seq_len
            );
        }
        TrainingMode::ContrastiveEmbedding => {
            println!(
                "   -> {} paires (Prompt/JSONAI) extraites pour alignement factoriel",
                paired_sequences.len()
            );
        }
    }

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

        match args.mode {
            TrainingMode::CausalLm => {
                for seq in &causal_sequences {
                    if seq.len() < 2 {
                        continue;
                    }
                    let x_arr = &seq[0..seq.len() - 1];
                    let y_arr = &seq[1..];

                    let x_data = Tensor::new(x_arr, &device)?;
                    let y_data = Tensor::new(y_arr, &device)?;

                    let (logits, _) = model.forward(&x_data, None)?;
                    let loss = loss::cross_entropy(&logits, &y_data)?;

                    opt.backward_step(&loss)?;
                    total_loss += loss.to_scalar::<f32>()?;
                    batch_count += 1;
                }
            }
            TrainingMode::ContrastiveEmbedding => {
                for (p_seq, j_seq) in &paired_sequences {
                    if p_seq.is_empty() || j_seq.is_empty() {
                        continue;
                    }
                    let p_data = Tensor::new(p_seq.as_slice(), &device)?;
                    let j_data = Tensor::new(j_seq.as_slice(), &device)?;

                    // Utilisation de forward_hidden pour manipuler l'espace vectoriel d'état, pas le logit
                    let (p_hidden, _) = model.forward_hidden(&p_data, None)?;
                    let (j_hidden, _) = model.forward_hidden(&j_data, None)?;

                    let p_emb = p_hidden.mean(0)?;
                    let j_emb = j_hidden.mean(0)?;

                    let p_norm = p_emb.sqr()?.sum_keepdim(candle_core::D::Minus1)?.sqrt()?;
                    let p_norm = p_norm.broadcast_add(&Tensor::new(&[1e-6_f32], &device)?)?;
                    let p_normed = p_emb.broadcast_div(&p_norm)?;

                    let j_norm = j_emb.sqr()?.sum_keepdim(candle_core::D::Minus1)?.sqrt()?;
                    let j_norm = j_norm.broadcast_add(&Tensor::new(&[1e-6_f32], &device)?)?;
                    let j_normed = j_emb.broadcast_div(&j_norm)?;

                    let loss = loss::mse(&p_normed, &j_normed)?;

                    opt.backward_step(&loss)?;
                    total_loss += loss.to_scalar::<f32>()?;
                    batch_count += 1;
                }
            }
        }

        let avg_loss = if batch_count > 0 {
            total_loss / batch_count as f32
        } else {
            0.0
        };
        final_loss = avg_loss;

        if epoch % 5 == 0 || epoch == 1 {
            println!(
                "   -> Epoch {:03}/{} | Loss: {:.6}",
                epoch, epochs, avg_loss
            );
        }
    }

    println!("✅ Apprentissage Terminé.");

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
            author: Some("Architecte R2D2 SFT Hybride".to_string()),
        },
        topology: ModelTopology {
            architecture: "Chimera-SSM-MoE (Byte-Level)".to_string(),
            quantization: QuantizationLevel::Bit1_58,
            parameters: Some(130_000_000),
            context_window: Some(4096),
        },
        metrics: Some(ModelMetrics {
            optimal_tasks: vec![format!("task_{:?}", args.mode)],
            training_loss: Some(final_loss),
            bench_tok_sec: None,
        }),
    };

    let manifest_toml = toml::to_string_pretty(&ops_manifest)?;
    std::fs::write(model_folder.join("manifest.toml"), manifest_toml)?;

    println!("📦 Agent SFT packagé avec succès : {:?}", model_folder);

    Ok(())
}
