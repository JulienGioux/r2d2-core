use anyhow::Result;
use r2d2_cortex::agent::CognitiveAgent;
use r2d2_cortex::models::reasoning_agent::ReasoningAgent;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use tracing::{error, info, warn};

#[derive(serde::Deserialize, Debug)]
struct KnowledgeChunk {
    id: usize,
    content: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialisation du traçage industriel (Zero-Bloat)
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("⚗️  [ALCHIMISTE] Démarrage du processeur asynchrone (RAG -> JSONAI)...");

    // 1. Initialisation de notre Pipeline Inférentiel Existant (L'existant du Frontend)
    info!("🔌 [ALCHIMISTE] Booting ReasoningAgent natif...");
    let mut agent = ReasoningAgent::new();

    if let Err(e) = agent.load().await {
        error!(
            "🚨 [ALCHIMISTE] Echec de l'Instanciation de l'Agent LLM (GGUF/Native) : {}",
            e
        );
        return Err(anyhow::anyhow!("Echec du démarrage natif."));
    }

    // 2. Lecture du Buffer local (Le Cerveau RAG)
    let meta_path = "knowledge_meta.json";
    let meta_data = match fs::read_to_string(meta_path) {
        Ok(data) => data,
        Err(_) => {
            warn!(
                "⚠️ [ALCHIMISTE] Le Buffer RAG ({}) est vide ou introuvable.",
                meta_path
            );
            return Ok(());
        }
    };

    let chunks: Vec<KnowledgeChunk> = serde_json::from_str(&meta_data).unwrap_or_default();
    if chunks.is_empty() {
        info!("💤 [ALCHIMISTE] Aucune donnée à synthétiser. Fin du processus.");
        return Ok(());
    }

    info!(
        "📊 [ALCHIMISTE] Faisceau mémoriel scanné : {} blocs sémantiques.",
        chunks.len()
    );

    // Fichier Cible Final : Le Sang JSONAI pour la Forteresse
    fs::create_dir_all("datasets").unwrap_or_default();
    let dataset_path = "datasets/genesis_sft_v2.jsonl";
    let mut dataset_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dataset_path)?;

    // 3. Boucle d'Ingénierie Sémantique
    for chunk in chunks {
        info!("=======================================================");
        info!(
            "🧬 [ALCHIMISTE] Synthèse du Chunk Sémantique ID#{}",
            chunk.id
        );

        let system_prompt = "Tu es R2D2-Alchimiste. Ton rôle strict est de lire un texte brut et de recracher la donnée formatée en standard JSONAI V3.0 pur. \nFormat de sortie :\n{\n  \"metadata\": {\n    \"is_fact\": true/false,\n    \"belief_state\": 0.9,\n    \"spatial_context\": \"string\",\n    \"temporal_context\": \"string\",\n    \"ontology_links\": []\n  },\n  \"html_fragment\": \"<div id='hx-target' aria-live='polite'>...</div>\",\n  \"semantic_vector_target\": \"Concept Résumé Absolu\"\n}\nTu ne dois produire AUCUN texte avant ou après l'objet JSON.";

        let user_prompt = format!(
            "Applique le standard JSONAI V3.0 rigoureusement sur cet extrait brut de mémoire RAG :\n\n{}",
            chunk.content
        );

        let merged_prompt = format!("System: {}\n\nUser: {}", system_prompt, user_prompt);

        // Appel direct du Moteur Natif pour le formatage
        match agent.generate_thought(&merged_prompt).await {
            Ok(result) => {
                let clean_json = result
                    .replace("```json", "")
                    .replace("```", "")
                    .trim()
                    .to_string();

                // MOCK Check avant insertion RAG -> si ça parse en JSON, c'est bon.
                let validation: Result<serde_json::Value, _> = serde_json::from_str(&clean_json);
                match validation {
                    Ok(mut parsed) => {
                        // Injection du contexte métier et origin
                        parsed["r2d2_cognitive_source"] = json!(format!("RAG_CHUNK_{}", chunk.id));
                        writeln!(dataset_file, "{}", serde_json::to_string(&parsed)?)?;
                        dataset_file.flush()?;
                        info!(
                            "✅ [ALCHIMISTE] Vecteur JSONAI coulé avec succès (Taille: {} octets)",
                            clean_json.len()
                        );
                    }
                    Err(e) => {
                        warn!("⚠️ [ALCHIMISTE] Le Modèle n'a pas respecté le format JSON Strict : {:?}", e);
                        // Stratégie résiliente : on loggue mais on ne l'intègre pas dans notre dataset "air-gapped"
                    }
                }
            }
            Err(e) => {
                error!("❌ [ALCHIMISTE] Erreur lors de l'inférence locale : {}", e);
            }
        }
    }

    info!("🏁 [ALCHIMISTE] FIN ALCHIMIQUE. Dataset Genesis SFT_V2 forgé.");
    let _ = agent.unload().await;

    Ok(())
}
