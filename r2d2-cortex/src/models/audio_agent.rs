use crate::agent::{CognitiveAgent, AgentError};
use async_trait::async_trait;
use tracing::{info, instrument};
use std::time::Instant;

/// Agent Cortical dédié à la transcription Audio via Whisper (Candle).
pub struct AudioAgent {
    // TODO: Stocker ici les tenseurs / tokenizer chargés via hf-hub
    active: bool,
}

impl AudioAgent {
    pub fn new() -> Self {
        Self { active: false }
    }

    /// Processus interne d'inférence ML (Whisper - Candle)
    async fn transcribe(&self, audio_payload: &str) -> Result<String, AgentError> {
        info!("AudioAgent: Chargement des poids Whisper via Candle (Simulé)...");
        // Pour l'instant, on suppose que l'input est le chemin du fichier audio ou son Base64.
        Ok(format!("Transcription simulée de {}", audio_payload))
    }
}

#[async_trait]
impl CognitiveAgent for AudioAgent {
    fn name(&self) -> &'static str {
        "AudioAgent-Whisper-Tiny"
    }

    async fn load(&mut self) -> Result<(), AgentError> {
        info!("Chargement de l'AudioAgent (Whisper) en VRAM...");
        self.active = true;
        Ok(())
    }

    async fn unload(&mut self) -> Result<(), AgentError> {
        info!("Déchargement de l'AudioAgent de la VRAM...");
        self.active = false;
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.active
    }

    #[instrument(skip_all, name = "AudioAgent::generate_thought")]
    async fn generate_thought(&mut self, prompt: &str) -> Result<String, AgentError> {
        let start = Instant::now();
        info!("🎙️ AudioAgent démarre l'ingestion asynchrone...");

        let transcription = self.transcribe(prompt).await?;

        // On forge la déduction sous format JsonAiV3 sans macro UUID pour éviter une dépendance.
        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
        let jsonai = format!(r#"{{
            "id": "audio-{}",
            "source": "{}",
            "timestamp": "2026-03-24T21:30:00Z",
            "is_fact": false,
            "belief_state": "Perspective",
            "consensus": "Uncertain",
            "content": "{}"
        }}"#, 
            timestamp,
            self.name(),
            transcription.replace("\"", "\\\"")
        );

        info!("Inférence audio accomplie en {:?}", start.elapsed());
        Ok(jsonai)
    }
}
