#![cfg(feature = "cdp_bridge")]

use axum::response::IntoResponse;
use axum::{routing::post, Router};
use r2d2_vampire::vampire_lord::notebook_api::NotebookApi;
use std::time::Duration;
use tokio::time::timeout;

/// 1. DROP GUARD : Assurance "Zero-Crash" & Anti-Zombie
struct ChromiumZombieGuard {
    process_id: Option<u32>,
}

impl Drop for ChromiumZombieGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.process_id {
            // Logique de kill OS natif pour prévenir les fuites de RAM/Zombies
            let _ = std::process::Command::new("kill")
                .arg("-9")
                .arg(pid.to_string())
                .output();
        }
    }
}

/// Simulation de l'ordonnanceur NotebookLM de Google
async fn mock_generate_stream() -> impl IntoResponse {
    // Utilisation de hyper/axum stream
    // On va retourner une simple reponse textuelle et bloquer avec sleep()
    // Pour simuler du chunk, on pourrait utiliser reqwest/hyper mais pour notre test de Timeout de base,
    // attendre indéfiniment sans fermer la connexion est suffisant pour valider le CircuitBreaker global.

    tokio::time::sleep(Duration::from_secs(10)).await;

    ")]}'\n[[ \"wrb.fr\", null, \"[[\\\"LA_VERITE_VRAIE\\\"]]\" ]]\n"
}

/// 2. LE SERVEUR AXUM FAKE (Usurpation de domaine)
async fn spawn_local_notebooklm() -> String {
    let app = Router::new()
        // Mock de l'endpoint exact tapé par le client reqwest (POST)
        .route("/_/LabsTailwindUi/data/google.internal.labs.tailwind.orchestration.v1.LabsTailwindOrchestrationService/GenerateFreeFormStreamed", post(mock_generate_stream));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    // Confinement asynchrone du serveur local pour éviter la famine
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://127.0.0.1:{}", port)
}

/// 3. TEST D'INTÉGRATION OPAQUE-BOX
#[tokio::test]
async fn test_simulate_reqwest_hijacking_stream() -> anyhow::Result<()> {
    // A. Boot du serveur local (Port Zéro Dynamique)
    let fake_ws_url = spawn_local_notebooklm().await;

    // B. Instanciation du vrai adaptateur (Zero-Mock) via SovereignBrowser
    let browser_res = r2d2_browser::SovereignBrowser::connect("Chrome_TEST").await;
    let browser = match browser_res {
        Ok(b) => b,
        Err(_) => {
            println!("SKIPPED: Aucun Navigateur disponible sur le port de debug 9222 ou en local. Mettez le navigateur en Debug Mode.");
            return Ok(());
        }
    };

    let tab = browser
        .new_page(&fake_ws_url)
        .await
        .map_err(|e| anyhow::anyhow!("new_page failed: {}", e))?;
    let tab = std::sync::Arc::new(tab);

    // Injection de la dépendance BASE_URL
    let api = NotebookApi::new(tab.clone(), Some(fake_ws_url)).await;

    let _guard = ChromiumZombieGuard { process_id: None };

    // C. Exécution: On appelle le vrai `chat_ask`.
    // Le serveur Axum va sleep 10s. On met un timeout de 2s autour du chat_ask
    // pour valider formellement la réponse au CircuitBreaker.

    // Evaluate natif CDP (utilisé dans chat_ask pour get csrfToken)
    let js_mock_env = r#"
        window.WIZ_global_data = { SNlM0e: "fake", FdrFJe: "fake" };
    "#;
    let _ = tab.evaluate(js_mock_env).await;

    let operation = api.chat_ask("notebook_fake_id", "prompt_test");

    // Circuit breaker simulé du caller
    let start = std::time::Instant::now();
    let result = timeout(Duration::from_secs(3), operation).await;
    let elapsed = start.elapsed();

    // D. Assertions
    // Le test DOIT terminer en erreur de timeout, précisément comme conçu !
    assert!(
        result.is_err(),
        "La requête devait s'interrompre en erreur de timeout !"
    );
    assert!(
        elapsed.as_secs() >= 2,
        "Le timeout s'est déclenché trop vite !"
    );
    assert!(
        elapsed.as_secs() < 5,
        "Le timeout s'est déclenché trop tard, Starvation avérée !"
    );

    Ok(())
}
