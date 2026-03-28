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
                info!(
                    "   [MoE] Activation Poids-Lourds de l'expert '{}'...",
                    expert
                );
                self.cortex
                    .activate(expert)
                    .await
                    .map_err(|e| anyhow::anyhow!("Échec activation MoE '{}': {:?}", expert, e))?;

                let jsonai = self
                    .cortex
                    .interact_with(expert, &prompt)
                    .await
                    .map_err(|e| anyhow::anyhow!("Échec d'inférence MoE '{}': {:?}", expert, e))?;

                moes_results.push(jsonai);
            }

            // Synthèse de Triangulation (DEBATED_SYNTHESIS) JSONAI v3
            let mut debates = Vec::new();
            for (i, res) in moes_results.iter().enumerate() {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(res) {
                    let expert_name = parsed.get("source").map(|s| s.to_string()).unwrap_or_else(|| "Unknown".to_string());
                    let content = parsed.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    debates.push(format!("[Expert {} - {}] : {}", i + 1, expert_name, content));
                } else {
                    debates.push(format!("[Expert {}] : {}", i + 1, res));
                }
            }

            let synthesis_content = debates.join("\n\n---\n\n");
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();

            let final_jsonai = format!(
                r#"{{
            "id": "moe-synthesis-{}",
            "source": "System",
            "timestamp": "2026-03-25T00:00:00Z",
            "is_fact": false,
            "belief_state": "Perspective",
            "consensus": "DebatedSynthesis",
            "content": "{}",
            "ontological_tags": ["Vision", "MoE", "Triangulation"],
            "dependencies": []
        }}"#,
                timestamp,
                synthesis_content.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n")
            );

            Ok(Fragment::<Signal>::new(final_jsonai))
        } else {
            // Routage Classique (Non-MoE)
            let (agent_name, prompt) = match stimulus.stimulus_type {
                StimulusType::Audio => (
                    "AudioAgent",
                    stimulus.payload_path.to_string_lossy().to_string(),
                ),
                _ => return Err(anyhow::anyhow!("Type de stimulus non pris en charge")),
            };

            info!(
                "-> Délégation à l'agent Cortical '{}' (Activation en RAM demandée)...",
                agent_name
            );
            self.cortex
                .activate(agent_name)
                .await
                .map_err(|e| anyhow::anyhow!("Échec d'allocation Cortex: {:?}", e))?;
            let jsonai = self
                .cortex
                .interact_with(agent_name, &prompt)
                .await
                .map_err(|e| anyhow::anyhow!("Échec d'inférence Cortex: {:?}", e))?;

            Ok(Fragment::<Signal>::new(jsonai))
        }
    }
}
