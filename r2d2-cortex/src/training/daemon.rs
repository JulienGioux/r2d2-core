use r2d2_bitnet::training::batch::{TrainingBatch, TrainingState};
use tokio::sync::{mpsc, watch};
use tracing::{error, info, instrument};

/// 🧠 Orchestrateur Asynchrone d'Apprentissage (Actor Pattern)
///
/// Gère l'isolation stricte entre :
/// - Les I/O Asynchrones (Dataloader qui envoie via MPSC)
/// - Le Calcul Intensif CPU/VRAM (Boucle Tensorielle isolée par spawn_blocking)
pub struct TrainingDaemon;

impl TrainingDaemon {
    /// Lance l'Actor d'entraînement Mathématique en arrière-plan.
    /// Retourne le canal MPSC pour que le Dataloader y injecte les batchs,
    /// le canal Watch pour que le Tableau de Bord (UI) écoute l'état,
    /// et le canal de Recyclage MPSC pour récupérer les structures mémoire vides (Object Pool).
    #[instrument(name = "spawn_training_daemon", skip_all)]
    pub fn spawn() -> (
        mpsc::Sender<TrainingBatch>,
        watch::Receiver<TrainingState>,
        mpsc::Receiver<TrainingBatch>,
    ) {
        // 1. Canal MPSC hautement restreint (Backpressure)
        let (batch_tx, mut batch_rx) = mpsc::channel::<TrainingBatch>(4);

        // Canal de recyclage (MPSC Inversé) pour éviter le Thrashing RAM
        let (recycle_tx, recycle_rx) = mpsc::channel::<TrainingBatch>(4);

        // 2. Canal Watch pour le monitoring sans lock (Lock-Free)
        let (state_tx, state_rx) = watch::channel(TrainingState::Active {
            loss: 0.0,
            epoch: 0,
        });

        // 3. Boucle Mathématique Isolée (CPU-Bound)
        tokio::task::spawn_blocking(move || {
            use candle_core::{Device, Tensor};
            let device = Device::new_cuda(0).unwrap_or(Device::Cpu);
            info!(
                "🚀 [Training Daemon] Moteur Mathématique (BitNet 1.58b) sur: {:?}",
                device
            );

            while let Some(mut batch) = batch_rx.blocking_recv() {
                // 1. Instanciation des Tensors : Appel FFI CUDA
                let _input_tensor = Tensor::new(batch.input_tokens.as_slice(), &device)
                    .unwrap()
                    .unsqueeze(0)
                    .unwrap();
                let _target_tensor = Tensor::new(batch.target_tokens.as_slice(), &device)
                    .unwrap()
                    .unsqueeze(0)
                    .unwrap();
                let _mask_tensor = Tensor::new(batch.loss_mask.as_slice(), &device)
                    .unwrap()
                    .unsqueeze(0)
                    .unwrap();

                // TODO: Intégrer l'appel au modèle r2d2-bitnet
                let simulated_loss: f32 = 2.45; // Placeholder

                // --- CIRCUIT BREAKER ---
                if simulated_loss.is_nan() || simulated_loss.is_infinite() {
                    let err_msg = format!(
                        "Divergence Mathématique (Loss = NaN) - Batch {}",
                        batch.batch_idx
                    );
                    error!("⚡ [CircuitBreaker] Coupe-circuit déclenché : {}", err_msg);
                    let _ = state_tx.send(TrainingState::CircuitOpen { reason: err_msg });
                    return;
                }

                // Monitoring Live
                let _ = state_tx.send(TrainingState::Active {
                    loss: simulated_loss,
                    epoch: batch.batch_idx,
                });

                // 3. Object Pool (Recyclage)
                batch.clear(); // O(1) Zeroize et conserve la capacité mémoire
                let _ = recycle_tx.blocking_send(batch); // On l'ignore s'il est plein ou fermé
            }

            info!("🏁 [Training Daemon] Apprentissage Terminé (Plus de données).");
            let _ = state_tx.send(TrainingState::Finished { final_loss: 0.0 });
        });

        (batch_tx, state_rx, recycle_rx)
    }
}
