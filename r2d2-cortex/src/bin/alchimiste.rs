use anyhow::Result;
use clap::{Parser, ValueEnum};
use r2d2_cortex::agent::CognitiveAgent;
use r2d2_cortex::models::reasoning_agent::ReasoningAgent;
use r2d2_registry::{DatasetIdentity, DatasetManifest, DatasetMeta, TaskTypology};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum TrainingMode {
    CausalLm,
    ContrastiveEmbedding,
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "R2D2 - Alchimiste (Dataset Registry Compliant)"
)]
struct Args {
    #[arg(short, long, value_enum, default_value_t = TrainingMode::CausalLm)]
    mode: TrainingMode,

    #[arg(short, long, default_value = "genesis_v1")]
    dataset_name: String,
}

#[derive(serde::Deserialize, Debug)]
struct KnowledgeChunk {
    id: usize,
    content: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    let args = Args::parse();
    info!(
        "⚗️  [ALCHIMISTE] Forgerie du Dataset '{}' (Mode: {:?})...",
        args.dataset_name, args.mode
    );

    let workspace_path =
        PathBuf::from("/home/jgx/source/R2D2/workspace/datasets").join(&args.dataset_name);
    fs::create_dir_all(&workspace_path)?;
    let data_file_path = workspace_path.join("data.jsonl");
    let manifest_path = workspace_path.join("manifest.toml");

    let mut dataset_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&data_file_path)?;

    info!("🔌 [ALCHIMISTE] Booting ReasoningAgent natif...");
    let mut agent = ReasoningAgent::new();

    if let Err(e) = agent.load().await {
        error!("🚨 [ALCHIMISTE] Echec de l'Instanciation : {}", e);
        return Err(anyhow::anyhow!("Echec du démarrage natif."));
    }

    let meta_path = "knowledge_meta.json";
    let meta_data = match fs::read_to_string(meta_path) {
        Ok(data) => data,
        Err(_) => {
            warn!("⚠️ [ALCHIMISTE] Le Buffer RAG ({}) est vide.", meta_path);
            return Ok(());
        }
    };

    let chunks: Vec<KnowledgeChunk> = serde_json::from_str(&meta_data).unwrap_or_default();
    if chunks.is_empty() {
        return Ok(());
    }

    let mut samples_count = 0;
    let mut total_bytes = 0;

    for chunk in chunks {
        info!("=======================================================");
        info!("🧬 Synthèse du Chunk Sémantique ID#{}", chunk.id);

        match args.mode {
            TrainingMode::CausalLm => {
                let system_prompt = "Tu es R2D2-Alchimiste. À partir du bloc de texte fourni, extrais 2 ou 3 angles d'apprentissage utiles et génère une liste EXACTE au format JSON. Chaque objet doit contenir 'question' et 'réponse'. Par exemple: [{\"question\": \"...\", \"réponse\": \"...\"}]";
                let user_prompt = format!(
                    "Génère le JSON sur ce texte (Chunk {}):\n\n{}",
                    chunk.id, chunk.content
                );
                let merged = format!("System: {}\n\nUser: {}", system_prompt, user_prompt);

                match agent.generate_thought(&merged).await {
                    Ok(result) => {
                        let clean = result
                            .replace("```json", "")
                            .replace("```", "")
                            .trim()
                            .to_string();
                        if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&clean) {
                            for item in arr {
                                if let (Some(q), Some(r)) = (
                                    item.get("question").and_then(|v| v.as_str()),
                                    item.get("réponse").and_then(|v| v.as_str()),
                                ) {
                                    let wrapped_text = format!("[INST] {} [/INST] {}", q, r);
                                    let causal_entry = json!({
                                        "text": wrapped_text,
                                        "r2d2_cognitive_source": format!("RAG_CHUNK_{}_CAUSAL", chunk.id)
                                    });
                                    let entry_str = serde_json::to_string(&causal_entry)?;
                                    writeln!(dataset_file, "{}", entry_str)?;
                                    total_bytes += entry_str.len() as u64 + 1;
                                    samples_count += 1;
                                }
                            }
                            info!("✅ [ALCHIMISTE] Paires Causales forgées ajoutées au lot.");
                        } else {
                            warn!("⚠️ [ALCHIMISTE] Array JSON Invalide pour le Causal.");
                        }
                    }
                    Err(e) => error!("❌ [ALCHIMISTE] Erreur lors de l'inférence locale : {}", e),
                }
            }
            TrainingMode::ContrastiveEmbedding => {
                let system_prompt = "Tu es R2D2-Alchimiste. Ton rôle strict est de lire un texte brut et de recracher la donnée formatée en standard JSONAI V3.0 pur.\nFormat de sortie :\n{\n  \"metadata\": {\n    \"is_fact\": true/false,\n    \"belief_state\": 0.9,\n    \"ontology_links\": []\n  },\n  \"html_fragment\": \"...\",\n  \"semantic_vector_target\": \"Concept Absolu\"\n}";
                let user_prompt = format!(
                    "Génère DEUX objets respectant rigoureusement JSONAI V3.0 (dans un tableau JSON [ {{...}}, {{...}} ]) pour cet extrait, en variant l'angle d'attaque (ex: 1 abstrait, 1 technique):\n\n{}",
                    chunk.content
                );

                let merged = format!("System: {}\n\nUser: {}", system_prompt, user_prompt);

                match agent.generate_thought(&merged).await {
                    Ok(result) => {
                        let clean = result
                            .replace("```json", "")
                            .replace("```", "")
                            .trim()
                            .to_string();
                        if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&clean) {
                            for mut item in arr {
                                item["r2d2_cognitive_source"] =
                                    json!(format!("RAG_CHUNK_{}_CONTRAST", chunk.id));
                                let prompt_contexte = format!(
                                    "Analyse l'information RAG {} sous l'angle approprié.",
                                    chunk.id
                                );
                                let json_str = serde_json::to_string(&item)?;
                                let combined_entry = json!({
                                    "text": format!("[INST] {} [/INST] {}", prompt_contexte, json_str)
                                });
                                let entry_str = serde_json::to_string(&combined_entry)?;
                                writeln!(dataset_file, "{}", entry_str)?;
                                total_bytes += entry_str.len() as u64 + 1;
                                samples_count += 1;
                            }
                            dataset_file.flush()?;
                            info!("✅ [ALCHIMISTE] Vecteurs JSONAI Contrastifs forgés ajoutés.");
                        } else {
                            warn!("⚠️ [ALCHIMISTE] Array JSON Invalide pour le Contrastif.");
                        }
                    }
                    Err(e) => error!("❌ [ALCHIMISTE] Erreur lors de l'inférence locale : {}", e),
                }
            }
        }
    }

    info!("🏁 [ALCHIMISTE] Traitement des Traces terminé. Formatage du DatasetManifest...");

    let dataset_format = match args.mode {
        TrainingMode::CausalLm => TaskTypology::CausalLm,
        TrainingMode::ContrastiveEmbedding => TaskTypology::ContrastiveEmbedding,
    };

    let manifest = DatasetManifest {
        identity: DatasetIdentity {
            uuid: Uuid::new_v4(),
            name: args.dataset_name.clone(),
            version: "1.0.0".to_string(),
            author: Some("R2D2-Alchimiste".to_string()),
        },
        format: dataset_format,
        meta: DatasetMeta {
            size_bytes: total_bytes,
            samples_count,
            source_corpus: "knowledge_meta.json".to_string(),
        },
    };

    let manifest_toml = toml::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, manifest_toml)?;

    info!(
        "📦 [ALCHIMISTE] Dataset packagé avec succès : {:?}",
        workspace_path
    );

    let _ = agent.unload().await;
    Ok(())
}
