use r2d2_adapter_candle::embedder::CandleEmbedder;
use r2d2_kernel::ports::TextEmbedder;
use r2d2_registry::{fetcher::ModelFetcher, ModelRegistry};
use serde_json::json;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🧪 DÉMARRAGE DE L'ALCHIMISTE (Data Prep Hors-Ligne) 🧪");

    let input_file = "workspace/datasets/raw_data.txt";
    let output_file = "workspace/datasets/synthetic_dataset/data.jsonl";

    if !std::path::Path::new(input_file).exists() {
        println!(
            "⚠️ Fichier source introuvable: {}. (Génération Mocked).",
            input_file
        );
        std::fs::create_dir_all("workspace/datasets/synthetic_dataset")?;
        let mut mock = File::create(input_file)?;
        writeln!(mock, "Ceci est un prompt test causal.")?;
        writeln!(mock, "Ceci est un prompt info NCE [/INST] {{ \"is_fact\": true, \"belief_state\": 1.0, \"description\": \"Synthèse InfoNCE\", \"debated_synthesis\": \"\" }}")?;
    }

    let registry = ModelRegistry::new("data/store/manifests/");
    let (_, embedder_config) = registry
        .find_by_name(&r2d2_registry::types::ModelId("minilm_l6_v2".to_string()))
        .await
        .expect("Manifeste minilm introuvable");

    let local_manifest = ModelFetcher::ensure_downloaded(
        &embedder_config,
        "sentence-transformers/all-MiniLM-L6-v2",
        "main",
        "model.safetensors",
    )
    .await
    .expect("Failed HF Download");

    println!("🔥 Initialisation du Modèle d'Embedding CPU/GPU...");
    let embedder = tokio::task::spawn_blocking(move || {
        CandleEmbedder::new(&local_manifest).expect("Erreur instanciation CandleEmbedder")
    })
    .await?;

    let file_in = File::open(input_file)?;
    let mut file_out = File::create(output_file)?;
    let reader = BufReader::new(file_in);

    let mut generated = 0;

    for line in reader.lines() {
        let text = line?;
        if text.trim().is_empty() {
            continue;
        }

        let target_text = if let Some(idx) = text.find(" [/INST] ") {
            text[idx + 9..].to_string()
        } else {
            text.clone() // Causal Target = itself
        };

        let v_data = embedder.embed_text(&target_text).await?;

        let json_line = json!({
            "text": text,
            "embedding": v_data.data
        });

        writeln!(file_out, "{}", serde_json::to_string(&json_line)?)?;
        generated += 1;
    }

    println!(
        "✅ Alchimie terminée. {} séquences vectorisées sauvées dans {}",
        generated, output_file
    );
    Ok(())
}
