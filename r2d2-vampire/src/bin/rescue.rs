use r2d2_browser::SovereignBrowser;
use r2d2_vampire::vampire_lord::notebook_api::NotebookApi;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();

    let browser = SovereignBrowser::connect("Chrome_GOOGLE").await?;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let tab =
        r2d2_browser::SovereignBrowser::get_or_new_tab(&browser, "notebooklm.google.com").await?;
    let api = NotebookApi::new(tab, None).await;
    api.tab.goto("https://notebooklm.google.com/").await?;
    api.tab.wait_for_navigation().await?;

    let notebooks = api.list_notebooks().await?;
    let rustarch_id = notebooks
        .iter()
        .find(|(_, title)| title.contains("[R2D2] RustArch"))
        .map(|(id, _)| id.clone());

    if let Some(id) = rustarch_id {
        let queries = [
            "State-of-the-art applied cryptography and Zero-Knowledge Proofs (ZKP) implemented in Rust. Hardware-accelerated cryptographic primitives, constant-time algorithms to prevent timing side-channels, elliptic curve cryptography (ECC), and secure multi-party computation architectures. Focus on formally verified and high-assurance cryptographic code strategies in the Rust ecosystem.",
            "Advanced tensor algebra, low-level BLAS (Basic Linear Algebra Subprograms), and CUDA/GPU hardware programming in Rust. Architectural patterns for memory-safe CustomOps, VRAM multiplexing, and mathematical models for 1.58-bit ternary weight network quantization. Academic focus on zero-cost hardware abstraction for LLM inference on edge devices.",
            "Mathematical formal verification of Rust programs using type theory and axiomatic logic. Deep analysis of Rust's memory models (Stacked Borrows, Tree Borrows), and invariant enforcement via the Newtype pattern and Type-State programming. Integration of formal proof tools like Kani, Prusti, or Creusot to mathematically guarantee the absence of memory violations in critical unsafe blocks.",
            "Information theory applied to deterministic state-machine design, Actor models, and Hexagonal (Ports and Adapters) architectures in Rust. Mathematical approaches to modeling concurrent lock-free systems, MIMO (Multiple-Input Multiple-Output) State Space Models (SSM), and epistemological data structures (Belief States) for autonomous agent communications."
        ];

        info!("🚀 Amorçage de l'Ogive (Pipeline Séquentiel SOTA) dans RustArch...");
        for q in queries.iter() {
            info!("🎯 Transmission cible RPC : {}", q);
            // Lancement de l'Agent Deep Research (Google)
            let _ = api.add_deep_search_source(&id, q).await;

            // Attente de quelques secondes pour l'enregistrement côté serveur Google
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            // Le Watcher prend le relai, observe, importe, et redonne la main dès que c'est fini.
            info!("👁️ Activation de l'intercepteur d'assimilation asynchrone...");
            if let Err(e) = api.auto_import_pending_searches(&id).await {
                tracing::warn!("Erreur lors de l'assimilation: {}", e);
            }
            info!("✅ Cycle Deep Search achevé. Préparation de l'ogive suivante si présente...");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }

    Ok(())
}
