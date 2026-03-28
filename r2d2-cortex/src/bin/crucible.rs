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
    
    let args: Vec<String> = std::env::args().collect();
    let start_index = if args.len() > 1 {
        let job = args[1].parse::<usize>().unwrap_or(1);
        if job > 0 { job - 1 } else { 0 }
    } else {
        0
    };

    let prompts: Vec<&str> = seed_content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#')) // Exclut les commentaires
        .collect();

    if prompts.is_empty() {
        tracing::error!("❌ La Matrice seed_prompts.txt est vide.");
        return Ok(());
    }

    if start_index >= prompts.len() {
        tracing::error!("❌ Job de départ ({}) est supérieur au nombre de prompts ({}).", start_index + 1, prompts.len());
        return Ok(());
    }

    info!("📚 Matrice chargée : {} Sujets Stratégiques identifiés. Démarrage au Job {}.", prompts.len(), start_index + 1);

    let mut agent = ReasoningAgent::new();
    agent.load().await?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("data/synthetic_dataset.jsonl")?;

    let mut failed_jobs = Vec::new();

    for (i, prompt) in prompts.iter().enumerate().skip(start_index) {
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
                tracing::error!("❌ Erreur (Sautée) sur le Job {} : {:?}", i + 1, e);
                failed_jobs.push((i, *prompt));
            }
        }
        
        info!("⏳ Repos thermique du Crucible (30s) avant le prochain Job...");
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    }

    // --- PHASE 4 : RETRY ENGINE ---
    if !failed_jobs.is_empty() {
        info!("=======================================================");
        info!("⚠️ DEUXIÈME PASSAGE (RETRY ENGINE) : Relance de {} jobs échoués...", failed_jobs.len());
        
        for (i, prompt) in failed_jobs {
            info!("=======================================================");
            info!("♻️ RELANCE CRUCIBLE JOB {} : {}", i + 1, prompt);
            
            match agent.run_crucible_distillation(prompt).await {
                Ok(synthesis) => {
                    let entry = json!({
                        "messages": [
                            { "role": "user", "content": prompt },
                            { "role": "assistant", "content": synthesis }
                        ]
                    });
                    writeln!(file, "{}", serde_json::to_string(&entry)?)?;
                    info!("💾 Dataset Entry de Sauvetage Conservée !");
                }
                Err(e) => {
                    tracing::error!("🤡 Échec Définitif (Retry) sur le Job {} : {:?}", i + 1, e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        }
    }

    info!("🏁 CAMPAGNE DE DISTILLATION TERMINÉE ! Les poids cognitifs vous attendent dans `synthetic_dataset.jsonl` !");
    Ok(())
}
