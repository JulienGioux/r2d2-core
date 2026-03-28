use anyhow::Result;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tracing::info;

use r2d2_cortex::agent::CognitiveAgent;
use r2d2_cortex::models::minilm_embedder::MiniLmEmbedderAgent;

/// Fonction de chunking basée sur les recommandations de l'Architecte (NotebookLM).
/// Taille de bloc: ~200 mots, Chevauchement: ~40 mots.
fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut chunks = Vec::new();
    let mut i = 0;

    while i < words.len() {
        let mut end = (i + chunk_size).min(words.len());
        
        // Arrondir conceptuellement à la fin d'une phrase si possible
        // (Recherche heuristique d'un point final dans le chevauchement)
        if end < words.len() {
            let mut search_idx = end;
            while search_idx > i && search_idx > end - overlap {
                if words[search_idx - 1].ends_with('.') || words[search_idx - 1].ends_with('\n') {
                    end = search_idx;
                    break;
                }
                search_idx -= 1;
            }
        }
        
        let chunk_words = &words[i..end];
        chunks.push(chunk_words.join(" "));
        
        if end == words.len() {
            break;
        }
        
        // Avancer de chunk_size moins l'overlap
        i = end - overlap;
    }

    chunks
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("🚀 [ASSIMILATOR] Booting Memory Assimilation Pipeline (Phase VIII)");

    // 1. Lecture du savoir
    let md_path = "./rustymaster_knowledge.md";
    if !Path::new(md_path).exists() {
        tracing::error!("Fichier {} introuvable.", md_path);
        return Ok(());
    }
    let text = std::fs::read_to_string(md_path)?;
    info!("   -> Source lue: {} octets.", text.len());

    // 2. Découpage Intelligent (100 mots au lieu de 200 pour garantir < 512 tokens même avec du Markdown riche)
    let chunks = chunk_text(&text, 100, 20);
    info!("   -> Découpage achevé : {} blocs sémantiques.", chunks.len());

    // 3. Charger le modèle d'Embedding MiniLM (Sémantique)
    let mut embedder = MiniLmEmbedderAgent::new();
    embedder.load().await.expect("Failed to load Embedder");

    // Fichiers Mmap de destination
    let mut bin_file = File::create("./knowledge.bin")?;
    
    // Le meta garde l'ordre exact, donc l'ID du meta = index dans le tensor
    let meta_file = File::create("./knowledge_meta.json")?;
    let mut meta_json = Vec::new();

    // 4. Ingestion Tensorielle
    info!("🧠 [ASSIMILATOR] Début de la neuralisation des {} blocs...", chunks.len());
    for (idx, chunk) in chunks.iter().enumerate() {
        // Axiom: préfixer par "passage: " pour l'enregistrement (is_query = false)
        match embedder.embed_raw(chunk, false).await {
            Ok(vec_f32) => {
                if vec_f32.len() != 384 {
                    tracing::error!("   -> ERREUR CRITIQUE: Dimension inattendue ({} != 384)", vec_f32.len());
                    return Ok(());
                }

                // Ecriture binaire Zero-Copy avec bytemuck
                let bytes: &[u8] = bytemuck::cast_slice(&vec_f32);
                if let Err(e) = bin_file.write_all(bytes) {
                    tracing::error!("   -> ERREUR CRITIQUE IO bin_file : {}", e);
                    return Ok(());
                }

                // Metadonnées
                meta_json.push(serde_json::json!({
                    "id": idx,
                    "content": chunk
                }));
                
                info!("   -> Progression: {}/{} assimilés.", idx + 1, chunks.len());
            }
            Err(e) => {
                tracing::error!("   -> ERREUR INFERENCE sur bloc {}: {:?}", idx, e);
                return Ok(());
            }
        }
    }

    if let Err(e) = serde_json::to_writer_pretty(meta_file, &meta_json) {
        tracing::error!("   -> ERREUR CRITIQUE IO meta_file : {}", e);
        return Ok(());
    }

    if let Err(e) = embedder.unload().await {
        tracing::error!("   -> ERREUR UNLOAD : {}", e);
    }
    
    info!("✅ [ASSIMILATOR] Création terminée ! Fichiers `knowledge.bin` et `knowledge_meta.json` sauvegardés.");

    Ok(())
}
