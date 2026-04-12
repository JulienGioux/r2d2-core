use r2d2_browser::SovereignBrowser;
use r2d2_vampire::vampire_lord::notebook_api::NotebookApi;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(
            "info,chromiumoxide=error",
        ))
        .with_writer(std::io::stderr)
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

        let prompt = "CONTEXTE ARCHITECTURAL : Sur notre monorepo R2D2, nous utilisons l'action GitHub `rustsec/audit-check` en CI. Or, elle échoue à cause de deux \"Warnings\" transitifs (RUSTSEC-2026-0097 pour `rand` et RUSTSEC-2024-0436 pour `paste`). Ces bibliothèques sont tirées par `sqlx 0.8` et `candle-core 0.10`, qu'il est impossible de forcer en `rand 0.9` sans tout casser (Incompatibilité RngCore). QUESTION : Nous refusons de masquer vulgairement le problème avec la clé `ignore` en dur (dette technique). Sans remplacer l'action GitHub par un script sh maison, quelle est la méthode ou le pattern absolu \"The Rust Way\" pour forcer l'outil `audit-check` (ou `cargo audit`) à accepter ces seuls \"Warnings (unmaintained/unsound)\" transitifs, tout en gardant une CI cassante sur de vraies failles de sécurité CVSS ?";
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
