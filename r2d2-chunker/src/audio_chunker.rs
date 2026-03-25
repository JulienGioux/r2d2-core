use crate::strategy::MediaChunker;
use anyhow::{anyhow, Result};
use r2d2_sensory::stimulus::{Stimulus, StimulusType};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tracing::{info, instrument};

/// Agent spécialisé dans la découpe des flux sonores pour Whisper.
pub struct AudioChunker {
    /// Fenêtre de découpe exacte pour le Transformer (16kHz * 30s = 480_000)
    samples_per_chunk: usize,
}

impl AudioChunker {
    pub fn new() -> Self {
        Self {
            // Whisper demande strictement 30s de frames à 16kHz (soit 480_000 floats).
            samples_per_chunk: 480_000,
        }
    }

    /// Fonction native (FFmpeg via Command) pour garantir une extraction PCM parfaite (16kHz, Mono, Float32).
    /// Remplace le décodeur Symphonia interne dont le downsampling linéaire créait de l'aliasing (bouillie).
    fn decode_audio_to_pcm(&self, path: &str) -> std::result::Result<Vec<f32>, anyhow::Error> {
        info!("📊 [DIAGNOSTIC] Extraction FFmpeg Pure: {}", path);
        info!("   -> Format ciblé: PCM s16le | 16000 Hz | Mono");

        let mut child = std::process::Command::new("ffmpeg")
            .args(&[
                "-i",
                path,
                "-f",
                "s16le",
                "-ac",
                "1", // Downmix forcé mono
                "-ar",
                "16000", // Downsampling qualitatif
                "-loglevel",
                "quiet",
                "-",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("Erreur lancement FFmpeg: {}", e))?;

        let mut raw_bytes = Vec::new();
        if let Some(mut stdout) = child.stdout.take() {
            use std::io::Read;
            stdout.read_to_end(&mut raw_bytes)?;
        }

        let status = child.wait()?;
        if !status.success() {
            let mut stderr_bytes = Vec::new();
            if let Some(mut err) = child.stderr.take() {
                use std::io::Read;
                let _ = err.read_to_end(&mut stderr_bytes);
            }
            return Err(anyhow!(
                "FFmpeg a échoué: {}",
                String::from_utf8_lossy(&stderr_bytes)
            ));
        }

        // Cast sécurisé des bytes FFmpeg s16le en vec de flottants f32 [-1.0, 1.0]
        let mut original_pcm = vec![0f32; raw_bytes.len() / 2];
        for (i, chunk) in raw_bytes.chunks_exact(2).enumerate() {
            let sample16 = i16::from_le_bytes(chunk.try_into().unwrap());
            original_pcm[i] = (sample16 as f32) / 32768.0;
        }

        Ok(original_pcm)
    }
}

impl Default for AudioChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaChunker for AudioChunker {
    fn chunk_strategy_definition(&self) -> &str {
        "30_sec_window_whisper"
    }

    #[instrument(skip(self, parent_stimulus))]
    fn chunk(&self, parent_stimulus: &Stimulus) -> Result<Vec<Stimulus>> {
        info!(
            "AudioChunker : Extraction intégrale de {}",
            parent_stimulus.payload_path.display()
        );

        let path_str = parent_stimulus.payload_path.to_string_lossy().to_string();
        let full_pcm = self.decode_audio_to_pcm(&path_str)?;
        let total_samples = full_pcm.len();

        info!("-> Flux global: {} samples natifs.", total_samples);

        let mut stimuli = Vec::new();
        let mut chunk_index = 0;

        // Découpage implacable en paquets de 30 secondes (480_000 samples max)
        // Whisper s'adapte dynamiquement (*Dynamic Positional Embeddings*) aux séquences de <30s sans broncher.
        // On NE PAD PLUS avec du vide pur (0.0), pour éradiquer 100% le terreau des Hallucinations !
        for chunk in full_pcm.chunks(self.samples_per_chunk) {
            let chunk_vec = chunk.to_vec();

            // Génération d'un fichier .bin brut pour transmission ultra-légère entre briques.
            let chunk_filename = format!(
                "chunk_audio_{}_part_{}.bin",
                parent_stimulus.id, chunk_index
            );
            let mut out_path = std::env::temp_dir();
            out_path.push(&chunk_filename);

            let mut f = File::create(&out_path)?;
            // Cast brutal des f32 en bytes
            let bytes: &[u8] = bytemuck::cast_slice(&chunk_vec);
            f.write_all(bytes)?;

            info!(
                "   [CHUNK {}] Fichier RAW 30s sauvé vers {:?}",
                chunk_index, out_path
            );

            let mut sub_stimulus = Stimulus::new(
                format!("{}-part-{}", parent_stimulus.id, chunk_index),
                StimulusType::Audio,
                out_path,
            );

            // Flag explicite pour Whisper : Inutile de lancer Symphonia.
            sub_stimulus.metadata = serde_json::json!({
                "format": "raw_pcm_f32le_normalized",
                "sample_rate": 16000,
                "strategy": self.chunk_strategy_definition()
            });

            stimuli.push(sub_stimulus);
            chunk_index += 1;
        }

        info!(
            "-> AudioChunking achevé : {} fragments générés.",
            stimuli.len()
        );
        Ok(stimuli)
    }
}
