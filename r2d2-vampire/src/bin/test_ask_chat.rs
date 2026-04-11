use r2d2_browser::SovereignBrowser;
use r2d2_vampire::vampire_lord::notebook_api::NotebookApi;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("🚀 Bootstrapping SovereignBrowser pour tester chat_ask...");
    let browser = SovereignBrowser::connect("Chrome_GOOGLE").await?;
    let tab =
        r2d2_browser::SovereignBrowser::get_or_new_tab(&browser, "notebooklm.google.com").await?;
    let api = NotebookApi::new(tab.clone(), None).await;

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
        api.tab.goto(&target_url).await?;
        api.tab.wait_for_navigation().await?;
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let prompt = "Quelle est la définition d'une architecture hexagonale ?";
        info!("🧠 Injection Ask_Chat => {:?}", prompt);

        match api.chat_ask(&uuid, prompt).await {
            Ok(rep) => {
                info!("========= REPONSE =========");
                info!("{}", rep);
            }
            Err(e) => {
                info!("❌ ECHEC chat_ask : {}", e);
            }
        }
    } else {
        info!("❌ Impossible de trouver RustyMaster");
    }
    Ok(())
}
