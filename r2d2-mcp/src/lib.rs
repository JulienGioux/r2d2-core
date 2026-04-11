//! # Brique 9 : Gateway MCP (Model Context Protocol)
//!
//! Expose l'Essaim R2D2 aux LLMs externes via le protocole standardisé MCP stdio.
//! Accepte des signaux bruts, les propulse dans le Kernel (Validation), puis
//! les sauvegarde dans le Blackboard PostgreSQL si acceptés.

pub mod actuator;
pub mod client;
pub mod hitl;
pub mod proxy;
pub mod registry;
pub mod vampire;

use anyhow::Result;
use r2d2_blackboard::{GlobalBlackboard, PostgresBlackboard};
use r2d2_cortex::{error::CortexError, CortexRegistry};
use r2d2_kernel::{Fragment, KernelError, Signal};
use r2d2_paradox::ParadoxSolver;
use std::sync::Arc;
use tracing::{info, instrument};

use actuator::PhysicalExecutorAdapter;
use proxy::SemanticProxy;
use registry::ToolRegistry;
pub use vampire::VampireWorker;

use r2d2_adapter_candle::CandleEmbedder;
use r2d2_kernel::ports::TextEmbedder;
use r2d2_registry::{fetcher::ModelFetcher, manifest::ModelManifest, ModelRegistry};

struct CandleEmbedderAgent {
    name: String,
    manifest: ModelManifest,
    embedder: Option<Arc<CandleEmbedder>>,
}

#[async_trait::async_trait]
impl r2d2_cortex::agent::CognitiveAgent for CandleEmbedderAgent {
    fn name(&self) -> &str {
        &self.name
    }

    async fn load(&mut self) -> Result<(), CortexError> {
        tracing::info!("🔌 [CORTEX] Download & Hot-Swap HF pour '{}'", self.name);

        let local_manifest = ModelFetcher::ensure_downloaded(
            &self.manifest,
            "sentence-transformers/all-MiniLM-L6-v2",
            "main",
            "model.safetensors",
        )
        .await
        .map_err(|e| CortexError::LoadError(e.to_string()))?;

        tracing::info!("   [CORTEX] Lancement de l'I/O Bloquant (mmap vers VRAM/RAM)");

        let emb: CandleEmbedder =
            tokio::task::spawn_blocking(move || CandleEmbedder::new(&local_manifest))
                .await
                .map_err(|e| CortexError::LoadError(e.to_string()))?
                .map_err(|e| CortexError::LoadError(e.to_string()))?;

        self.embedder = Some(Arc::new(emb));
        Ok(())
    }

    async fn unload(&mut self) -> Result<(), CortexError> {
        tracing::info!(
            "   [CORTEX] Drop inconditionnel des vues RAM pour '{}'.",
            self.name
        );
        self.embedder = None;
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.embedder.is_some()
    }

    async fn generate_thought(&mut self, prompt: &str) -> Result<String, CortexError> {
        if let Some(emb) = &self.embedder {
            let vec_f32 = emb
                .embed_text(prompt)
                .await
                .map_err(|e| CortexError::InferenceError(format!("{:?}", e)))?;
            Ok(serde_json::to_string(&vec_f32.data).unwrap_or_default())
        } else {
            Err(CortexError::NotActive)
        }
    }
}

/// Le chef d'orchestre qui relie le MCP à l'Essaim R2D2
pub struct McpGateway {
    validator: ParadoxSolver,
    blackboard: PostgresBlackboard,
    cortex: Arc<CortexRegistry>,
    pub proxy: SemanticProxy,
    pub registry: ToolRegistry,
    executor: PhysicalExecutorAdapter,
}

impl McpGateway {
    pub async fn new(db_url: &str) -> Result<Self> {
        let blackboard = PostgresBlackboard::new(db_url).await?;

        info!("Initialisation du Registre Cortex (Plug & Play)...");
        let cortex = Arc::new(CortexRegistry::new());

        let reg = ModelRegistry::new("data/store/manifests/");
        if let Some((_, embedder_config)) = reg
            .find_by_name(&r2d2_registry::types::ModelId("minilm_l6_v2".to_string()))
            .await
        {
            let glued_agent = CandleEmbedderAgent {
                name: "Multilingual-E5-Small".to_string(),
                manifest: embedder_config,
                embedder: None,
            };
            cortex.register_agent(Box::new(glued_agent)).await;
        }

        Ok(Self {
            validator: ParadoxSolver::new(),
            blackboard,
            cortex,
            proxy: SemanticProxy::new(),
            registry: ToolRegistry::new(),
            executor: PhysicalExecutorAdapter::new("docker", 5), // Sandbox Docker avec Timeout de 5s
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
        let validated_fragment = unverified.verify(&self.validator).await?;

        info!(
            "Pensée de {} vérifiée et certifiée par Parad0x !",
            agent_name
        );

        // 4. Finaliser en SecureMemGuard pour transiter sans fuite RAM (Typestate 4)
        let guard = validated_fragment.finalize();

        // 4.5. ROUTAGE ET EXÉCUTION RÉFLEXE (SYSTÈME 1)
        {
            let payload_str = &guard.expose_payload().payload;
            if let Some(start) = payload_str.find("[REFLEX_ACTION: ") {
                if let Some(end) = payload_str[start..].find("]") {
                    let action_payload = &payload_str[start + 16..start + end];
                    tracing::warn!(
                        "⚡ [ORCHESTRATEUR] Directive Système 1 reçue ! Déploiement immédiat de l'Actuateur Physique Sandboxé."
                    );

                    // Exécution formelle du réflexe (Bloque virtuellement mais protégé par le Timeout de 5s interne)
                    if let Err(e) = self.executor.execute_reflex(action_payload).await {
                        tracing::error!(
                            "❌ [ORCHESTRATEUR] Échec critique du Bras Physique: {}",
                            e
                        );
                    }
                }
            }
        }

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
