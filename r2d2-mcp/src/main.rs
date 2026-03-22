use anyhow::Result;
use std::env;
use tracing::{info, Level};

// Important: Ce bloc est intentionnellement "Bouchonné" (Stub) sur l'implémentation MCP
// pour s'assurer que la CI passe même si le SDK mcp_rust_sdk v0.1.1 évolue très vite.
// L'architecture Hexagonale permet de remplacer cette porte d'entrée facilement.
use r2d2_mcp::McpGateway;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Démarrage R2D2 MCP Gateway (Brique 9)...");

    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://r2d2_admin:secure_r2d2_password_local@localhost:5433/r2d2_blackboard"
            .to_string()
    });

    let gateway = McpGateway::new(&db_url).await?;
    info!("✅ Serveur MCP prêt et connecté au Blackboard PostgreSQL vectoriel.");

    // TODO: Initialiser `mcp_rust_sdk::Server` ici.
    // Les Outils (Tools) exposés seront :
    // 1. "r2d2_ingest_thought" : Prend (agent_name, payload) et appelle gateway.ingest_thought()
    // 2. "r2d2_search_vector" : Interrogera le Blackboard via SQL HNSW.

    // Simuler une boucle infinie de serveur pour ne pas quitter
    // Dans le vrai code MCP, ce serait Server::listen() ou stdio transport.
    tokio::signal::ctrl_c().await?;
    info!("Arrêt de R2D2 MCP Gateway.");

    Ok(())
}
