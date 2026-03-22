//! # Brique 9 : Gateway MCP (Model Context Protocol)
//!
//! Expose l'Essaim R2D2 aux LLMs externes via le protocole standardisé MCP stdio.
//! Accepte des signaux bruts, les propulse dans le Kernel (Validation), puis
//! les sauvegarde dans le Blackboard PostgreSQL si acceptés.

use anyhow::Result;
use r2d2_blackboard::{GlobalBlackboard, PostgresBlackboard};
use r2d2_kernel::{Fragment, KernelError, Signal};
use r2d2_paradox::ParadoxSolver;
use tracing::{info, instrument};

/// Le chef d'orchestre qui relie le MCP à l'Essaim R2D2
pub struct McpGateway {
    validator: ParadoxSolver,
    blackboard: PostgresBlackboard,
}

impl McpGateway {
    pub async fn new(db_url: &str) -> Result<Self> {
        let blackboard = PostgresBlackboard::new(db_url).await?;
        Ok(Self {
            validator: ParadoxSolver,
            blackboard,
        })
    }

    /// Ingère la connaissance proposée par un agent distant via MCP.
    /// Traite toute la chaîne d'États (Typestate) jusqu'à la base de données.
    #[instrument(skip(self, payload))]
    pub async fn ingest_thought(
        &self,
        thought_id: &str,
        agent_name: &str,
        payload: String,
    ) -> Result<String, KernelError> {
        info!("MCP a reçu une nouvelle pensée de l'agent {}", agent_name);

        // 1. Initialiser le Signal (Typestate 1)
        let signal = Fragment::<Signal>::new(payload);

        // 2. Tenter de parser le Signal en Unverified (Typestate 2)
        let unverified = signal.parse()?;

        // 3. Soumettre le fragment au Paradox Engine (Typestate 3)
        // La méthode .verify() consomme le fragment et recrache soit Validated soit une Erreur.
        let validated_fragment = unverified.verify(&self.validator)?;

        info!(
            "Pensée de {} vérifiée et certifiée par Parad0x !",
            agent_name
        );

        // 4. Finaliser en SecureMemGuard pour transiter sans fuite RAM (Typestate 4)
        let guard = validated_fragment.finalize();

        // 5. Ancrer définitivement la mémoire dans la Brique 7 (Base de données Vectorielle)
        // Le guard sera consommé et effacé de la RAM par le PostgresBlackboard.
        let saved_id = self
            .blackboard
            .anchor_fragment(guard)
            .await
            .map_err(|e| KernelError::ValidationFailed(e.to_string()))?;

        info!(
            "Ancrage réussi dans le Blackboard vectoriel sous l'ID : {}",
            saved_id
        );

        Ok(saved_id)
    }
}
