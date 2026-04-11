use std::time::Duration;

fn main() {
    let b = r2d2_browser::SovereignBrowser::connect("chrome-profile").unwrap();
    let tab = b.new_tab().unwrap();
    tab.navigate_to("https://notebooklm.google.com/notebook/4dd65131-ea87-47a3-8958-a647351c4050")
        .unwrap();
    tab.wait_until_navigated().unwrap();
    std::thread::sleep(Duration::from_secs(5));

    let js = "document.body.innerHTML";
    if let Ok(res) = tab.evaluate(js, false) {
        if let Some(val) = res.value {
            if let Some(html) = val.as_str() {
                std::fs::write("/tmp/dom_dump_real.html", html).unwrap();
            }
        }
    }
}
