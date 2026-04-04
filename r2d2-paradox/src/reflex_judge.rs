use anyhow::Result;
use pgvector::Vector;
use r2d2_blackboard::PostgresBlackboard;
use r2d2_cortex::agent::CognitiveAgent;
use r2d2_cortex::models::minilm_embedder::MiniLmEmbedderAgent;
use std::sync::Arc;
use tracing::{info, warn};

/// Actions de Routage Hybride
/// Définit l'instruction claire du Système 1 pour l'Orchestrateur (Architecture Hexagonale)
pub enum RoutingAction {
    /// Action connue en mémoire, on doit exécuter directement ce payload sans réflexion LLM.
    Reflex(String),
    /// Situation ambiguë ou inconnue, on l'envoie au cortex (LLM lent).
    Cognitive(String),
    /// Bruit de fond non pertinent (à ignorer).
    Ignore,
}

/// Système 1 de Kahneman : Le juge réflexe (Mémoire Vectorielle via pgvector)
/// Évaluation sémantique ultra-rapide (MiniLM Local -> Postgres pgvector HNSW)
pub struct ReflexJudge {
    embedder: MiniLmEmbedderAgent,
    blackboard: Option<Arc<PostgresBlackboard>>,
    cognitive_threshold: f32, // Seuil à partir duquel on escalade au Système 2
}

impl ReflexJudge {
    pub fn new() -> Self {
        Self {
            embedder: MiniLmEmbedderAgent::new(),
            blackboard: None,
            cognitive_threshold: 0.90, // Cosinus > 0.90 => Similarité extrêmement forte
        }
    }
}

impl Default for ReflexJudge {
    fn default() -> Self {
        Self::new()
    }
}

impl ReflexJudge {
    pub fn with_blackboard(mut self, blackboard: Arc<PostgresBlackboard>) -> Self {
        self.blackboard = Some(blackboard);
        self
    }

    /// Charge le modèle Candle Bare-Metal.
    pub async fn initialize(&mut self) -> Result<()> {
        info!("🧠 [SYSTÈME 1] Initialisation de la Mémoire Réflexe (MiniLM)...");
        self.embedder
            .load()
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Ancrage Sémantique de ce qui est "interdit"
        if let Some(bb) = &self.blackboard {
            let danger_concept = "Erreur, contradiction critique, absurde, faux, mensonge, impossible, violation, sécurité compromise.";
            let embed_danger = self
                .embedder
                .embed_raw(danger_concept, true)
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;

            // Sauvegarde dans le Blackboard comme Réflexe pré-défini
            bb.save_reflex(
                Vector::from(embed_danger),
                r#"{"action": "escalate", "reason": "danger_sémantique"}"#,
            )
            .await?;

            info!("🧠 [SYSTÈME 1] Ancrage sémantique du Danger (Dissonance) écrit dans PostgreSQL (pgvector).");
        } else {
            warn!("⚠️ [SYSTÈME 1] Aucun Blackboard lié. La base réflexe ne sera pas joignable.");
        }

        Ok(())
    }

    /// Évalue rapidement un payload sémantique via son embedding local.
    /// Renvoie une directive `RoutingAction` découplée de l'exécution pure.
    #[tracing::instrument(skip(self, stimulus))]
    pub async fn hybrid_evaluate(&mut self, stimulus: &str) -> Result<RoutingAction> {
        if !self.embedder.is_active() {
            warn!(
                "⚠️ [SYSTÈME 1] MiniLM non chargé. Fallback vers le Cortex Cognitif (Système 2)."
            );
            return Ok(RoutingAction::Cognitive(stimulus.to_string()));
        }

        let bb = match &self.blackboard {
            Some(b) => b,
            None => {
                warn!("⚠️ [SYSTÈME 1] Blackboard manquant, escalade vers le Système 2 par défaut.");
                return Ok(RoutingAction::Cognitive(stimulus.to_string()));
            }
        };

        // 1. Vectorisation Locale Bare-Metal (< 30ms)
        let thought_embed = self
            .embedder
            .embed_raw(stimulus, false)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        info!("⚡ [SYSTÈME 1] Requête de la mémoire réflexe distante (PostgreSQL)...");

        // 2. Recherche Strict HNSW via pgvector
        let reflex_opt = bb
            .find_matching_reflex(Vector::from(thought_embed), self.cognitive_threshold)
            .await?;

        // 3. Routing Hexagonal
        match reflex_opt {
            Some((action_payload, similarity)) => {
                info!(
                    sim = similarity,
                    "🎯 [SYSTÈME 1] Mémoire Réflexe Touchée ! Court-circuit du Système 2 activé."
                );

                if action_payload.contains("\"escalate\"") {
                    // Si c'est techniquement un danger, on route vers le cognitif pour statuer,
                    // OU on bloque (ici, pour l'exemple, on bloque en signalant le danger au Système 2).
                    warn!("📛 [SYSTÈME 1] Réflexe de Survie Actif.");
                    Ok(RoutingAction::Cognitive(format!(
                        "URGENCE DETECTEE: {}",
                        stimulus
                    )))
                } else {
                    Ok(RoutingAction::Reflex(action_payload))
                }
            }
            None => {
                info!("⏳ [SYSTÈME 1] Stimulus inconnu ou complexe. Transfert au Système 2.");
                Ok(RoutingAction::Cognitive(stimulus.to_string()))
            }
        }
    }
}
