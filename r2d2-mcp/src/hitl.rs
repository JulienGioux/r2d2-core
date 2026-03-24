use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// ============================================================================
/// 🛑 HUMAN-IN-THE-LOOP (HITL) : VALIDATION MÉCANIQUE DES ACTES DESTRUCTEURS
/// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HitlDecision {
    Approved,
    Rejected,
    Escalated,
}

pub struct HumanInTheLoop {}

impl Default for HumanInTheLoop {
    fn default() -> Self {
        Self::new()
    }
}

impl HumanInTheLoop {
    pub fn new() -> Self {
        Self {}
    }

    /// Suspend l'exécution d'un outil critique et demande explicitement
    /// la validation de l'opérateur humain (Terminal interactif ou Push Notification).
    pub async fn request_authorization(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
        agent_role: &str,
    ) -> anyhow::Result<HitlDecision> {
        warn!(
            "🚨 [HITL] L'agent '{}' demande l'exécution de l'outil critique '{}'.",
            agent_role, tool_name
        );
        info!("Arguments soumis : {}", arguments);

        // Dans un environnement terminal natif (stdio interactif), on pourrait bloquer ici.
        // Toutefois, en mode MCP stdio (piloté par Claude), on ne peut pas scanner Stdin,
        // car Stdin est utilisé pour le canal RPC de Claude.
        // Une implémentation industrielle utiliserait un canal out-of-band (ex: SSE ou popup UI).
        // Pour le POC, si l'outil est classé destructeur (ex: delete), on rejette systématiquement
        // sauf si un flag 'override_hitl' est glissé manuellement dans les vars d'env.

        if std::env::var("R2D2_DISABLE_HITL").unwrap_or_default() == "1" {
            warn!("⚠️ [HITL] R2D2_DISABLE_HITL est activé. Autorisation forcée (DANGER).");
            return Ok(HitlDecision::Approved);
        }

        warn!("🛑 [HITL] Mode strict : Exécution bloquée en l'absence d'approbation humaine OOB.");
        // Pour la Brique 9, on simule un rejet par défaut des outils non-whitelisted.
        Ok(HitlDecision::Rejected)
    }
}
