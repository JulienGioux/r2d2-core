use r2d2_registry::{RawDataArtifact, StateContrastive, ValidatedSample};
use std::marker::PhantomData;
use std::sync::mpsc::{Receiver, SyncSender};
use tracing::{info, warn};

/// Moteur de transformation (ETL) qui convertit les RawDataArtifacts
/// en ValidatedSample robustement typés.
#[derive(Default)]
pub struct AlchimisteEngine {}

impl AlchimisteEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Boucle de forgerie.
    /// Reçoit des données via rx et émet la donnée validée via tx (Backpressure).
    pub fn digest_loop(
        &mut self,
        rx: Receiver<RawDataArtifact>,
        tx: SyncSender<ValidatedSample<StateContrastive>>,
    ) {
        info!("⚗️  [FORGE] Alchimiste opérationnel. En attente de minerai...");

        while let Ok(raw_data) = rx.recv() {
            info!(
                "⚗️  [FORGE] Réception d'un artefact (Hash: {}). Début du Split Contrastif...",
                raw_data.source_hash
            );

            // Simulation du parse / tokenization
            // Normalement, l'ETL utilise un Tokenizer BPE pour convertir le texte.
            // Vu qu'on n'a pas le tokenizer localement injecté ici, on simule l'extraction.

            // En ciblant directement les octets, on garantit un comportement
            // "Zéro-Crash" même si la découpe tombe au milieu d'un caractère UTF-8 Asiatique (3 bytes).
            // Le bitnet 256 de vocabulaire (Byte-Level) ingérera ça naturellement.
            let text = raw_data.payload.to_string();
            let bytes = text.as_bytes();
            let mid = bytes.len() / 2;
            let p_seq: Vec<u32> = bytes[..mid].iter().map(|&b| b as u32).collect();
            let j_seq: Vec<u32> = bytes[mid..].iter().map(|&b| b as u32).collect();

            let sample = ValidatedSample {
                _state: PhantomData,
                payload: (p_seq, j_seq),
            };

            if let Err(e) = tx.send(sample) {
                warn!("⚠️ [FORGE] Le canal vers le Cortex a expiré : {}", e);
                break;
            }

            info!("✅ [FORGE] Artefact décomposé en Paire Contrastive et transmis au Cortex !");
        }

        info!("⚗️ [FORGE] Plus aucun minerai reçu. Fermeture de la Forge.");
    }
}
