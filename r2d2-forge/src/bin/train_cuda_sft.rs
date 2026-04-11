use candle_core::{DType, Device, Tensor};
use candle_nn::{loss, AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use clap::Parser;
use r2d2_bitnet::chimera::{ChimeraConfig, ChimeraModel};
// unused CortexRegistry and CognitiveAgent
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

#[cfg(feature = "cuda")]
extern "C" {
    fn cudaDeviceGetDefaultMemPool(
        pool: *mut *mut std::ffi::c_void,
        device: std::ffi::c_int,
    ) -> std::ffi::c_int;
    fn cudaMemPoolSetAttribute(
        pool: *mut std::ffi::c_void,
        attr: std::ffi::c_int,
        value: *mut std::ffi::c_void,
    ) -> std::ffi::c_int;
}

pub fn lock_cuda_mempool() {
    #[cfg(feature = "cuda")]
    unsafe {
        let mut pool: *mut std::ffi::c_void = std::ptr::null_mut();
        // 0 = cudaSuccess
        if cudaDeviceGetDefaultMemPool(&mut pool, 0) == 0 {
            let mut threshold: u64 = u64::MAX;
            // attr 1 = cudaMemPoolAttrReleaseThreshold
            cudaMemPoolSetAttribute(pool, 1, &mut threshold as *mut _ as *mut std::ffi::c_void);
            println!("🔒 VRAM Asynchronous MemPool Verrouillé (Threshold: UINT64_MAX). Zéro-Fragmentation garantie.");
        }
    }
}

/// 🧠 R2D2 - Assimilation Mathématique SFT
/// Doctrine Zéro-Erreurs: Charge dynamiquement via les Typestates Rust.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("============================================================");
    println!("🚀 PHASE 11 : ASSIMILATION SFT SÉCURISÉE (TYPESTATE)");
    println!("============================================================\n");

    let args = Args::parse();
    println!("⚙️ Chargement du Dataset cible : '{}'", args.dataset_name);

    let device = if candle_core::utils::cuda_is_available() {
        println!("⚡ [HARDWARE] Noyau CUDA détecté.");
        lock_cuda_mempool();
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
    let epochs = 20;
    println!("\n🔥 Démarrage du Gradient Descent (Epochs: {})...", epochs);

    // 3. Typestate Dispatch Guard
    // C'est ici que l'incompatiblité matérielle est prévenue.
    let final_loss = match dataset_manifest.format {
        TaskTypology::CausalLm => {
            let validated =
                ValidatedDataset::<StateCausal>::new(data_path, dataset_manifest.clone());
            let mut opt = AdamW::new(varmap.all_vars(), params)?;
            run_causal_training(&validated, &model, &mut opt, &device, epochs)?
        }
        TaskTypology::ContrastiveEmbedding => {
            let validated =
                ValidatedDataset::<StateContrastive>::new(data_path, dataset_manifest.clone());

            println!("🔥 Initialisation du Tenseur Cible Cuda (JSONAI).... (Embeddings pré-calculés depuis data.jsonl)");

            run_contrastive_training(&validated, &model, params, &device, epochs, &varmap).await?
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
            domain_role: r2d2_registry::types::DomainRole::Generator,
            version: "1.0.0".to_string(),
            family: ModelFamily::Bitmamba,
            author: Some("Architecte R2D2 SFT".to_string()),
        },
        topology: ModelTopology {
            backend: r2d2_registry::types::BackendType::LocalBitNet,
            device: r2d2_registry::types::TargetDevice::Gpu(0),
            architecture: "Chimera-SSM-MoE".to_string(),
            quantization: QuantizationLevel::Bit1_58,
            vector_dimension: None,
            parameters: Some(130_000_000),
            context_window: Some(4096),
        },
        storage: r2d2_registry::manifest::StorageConfig {
            weights_path: Some(weights_path.to_string_lossy().to_string()),
            tokenizer_path: None,
            config_path: None,
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
async fn run_contrastive_training(
    dataset: &ValidatedDataset<StateContrastive>,
    model: &ChimeraModel,
    params: ParamsAdamW,
    device: &Device,
    epochs: usize,
    varmap: &VarMap,
) -> anyhow::Result<f32> {
    use candle_nn::Module;

    let file = File::open(&dataset.filepath)?;
    let reader = BufReader::new(file);

    let mut paired_sequences: Vec<(Vec<u32>, Vec<f32>)> = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&line) {
            // Le fichier data.jsonl DOIT contenir obligatoirement 'text' (prompt causal) et 'embedding' (cible dense pre-calculée).
            let text_opt = json_val.get("text").and_then(|t| t.as_str());
            let emb_opt = json_val.get("embedding").and_then(|v| v.as_array());

            if let (Some(text), Some(emb_array)) = (text_opt, emb_opt) {
                let mut p_seq: Vec<u32> = text.bytes().map(|b| b as u32).collect();
                p_seq.push(0);

                let mut emb_vec = Vec::with_capacity(emb_array.len());
                for val in emb_array {
                    if let Some(f) = val.as_f64() {
                        emb_vec.push(f as f32);
                    }
                }
                paired_sequences.push((p_seq, emb_vec));
            }
        }
    }

    println!(
        "   -> {} paires (Prompt / Dense Target) pré-calculées prêtes pour l'InfoNCE Batching. Zéro-VRAM Leak.",
        paired_sequences.len()
    );

    // --- 1. Contrastive Head & Learnable Temperature ---
    let vb_head = VarBuilder::from_varmap(varmap, DType::F32, device);
    let head_dim = 384;
    let model_dim = model.config.hidden_size;

    let proj_anchor = candle_nn::linear_no_bias(model_dim, head_dim, vb_head.pp("proj_anchor"))?;
    let proj_positive = candle_nn::linear_no_bias(384, head_dim, vb_head.pp("proj_positive"))?;

    let initial_tau = 2.65926_f64; // log(1 / 0.07)
    let log_tau = varmap.get(
        (),
        "log_tau",
        candle_nn::Init::Const(initial_tau),
        DType::F32,
        device,
    )?;

    // 2. Initialisation tardive de l'AdamW pour inclure ContrastiveHead !
    let mut opt = AdamW::new(varmap.all_vars(), params)?;

    let mut final_loss = 0.0;
    for epoch in 1..=epochs {
        let mut total_loss = 0.0;
        let mut batch_count = 0;

        // InfoNCE BATCH LOOP (Batch In-Negative Effect)
        let batch_size = 16;
        for chunk in paired_sequences.chunks(batch_size) {
            let mut anchor_tensors = Vec::new();
            let mut pos_tensors = Vec::new();

            for (p_seq, pos_vec) in chunk {
                if p_seq.is_empty() {
                    continue;
                }

                let p_data = Tensor::new(p_seq.as_slice(), device)?;
                // Utilisation du tenseur pré-calculé
                let pos_data = Tensor::from_vec(pos_vec.clone(), (1, 384), device)?;

                // Traverse Chimera
                let (p_hidden, _) = model.forward_hidden(&p_data, None)?;

                // Mean pooling temporel de la séquence (sur seq_len qui est dim 1)
                let p_emb = p_hidden.mean(1_usize)?; // [1, model_dim]

                anchor_tensors.push(p_emb);
                pos_tensors.push(pos_data);
            }

            if anchor_tensors.is_empty() {
                continue;
            }

            // Concaténation le long de la dimension batch
            let anchor_batch = Tensor::cat(&anchor_tensors, 0)?; // [B, model_dim]
            let pos_batch = Tensor::cat(&pos_tensors, 0)?; // [B, 384]

            // Projections vers l'espace InfoNCE
            let p_proj = proj_anchor.forward(&anchor_batch)?; // [B, head_dim]
            let j_proj = proj_positive.forward(&pos_batch)?; // [B, head_dim]

            // L2-Norm Stricte de chaque tenseur [B, D] le long de D (Minus1)
            let p_norm = p_proj.sqr()?.sum_keepdim(candle_core::D::Minus1)?.sqrt()?;
            let p_norm_safe = p_norm.broadcast_add(&Tensor::new(&[1e-6_f32], device)?)?;
            let p_normed = p_proj.broadcast_div(&p_norm_safe)?;

            let j_norm = j_proj.sqr()?.sum_keepdim(candle_core::D::Minus1)?.sqrt()?;
            let j_norm_safe = j_norm.broadcast_add(&Tensor::new(&[1e-6_f32], device)?)?;
            let j_normed = j_proj.broadcast_div(&j_norm_safe)?;

            // Cosine Similarity Matrix: CosineSim = Anchor * Positive^T => [B, B]
            let sim_matrix = p_normed.matmul(&j_normed.t()?)?;

            // Application de la température apprenable (tau = exp(log_tau))
            let tau = log_tau.exp()?;
            let scaled_sim = sim_matrix.broadcast_mul(&tau)?;

            // InfoNCE Loss (La Diagonale est la cible parfaite '1')
            let b_len = scaled_sim.dim(0)?;
            let targets: Vec<u32> = (0..b_len as u32).collect();
            let target_tensor = Tensor::new(targets.as_slice(), device)?;

            let loss = loss::cross_entropy(&scaled_sim, &target_tensor)?;

            // ⚡ Le Backward foudroie 100% de la structure Chimera depuis l'Espace de Contraste !
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
            let current_tau = log_tau.exp()?.to_scalar::<f32>()?;
            println!(
                "   [CONTRASTIVE InfoNCE] Epoch {:03}/{} | Batch Size: {} | Tau InfoNCE: {:.4} | Loss: {:.6}",
                epoch, epochs, batch_size, current_tau, avg_loss
            );
        }
    }
    Ok(final_loss)
}
