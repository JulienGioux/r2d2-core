use r2d2_cortex::ChimeraTrainer;
use r2d2_forge::AlchimisteEngine;
use r2d2_mcp::VampireWorker;
use r2d2_registry::{RawDataArtifact, StateContrastive, ValidatedSample};
use std::sync::mpsc;
use std::thread;
use tracing::info;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("============================================================");
    info!("🚀 [ORCHESTRATOR] Démarrage du Monolithe R2D2 Bare-Metal");
    info!("============================================================");

    // 1. TOPOLOGIE DES CANAUX : La Contre-Pression (Backpressure)
    // sync_channel bloque le réseau si la forge/le cortex est saturé.
    let (tx_raw, rx_raw) = mpsc::sync_channel::<RawDataArtifact>(1024);
    let (tx_validated, rx_validated) = mpsc::sync_channel::<ValidatedSample<StateContrastive>>(128);

    // 2. LE MOISSONNEUR (r2d2-mcp) : Thread Isolé
    let vampire_handle = thread::Builder::new()
        .name("r2d2-ingress-vampire".into())
        .spawn(move || {
            let mut vampire = VampireWorker::new("mcp://notebooklm.expert");
            vampire.harvest_loop(tx_raw);
        })
        .expect("Erreur fatale: Impossible d'allouer le thread Vampire");

    // 3. LA FORGE (r2d2-forge) : Thread Isolé
    let forge_handle = thread::Builder::new()
        .name("r2d2-core-forge".into())
        .spawn(move || {
            let mut forge = AlchimisteEngine::new();
            forge.digest_loop(rx_raw, tx_validated);
        })
        .expect("Erreur fatale: Impossible d'allouer le thread Forge");

    // 4. LE CORTEX (r2d2-cortex) : Thread Isolé
    let cortex_handle = thread::Builder::new()
        .name("r2d2-cuda-cortex".into())
        .spawn(move || {
            let mut trainer = ChimeraTrainer::new()
                .expect("Erreur fatale: Impossible d'initialiser le Cerveau CUDA");
            trainer.training_loop(rx_validated);
        })
        .expect("Erreur fatale: Impossible d'allouer le thread Cortex");

    // 5. ATTENTE DE FIN DE VIE
    vampire_handle.join().unwrap();
    info!("🛑 [ORCHESTRATOR] Thread Vampire arrêté avec succès.");

    forge_handle.join().unwrap();
    info!("🛑 [ORCHESTRATOR] Thread Forge arrêté avec succès.");

    cortex_handle.join().unwrap();
    info!("🛑 [ORCHESTRATOR] Thread Cortex arrêté avec succès.");

    info!("============================================================");
    info!("🏁 [ORCHESTRATOR] Pipeline Bare-Metal validé et terminé.");
    info!("============================================================");
}
