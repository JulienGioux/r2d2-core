//! # Brique 9 : Gateway MCP (Model Context Protocol)
//!
//! Expose l'Essaim R2D2 aux LLMs externes via le protocole standardisé MCP stdio.
//! Accepte des signaux bruts, les propulse dans le Kernel (Validation), puis
//! les sauvegarde dans le Blackboard PostgreSQL si acceptés.

pub mod client;
pub mod hitl;
pub mod proxy;
pub mod registry;

use anyhow::Result;
use r2d2_blackboard::{GlobalBlackboard, PostgresBlackboard};
use r2d2_cortex::{
    agent::AgentError,
    models::{bitnet_agent::BitNetAgent, minilm_embedder::MiniLmEmbedderAgent},
    CortexRegistry,
};
use r2d2_kernel::{Fragment, KernelError, Signal};
use r2d2_paradox::ParadoxSolver;
use std::sync::Arc;
use tracing::{info, instrument};

use proxy::SemanticProxy;
use registry::ToolRegistry;

/// Le chef d'orchestre qui relie le MCP à l'Essaim R2D2
pub struct McpGateway {
    validator: ParadoxSolver,
    blackboard: PostgresBlackboard,
    cortex: Arc<CortexRegistry>,
    pub proxy: SemanticProxy,
    pub registry: ToolRegistry,
}

impl McpGateway {
    pub async fn new(db_url: &str) -> Result<Self> {
        let blackboard = PostgresBlackboard::new(db_url).await?;

        info!("Initialisation du Registre Cortex (Plug & Play)...");
        let cortex = Arc::new(CortexRegistry::new());

        // Configuration de l'Agent d'Embedding par défaut
        let embedder = Box::new(MiniLmEmbedderAgent::new());
        cortex.register_agent(embedder).await;

        // Configuration du Cœur Cognitif Natif (R2D2-BitNet)
        let bitnet_core = Box::new(BitNetAgent::new());
        cortex.register_agent(bitnet_core).await;

        // Activation immédiate pour charger les poids en RAM (Hot-Load)
        cortex
            .activate("Multilingual-E5-Small")
            .await
            .map_err(|e: AgentError| anyhow::anyhow!(e))?;

        Ok(Self {
            validator: ParadoxSolver,
            blackboard,
            cortex,
            proxy: SemanticProxy::new(),
            registry: ToolRegistry::new(),
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

    /// Outil de recherche HNSW sur la mémoire vectorielle.
    #[instrument(skip(self))]
    pub async fn search_memory(&self, _query: &str) -> Result<String, KernelError> {
        info!("MCP a demandé une exhumation mémorielle : {}", _query);

        // Appel souverain au Cortex pour générer le vecteur d'Embedding localement !
        let vec_json = self
            .cortex
            .interact_with("Multilingual-E5-Small", _query)
            .await
            .map_err(|e| KernelError::ValidationFailed(format!("Cortex Error: {}", e)))?;

        // Le JSON est un array standard désérialisé en Rust
        let embed_vec: Vec<f32> = serde_json::from_str(&vec_json)
            .map_err(|e| KernelError::ValidationFailed(e.to_string()))?;

        // Transformation en tenseur SQL pgvector (Dimension 384)
        let vector = pgvector::Vector::from(embed_vec);

        let results = self
            .blackboard
            .recall_memory(vector, 5)
            .await
            .map_err(|e| KernelError::ValidationFailed(e.to_string()))?;

        if results.is_empty() {
            return Ok("Aucun souvenir pertinent trouvé dans le Blackboard.".to_string());
        }

        let mut output = String::new();
        for (i, res) in results.iter().enumerate() {
            output.push_str(&format!("[Souvenir {}]: {}\n", i + 1, res));
        }

        Ok(output)
    }
}
