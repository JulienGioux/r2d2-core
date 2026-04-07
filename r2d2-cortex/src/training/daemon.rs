use candle_core::{Device, Tensor};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use r2d2_bitnet::model::{BitNetConfig, BitNetModel};
use r2d2_bitnet::training::batch::{TrainingBatch, TrainingState};
use r2d2_bitnet::training::optimizer::TwoStageAdamW;
use r2d2_bitnet::weights::TrainingWeights;
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
            let device = Device::new_cuda(0).unwrap_or(Device::Cpu);
            info!(
                "🚀 [Training Daemon] Moteur Mathématique (BitNet 1.58b) sur: {:?}",
                device
            );

            // --- INSTANCIATION CANDLE / BITNET ---
            let varmap = VarMap::new();
            let vb = VarBuilder::from_varmap(&varmap, candle_core::DType::F32, &device);
            let config = BitNetConfig::default();

            let model = match BitNetModel::<TrainingWeights>::load_train(vb, &config) {
                Ok(m) => m,
                Err(e) => {
                    error!("Impossible d'instancier le modèle dynamique: {}", e);
                    let _ = state_tx.send(TrainingState::CircuitOpen {
                        reason: "Erreur Modèle".to_string(),
                    });
                    return;
                }
            };

            let adamw_params = ParamsAdamW {
                lr: 1e-4,
                weight_decay: 0.01,
                ..Default::default()
            };

            let adamw = match AdamW::new(varmap.all_vars(), adamw_params) {
                Ok(opt) => opt,
                Err(e) => {
                    error!("Impossible d'initialiser AdamW: {}", e);
                    let _ = state_tx.send(TrainingState::CircuitOpen {
                        reason: "Erreur Optimizer".to_string(),
                    });
                    return;
                }
            };

            // Total steps pour le TwoStage: supposons 1000 pour la PoC.
            let mut optimizer = TwoStageAdamW::new(adamw, 1000, 0.8);

            while let Some(mut batch) = batch_rx.blocking_recv() {
                // 1. Instanciation des Tensors : Appel FFI
                let input_tensor = Tensor::new(batch.input_tokens.as_slice(), &device)
                    .unwrap()
                    .unsqueeze(0)
                    .unwrap();
                let target_tensor = Tensor::new(batch.target_tokens.as_slice(), &device)
                    .unwrap()
                    .unsqueeze(0)
                    .unwrap();

                // 2. Forward pass : Traque le graphe de calcul
                let logits = match model.forward(&input_tensor) {
                    Ok(l) => l,
                    Err(e) => {
                        error!("Erreur Forward Pass: {}", e);
                        continue;
                    }
                };

                // Redimensionnement pour correspondre à (N_tokens, Vocab_Size) et cibles 1D
                let seq_len = batch.input_tokens.len();
                let vocab_size = config.vocab_size;
                let logits_2d = match logits.reshape((seq_len, vocab_size)) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let target_flat = match target_tensor.flatten_all() {
                    Ok(t) => t,
                    Err(_) => continue,
                };

                let loss = match candle_nn::loss::cross_entropy(&logits_2d, &target_flat) {
                    Ok(l) => l,
                    Err(e) => {
                        error!("Erreur Entropie Croisée: {}", e);
                        continue;
                    }
                };

                // 3. Backward Pass (Straight-Through Estimator interne BitLinear)
                if let Err(e) = optimizer.backward_step(&loss, batch.batch_idx) {
                    error!("Erreur lors de la descente de gradient: {}", e);
                }

                let actual_loss = loss.to_scalar::<f32>().unwrap_or(f32::NAN);

                // --- CIRCUIT BREAKER ---
                if actual_loss.is_nan() || actual_loss.is_infinite() {
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
                    loss: actual_loss,
                    epoch: batch.batch_idx,
                });

                // 4. Object Pool (Recyclage)
                batch.clear(); // O(1) Zeroize
                let _ = recycle_tx.blocking_send(batch);
            }

            info!("🏁 [Training Daemon] Apprentissage Terminé (Plus de données).");
            let _ = state_tx.send(TrainingState::Finished { final_loss: 0.0 });
        });

        (batch_tx, state_rx, recycle_rx)
    }
}
