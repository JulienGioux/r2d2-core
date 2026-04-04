use r2d2_mcp::client::McpUniversalClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{self};
use std::io::Write;
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct VampireJob {
    id: String,
    theme: String,
    notebook: String,
    vampire_status: String,
    #[serde(default)]
    provider: String,
}

fn atomic_update_job_status(job_id: &str, new_status: &str) {
    if let Ok(content) = fs::read_to_string("data/vampire_queue.json") {
        if let Ok(mut queue) = serde_json::from_str::<Vec<VampireJob>>(&content) {
            for job in queue.iter_mut() {
                if job.id == job_id {
                    job.vampire_status = new_status.to_string();
                }
            }
            if let Ok(json_str) = serde_json::to_string_pretty(&queue) {
                let _ = fs::write("data/vampire_queue.json", json_str);
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    info!("🦇 Réveil du VAMPIRE (Queue Manager Edition) 🦇");
    fs::create_dir_all("data").unwrap_or_default();

    let queue_path = "data/vampire_queue.json";
    let queue_content = match fs::read_to_string(queue_path) {
        Ok(c) => c,
        Err(_) => {
            warn!("⚠️ La File d'Attente est vide ou inexistante.");
            return Ok(());
        }
    };

    let queue: Vec<VampireJob> = serde_json::from_str(&queue_content).unwrap_or_default();
    let pending_jobs: Vec<VampireJob> = queue
        .into_iter()
        .filter(|j| j.vampire_status == "EN ATTENTE")
        .collect();

    if pending_jobs.is_empty() {
        info!("💤 Aucun Job en attente. Retour au sommeil intersidéral.");
        return Ok(());
    }

    info!(
        "📋 {} Fiche(s) d'Apprentissage planifiée(s).",
        pending_jobs.len()
    );

    let mut dataset_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("data/synthetic_dataset.jsonl")?;

    for (i, job) in pending_jobs.iter().enumerate() {
        info!("=======================================================");
        info!(
            "🩸 MISSION [{}/{}] : Scrapping '{}' via [{}]",
            i + 1,
            pending_jobs.len(),
            job.theme,
            job.provider
        );

        // Spawn le MCP correct selon le provider
        let mcp_name = if job.provider == "github" {
            "@modelcontextprotocol/server-github"
        } else {
            "notebooklm-mcp"
        };
        let cmd_string = format!("npx -y {}", mcp_name);

        let (cmd, args) = if cfg!(target_os = "windows") {
            ("cmd", vec!["/c", cmd_string.as_str()])
        } else {
            ("bash", vec!["-c", cmd_string.as_str()])
        };

        info!("🔌 Démarrage du sous-processus `{}`...", mcp_name);
        let env_vars = r2d2_cortex::security::vault::Vault::get_runtime_env(&job.provider);
        let mut mcp_client = match McpUniversalClient::spawn(cmd, &args, Some(env_vars)).await {
            Ok(c) => c,
            Err(e) => {
                error!("❌ Impossible de lancer MCP {}: {}", mcp_name, e);
                atomic_update_job_status(&job.id, "ERREUR");
                continue;
            }
        };

        match mcp_client.initialize().await {
            Ok(init_res) => {
                info!("✅ MCP Initialisé : {:?}", init_res);
            }
            Err(e) => {
                error!(
                    "❌ Impossible d'initialiser MCP {} (Problème de token ou crash serveur): {}",
                    mcp_name, e
                );
                atomic_update_job_status(&job.id, "ERREUR");
                continue;
            }
        };

        let (tool_name, mcp_args, system_prompt) = if job.provider == "github" {
            let repo = &job.notebook; // UI stores repo link in notebook field
            (
                "search_code",
                json!({ "q": format!("repo:{}", repo) }),
                "You are an expert GitHub repository analyzer extracting key codebase structures into synthetic documentation."
            )
        } else {
            (
                "ask_question",
                json!({
                    "question": format!("Génère une fiche d'apprentissage exhaustive et très détaillée sous forme de Quiz (au moins 10 questions/réponses avancées) sur le thème suivant : '{}'. Base-toi EXCLUSIVEMENT sur tes sources. Sépare bien chaque question et réponse.", job.theme),
                    "notebook_id": job.notebook
                }),
                "You are a specialized agent providing quizzes based on the NotebookLM knowledge representation."
            )
        };

        match mcp_client.call_tool(tool_name, mcp_args).await {
            Ok(response) => {
                let mut synthesis = String::new();
                if let Some(content_array) = response.get("content").and_then(|c| c.as_array()) {
                    for item in content_array {
                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                            synthesis.push_str(text);
                            synthesis.push('\n');
                        }
                    }
                } else if let Some(text) = response.as_str() {
                    synthesis = text.to_string();
                }

                if synthesis.is_empty() {
                    synthesis = format!("{:?}", response);
                }

                let entry = json!({
                    "mission_id": job.id,
                    "theme": job.theme,
                    "notebook": job.notebook,
                    "provider": job.provider,
                    "messages": [
                        { "role": "system", "content": system_prompt },
                        { "role": "user", "content": format!("Analyse ce corpus : {}", job.theme) },
                        { "role": "assistant", "content": synthesis.trim() }
                    ]
                });

                writeln!(dataset_file, "{}", serde_json::to_string(&entry)?)?;
                dataset_file.flush()?;

                info!(
                    "💾 Connaissance extraite et stockée (Taille: {} bytes) !",
                    synthesis.len()
                );
                atomic_update_job_status(&job.id, "VAMPIRISÉ");
            }
            Err(e) => {
                error!(
                    "❌ Erreur d'Extraction MCP pour le job {} : {:?}",
                    job.theme, e
                );
                atomic_update_job_status(&job.id, "ERREUR");
            }
        }
        let _ = mcp_client.shutdown().await;

        if i + 1 < pending_jobs.len() {
            info!("⏳ Repos digestif (60s) pour contourner le Rate-Limit...");
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    }

    info!("🏁 CAMPAGNE DE VAMPIRISATION TERMINÉE ! La Connaissance est en nous.");

    Ok(())
}
