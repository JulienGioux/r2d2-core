use clap::Parser;
use std::process::Command;
use tracing::{info, Level};

#[derive(Parser, Debug)]
#[command(
    name = "r2d2-bridge",
    about = "Sovereign CDP Proxy Daemon for Windows/Linux multiplexing"
)]
struct Args {
    #[arg(short, long, default_value = "9222")]
    port: u16,
    #[arg(long, default_value = "Chrome_WSL_Debug")]
    profile: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    let args = Args::parse();

    info!("🚀 R2D2 Sovereign Bridge Daemon Initialisé");

    let chrome_path = headless_chrome::browser::default_executable()
        .map_err(|e| anyhow::anyhow!("Chrome introuvable sur ce système: {}", e))?;

    info!("Navigateur localisé : {:?}", chrome_path);

    let profile_dir = dirs::data_local_dir()
        .unwrap_or_default()
        .join(&args.profile);

    info!("Dossier de profil isolé : {:?}", profile_dir);

    // Étape 2 : Lancement isolé
    let mut child = Command::new(chrome_path)
        .arg("--remote-allow-origins=*")
        .arg(format!("--remote-debugging-port={}", args.port))
        .arg("--remote-debugging-address=0.0.0.0") // Listen on all interfaces
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg(format!("--user-data-dir={}", profile_dir.display()))
        .spawn()?;

    info!(
        "🌐 Chrome Démarré avec CDP Sécurisé Inratable sur 0.0.0.0:{}",
        args.port
    );
    info!("ℹ️  Si une alerte pare-feu Windows apparaît, veuillez l'accepter.");
    info!("Appuyez sur CTRL+C pour fermer le pont.");

    child.wait()?;
    info!("Fermeture du pont CDP.");

    Ok(())
}
