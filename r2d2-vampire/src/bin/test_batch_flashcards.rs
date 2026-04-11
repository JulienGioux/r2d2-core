use r2d2_browser::SovereignBrowser;
use r2d2_vampire::vampire_lord::artifact_engine::ArtifactEngine;
use r2d2_vampire::vampire_lord::notebook_api::NotebookApi;
use r2d2_vampire::vampire_lord::types::{QuizDifficulty, QuizQuantity};
use std::env;
use std::path::Path;
use tokio::fs;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("🚀 Bootstrapping SovereignBrowser pour le Batch Interactif...");
    let browser = SovereignBrowser::connect("Chrome_GOOGLE").await?;
    let tab =
        r2d2_browser::SovereignBrowser::get_or_new_tab(&browser, "notebooklm.google.com").await?;
    let api = NotebookApi::new(tab.clone(), None).await;

    api.tab.goto("https://notebooklm.google.com/").await?;
    api.tab.wait_for_navigation().await?;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // 1. Récupération de l'UUID de RustArch
    let notebooks = api.list_notebooks().await?;
    let rustarch_id = notebooks
        .iter()
        .find(|(_, title)| title.to_lowercase().contains("rustarch"))
        .map(|(id, _)| id.clone());

    let uuid = match rustarch_id {
        Some(id) => {
            info!("✅ UUID RustArch trouvé : {}", id);
            id
        }
        None => {
            error!("❌ Impossible de trouver RustArch dans l'inventaire Google.");
            return Ok(());
        }
    };

    // 2. Navigation vers la page de RustArch (Nécessaire pour les Cookies & CSRF)
    let target_url = format!("https://notebooklm.google.com/notebook/{}", uuid);
    api.tab.goto(&target_url).await?;
    api.tab.wait_for_navigation().await?;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // 3. Récupération du Prompt via Argument
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        error!("❌ Usage: cargo run --bin test_batch_flashcards <PROMPT_OU_FICHIER_PROMPT>");
        return Ok(());
    }

    let input_arg = &args[1];
    let base_prompt = if Path::new(input_arg).exists() {
        fs::read_to_string(input_arg).await?.trim().to_string()
    } else {
        input_arg.trim().to_string()
    };

    if base_prompt.is_empty() {
        error!("❌ Le prompt est vide. Annulation.");
        return Ok(());
    }

    info!("🔥 Démarrage du Batch Concurrency (3 Niveaux Simulés)...");

    fs::create_dir_all("artifacts_flashcards").await?;

    let lvl1_prompt = format!("[NIVEAU 1 - DEBUTANT] Génère des flashcards (Questions/Réponses) sur le sujet suivant. Reste simple, concis et formateur. Sujet: {}", base_prompt);
    let lvl2_prompt = format!("[NIVEAU 2 - INTERMEDIAIRE] Génère des flashcards (Questions/Réponses) très détaillées sur le sujet suivant. Explore les cas d'usages pratiques. Sujet: {}", base_prompt);
    let lvl3_prompt = format!("[NIVEAU 3 - EXPERT] Génère des flashcards (Questions/Réponses) industrielles et mathématiques sur le sujet suivant. Explique le code et le bas niveau, densité maximale. Sujet: {}", base_prompt);

    let engine1 = ArtifactEngine::new(api.clone());
    let engine2 = ArtifactEngine::new(api.clone());
    let engine3 = ArtifactEngine::new(api.clone());

    let uuid1 = uuid.clone();
    let uuid2 = uuid.clone();
    let uuid3 = uuid.clone();

    // L'exécution se fait de manière asynchrone non-bloquante avec Rate Limiting Semaphore.
    let (res1, res2, res3) = tokio::join!(
        tokio::spawn(async move {
            info!("🟢 Lancement Job Lvl 1 (Easy)...");
            engine1
                .forge_flashcards(
                    uuid1,
                    lvl1_prompt,
                    QuizQuantity::Standard,
                    QuizDifficulty::Easy,
                )
                .await
        }),
        tokio::spawn(async move {
            info!("🟡 Lancement Job Lvl 2 (Medium)...");
            engine2
                .forge_flashcards(
                    uuid2,
                    lvl2_prompt,
                    QuizQuantity::Standard,
                    QuizDifficulty::Medium,
                )
                .await
        }),
        tokio::spawn(async move {
            info!("🔴 Lancement Job Lvl 3 (Hard)...");
            engine3
                .forge_flashcards(
                    uuid3,
                    lvl3_prompt,
                    QuizQuantity::Standard,
                    QuizDifficulty::Hard,
                )
                .await
        })
    );

    // 4. Traitement et Sauvegarde en JSON
    let results = [("NIVEAU_1", res1), ("NIVEAU_2", res2), ("NIVEAU_3", res3)];

    for (niveau, handle_res) in results {
        match handle_res {
            Ok(Ok(reponse_json)) => {
                let filename = format!(
                    "artifacts_flashcards/rustarch_{}.json",
                    niveau.to_lowercase()
                );

                let content_str = serde_json::to_string_pretty(&reponse_json)?;

                fs::write(&filename, content_str).await?;
                info!("✅ {} -> Sauvegardé JSON natif dans {}", niveau, filename);
            }
            Ok(Err(e)) => error!("❌ Erreur Moteur Artifact sur {}: {}", niveau, e),
            Err(e) => error!("❌ Erreur Tokio Spawn sur {}: {}", niveau, e),
        }
    }

    info!("🏆 BATCH TERMINE ! Les fiches ont été identifiées et cataloguées avec succès.");

    Ok(())
}
