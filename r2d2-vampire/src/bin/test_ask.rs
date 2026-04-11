use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

use r2d2_blackboard::PostgresBlackboard;
use r2d2_browser::SovereignBrowser;
use r2d2_vampire::vampire_lord::harvester::Harvester;
use r2d2_vampire::vampire_lord::notebook_api::NotebookApi;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("🚀 [TEST BATCH WORKER] Bootstrapping SovereignBrowser...");
    let browser = SovereignBrowser::connect("Chrome_GOOGLE").await?;

    let tab =
        r2d2_browser::SovereignBrowser::get_or_new_tab(&browser, "notebooklm.google.com").await?;
    let api = NotebookApi::new(tab.clone(), None).await;

    // Optional: Connect to DB and spawn Harvester if Postgres is running
    // Pour l'évaluation zéro-mock, nous effectuons le test E2E.
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/r2d2".to_string());

    info!("🔎 Connexion Blackboard ({database_url})...");
    let (notify_tx, notify_rx) = mpsc::channel(1);

    if let Ok(blackboard) = PostgresBlackboard::new(&database_url).await {
        info!("✅ Blackboard asynchrone connecté !");

        // Spawn le Harvester Isolément
        let mut harvester = Harvester::new(blackboard.clone(), notify_rx);
        harvester.attach_cdp(Arc::new(api.clone())); // Dirty clone for test, in real app, wrapped nicely

        tokio::spawn(async move {
            harvester.run().await;
        });

        // TDD E2E: Ajouter une tâche fictive pour voir si elle remonte
        info!("⚙️ Simulation ajout tâche Flashcards...");
        let expert_fake = "test_ask_expert_uuid";
        match blackboard
            .enqueue_flashcard_task(expert_fake, None, Some(2), Some(2))
            .await
        {
            Ok(id) => {
                info!("Tâche Flashcard insérée avec succès en DB, ID: {id}");
                let _ = notify_tx.send(()).await;
            }
            Err(e) => {
                info!("Impossible d'insérer en DB (attendu si les tables ne sont pas encore formées): {}", e)
            }
        }
    } else {
        info!("⚠️ Blackboard (Postgres) non joignable. MOCK pure RPC.");
    }

    info!("🔎 Recherche de l'Expert RustyMaster dans la bibliothèque...");
    api.tab.goto("https://notebooklm.google.com/").await?;
    api.tab.wait_for_navigation().await?;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let notebooks = api.list_notebooks().await?;
    let rustymaster_id = notebooks
        .iter()
        .find(|(_, title)| title.to_lowercase().contains("rustymaster"))
        .map(|(id, _)| id.clone());

    if let Some(uuid) = rustymaster_id {
        info!("✅ UUID RustyMaster trouvé : {}", uuid);

        info!("🧠 Test RPC Pur batchexecute: `list_artifacts`");
        match api.list_artifacts(&uuid).await {
            Ok(res) => {
                info!("========= ARTÉFACTS POUR RUSTYMASTER =========");
                let json_pretty = serde_json::to_string_pretty(&res).unwrap_or_default();
                info!("{}", json_pretty);
                info!("===============================================");
            }
            Err(e) => info!("❌ ECHEC RPC gArtLc: {}", e),
        }
    }

    Ok(())
}
