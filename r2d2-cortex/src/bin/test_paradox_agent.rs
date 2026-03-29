use anyhow::Result;

use r2d2_cortex::agent::CognitiveAgent;
use r2d2_cortex::models::reasoning_agent::ReasoningAgent;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 [WORKSPACE] Démarrage du test d'intégration OS / ParadoxEngine");

    // Instanciation de l'agent métier via le trait défini par le Chef
    let mut reasoning_agent = ReasoningAgent::new();

    // 1. Boot : Allocation VRAM/CPU et montage du réseau scalaire
    reasoning_agent
        .load()
        .await
        .expect("Échec du chargement structurel de l'Agent");

    // 2. Ingestion Multimodale : Simulation d'un retour du SensoryGateway
    println!("📡 [WORKSPACE] Injection de la question cognitive : 'Quels sont les axiomes de R2D2 sur la Tokenisation ?'");
    let json_inference = reasoning_agent
        .generate_thought(
            "Explique moi les axiomes de R2D2 sur l'architecture hexagonale et tokio.",
        )
        .await?;

    println!("\n============================================================");
    println!("🧠 RÉPONSE FORMELLE DE L'AGENT DE RAISONNEMENT (JsonAiV3)");
    println!("============================================================");
    println!("{}", json_inference);
    println!("============================================================\n");

    // 3. Extinction propre (Drop des tenseurs)
    reasoning_agent.unload().await?;

    println!("🛑 [WORKSPACE] Arrêt souverain du Cortex sans erreur mémoire.");
    Ok(())
}
