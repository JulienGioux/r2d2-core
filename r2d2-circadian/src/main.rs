use anyhow::Result;
use r2d2_blackboard::PostgresBlackboard;
use r2d2_circadian::CircadianDaemon;
use r2d2_cortex::{
    models::{
        audio_agent::AudioAgent, reasoning_agent::ReasoningAgent, vision_agent::VisionAgentLlava,
        vision_agent_qwen::VisionAgentQwen,
    },
    CortexRegistry,
};
use r2d2_paradox::ParadoxSolver;
use std::env;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialisation de l'Observabilité Industrielle
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Échec du Tracing");

    info!("===============================================");
    info!("🌙 DÉMARRAGE DU DAEMON R2D2-CIRCADIAN (MCTS) 🌙");
    info!("===============================================");

    // 2. Variables d'environnement
    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://r2d2_admin:secure_r2d2_password_local@localhost:5433/r2d2_blackboard"
            .to_string()
    });

    // 3. Initialisation des composants cœurs
    info!("Connexion au Vector Blackboard...");
    let blackboard = Arc::new(PostgresBlackboard::new(&db_url).await?);

    info!("Initialisation du Moteur de Résolution Paradoxale (Système 1 & 2)...");
    let mut reflex_judge =
        r2d2_paradox::reflex_judge::ReflexJudge::new().with_blackboard(blackboard.clone());
    if let Err(e) = reflex_judge.initialize().await {
        tracing::warn!(
            "Erreur init Système 1 (Réflexe) : {}. On continue en Mode Fallback.",
            e
        );
    }
    let reflex = Arc::new(tokio::sync::Mutex::new(reflex_judge));
    let solver = Arc::new(ParadoxSolver::new().with_reflex(reflex));

    info!("Chargement du Registre Cortex (Plugins IA)...");
    let cortex = Arc::new(CortexRegistry::new());

    use r2d2_adapter_candle::CandleEmbedder;
    use r2d2_cortex::error::CortexError;
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
            tracing::info!(
                "🔌 [CORTEX] Circadian Daemon: Download & Hot-Swap pour '{}'",
                self.name
            );
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
    cortex.register_agent(Box::new(AudioAgent::new())).await;
    cortex
        .register_agent(Box::new(VisionAgentLlava::new()))
        .await;
    cortex
        .register_agent(Box::new(VisionAgentQwen::new()))
        .await;
    cortex.register_agent(Box::new(ReasoningAgent::new())).await;

    // Activation à chaud de l'agent de Raisonnement
    cortex
        .activate("Paradox-MultiAPI Router")
        .await
        .map_err(|e: CortexError| anyhow::anyhow!(e))?;

    // 4. Instanciation du Métabolisme (Polling = 60 secondes pour les tests)
    // Seuil de tolérance à l'entropie = 0.85 (Demande User)
    let (mut daemon, _rx) = CircadianDaemon::new(0.85, 60, blackboard, cortex, solver);

    // 5. Lancement de la boucle asynchrone infinie
    // (Pour le binaire standalone, on crée un canal de shutdown factice qui n'est jamais trigger)
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Possibilité d'attacher le tokio::signal::ctrl_c ici dans le futur
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        tracing::warn!("SIGINT détecté par le standalone. Lancement du Shutdown...");
        let _ = _shutdown_tx.send(true);
    });

    daemon.start_homeostasis_loop(shutdown_rx).await?;

    Ok(())
}
