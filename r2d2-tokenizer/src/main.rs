use anyhow::{Context, Result};
use hf_hub::api::sync::Api;
use std::fs;
use std::path::Path;
use tokenizers::Tokenizer;
use tracing::{info, Level};

const HIDDEN_DIM: f64 = 2048.0;     // Standard pour ~1.5B paramètres
const BYTES_PER_PARAM: f64 = 2.0;   // L'Embedding est en FP16/BF16 (Pas BitNet)
const NUM_MATRICES: f64 = 2.0;      // E (Input Embedding) + W_out (Unembedding Head)

struct TokenizerStats {
    name: String,
    vocab_size: f64,
    vram_mb: f64,
    compression: f64,
    score: f64,
}

fn main() -> Result<()> {
    // Initialisation du système de logs
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    info!("🚀 R2D2-Tokenizer: Initialisation du Benchmark 'Data-Driven'");

    // 1. Lire le dataset
    let dataset_path = Path::new("data/synthetic_dataset.jsonl");
    if !dataset_path.exists() {
        // Fallback pour cargo depuis le sous-dossier
        let dataset_path = Path::new("../data/synthetic_dataset.jsonl");
        if !dataset_path.exists() {
            anyhow::bail!("Fichier de test introuvable");
        }
    }
    let dataset_path = if Path::new("data/synthetic_dataset.jsonl").exists() {
        Path::new("data/synthetic_dataset.jsonl")
    } else {
        Path::new("../data/synthetic_dataset.jsonl")
    };

    
    info!("📖 Chargement du corpus en RAM...");
    let dataset_text = fs::read_to_string(dataset_path)
        .context("Échec de lecture de synthetic_dataset.jsonl")?;
    
    let total_bytes = dataset_text.len() as f64;
    info!("✅ Corpus chargé: {:.2} Ko d'octets bruts", total_bytes / 1024.0);

    // 2. Initialiser le client HF_HUB
    let api = Api::new()?;

    // Les concurrents du test
    let candidates = vec![
        ("LLaMA-3 128k", "NousResearch/Meta-Llama-3-8B"),
        ("Tekken (Mistral NeMo)", "mistralai/Mistral-Nemo-Base-2407"),
        ("Qwen-2.5", "Qwen/Qwen2.5-1.5B"),
    ];

    let mut leaderboard = Vec::new();

    for (name, repo_id) in candidates {
        info!("---");
        info!("⏳ Téléchargement/Vérification de {} [{}]", name, repo_id);
        
        let repo = api.model(repo_id.to_string());
        let tokenizer_file = match repo.get("tokenizer.json") {
            Ok(path) => path,
            Err(e) => {
                info!("❌ Erreur accès {}: {}. Passage au suivant.", name, e);
                continue;
            }
        };

        let tokenizer = Tokenizer::from_file(&tokenizer_file)
            .map_err(|e| anyhow::anyhow!("Échec instanciation tokenizer: {}", e))?;

        // Métriques de Taille
        let vocab_size = tokenizer.get_vocab_size(true) as f64;
        let vram_mb = (vocab_size * HIDDEN_DIM * NUM_MATRICES * BYTES_PER_PARAM) / (1024.0 * 1024.0);

        // Benchmark de Compression
        info!("⚙️ Encodage du corpus avec {}... (V={} jetons)", name, vocab_size);
        let encoding = tokenizer.encode(dataset_text.as_str(), false)
            .map_err(|e| anyhow::anyhow!("Erreur lors de l'encodage: {:?}", e))?;
            
        let total_tokens = encoding.get_ids().len() as f64;
        let compression_ratio = total_bytes / total_tokens;
        let efficiency_score = (compression_ratio / vram_mb) * 1000.0;

        leaderboard.push(TokenizerStats {
            name: name.to_string(),
            vocab_size,
            vram_mb,
            compression: compression_ratio,
            score: efficiency_score,
        });

        info!("✔️ {} calculé !", name);
    }

    // Affichage des Résultats
    info!("==================================================");
    info!("🏆 RÉSULTATS DU BENCHMARK TOKENS vs VRAM R2D2");
    info!("==================================================");
    
    // Tri décroissant selon le Score d'Efficacité R2D2
    leaderboard.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    for (i, stats) in leaderboard.iter().enumerate() {
        info!("#{}: {}", i + 1, stats.name);
        info!("   Vocabulaire : {} jetons", stats.vocab_size);
        info!("   Coût VRAM   : {:.2} Mo (Embedding+Head)", stats.vram_mb);
        info!("   Compression : {:.2} octets / jeton", stats.compression);
        info!("   SCORE R2D2  : {:.2}", stats.score);
        info!("--------------------------------------------------");
    }

    info!("Analyse terminée. L'ingénieur doit sélectionner le modèle avec le SCORE le plus élevé, et potentiellement aligner le V_SIZE à un multiple de 128 dans BitLinear.");

    Ok(())
}
