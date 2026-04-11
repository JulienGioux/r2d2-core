use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use std::sync::Arc;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum BrowserError {
    #[error("Erreur de connexion CDP (Socket/Websocket): {0}")]
    ConnectionFailed(String),
    #[error("Impossible de créer le profile utilisateur et les LaunchOptions: {0}")]
    ProfileCreationError(String),
    #[error("Échec d'instanciation de Chromium: {0}")]
    SpawnFailed(String),
}

pub struct SovereignBrowser;

impl SovereignBrowser {
    /// Récupère l'IP Gateway de Windows depuis WSL (via la commande ip route)
    fn get_wsl_gateway() -> String {
        if let Ok(output) = std::process::Command::new("sh")
            .arg("-c")
            .arg("ip route show default | awk '{print $3}'")
            .output()
        {
            let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !ip.is_empty() {
                return ip;
            }
        }
        "127.0.0.1".to_string()
    }

    /// Découverte heuristique du binaire PowerShell sur l'hôte Windows
    fn find_powershell() -> String {
        let commands = [
            "command -v pwsh.exe",
            "command -v powershell.exe",
            "ls /mnt/c/Program\\ Files/PowerShell/*/pwsh.exe 2>/dev/null | head -n 1",
            "ls /mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe 2>/dev/null",
        ];
        for cmd in commands.iter() {
            if let Ok(output) = std::process::Command::new("sh").arg("-c").arg(cmd).output() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return path;
                }
            }
        }
        "powershell.exe".to_string() // Fallback extrême
    }

    /// Démarre le proxy asynchrone furtif sur l'hôte Windows s'il manque.
    fn attempt_windows_bridge_ignition() {
        info!("🔧 Tentative d'auto-démarrage du Pont furtif via PowerShell/WSL...");
        let script = "Start-Process \"$env:LOCALAPPDATA\\Programs\\Antigravity\\target\\x86_64-pc-windows-msvc\\release\\r2d2-bridge.exe\" -WindowStyle Hidden";
        let pwsh_bin = Self::find_powershell();
        info!("Détection de l'exécuteur PowerShell: {}", pwsh_bin);
        let _ = std::process::Command::new(pwsh_bin)
            .arg("-NoProfile")
            .arg("-Command")
            .arg(script)
            .status();
        std::thread::sleep(std::time::Duration::from_secs(3));
    }

    /// Tueur de processus fantôme pour empêcher l'accumulation de bridges.
    pub fn shutdown_windows_bridge() {
        info!("🧹 Nettoyage Sovereign : Arrêt des processus fantômes (r2d2-bridge.exe)...");
        let script = "Stop-Process -Name r2d2-bridge -Force -ErrorAction SilentlyContinue";
        let pwsh_bin = Self::find_powershell();
        let _ = std::process::Command::new(pwsh_bin)
            .arg("-NoProfile")
            .arg("-Command")
            .arg(script)
            .status();
    }

    /// Lance la boucle asynchrone Tokio vitale pour Chromiumoxide
    fn prime_actor_loop(mut handler: chromiumoxide::Handler) {
        tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                if h.is_err() {
                    break;
                }
            }
        });
    }

    /// Tente de se connecter au relais CDP Windows ou lance un Chromium local asynchrone en fallback.
    pub async fn connect(profile_name: &str) -> Result<Browser, BrowserError> {
        let base_dir =
            if let Some(proj_dirs) = directories::ProjectDirs::from("com", "R2D2", "Vampire") {
                proj_dirs.config_dir().to_path_buf()
            } else {
                std::env::current_dir()
                    .unwrap_or_default()
                    .join(".r2d2-vampire")
            };
        let _ = std::fs::create_dir_all(&base_dir);
        let profile_dir = base_dir.join(profile_name);

        let host_ip = Self::get_wsl_gateway();
        info!(
            "🔎 Recherche du Pont CDP sur l'Hôte Windows (IP: {})...",
            host_ip
        );

        let mut retry_count = 0;
        loop {
            let proxy_client = reqwest::Client::builder().no_proxy().build().unwrap();
            match proxy_client
                .get(format!("http://{}:9222/json/version", host_ip))
                .send()
                .await
            {
                Ok(resp) => {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(ws_url) =
                            json.get("webSocketDebuggerUrl").and_then(|v| v.as_str())
                        {
                            info!("🔌 Attachement asynchrone furtif au navigateur relais (Chrome Host) via CDP !");
                            let (browser, handler) = Browser::connect(ws_url)
                                .await
                                .map_err(|e| BrowserError::ConnectionFailed(e.to_string()))?;

                            Self::prime_actor_loop(handler);
                            return Ok(browser);
                        }
                    }
                    break;
                }
                Err(e) => {
                    if retry_count == 0 {
                        info!("⚠️ Pont inactif (Erreur: {}). Tentative d'Ignition...", e);
                        Self::attempt_windows_bridge_ignition();
                        retry_count += 1;
                    } else {
                        info!("⚠️ Pont indisponible même après Ignition. Démarrage d'une instance Chromium isolée en dernier recours...");
                        break;
                    }
                }
            }
        }

        let config = BrowserConfig::builder()
            .user_data_dir(profile_dir)
            .build()
            .map_err(|e| BrowserError::ProfileCreationError(e.to_string()))?;

        let (browser, handler) = Browser::launch(config)
            .await
            .map_err(|e| BrowserError::SpawnFailed(e.to_string()))?;

        Self::prime_actor_loop(handler);
        Ok(browser)
    }

    /// Tente de réutiliser un onglet existant contenant `url_matcher`, sinon en crée un nouveau.
    pub async fn get_or_new_tab(
        browser: &Browser,
        url_matcher: &str,
    ) -> Result<Arc<chromiumoxide::Page>, BrowserError> {
        let pages = browser
            .pages()
            .await
            .map_err(|e| BrowserError::SpawnFailed(format!("Echec lecture pages: {:?}", e)))?;

        for p in pages {
            let url = p.url().await.unwrap_or_default();
            if url.as_ref().is_some_and(|u| u.contains(url_matcher))
                || (url.is_none() && url_matcher == "notebooklm")
            {
                return Ok(Arc::new(p));
            }
        }

        let new_page = browser
            .new_page("about:blank")
            .await
            .map_err(|e| BrowserError::SpawnFailed(format!("Echec creation onglet: {:?}", e)))?;

        Ok(Arc::new(new_page))
    }
}
