use clap::Parser;
use r2d2_mcp_core::{DynamicToolResolver, McpTool};
use r2d2_vampire::core::consultant_store::ConsultantStore;
use r2d2_vampire::core::SuperMcpServer;
use r2d2_vampire::tools::expert_forge::ForgeExpertTool;
use r2d2_vampire::tools::expert_manager::{
    AddNotebookExpertTool, ListNotebookExpertsTool, RemoveNotebookExpertTool, ToggleCdpBridgeTool,
};
use r2d2_vampire::tools::notebook_lm::NotebookLmTool;
use r2d2_vampire::tools::sync_notebooks::SyncNotebooksTool;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Le mode d'interface du serveur ('json', 'ipc', 'both')
    #[arg(short, long, default_value = "json")]
    mode: String,

    /// Le chemin du socket Unix en mode IPC
    #[arg(short, long, default_value = "/tmp/r2d2-vampire.sock")]
    socket: String,
}

struct VampireToolResolver {
    store: Arc<ConsultantStore>,
}

#[async_trait::async_trait]
impl DynamicToolResolver for VampireToolResolver {
    async fn list_dynamic_tools(&self) -> Vec<Value> {
        let mut list = vec![];
        if let Ok(guard) = self.store.data.read() {
            for (name, expert_data) in guard.iter() {
                if expert_data.enabled {
                    if let Some(target_url) = &expert_data.url {
                        let tool = NotebookLmTool::new(name.clone(), target_url.clone());
                        list.push(json!({
                            "name": tool.name(),
                            "description": tool.description(),
                            "inputSchema": tool.input_schema()
                        }));
                    }
                }
            }
        }
        list
    }

    async fn call_dynamic_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Option<Result<Value, anyhow::Error>> {
        if let Some(expert_key) = name.strip_prefix("ask_") {
            let url_opt = {
                let guard = self.store.data.read().ok()?;
                let expert = guard.get(expert_key)?;
                if expert.enabled {
                    expert.url.clone()
                } else {
                    None
                }
            };

            if let Some(url) = url_opt {
                // Instanciation Just-In-Time du tool NotebookLM
                let tool = NotebookLmTool::new(expert_key.to_string(), url);
                return Some(tool.call(arguments).await);
            }
        }
        None
    }
}

#[tokio::main]
async fn main() {
    // CRITIQUE : En MCP, TOUT log doit aller sur STDERR, sinon le JSON STDOUT est corrompu.
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    let mut server = SuperMcpServer::new("r2d2-vampire", "1.0.0");

    // Instanciation de l'Arbre d'Etat (State Manager de la Flotte d'Experts)
    let store = ConsultantStore::new();

    // Enregistrement des outils statiques atomiques d'administration
    server.register_tool(std::sync::Arc::new(AddNotebookExpertTool {
        store: Arc::clone(&store),
    }));
    server.register_tool(std::sync::Arc::new(RemoveNotebookExpertTool {
        store: Arc::clone(&store),
    }));
    server.register_tool(std::sync::Arc::new(ListNotebookExpertsTool {
        store: Arc::clone(&store),
    }));
    server.register_tool(std::sync::Arc::new(ToggleCdpBridgeTool));
    server.register_tool(std::sync::Arc::new(ForgeExpertTool {
        store: Arc::clone(&store),
    }));
    server.register_tool(std::sync::Arc::new(SyncNotebooksTool::new(Arc::clone(
        &store,
    ))));
    server.register_tool(std::sync::Arc::new(
        r2d2_vampire::tools::purge_untitled::PurgeUntitledTool,
    ));

    // Instanciation DYNAMIQUE de la résolution Hot-Swap :
    // Au lieu d'enregistrer les outils en statique, l'orchestrateur demandera au resolver au call & list !
    server.dynamic_resolver = Some(Arc::new(VampireToolResolver {
        store: Arc::clone(&store),
    }));

    if let Ok(token_str) = std::env::var("GITHUB_TOKEN") {
        info!("🔑 Jeton GITHUB_TOKEN détecté (Zero-Trust).");
        let token =
            r2d2_vampire::tools::github::GithubToken(secrecy::SecretString::from(token_str));
        match r2d2_vampire::tools::github::ReqwestGithubClient::new(token) {
            Ok(client) => {
                let tool =
                    r2d2_vampire::tools::github::GithubTool::new(std::sync::Arc::new(client));
                server.register_tool(std::sync::Arc::new(tool));
            }
            Err(e) => tracing::error!("Impossible d'initialiser Github : {}", e),
        }
    } else {
        info!("ℹ️ Aucun jeton GITHUB_TOKEN configuré. Le Module Github ne sera pas chargé.");
    }

    let shared_server = Arc::new(server);
    let mut tasks = vec![];

    if args.mode == "json" || args.mode == "both" {
        let s1 = Arc::clone(&shared_server);
        tasks.push(tokio::spawn(async move {
            r2d2_vampire::core::stdio_pipeline::start_mcp_pipeline(s1).await;
        }));
    }

    if args.mode == "ipc" || args.mode == "both" {
        let s2 = Arc::clone(&shared_server);
        let path = args.socket.clone();
        tasks.push(tokio::spawn(async move {
            r2d2_vampire::core::ipc_pipeline::start_unix_socket(s2, &path).await;
        }));
    }

    // Await all pipelines
    for handle in tasks {
        let _ = handle.await;
    }
}
