use r2d2_browser::SovereignBrowser;
use r2d2_vampire::vampire_lord::notebook_api::NotebookApi;
use r2d2_vampire::vampire_lord::types::ArtifactStatus;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let browser = SovereignBrowser::connect("Chrome_GOOGLE").await?;
    let tab =
        r2d2_browser::SovereignBrowser::get_or_new_tab(&browser, "notebooklm.google.com").await?;
    let api = NotebookApi::new(tab, None).await;

    api.tab.goto("https://notebooklm.google.com/").await?;
    api.tab.wait_for_navigation().await?;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let notebooks = api.list_notebooks().await?;
    let rustarch_id = notebooks
        .iter()
        .find(|(_, title)| title.to_lowercase().contains("rustarch"))
        .map(|(id, _)| id.clone());

    if let Some(uuid) = rustarch_id {
        tracing::info!("✅ Notebook ID: {}", uuid);

        let artifacts = api.list_artifacts(&uuid).await?;
        for art in &artifacts {
            tracing::info!(
                "Found artifact: {} - {:?} - {}",
                art.id,
                art.title,
                art.status
            );
            if art.status == ArtifactStatus::Completed {
                tracing::info!("Fetching data for {}", art.id);
                match api.fetch_artifact_data(&uuid, &art.id).await {
                    Ok(data) => {
                        let json_str = serde_json::to_string_pretty(&data)?;
                        tracing::info!("DATA:\n{}", json_str);
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::error!("Error fetching: {}", e);
                    }
                }
            }
        }
    } else {
        tracing::error!("Notebook not found");
    }

    Ok(())
}
