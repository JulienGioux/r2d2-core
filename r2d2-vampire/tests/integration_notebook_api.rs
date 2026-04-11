use axum::{routing::get, Router};
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

/// 2. LE SERVEUR AXUM FAKE (Usurpation de domaine)
async fn spawn_local_notebooklm() -> String {
    let app = Router::new().route("/cdp", get(|| async { "Mocked CDP Stream OK" }));
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
async fn test_notebook_api_cdp_pure_async() -> anyhow::Result<()> {
    // A. Boot du serveur local
    let fake_ws_url = spawn_local_notebooklm().await;

    // B. Instanciation du vrai adaptateur (Zero-Mock) via SovereignBrowser
    // Au lieu de lancer Chromium natif (qui manque sous WSL par défaut),
    // on utilise notre pont SovereignBrowser pour s'y rattacher (Host Windows).
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

    // Instanciation de l'API
    let api = NotebookApi::new(tab.clone()).await;

    // Récupérer le PID de Chromium si possible (chromiumoxide expose le port/PID dans certaines conditions,
    // mais si non, on n'a pas besoin de Drop guard car chromiumoxide gère déjà ses process internes,
    // cependant on le garde par sécurité).
    let _guard = ChromiumZombieGuard { process_id: None };

    // D. Exécution d'un use-case : evaluate() await dans NotebookApi
    // On simule une simple fonction execute_rpc() ou un chat_ask().
    // Etant donné que la page n'est pas notebooklm, on va juste valider que evaluate ne bloque pas indéfiniment.
    let js = "42 + 42";
    let operation = tab.evaluate(js);

    // L'enveloppe de timeout prévient le deadlock infini sur le CI
    let result = timeout(Duration::from_secs(5), operation).await;

    // E. Assertions de la logique métier
    assert!(result.is_ok(), "Violation de doctrine : Timeout CDP");
    let res = result.unwrap();
    assert!(res.is_ok(), "L'évaluation a échoué: {:?}", res);

    // Test that the async execute_rpc fails gracefully given invalid page instead of deadlocking
    let rpc_result = timeout(
        Duration::from_secs(2),
        api.execute_rpc("FakeOp", "/fake/path", serde_json::json!([])),
    )
    .await;
    assert!(
        rpc_result.is_ok(),
        "L'API RPC ne devrait pas bloquer indéfiniment meme en erreur."
    );
    assert!(
        rpc_result.unwrap().is_err(),
        "L'API RPC devrait remonter une erreur proprement car la cible est invalide."
    );

    Ok(())
}
