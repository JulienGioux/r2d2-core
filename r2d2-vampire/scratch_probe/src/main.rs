use headless_chrome::Browser;

fn main() {
    let ws_url = "ws://127.0.0.1:9222/devtools/browser/xyz".to_string();
    let b = Browser::connect(ws_url).unwrap();
}
