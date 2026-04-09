use candle_core::{DType, Device, Tensor};
use candle_nn::{loss, AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use r2d2_bitnet::chimera::{ChimeraConfig, ChimeraModel};
use r2d2_registry::{StateContrastive, ValidatedSample};
use std::sync::mpsc::Receiver;
use tracing::{error, info, warn};

pub struct ChimeraTrainer {
    model: ChimeraModel,
    opt: AdamW,
    device: Device,
    epochs_run: usize,
    _varmap: VarMap,
}

impl ChimeraTrainer {
    pub fn new() -> anyhow::Result<Self> {
        info!("🧠 [CORTEX] Initialisation matérielle du Moteur Tensoriel...");

        let device = if candle_core::utils::cuda_is_available() {
            info!("⚡ [HARDWARE] CUDA Cœur Graphique détecté.");
            Device::new_cuda(0)?
        } else {
            warn!("⚠️ [HARDWARE] CUDA introuvable. Repli CPU activé.");
            Device::Cpu
        };

        let mut config = ChimeraConfig::reduced();
        config.vocab_size = 256; // Format Byte-Level

        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let model = ChimeraModel::new_qat(&config, vb)?;

        let params = ParamsAdamW {
            lr: 0.005,
            weight_decay: 0.01,
            ..Default::default()
        };
        let opt = AdamW::new(varmap.all_vars(), params)?;

        Ok(Self {
            model,
            opt,
            device,
            epochs_run: 0,
            _varmap: varmap,
        })
    }

    /// Boucle de réception Bare-Metal (Backpressure consummation)
    /// Le compilateur garantit l'ingestion stricte de StateContrastive
    pub fn training_loop(&mut self, rx: Receiver<ValidatedSample<StateContrastive>>) {
        info!("🧠 [CORTEX] Cerveau tensoriel chargé. Attente du minerais Contrastif.");
        const ACCUMULATION_STEPS: usize = 16;
        let mut step_count = 0;
        let mut losses = Vec::with_capacity(ACCUMULATION_STEPS);

        // Tant que la boucle reçoit des éléments, le thread vit
        // Si rx.recv() bloque, l'OS désactive ce thread (consommation CPU 0%)
        while let Ok(pair) = rx.recv() {
            let (ref p_seq, ref j_seq) = pair.payload;

            // Sécurité spatiale : on ignore les séquences vides
            if p_seq.is_empty() || j_seq.is_empty() {
                continue;
            }

            // --- 1. FORWARD PASS (Batch=1) ---
            match self.compute_contrastive_loss(p_seq, j_seq) {
                Ok(loss) => {
                    losses.push(loss);
                    step_count += 1;
                    info!(
                        "⏳ [CORTEX] Ingestion RAG calculée ({}/{})",
                        step_count % ACCUMULATION_STEPS,
                        ACCUMULATION_STEPS
                    );

                    // --- 2. ACCUMULATION ET BACKWARD (Tous les 16 échantillons) ---
                    if step_count % ACCUMULATION_STEPS == 0 {
                        info!("🧠 [CORTEX] Jauge Accumulation pleine. Rétropropagation globale !");

                        // Puisque le Modèle est compact (130M), on peut accumuler les Graphes purs sans OOM.
                        if let Ok(stacked) = Tensor::stack(&losses, 0) {
                            if let Ok(total_loss) = stacked.mean(0) {
                                if let Err(e) = self.opt.backward_step(&total_loss) {
                                    error!("❌ [CORTEX] Échec du Step de l'Optimiseur : {}", e);
                                } else {
                                    self.epochs_run += 1;
                                    info!("✅ [CONTRASTIVE] Apprentissage par accumulation terminé avec succès.");
                                }
                            }
                        } else {
                            error!("❌ [CORTEX] Impossible de stacker les tenseurs de perte.");
                        }

                        // Libération manuelle des références VRAM pour l'accumulation suivante
                        losses.clear();
                    }
                }
                Err(e) => error!(
                    "❌ [CORTEX] Erreur fatale du pipeline tensoriel (Forward): {}",
                    e
                ),
            }
        }

        // --- FLUSH RÉSIDUEL (ZÉRO-AMNÉSIE TENSORIELLE) ---
        // Sécurise les derniers échantillons non-multiples de 16 avant extinction.
        if !losses.is_empty() {
            info!(
                "🧠 [CORTEX] Canal tari. Lancement du Flush du lot résiduel (Taille: {})",
                losses.len()
            );
            if let Ok(stacked) = Tensor::stack(&losses, 0) {
                if let Ok(total_loss) = stacked.mean(0) {
                    if let Err(e) = self.opt.backward_step(&total_loss) {
                        error!("❌ [CORTEX] Échec du Step Résiduel : {}", e);
                    } else {
                        self.epochs_run += 1;
                        info!("✅ [CONTRASTIVE] Flush d'Apprentissage terminé avec succès. 100% de rétention.");
                    }
                }
            }
            losses.clear();
        }

        info!("🧠 [CORTEX] Canal rompu. Mise en quarantaine VRAM.");
    }

    /// Extrait la logique mathématique pure de calcul d'erreur (MSE Loss + L2 Norm)
    #[inline(always)]
    fn compute_contrastive_loss(&mut self, p_seq: &[u32], j_seq: &[u32]) -> anyhow::Result<Tensor> {
        // Host vers Device
        let p_data = Tensor::new(p_seq, &self.device)?;
        let j_data = Tensor::new(j_seq, &self.device)?;

        // Forward Pass (Passage par le modèle BitMamba)
        let (p_hidden, _) = self.model.forward_hidden(&p_data, None)?;
        let (j_hidden, _) = self.model.forward_hidden(&j_data, None)?;

        // Mean Pooling (L'Embedding est la moyenne de tous les jetons)
        let p_emb = p_hidden.mean(0)?;
        let j_emb = j_hidden.mean(0)?;

        // Normalisation L2 (P_Norm)
        let p_norm = p_emb.sqr()?.sum_keepdim(candle_core::D::Minus1)?.sqrt()?;
        let p_norm = p_norm.broadcast_add(&Tensor::new(&[1e-6_f32], &self.device)?)?;
        let p_normed = p_emb.broadcast_div(&p_norm)?;

        // Normalisation L2 (J_Norm)
        let j_norm = j_emb.sqr()?.sum_keepdim(candle_core::D::Minus1)?.sqrt()?;
        let j_norm = j_norm.broadcast_add(&Tensor::new(&[1e-6_f32], &self.device)?)?;
        let j_normed = j_emb.broadcast_div(&j_norm)?;

        // MSE Loss Mathématique
        let loss = loss::mse(&p_normed, &j_normed)?;
        Ok(loss)
    }
}
