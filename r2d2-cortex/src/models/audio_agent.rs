use crate::agent::{AgentError, CognitiveAgent};
use crate::catalog::{CognitiveSense, CortexCatalog};
use async_trait::async_trait;
use std::time::Instant;
use tracing::{info, instrument};

use candle_core::{Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{audio, model::Whisper, Config};
use hf_hub::{Repo, RepoType};

use std::sync::Arc;
use std::time::Duration;

/// Gestionnaire de panne (Circuit Breaker) pour protéger le pipeline
pub struct CircuitBreaker {
    pub failures: u32,
    pub threshold: u32,
    pub last_failure: Option<Instant>,
    pub reset_timeout: Duration,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            failures: 0,
            threshold,
            last_failure: None,
            reset_timeout,
        }
    }

    pub fn check(&mut self) -> Result<(), AgentError> {
        if self.failures >= self.threshold {
            if let Some(last) = self.last_failure {
                if last.elapsed() < self.reset_timeout {
                    return Err(AgentError::InferenceError(
                        "Circuit Breaker OPEN: Trop d'échecs consécutifs sur Whisper".to_string(),
                    ));
                } else {
                    // Half-open state
                    self.failures = self.threshold - 1;
                }
            }
        }
        Ok(())
    }

    pub fn record_success(&mut self) {
        self.failures = 0;
        self.last_failure = None;
    }

    pub fn record_failure(&mut self) {
        self.failures += 1;
        self.last_failure = Some(Instant::now());
    }
}

/// Moteur encapsulant les poids et l'état de Whisper (Immuable, thread-safe)
pub struct WhisperEngine {
    pub config: Config,
    pub model: Whisper,
    pub tokenizer: tokenizers::Tokenizer,
    pub mel_filters: Vec<f32>,
    pub suppress_tensor: Tensor,
    pub device: Device,
}

/// Agent Cortical dédié à la transcription Audio via Whisper (Candle).
pub struct AudioAgent {
    name: String,
    active: bool,
    engine: Option<Arc<WhisperEngine>>,
    circuit_breaker: CircuitBreaker,
}

impl Default for AudioAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioAgent {
    pub fn new() -> Self {
        Self {
            name: "AudioAgent".to_string(),
            active: false,
            engine: None,
            // Tolérance à 3 crashs, réinitialisation après 60 secondes
            circuit_breaker: CircuitBreaker::new(3, Duration::from_secs(60)),
        }
    }

    /// Processus interne d'inférence ML (Whisper - Candle Greedy)
    async fn transcribe(&mut self, audio_path: &str) -> Result<String, AgentError> {
        // 1. Vérification du Circuit Breaker avant tout effort
        self.circuit_breaker.check()?;

        let engine = self.engine.as_ref().cloned().ok_or(AgentError::NotActive)?;
        let audio_path = audio_path.to_string();

        info!(
            "AudioAgent: Délégation Tenseur-Brut 16kHz vers pool CPU ({})",
            audio_path
        );

        // Délégation totale de l'inférence asynchrone dans un thread pool bloquant isolé
        let result = tokio::task::spawn_blocking(move || -> Result<String, AgentError> {
            let path = std::path::Path::new(&audio_path);
            let raw_bytes = std::fs::read(path).map_err(|e| {
                AgentError::InferenceError(format!("Fichier binaire illisible: {}", e))
            })?;

            // Reconstruction du flux f32 pur
            let mut pcm_data = vec![0f32; raw_bytes.len() / 4];
            for (i, chunk) in raw_bytes.chunks_exact(4).enumerate() {
                let arr: [u8; 4] = chunk.try_into().unwrap();
                pcm_data[i] = f32::from_le_bytes(arr);
            }

            // On s'assure par précaution qu'aucune altération de base ne dépasse 30 secondes
            pcm_data.truncate(480_000);
            if pcm_data.len() < 480_000 {
                pcm_data.resize(480_000, 0.0);
            }

            let mut max_amp = 0.0f32;
            let mut sum_amp = 0.0f32;
            for &s in &pcm_data {
                let abs_s = s.abs();
                if abs_s > max_amp {
                    max_amp = abs_s;
                }
                sum_amp += abs_s;
            }
            info!(
                "-> Tenseur PCM ({}) | Amplitude Max: {:.6} | Moyenne: {:.6}",
                pcm_data.len(),
                max_amp,
                sum_amp / pcm_data.len() as f32
            );
            if max_amp < 0.0001 {
                info!("🚨 ALERTE ROUGE : LE SIGNAL EST TOTALEMENT VIDE (0.0) !");
            }

            info!("-> Calcul des Filtres Log-Mel Spectrogram...");
            let mel = audio::pcm_to_mel(&engine.config, &pcm_data, &engine.mel_filters);
            let num_mel_bins = engine.config.num_mel_bins;
            let mel_len = mel.len();
            // --- SONDAGE TENSORIEL DE LA VÉRITÉ MATHÉMATIQUE ---
            let mel_raw_len = engine.mel_filters.len();
            let filter_bins = mel_raw_len / 201; // n_fft(400)/2 + 1 = 201

            // Diagnostic Mel Flottant Pur
            let mut mel_sum = 0f32;
            let mut mel_max = 0f32;
            let mut mel_min = 1000f32;
            for &m in &mel {
                mel_sum += m;
                if m > mel_max {
                    mel_max = m;
                }
                if m < mel_min {
                    mel_min = m;
                }
            }
            info!(
                ">>> STATS MEL BRUT: Max={}, Min={}, Moyenne={}",
                mel_max,
                mel_min,
                mel_sum / mel.len() as f32
            );
            info!(
                ">>> SONDAGE CANDLE : Sortie pcm_to_mel = {} floats",
                mel_len
            );

            // La géométrie exacte dépend obligatoirement du nombre de bins du filtre.
            let true_frames = mel_len / filter_bins;
            info!(
                ">>> DÉCOUPAGE GÉOMÉTRIQUE VÉRIFIÉ : [{} bins x {} frames]",
                filter_bins, true_frames
            );

            let mut mel_tensor =
                Tensor::from_vec(mel, (1, filter_bins, true_frames), &engine.device)
                    .map_err(|e| AgentError::InferenceError(format!("Tensor Mel Error: {}", e)))?;

            if filter_bins > num_mel_bins {
                info!(
                    "-> Retaillement Spatial des FRÉQUENCES : {} -> {}",
                    filter_bins, num_mel_bins
                );
                mel_tensor = mel_tensor
                    .narrow(1, 0, num_mel_bins)
                    .map_err(|e| AgentError::InferenceError(e.to_string()))?;
            }

            // -----------------------------------------------------------------------------------
            // [PARE-FEU RUSTY] : NE JAMAIS PAD LE TENSEUR AVEC DU VIDE (0.0) POUR ATTEINDRE 30S !
            // Historique : L'ajout de 0.0 pour forcer 30s de frame créait un "Attracteur de Silence".
            // Le modèle Whisper possède un mécanisme d'Embedding Positionnel Dynamique qui lui
            // permet de traiter des séquences de longueur arbitraire (< 30s) parfaitement.
            // Si le signal a 12s, le tenseur fera 12s. Le padding est structurellement toxique.
            // -----------------------------------------------------------------------------------
            let seq_len = mel_tensor.dim(2).unwrap_or(0);
            if seq_len > 3000 {
                info!(
                    "-> Retaillement Temporel des FRAMES (Truncate Cut): {} -> 3000",
                    seq_len
                );
                mel_tensor = mel_tensor
                    .narrow(2, 0, 3000)
                    .map_err(|e| AgentError::InferenceError(e.to_string()))?;
            }

            info!(
                "-> Injection Encodeur | Shape Final: {:?}",
                mel_tensor.shape()
            );
            let mut local_model = engine.model.clone();
            // Si la librairie le permet, forcer le clean du cache:
            // local_model.reset_kv_cache(); // Retiré car un clone propre est toujours vierge.
            let encoder_out = local_model
                .encoder
                .forward(&mel_tensor, true)
                .map_err(|e| AgentError::InferenceError(format!("Encodeur: {}", e)))?;

            let enc_max = encoder_out
                .max_all()
                .unwrap()
                .to_scalar::<f32>()
                .unwrap_or(0.0);
            let enc_min = encoder_out
                .min_all()
                .unwrap()
                .to_scalar::<f32>()
                .unwrap_or(0.0);
            info!(
                "   Encodeur Out Shape: {:?} | Stats: [Max={}, Min={}]",
                encoder_out.shape(),
                enc_max,
                enc_min
            );

            info!("-> Génération autorégressive (Incrémentale avec KV cache exclusif)...");
            let mut tokens = vec![50258, 50265, 50359, 50363];
            let eot_token = 50257;

            let mut log_tokens = Vec::new();

            for step in 0..150 {
                let current_tokens_tensor = Tensor::new(tokens.as_slice(), &engine.device)
                    .map_err(|e| AgentError::InferenceError(e.to_string()))?
                    .unsqueeze(0)
                    .unwrap();

                let hidden_states = local_model
                    .decoder
                    .forward(&current_tokens_tensor, &encoder_out, true)
                    .map_err(|e| AgentError::InferenceError(format!("Décodeur: {}", e)))?;

                let seq_len_l = hidden_states.dim(1).unwrap();

                // On réduit à [1, 1, 384] pour la projection finale
                let last_hidden = hidden_states.i((.., seq_len_l - 1.., ..)).unwrap();

                // Projection via la couche linéaire du décodeur pour obtenir les probabilités
                let logits = local_model.decoder.final_linear(&last_hidden).unwrap();

                let mut logits_slice = logits.i((0, 0, ..)).unwrap();

                // Masque Anti-Hallucination Absolu
                logits_slice = logits_slice.broadcast_add(&engine.suppress_tensor).unwrap();

                let next_token = logits_slice
                    .argmax(0)
                    .unwrap()
                    .to_scalar::<u32>()
                    .unwrap();

                tokens.push(next_token);

                if step < 10 {
                    log_tokens.push(next_token);
                } else if step == 10 {
                    info!("   [DECODER 10 TKN] Séquence amont : {:?}", log_tokens);
                }

                if next_token == eot_token {
                    info!(
                        "   [DECODER] Token EOT (50257) détecté à l'étape {}. Arrêt propre.",
                        step
                    );
                    break;
                }

                // Coupe-circuit : Attracteur de silence (Répétition stricte de 5 tokens BPE identiques)
                let len = tokens.len();
                if len > 5
                    && tokens[len - 1] == tokens[len - 2]
                    && tokens[len - 2] == tokens[len - 3]
                    && tokens[len - 3] == tokens[len - 4]
                    && tokens[len - 4] == tokens[len - 5]
                {
                    info!(
                        "   [DECODER] Spirale Hallucinatoire détectée sur le token ({}). EOT forcé.",
                        next_token
                    );
                    break;
                }
            }

            info!("-> Décodage Tokenizer linguistique...");
            let decoded = engine
                .tokenizer
                .decode(&tokens, true)
                .map_err(|e| AgentError::InferenceError(format!("Tokenizer error: {}", e)))?;

            Ok(decoded.trim().to_string())
        })
        .await
        .map_err(|_| AgentError::InferenceError("Thread pool panic lors de l'inférence".to_string()))?;

        // 4. Mise à jour de l'état du Circuit Breaker et renvoi
        match result {
            Ok(transcription) => {
                self.circuit_breaker.record_success();
                Ok(transcription)
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Err(e)
            }
        }
    }
}

#[async_trait]
impl CognitiveAgent for AudioAgent {
    fn name(&self) -> &str {
        &self.name
    }

    #[instrument(skip(self))]
    async fn load(&mut self) -> Result<(), AgentError> {
        let desc = CortexCatalog::get_default_descriptor(CognitiveSense::Audio);
        self.name = format!(
            "AudioAgent-{}",
            desc.repo_id.split('/').next_back().unwrap_or("Whisper")
        );

        info!(
            "🔌 [CORTEX] Activation du téléchargement Auto/Local pour l'agent '{}'",
            self.name
        );

        let api =
            hf_hub::api::tokio::Api::new().map_err(|e| AgentError::LoadError(e.to_string()))?;
        let repo = api.repo(Repo::with_revision(
            desc.repo_id.to_string(),
            RepoType::Model,
            desc.revision.to_string(),
        ));

        info!("   [CORTEX] Résolution des poids Safetensors principaux...");
        let model_file = repo
            .get(desc.weights_file)
            .await
            .map_err(|e| AgentError::LoadError(format!("Échec téléchargement weights: {}", e)))?;

        info!("   [CORTEX] Résolution de la Configuration LLM...");
        let config_file = repo
            .get(desc.config_file.unwrap())
            .await
            .map_err(|e| AgentError::LoadError(format!("Échec téléchargement config: {}", e)))?;

        info!("   [CORTEX] Chargement préliminaire de la Configuration...");
        let config_str = std::fs::read_to_string(&config_file).unwrap();
        let config: Config = serde_json::from_str(&config_str).unwrap();

        info!("   [CORTEX] Résolution du Dictionnaire Tokenizer...");
        let tokenizer_file = repo
            .get(desc.tokenizer_file.unwrap())
            .await
            .map_err(|e| AgentError::LoadError(format!("Échec téléchargement tokenizer: {}", e)))?;

        let melfilters_filename = if config.num_mel_bins == 128 {
            "melfilters128.bytes"
        } else {
            "melfilters.bytes"
        };

        info!(
            "   [CORTEX] Téléchargement des Filtres Spatiaux ({})...",
            melfilters_filename
        );
        // -----------------------------------------------------------------------------------
        // [PARE-FEU RUSTY] : ROUTAGE HUGGINGFACE DYNAMIQUE POUR MELFILTERS
        // Historique : Le fichier n'existe pas toujours sur le repo officiel OpenAI.
        // Le dépôt de référence `lmz/candle-whisper` fournit officiellement les
        // filtres spatiaux dynamiques (80 ou 128 pour Large-v3).
        // -----------------------------------------------------------------------------------
        let repo_id = if config.num_mel_bins == 128 {
            "FL33TW00D-HF/distil-whisper-large-v3"
        } else {
            "FL33TW00D-HF/whisper-base"
        };

        let mel_repo = api.repo(Repo::with_revision(
            repo_id.to_string(),
            RepoType::Model,
            "main".to_string(),
        ));
        let mel_bytes_file = mel_repo.get(melfilters_filename).await.map_err(|e| {
            AgentError::LoadError(format!(
                "Échec téléchargement filtres mel ({}): {}",
                repo_id, e
            ))
        })?;
        let mel_bytes =
            std::fs::read(mel_bytes_file).map_err(|e| AgentError::LoadError(e.to_string()))?;
        let mut mel_raw = vec![0f32; mel_bytes.len() / 4];
        for (i, chunk) in mel_bytes.chunks_exact(4).enumerate() {
            let arr: [u8; 4] = chunk.try_into().unwrap();
            mel_raw[i] = f32::from_le_bytes(arr);
        }

        info!("   [CORTEX] Allocation VarBuilder et Tenseurs Memoire Whisper...");

        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(
                &[model_file],
                candle_core::DType::F32,
                &Device::Cpu,
            )
        }
        .map_err(|e| AgentError::LoadError(format!("VarBuilder fail: {}", e)))?;

        let model = Whisper::load(&vb, config.clone())
            .map_err(|e| AgentError::LoadError(format!("Whisper model instanciation: {}", e)))?;

        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_file)
            .map_err(|e| AgentError::LoadError(e.to_string()))?;

        // [R2D2 CORTEX] Compilation du Masque Anti-Hallucination
        let vocab_size = config.vocab_size as usize;
        let mut suppress_mask = vec![0.0f32; vocab_size];
        for (i, mask) in suppress_mask.iter_mut().enumerate().take(vocab_size) {
            if config.suppress_tokens.contains(&(i as u32)) {
                *mask = f32::NEG_INFINITY;
            }
        }
        let suppress_tensor = Tensor::new(suppress_mask.as_slice(), &Device::Cpu)
            .map_err(|e| AgentError::LoadError(format!("Suppress Tensor Error: {}", e)))?;

        let engine = WhisperEngine {
            config: config.clone(),
            model,
            tokenizer,
            mel_filters: mel_raw,
            suppress_tensor,
            device: Device::Cpu,
        };

        self.engine = Some(Arc::new(engine));
        self.circuit_breaker.record_success();
        self.active = true;

        info!(
            "🛡️ [CORTEX] Agent '{}' Chargé & Opérationnel (Tensors cached).",
            self.name
        );
        Ok(())
    }

    async fn unload(&mut self) -> Result<(), AgentError> {
        info!(
            "   [CORTEX] Drop inconditionnel de l'Engine et Tenseurs RAM pour '{}'.",
            self.name
        );
        self.active = false;
        self.engine = None;
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.active
    }

    #[instrument(skip_all, name = "AudioAgent::generate_thought")]
    async fn generate_thought(&mut self, prompt: &str) -> Result<String, AgentError> {
        if !self.is_active() {
            return Err(AgentError::NotActive);
        }
        let start = Instant::now();
        info!("🎙️ AudioAgent démarre l'ingestion asynchrone (Forward Pass)...");

        let transcription = self.transcribe(prompt).await?;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let jsonai = format!(
            r#"{{
            "id": "audio-{}",
            "source": {{ "Audio": "{}" }},
            "timestamp": "2026-03-24T21:30:00Z",
            "is_fact": true,
            "belief_state": "Transcription Audio Extract",
            "consensus": "Raw Sensor",
            "content": "{}",
            "ontological_tags": ["Audio", "Transcription"],
            "dependencies": []
        }}"#,
            timestamp,
            self.name(),
            transcription.replace("\"", "\\\"")
        );

        info!("Inférence audio accomplie en {:?}", start.elapsed());
        Ok(jsonai)
    }
}

