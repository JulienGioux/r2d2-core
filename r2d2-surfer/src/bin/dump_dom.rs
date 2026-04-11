use std::time::Duration;

#[tokio::main]
async fn main() {
    let browser = r2d2_browser::SovereignBrowser::connect("chrome-profile")
        .await
        .unwrap();
    let tab = browser
        .new_page("https://notebooklm.google.com/notebook/4dd65131-ea87-47a3-8958-a647351c4050")
        .await
        .unwrap();
    tab.wait_for_navigation().await.unwrap();
    tokio::time::sleep(Duration::from_secs(5)).await;

    let js = "document.body.innerHTML";
    if let Ok(res) = tab.evaluate(js).await {
        if let Some(val) = res.value() {
            if let Some(html) = val.as_str() {
                tokio::fs::write("/tmp/dom_dump_real.html", html)
                    .await
                    .unwrap();
            }
        }
    }
}
