use r2d2_chunker::{AudioChunker, MediaChunker};
use r2d2_cortex::agent::CognitiveAgent;
use r2d2_cortex::models::audio_agent::AudioAgent;
use r2d2_sensory::stimulus::{Stimulus, StimulusType};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    println!("🛠️ [TEST BENCH] Démarrage de la Forge d'Ingestion R2D2 (Via l'Orchestrateur MCP)...");

    let target_file = "/mnt/d/XXXX/R2D2/media_test/a8c9d829-8995-4fac-b982-6118835ff3af .ogg";
    println!(
        "📀 [PRE-PROCESSING] Fichier cible (Haute Densité) : {}",
        target_file
    );

    let parent_stimulus = Stimulus::new(
        "long-audio".to_string(),
        StimulusType::Audio,
        PathBuf::from(target_file),
    );

    println!("✂️ [PRE-PROCESSING] Démarrage du Moteur de Chunk Automatique...");
    let chunker = AudioChunker::new();
    let fragments = chunker
        .chunk(&parent_stimulus)
        .expect("Échec du hachage audio !");

    println!(
        "📦 [PRE-PROCESSING] {} fragments de 30s purs ont été extraits en f32 binaire.",
        fragments.len()
    );

    let mut agent = AudioAgent::new();
    println!("🔌 [CORTEX] Chargement de l'agent (Whisper) en VRAM...");
    agent
        .load()
        .await
        .expect("Le chargement des tenseurs Whisper a échoué.");

    for (i, frag) in fragments.iter().enumerate() {
        println!(
            "\n🎙️ [INFERENCE] Ingestion du fragment {}/{} ({})",
            i + 1,
            fragments.len(),
            frag.payload_path.display()
        );
        let result = agent
            .generate_thought(&frag.payload_path.to_string_lossy())
            .await;

        match result {
            Ok(jsonai) => {
                println!("✅ [SUCCÈS] Fragment transcrit ! Payload JSONAI-v3 :");
                println!("{}", jsonai);
            }
            Err(e) => {
                eprintln!("❌ [ERREUR] Échec sur le fragment {} : {:?}", i + 1, e);
            }
        }
    }
    println!("\n🏁 [TEST BENCH] Tous les fragments ont été traités avec succès.");
}
