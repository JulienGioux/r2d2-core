use r2d2_browser::SovereignBrowser;
use r2d2_vampire::vampire_lord::notebook_api::NotebookApi;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();

    info!("🚀 [TEST V3] Bootstrapping SovereignBrowser...");
    let browser = SovereignBrowser::connect("Chrome_GOOGLE").await?;

    // Fetch notebooks list to find RustyMaster
    let tab =
        r2d2_browser::SovereignBrowser::get_or_new_tab(&browser, "notebooklm.google.com").await?;
    let api = NotebookApi::new(tab.clone()).await;

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

        let target_url = format!("https://notebooklm.google.com/notebook/{}", uuid);
        info!("Redirection Pont CDP vers : {}", target_url);

        api.tab.goto(&target_url).await?;
        api.tab.wait_for_navigation().await?;
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let prompt = "Analyse le composant DreamSimulator dans r2d2-circadian. Comment implémenter le ParadoxSolver ?";
        info!(
            "🧠 Injection Ask_Chat (V3 ReadableStream CDP Async) => {:?}",
            prompt
        );

        match api.chat_ask(&uuid, prompt).await {
            Ok(rep) => {
                info!("====================== CHAMPAGNE ======================");
                info!("{}", rep);
                info!("=======================================================");
            }
            Err(e) => {
                info!("❌ ECHEC CRITIQUE V3 : {}", e);
            }
        }
    } else {
        info!("❌ Impossible de trouver RustyMaster dans vos carnets !");
    }

    Ok(())
}
