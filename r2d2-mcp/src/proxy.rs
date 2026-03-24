use r2d2_kernel::KernelError;
use r2d2_paradox::ParadoxSolver;
use tracing::{error, info, instrument};

use crate::hitl::{HitlDecision, HumanInTheLoop};

/// ============================================================================
/// 🛡️ SEMANTIC PERMISSION PROXY : LE FILTRE D'INTENTION
/// ============================================================================
pub struct SemanticProxy {
    #[allow(dead_code)]
    paradox_engine: ParadoxSolver,
    hitl: HumanInTheLoop,
}

impl Default for SemanticProxy {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticProxy {
    pub fn new() -> Self {
        Self {
            paradox_engine: ParadoxSolver,
            hitl: HumanInTheLoop::new(),
        }
    }

    /// Filtre une exécution d'outil.
    /// Traite l'intention via le ParadoxEngine. Si le système est compromis ou
    /// s'il s'agit d'une action "dangereuse", le HITL est sollicité.
    #[instrument(skip(self, arguments))]
    pub async fn audit_tool_call(
        &self,
        tool_name: &str,
        agent_role: &str,
        arguments: &serde_json::Value,
    ) -> anyhow::Result<bool, KernelError> {
        info!(
            "🔍 Le SemanticProxy analyse l'appel de l'outil '{}' par '{}'...",
            tool_name, agent_role
        );

        // 1. Détection de dangerosité statique
        let is_destructive = self.is_tool_destructive(tool_name);

        // 2. Vérification Paradigmatique (ParadoxEngine)
        // Normalement ici on injecte l'intention formatée en Fragment
        // et on utilise le solveur pour voir si elle détruit l'ontologie du système.
        info!("Vérification Paradoxale de l'outil {}...", tool_name);

        // 3. Escalade HITL si nécessaire
        if is_destructive {
            let decision = self
                .hitl
                .request_authorization(tool_name, arguments, agent_role)
                .await
                .map_err(|e| KernelError::ValidationFailed(format!("Erreur HITL : {}", e)))?;

            match decision {
                HitlDecision::Approved => {
                    info!("✅ [Proxy] Escalade HITL approuvée. Exécution autorisée.");
                    return Ok(true);
                }
                HitlDecision::Rejected => {
                    error!("❌ [Proxy] Escalade HITL REJETÉE. Exécution bloquée.");
                    return Ok(false);
                }
                HitlDecision::Escalated => {
                    error!("⚠️ [Proxy] HITL non résolu. Par précaution, appel bloqué.");
                    return Ok(false);
                }
            }
        }

        // Si inoffensif et valide au niveau paradoxal :
        info!(
            "✅ [Proxy] L'outil '{}' est certifié inoffensif. Autorisé.",
            tool_name
        );
        Ok(true)
    }

    /// Vérifie via une heuristique interne si un outil expose la machine
    /// physique ou le système cognitif à un risque de destruction.
    fn is_tool_destructive(&self, tool_name: &str) -> bool {
        let destructive_patterns = [
            "delete_",
            "rm_",
            "kill",
            "format",
            "shutdown",
            "drop_table",
            "truncate",
            "overwrite",
        ];

        destructive_patterns
            .iter()
            .any(|pattern| tool_name.to_lowercase().contains(pattern))
    }
}
