use candle_core::{DType, Device, Tensor};
use candle_nn::{loss, AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use clap::Parser;
use inquire::{Select, Text};
use r2d2_bitnet::chimera::{ChimeraConfig, ChimeraModel};
use r2d2_registry::{
    ModelFamily, ModelId, ModelIdentity, ModelManifest, ModelMetrics, ModelTopology,
    QuantizationLevel, TaskTypology,
};
use std::fs;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Bypasser les questions interactives (Mode CI/CD)
    #[arg(short, long)]
    yes: bool,
}

#[derive(Debug, Clone)]
enum AgentProfile {
    UltraLight,
    Coder,
    Generalist,
}

impl std::fmt::Display for AgentProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentProfile::UltraLight => write!(
                f,
                "🤖 Ultra-Léger (Idéal Expérimentation/WSL - 130M params)"
            ),
            AgentProfile::Coder => {
                write!(f, "🖥️ Codeur/Logique (Intermédiaire - nécessite 8Go VRAM)")
            }
            AgentProfile::Generalist => {
                write!(f, "🧠 Discussion Globale (Complet - nécessite 24Go VRAM)")
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("============================================================");
    println!("🚀 INITIALISATION DE LA FORGE SOUVERAINE (Créateur d'Agent)");
    println!("============================================================\n");

    let (profile, agent_name) = if args.yes {
        println!("⚙️ Mode CI/CD activé : Paramètres par défaut chargés.");
        (AgentProfile::UltraLight, "Paradox-Alpha".to_string())
    } else {
        let profiles = vec![
            AgentProfile::UltraLight,
            AgentProfile::Coder,
            AgentProfile::Generalist,
        ];

        let p = Select::new(
            "Quel type d'Agent cognitif souhaitez-vous générer ?",
            profiles,
        )
        .prompt()?;
        let n = Text::new("Quel nom lui donner (ex: MonR2D2) ?")
            .with_default("Paradox-Alpha")
            .prompt()?;
        (p, n)
    };

    // Configuration mathématique en fonction du profil 'Gros Noob'
    let (config, architecture, param_count): (ChimeraConfig, &str, u64) = match profile {
        AgentProfile::UltraLight => (
            ChimeraConfig::reduced(),
            "Chimera-SSM-MoE (Micro)",
            130_000_000u64,
        ),
        AgentProfile::Coder => {
            let mut c = ChimeraConfig::b1_58_3b();
            c.hidden_size = 1024;
            c.num_experts = 4;
            (c, "Chimera-SSM-MoE (Coder)", 1_500_000_000u64)
        }
        AgentProfile::Generalist => (
            ChimeraConfig::b1_58_3b(),
            "Chimera-SSM-MoE (Complet)",
            3_000_000_000u64,
        ),
    };

    println!(
        "\n🧠 [{}] Création de l'architecture mathématique ({}) en cours...",
        agent_name, architecture
    );

    let device = Device::Cpu;
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let model =
        ChimeraModel::new_qat(&config, vb).map_err(|e| anyhow::anyhow!("Erreur Moteur: {}", e))?;

    // --- ENTRAINEMENT (Synthétique) ---
    println!("⚔️ Démarrage de l'apprentissage (Forge)...");
    // Entrées: Séquence 1D directe (Pas de Batch) pour satisfaire SsmBlock (Dims2)
    let x_data = Tensor::new(&[1u32, 2, 3, 4], &device)?;
    let y_data = Tensor::new(&[2u32, 3, 4, 1], &device)?;

    let params = ParamsAdamW {
        lr: 0.01,
        weight_decay: 0.05,
        ..Default::default()
    };
    let mut opt = AdamW::new(varmap.all_vars(), params).unwrap();

    let epochs = 50; // Raccourci pour éviter de faire attendre le user local
    let mut final_loss = 0.0;

    for _epoch in 1..=epochs {
        let (logits, _) = model.forward(&x_data, None).unwrap();
        let loss = loss::cross_entropy(&logits, &y_data).unwrap();
        opt.backward_step(&loss).unwrap();
        final_loss = loss.to_scalar::<f32>().unwrap();
    }

    println!(
        "✅ Entraînement terminé. Loss finale atteinte : {:.4}",
        final_loss
    );
    println!("------------------------------------------------------------");

    // --- R2D2 MODEL PACKAGING (RMP) Registre MLOps ---
    let uuid = Uuid::new_v4();
    let version = "1.0.0".to_string();
    let base_model_path = std::path::PathBuf::from("/home/jgx/source/R2D2/workspace/models");
    let model_folder = base_model_path
        .join("bitmamba")
        .join(format!("{}_v{}", agent_name, version));

    fs::create_dir_all(&model_folder)?;

    // Sauvegarde des Poids Safetensors
    let weights_path = model_folder.join("weights.safetensors");
    varmap.save(&weights_path).unwrap();

    // Construction du Passeport (Manifest.toml)
    let ops_manifest = ModelManifest {
        identity: ModelIdentity {
            uuid,
            name: ModelId(agent_name.clone()),
            version,
            family: ModelFamily::Bitmamba,
            author: Some("La Forge R2D2".to_string()),
        },
        topology: ModelTopology {
            architecture: architecture.to_string(),
            quantization: QuantizationLevel::Bit1_58,
            parameters: Some(param_count),
            context_window: Some(4096), // SSM = Context Window Logiquement illimité, on met 4096 technique.
        },
        format: TaskTypology::CausalLm,
        metrics: Some(ModelMetrics {
            optimal_tasks: vec!["reasoning".to_string(), "testing".to_string()],
            training_loss: Some(final_loss),
            bench_tok_sec: None,
        }),
    };

    let manifest_toml = toml::to_string_pretty(&ops_manifest)?;
    fs::write(model_folder.join("manifest.toml"), manifest_toml)?;

    println!("📦 Package généré avec succès dans le Registre !");
    println!("   -> Emplacement : {:?}", model_folder);
    println!("   -> UUID : {}", uuid);
    println!("   -> Fichiers : weights.safetensors, manifest.toml");
    println!(
        "\n🔥 L'Agent [{}] est prêt pour le déploiement.",
        agent_name
    );

    Ok(())
}
