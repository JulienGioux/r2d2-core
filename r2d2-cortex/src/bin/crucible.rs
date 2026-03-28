use r2d2_cortex::models::reasoning_agent::ReasoningAgent;
use r2d2_cortex::agent::CognitiveAgent;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use std::fs::{self, OpenOptions};
use std::io::Write;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("🔥 Démarrage de THE CRUCIBLE (Brique XIII - Distillation Factory) 🔥");

    // Création du répertoire de sortie s'il n'existe pas
    fs::create_dir_all("data").unwrap_or_default();

    // Lecture dynamique des Seed Prompts "Militaro-Industriels"
    let seed_content = fs::read_to_string("data/seed_prompts.txt")
        .expect("⚠️ Le fichier data/seed_prompts.txt est introuvable. Créez-le !");
    
    let prompts: Vec<&str> = seed_content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#')) // Exclut les commentaires
        .collect();

    if prompts.is_empty() {
        tracing::error!("❌ La Matrice seed_prompts.txt est vide.");
        return Ok(());
    }

    info!("📚 Matrice chargée : {} Sujets Stratégiques identifiés.", prompts.len());

    let mut agent = ReasoningAgent::new();
    agent.load().await?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("data/synthetic_dataset.jsonl")?;

    for (i, prompt) in prompts.iter().enumerate() {
        info!("=======================================================");
        info!("🧪 CRUCIBLE JOB {}/{} : {}", i + 1, prompts.len(), prompt);
        
        match agent.run_crucible_distillation(prompt).await {
            Ok(synthesis) => {
                let entry = json!({
                    "messages": [
                        { "role": "user", "content": prompt },
                        { "role": "assistant", "content": synthesis }
                    ]
                });
                writeln!(file, "{}", serde_json::to_string(&entry)?)?;
                info!("💾 Dataset Entry Sauvegardée dans data/synthetic_dataset.jsonl !");
            }
            Err(e) => {
                tracing::error!("❌ Erreur sur le Job {} : {:?}", i + 1, e);
            }
        }
        
        info!("⏳ Repos thermique du Crucible (30s) avant le prochain Job...");
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    }

    info!("🏁 CAMPAGNE DE DISTILLATION TERMINÉE ! Les poids cognitifs vous attendent dans `synthetic_dataset.jsonl` !");
    Ok(())
}
