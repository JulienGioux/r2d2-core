use anyhow::Result;
use r2d2_blackboard::PostgresBlackboard;
use r2d2_circadian::CircadianDaemon;
use r2d2_cortex::{
    agent::AgentError,
    models::{bitnet_agent::BitNetAgent, minilm_embedder::MiniLmEmbedderAgent, audio_agent::AudioAgent, vision_agent::VisionAgentLlava, vision_agent_qwen::VisionAgentQwen},
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

    info!("Initialisation du Moteur de Résolution Paradoxale...");
    let solver = Arc::new(ParadoxSolver);

    info!("Chargement du Registre Cortex (Plugins IA)...");
    let cortex = Arc::new(CortexRegistry::new());

    cortex
        .register_agent(Box::new(MiniLmEmbedderAgent::new()))
        .await;
    cortex.register_agent(Box::new(BitNetAgent::new())).await;
    cortex.register_agent(Box::new(AudioAgent::new())).await;
    cortex.register_agent(Box::new(VisionAgentLlava::new())).await;
    cortex.register_agent(Box::new(VisionAgentQwen::new())).await;

    // Activation à chaud de l'agent 1.58-bit pour la réflexion locale
    cortex
        .activate("BitNet-1.58b-Cognitive")
        .await
        .map_err(|e: AgentError| anyhow::anyhow!(e))?;

    // 4. Instanciation du Métabolisme (Polling = 60 secondes pour les tests)
    // Seuil de tolérance à l'entropie = 0.8
    let mut daemon = CircadianDaemon::new(0.8, 60, blackboard, cortex, solver);

    // 5. Lancement de la boucle asynchrone infinie
    daemon.start_homeostasis_loop().await?;

    Ok(())
}
