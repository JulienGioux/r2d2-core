use crate::stimulus::{Stimulus, StimulusType};
use anyhow::Result;
use r2d2_cortex::CortexRegistry;
use r2d2_kernel::{Fragment, Signal};
use std::sync::Arc;
use tracing::{info, instrument};

/// ============================================================================
/// 🧠 SENSORY GATEWAY
/// ============================================================================
/// Le port d'entrée principal. Il reçoit l'information brute et décide à quel(s)
/// agent(s) du Cortex il doit déléguer l'extraction de sens.
pub struct SensoryGateway {
    cortex: Arc<CortexRegistry>,
}

impl SensoryGateway {
    pub fn new(cortex: Arc<CortexRegistry>) -> Self {
        Self { cortex }
    }

    /// Ingère un stimulus du monde réel, et délègue au Cortex l'interprétation.
    /// Retourne un Fragment<Signal> prêt à être validé par le ParadoxEngine.
    #[instrument(skip_all, fields(id = %stimulus.id))]
    pub async fn ingest(&self, stimulus: Stimulus) -> Result<Fragment<Signal>> {
        info!(
            "SENSORY GATEWAY : Réception d'un stimulus de type {:?}",
            stimulus.stimulus_type
        );

        let system_prompt = match stimulus.stimulus_type {
            StimulusType::Audio => "Transcribe this audio precisely. Return JSONAI.",
            StimulusType::Visual => "Analyze this image/frame and describe the scene focusing on logical details. Return JSONAI.",
            _ => "Extract semantic facts from this input. Return JSONAI."
        };

        info!(
            "-> Délégation à l'agent Cortical (Whisper/LLaVA en attente)... ({})",
            system_prompt
        );

        // Simulation d'un retour JSONAI brut non validé (Pour la phase de test)
        let synthetic_jsonai = format!(
            r#"{{
            "id": "stimulus-{}",
            "source": "CognitiveAgent",
            "timestamp": "2026-03-24T20:00:00Z",
            "is_fact": false,
            "belief_state": "Perspective",
            "consensus": "Uncertain",
            "content": "Stimulus {} ingéré mais non entièrement traité via ML."
        }}"#,
            stimulus.id, stimulus.id
        );

        Ok(Fragment::<Signal>::new(synthetic_jsonai))
    }
}
