use r2d2_bitnet::training::batch::TrainingBatch;
use std::path::Path;
use tokenizers::Tokenizer;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::sync::mpsc;
use tracing::{error, info, instrument};

/// Chargeur de corpus JSONAI optimisé Asynchrone (I/O Bound).
pub struct JsonAiDataloader {
    tokenizer: Tokenizer,
    batch_tx: mpsc::Sender<TrainingBatch>,
    recycle_rx: mpsc::Receiver<TrainingBatch>,
}

impl JsonAiDataloader {
    /// Initialise le Dataloader avec le canal MPSC connecté au TrainingDaemon.
    pub fn new(
        tokenizer_path: &Path,
        batch_tx: mpsc::Sender<TrainingBatch>,
        recycle_rx: mpsc::Receiver<TrainingBatch>,
    ) -> anyhow::Result<Self> {
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Erreur Tokenizer: {}", e))?;

        Ok(Self {
            tokenizer,
            batch_tx,
            recycle_rx,
        })
    }

    /// Démarre le streaming asynchrone depuis le disque.
    /// Lit ligne par ligne, convertit en Tensors et pousse dans la file d'attente MPSC.
    /// L'appel suspension `send().await` garantit la pureté de la **Backpressure**.
    #[instrument(skip(self), name = "stream_dataset")]
    pub async fn stream_dataset(&mut self, dataset_path: &Path) -> anyhow::Result<()> {
        info!("📖 Ouverture du flux de données : {:?}", dataset_path);

        let file = tokio::fs::File::open(dataset_path).await?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        let mut batch_idx = 0;

        loop {
            line.clear();

            // Faille 1 : Hardening OOM. Restriction à 2MB par ligne lue en RAM.
            // On limite le nombre d'octets que .read_line() peut ingérer.
            let mut taker = (&mut reader).take(2 * 1024 * 1024);
            let n_bytes = taker.read_line(&mut line).await?;

            if n_bytes == 0 {
                break; // EOF
            }

            if n_bytes == 2 * 1024 * 1024 && !line.ends_with('\n') {
                tracing::warn!(
                    "Ligne malformée dépassant 2MB (Risque OOM). Ligne ignorée silencieusement."
                );
                // Purge du reste de la ligne pour ne pas polluer l'itération suivante
                let mut trash = Vec::new();
                let _ =
                    tokio::io::AsyncBufReadExt::read_until(&mut reader, b'\n', &mut trash).await?;
                continue;
            }

            // Nettoyage rapide de la ligne JSON
            let payload = line.trim();
            if payload.is_empty() {
                continue;
            }

            // --- 1. Tokenisation ---
            let encoding = self
                .tokenizer
                .encode(payload, true)
                .map_err(|e| anyhow::anyhow!("Tokenization error : {}", e))?;

            let tokens = encoding.get_ids().to_vec();

            // --- 2. Masking Sémantique Sécure (JSONAI v5.1 Masked Loss) ---
            let offsets = encoding.get_offsets();
            let mut mask_vec = Vec::with_capacity(tokens.len());

            for (start, end) in offsets {
                // Faille 3 : Hardening Panique UTF-8 (slice sécurisé)
                if payload.is_char_boundary(*start) && payload.is_char_boundary(*end) {
                    let slice = &payload[*start..*end];
                    if slice.contains('{')
                        || slice.contains('}')
                        || slice.contains('[')
                        || slice.contains(']')
                        || slice.contains("\"provenances\"")
                        || slice.contains("\"is_fact\"")
                    {
                        mask_vec.push(0.0f32);
                    } else {
                        mask_vec.push(1.0f32);
                    }
                } else {
                    // Si l'offset chevauche une frontière UTF-8, le jeton est "sale".
                    // Poids 0.0 pour forcer le modèle à ignorer cette erreur Tokenizer.
                    mask_vec.push(0.0f32);
                }
            }

            // --- 3. Décalage Causal (Next Token Prediction) en O(1) Slicing Sécure ---
            let seq_len = tokens.len();
            if seq_len < 2 {
                continue;
            }

            // Faille 2 : Object Pool Recycling (MPSC Thrashing Fix) & Slicing O(1)
            let mut batch = if let Ok(mut b) = self.recycle_rx.try_recv() {
                b.batch_idx = batch_idx;
                b
            } else {
                TrainingBatch::new_with_capacity(seq_len)
            };

            batch.input_tokens.extend_from_slice(&tokens[..seq_len - 1]);
            batch.target_tokens.extend_from_slice(&tokens[1..]);
            batch.loss_mask.extend_from_slice(&mask_vec[1..]);

            // --- 4. Envoi (Backpressure Automatique) ---
            if let Err(e) = self.batch_tx.send(batch).await {
                error!(
                    "⚡ Le canal d'entrainement est coupé. Moteur mort ou Circuit Ouvert : {}",
                    e
                );
                break; // Le RAII coupe spontanément l'injection.
            }

            batch_idx += 1;
        }

        info!("✅ Streaming de {} batchs terminé avec succès.", batch_idx);
        Ok(())
    }
}
