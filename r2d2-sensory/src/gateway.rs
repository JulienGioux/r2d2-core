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

        if stimulus.stimulus_type == StimulusType::Visual {
            // Séquence Mixture of Experts (MoE) : Inférence Séquentielle (RAM < 8Go)
            info!("-> Démarrage de la séquence Visual Mixture-of-Experts (MoE)...");
            
            let experts = [
                "VisionAgent-Llava".to_string(),
                "VisionAgent-Qwen".to_string(), // Séquence MoE validée.
            ];
            
            let mut moes_results = Vec::new();
            let prompt = stimulus.payload_path.to_string_lossy().to_string();

            for expert in &experts {
                info!("   [MoE] Activation Poids-Lourds de l'expert '{}'...", expert);
                self.cortex.activate(expert).await
                    .map_err(|e| anyhow::anyhow!("Échec activation MoE '{}': {:?}", expert, e))?;
                
                let jsonai = self.cortex.interact_with(expert, &prompt).await
                    .map_err(|e| anyhow::anyhow!("Échec d'inférence MoE '{}': {:?}", expert, e))?;
                
                moes_results.push(jsonai);
            }

            // Synthèse temporaire (En attendant le ParadoxEngine pour le vrai DEBATED_SYNTHESIS)
            // On renvoie le résultat du dernier expert (qui encapsule les métadonnées JSONAI v3)
            let final_jsonai = moes_results.pop().unwrap_or_else(|| String::from("Aucun expert"));
            
            Ok(Fragment::<Signal>::new(final_jsonai))
        } else {
            // Routage Classique (Non-MoE)
            let (agent_name, prompt) = match stimulus.stimulus_type {
                StimulusType::Audio => ("AudioAgent", stimulus.payload_path.to_string_lossy().to_string()),
                _ => return Err(anyhow::anyhow!("Type de stimulus non pris en charge")),
            };

            info!("-> Délégation à l'agent Cortical '{}' (Activation en RAM demandée)...", agent_name);
            self.cortex.activate(agent_name).await
                .map_err(|e| anyhow::anyhow!("Échec d'allocation Cortex: {:?}", e))?;
            let jsonai = self.cortex.interact_with(agent_name, &prompt).await
                .map_err(|e| anyhow::anyhow!("Échec d'inférence Cortex: {:?}", e))?;

            Ok(Fragment::<Signal>::new(jsonai))
        }
    }
}
