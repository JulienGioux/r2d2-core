use anyhow::Result;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use tracing::info;

use r2d2_cortex::agent::CognitiveAgent;
use r2d2_cortex::models::minilm_embedder::MiniLmEmbedderAgent;

// Fonction de chunking retirée au profit de r2d2_chunker::TextChunker

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("🚀 [ASSIMILATOR] Booting Memory Assimilation Pipeline (La Forge)");

    let args: Vec<String> = std::env::args().collect();
    let mut target_mission = None;
    if let Some(idx) = args.iter().position(|a| a == "--mission") {
        if idx + 1 < args.len() {
            target_mission = Some(args[idx + 1].clone());
            info!(
                "🎯 Action ciblée activée. Mission ID: {}",
                target_mission.as_ref().unwrap()
            );
        }
    }

    let dataset_path = "data/synthetic_dataset.jsonl";
    if !Path::new(dataset_path).exists() {
        tracing::error!(
            "Fichier {} introuvable. Aucune donnée vampirisée.",
            dataset_path
        );
        return Ok(());
    }

    let file = File::open(dataset_path)?;
    let reader = std::io::BufReader::new(file);
    use std::io::BufRead;

    let mut all_chunks = Vec::new();

    for l in reader.lines().map_while(Result::ok) {
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(&l) {
            // Filtre ciblé sur une mission spécifique si demandé
            if let Some(target) = &target_mission {
                if let Some(m_id) = entry.get("mission_id").and_then(|v| v.as_str()) {
                    if m_id != target {
                        continue; // On ignore ce qui n'est pas la mission ciblée
                    }
                } else {
                    continue; // Pas de mission ID = on ignore
                }
            }

            let theme = entry
                .get("theme")
                .and_then(|v| v.as_str())
                .unwrap_or("Connaissance Générale");

            if let Some(messages) = entry.get("messages").and_then(|v| v.as_array()) {
                for msg in messages {
                    if msg.get("role").and_then(|v| v.as_str()) == Some("assistant") {
                        if let Some(content_str) = msg.get("content").and_then(|v| v.as_str()) {
                            // Parsing du JSON interne généré par NotebookLM
                            if let Ok(inner) =
                                serde_json::from_str::<serde_json::Value>(content_str)
                            {
                                // SECURITE ZERO-TRUST: Validation Formelle
                                let success = inner
                                    .get("success")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);
                                if !success {
                                    tracing::warn!("🛡️ [ZERO-TRUST] Rejet d'un payload invalide (Trace Erreur détectée).");
                                    continue;
                                }

                                // Extraction pure
                                if let Some(answer) = inner
                                    .get("data")
                                    .and_then(|d| d.get("answer"))
                                    .and_then(|a| a.as_str())
                                {
                                    let full_text = format!("Thème: {}\n\n{}", theme, answer);
                                    let chunks =
                                        r2d2_chunker::TextChunker::chunk_text(&full_text, 200, 40);
                                    all_chunks.extend(chunks);
                                }
                            } else {
                                tracing::warn!("🛡️ [ZERO-TRUST] Impossible de parser le contenu JSON interne. Rejeté.");
                            }
                        }
                    }
                }
            }
        }
    }

    if all_chunks.is_empty() {
        tracing::warn!("⚠️ Aucun fragment de connaissance extractible trouvé pour l'assimilation.");
        return Ok(());
    }

    info!(
        "   -> Extraction réussie et purifiée. Blocs sémantiques: {}",
        all_chunks.len()
    );

    let mut embedder = MiniLmEmbedderAgent::new();
    embedder.load().await.expect("Failed to load Embedder");

    // Mode Append pour la Forge: on concatène au savoir existant
    let bin_path = "./knowledge.bin";
    let meta_path = "./knowledge_meta.json";

    let mut bin_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(bin_path)?;

    let mut meta_json = if Path::new(meta_path).exists() {
        let meta_data = std::fs::read_to_string(meta_path).unwrap_or_else(|_| "[]".to_string());
        serde_json::from_str::<Vec<serde_json::Value>>(&meta_data).unwrap_or_default()
    } else {
        Vec::new()
    };

    let base_idx = meta_json.len();

    info!(
        "🧠 [FORGE] Forge activée. Existant: {} blocs. Ajout de {} blocs...",
        base_idx,
        all_chunks.len()
    );

    for (i, chunk) in all_chunks.iter().enumerate() {
        match embedder.embed_raw(chunk, false).await {
            Ok(vec_f32) => {
                if vec_f32.len() != 384 {
                    tracing::error!(
                        "   -> ERREUR CRITIQUE: Dimension inattendue ({} != 384)",
                        vec_f32.len()
                    );
                    continue;
                }

                let bytes: &[u8] = bytemuck::cast_slice(&vec_f32);
                if let Err(e) = bin_file.write_all(bytes) {
                    tracing::error!("   -> ERREUR CRITIQUE IO bin_file : {}", e);
                    break;
                }

                meta_json.push(serde_json::json!({
                    "id": base_idx + i,
                    "content": chunk
                }));

                info!("   -> Coulée du Vecteur: {}/{}...", i + 1, all_chunks.len());
            }
            Err(e) => {
                tracing::error!("   -> ERREUR INFERENCE sur bloc {}: {:?}", base_idx + i, e);
                break;
            }
        }
    }

    let mut meta_file = File::create(meta_path)?;
    if let Err(e) = serde_json::to_writer_pretty(&mut meta_file, &meta_json) {
        tracing::error!("   -> ERREUR CRITIQUE IO meta_file : {}", e);
        return Ok(());
    }

    if let Err(e) = embedder.unload().await {
        tracing::error!("   -> ERREUR UNLOAD : {}", e);
    }

    info!("✅ [FORGE] Lingots coulés ! L'IA est désormais mise à niveau.");

    Ok(())
}
