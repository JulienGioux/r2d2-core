use hf_hub::api::sync::Api;
use std::fs;
use std::path::Path;
use tokenizers::Tokenizer;
use tracing::{info, Level};

const HIDDEN_DIM: f64 = 2048.0; // Taille standard de la dimension cachée pour notre réseau 1.58b
const BYTES_PER_PARAM: f64 = 2.0; // Précision FP16 pour la couche d'Embedding/Unembedding
const NUM_MATRICES: f64 = 2.0; // Embedding In + Projection Head Out

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("🚀 Démarrage de l'Audit Spatial des Tokenizers (Zero-Trust)");

    let dataset_path = Path::new("data/synthetic_dataset.jsonl");
    if !dataset_path.exists() {
        anyhow::bail!("Fichier dataset cible non trouvé à: {:?}", dataset_path);
    }

    let dataset_text = fs::read_to_string(dataset_path)?;
    let total_bytes = dataset_text.len() as f64;
    info!(
        "📂 Dataset chargé : {:.2} Mo",
        total_bytes / 1024.0 / 1024.0
    );

    let api = Api::new().expect("HF Hub API Init failed");

    // Les Dépôts 100% publics sans besoin de Auth Token
    let tokenizers_to_test = vec![
        ("LLaMA-3 (Meta)", "NousResearch/Meta-Llama-3-8B"),
        ("Tekken (Mistral NeMo)", "mistralai/Mistral-Nemo-Base-2407"),
        ("Qwen-2.5 (Alibaba)", "Qwen/Qwen2.5-1.5B"),
    ];

    let mut tier_list = Vec::new();

    for (name, repo_id) in tokenizers_to_test {
        info!(
            "--- Téléchargement/Lecture du dictionnaire {} [{}] ---",
            name, repo_id
        );

        let repo = api.model(repo_id.to_string());

        // Cache du tokenizer
        let tokenizer_file = repo
            .get("tokenizer.json")
            .expect("Failed to download tokenizer.json");
        let tokenizer =
            Tokenizer::from_file(&tokenizer_file).expect("Failed to parse Byte-Level BPE");

        let vocab_size = tokenizer.get_vocab_size(true) as f64;

        // 1. Calcul strict de la VRAM sacrée (Padding n'est pas inclus ici pour simplifier)
        let vram_mb =
            (vocab_size * HIDDEN_DIM * NUM_MATRICES * BYTES_PER_PARAM) / (1024.0 * 1024.0);

        // 2. Encodage lourd du jsonl (Simulation mécanique du Dataloader)
        let encoding = tokenizer.encode(dataset_text.clone(), false).unwrap();
        let total_tokens = encoding.get_ids().len() as f64;

        // 3. Mathématiques du Tiers List Architect
        let compression_ratio = total_bytes / total_tokens;
        let efficiency_score = (compression_ratio / vram_mb) * 1000.0;

        tier_list.push((
            name.to_string(),
            repo_id.to_string(),
            vocab_size,
            vram_mb,
            compression_ratio,
            efficiency_score,
            tokenizer_file.clone(),
        ));
    }

    // Affichage des Résultats
    println!("\n\n============================================================");
    println!("⚔️  TIERS LIST DES TOKENIZERS R2D2-CORTEX (BITNET)  ⚔️");
    println!("============================================================");

    // Trier par score décroissant
    tier_list.sort_by(|a, b| b.5.partial_cmp(&a.5).unwrap());

    for (i, (name, repo_id, vocab, vram, comp, score, _file)) in tier_list.iter().enumerate() {
        println!("[#{}] : {}", i + 1, name);
        println!("  - Repo       : {}", repo_id);
        println!("  - Vocab      : {:.0} jetons", vocab);
        println!(
            "  - Coût VRAM  : {:.2} Mo (Couches Hautes-Précisions)",
            vram
        );
        println!(
            "  - Densité    : {:.2} octets par jeton (Plus haut = Mieux compresse)",
            comp
        );
        println!("  => SCORE R2D2: {:.2} pts\n", score);
    }

    let winner = &tier_list[0];
    println!(
        "🏆 VAINQUEUR ABSOLU : {} avec un score de {:.2}",
        winner.0, winner.5
    );

    // Copier le fichier gagnant vers ./data/tokenizer.json
    let dest_path = Path::new("data/tokenizer.json");
    if let Err(e) = fs::copy(&winner.6, dest_path) {
        println!(
            "⚠️ Erreur lors de la copie du tokenizer gagnant vers {:?} : {}",
            dest_path, e
        );
    } else {
        println!(
            "✅ Le dictionnaire de {} a été scellé dans {:?}",
            winner.0, dest_path
        );
    }

    Ok(())
}
