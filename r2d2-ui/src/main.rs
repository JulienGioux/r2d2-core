use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::{delete, get, post},
    Form, Router,
};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use async_stream::stream;
use axum::response::sse::{Event, Sse};
use r2d2_blackboard::PostgresBlackboard;
use r2d2_circadian::sensory::VibeVector;
use r2d2_circadian::CircadianDaemon;
use r2d2_cortex::agent::CognitiveAgent;
use r2d2_cortex::models::reasoning_agent::DebateEvent;
use r2d2_cortex::models::reasoning_agent::ReasoningAgent;
use r2d2_cortex::CortexRegistry;
use r2d2_paradox::ParadoxSolver;
use std::convert::Infallible;
use std::time::Duration;
use std::time::Instant;
use sysinfo::System;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

mod chat_history;
mod logger;

#[derive(Clone)]
struct AppState {
    agent: Arc<Mutex<ReasoningAgent>>,
    pending_debate_prompt: Arc<Mutex<Option<String>>>,
    current_chat_session: Arc<Mutex<String>>,
    log_tx: tokio::sync::broadcast::Sender<String>,
    sys: Arc<Mutex<System>>,
    start_time: Instant,
    circadian_rx: tokio::sync::watch::Receiver<Arc<VibeVector>>,
    blackboard: Arc<PostgresBlackboard>,
    hf_models_cache: Arc<tokio::sync::RwLock<Vec<String>>>,
    chat_status: Arc<tokio::sync::RwLock<String>>,
}

#[derive(Template)]
#[template(path = "base.html")]
struct IndexTemplate {
    status: &'static str,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    pub total_ram_gb: String,
    pub used_ram_gb: String,
    pub ram_percent: i32,
    #[allow(dead_code)]
    pub active_agents_count: usize,
    pub memory_vectors_count: usize,
    pub alert_threshold: bool,
    pub uptime_formatted: String,
    #[allow(dead_code)]
    pub core_count: usize,
    #[allow(dead_code)]
    #[allow(dead_code)]
    pub entropy: f32,
    pub dissonance: f32,
    pub tension: f32,
}

#[derive(Template)]
#[template(path = "cortex.html")]
struct CortexTemplate {}

#[derive(Template)]
#[template(path = "chat.html")]
struct ChatTemplate {
    history_html: String,
    #[allow(dead_code)]
    github_configured: bool,
    #[allow(dead_code)]
    active_sources_html: String,
    library_repos: Vec<String>,
}

#[derive(Template)]
#[template(path = "memory.html")]
struct MemoryTemplate {
    #[allow(dead_code)]
    pub total_vectors: usize,
    pub sample_axioms: Vec<(usize, String)>,
}

pub struct KeyView {
    pub provider: String,
    pub masked_value: String,
    pub is_set: bool,
}

#[derive(Template)]
#[template(path = "admin.html")]
struct AdminTemplate {
    keys: Vec<KeyView>,
}

pub struct McpToolView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub command: String,
    pub is_enabled: bool,
}

pub struct CuratedModel {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Template)]
#[template(path = "store.html")]
struct StoreTemplate {
    local_models: Vec<(String, String)>,
    mcp_tools: Vec<McpToolView>,
    curated_models: Vec<CuratedModel>,
    db_error: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ModelRoleMapping {
    pub roles: std::collections::HashMap<String, String>,
}

pub async fn read_model_roles() -> ModelRoleMapping {
    if let Ok(data) = tokio::fs::read_to_string("data/models.json").await {
        if let Ok(map) = serde_json::from_str(&data) {
            return map;
        }
    }
    ModelRoleMapping {
        roles: std::collections::HashMap::new(),
    }
}

pub async fn write_model_roles(mapping: &ModelRoleMapping) {
    if let Ok(data) = serde_json::to_string_pretty(mapping) {
        let _ = tokio::fs::create_dir_all("data").await;
        let _ = tokio::fs::write("data/models.json", data).await;
    }
}

pub async fn list_local_hf_models() -> Vec<String> {
    tokio::task::spawn_blocking(|| {
        let mut local = vec![];
        if let Some(home) = dirs::home_dir() {
            let hf_hub = home.join(".cache/huggingface/hub");
            if let Ok(entries) = std::fs::read_dir(hf_hub) {
                for entry in entries.flatten() {
                    if let Ok(name) = entry.file_name().into_string() {
                        if name.starts_with("models--") {
                            local.push(name.replace("models--", "").replace("--", "/"));
                        }
                    }
                }
            }
        }
        local.sort();
        if local.is_empty() {
            local.push("Aucun modèle détecté dans ~/.cache/huggingface".to_string());
        }
        local
    })
    .await
    .unwrap_or_else(|_| vec!["(Erreur lors du scan du cache local HF)".to_string()])
}

#[derive(Template)]
#[template(
    source = r#"
<div class="message user">
    <div class="message-avatar"><i data-lucide="user" style="width: 18px;"></i></div>
    <div class="message-content">
        <div class="message-sender">User</div>
        <div class="message-bubble">{{ prompt|safe }}</div>
    </div>
</div>
{{ mcp_feedback|safe }}
<div class="message system">
    <div class="message-avatar"><i data-lucide="brain-circuit" style="width: 18px;"></i></div>
    <div class="message-content" style="width: 100%;">
        <div class="message-sender" style="justify-content: space-between; width: 100%;">
            <span style="color: var(--accent-brand);">{{ model_name }}</span>
            <div class="view-toggles" style="display: flex; gap: 4px;">
                <button type="button" class="btn-icon-small" onclick="const p=this.parentElement.parentElement.parentElement; p.querySelectorAll('.tab-content').forEach(el=>el.style.display='none'); p.querySelectorAll('.tab-content')[0].style.display='block'; this.parentElement.querySelectorAll('button').forEach(b=>b.style.background='transparent'); this.style.background='var(--bg-active)';" style="background: var(--bg-active);">MD</button>
                <button type="button" class="btn-icon-small" onclick="const p=this.parentElement.parentElement.parentElement; p.querySelectorAll('.tab-content').forEach(el=>el.style.display='none'); p.querySelectorAll('.tab-content')[1].style.display='block'; this.parentElement.querySelectorAll('button').forEach(b=>b.style.background='transparent'); this.style.background='var(--bg-active)';">JSON</button>
                <button type="button" class="btn-icon-small" onclick="const p=this.parentElement.parentElement.parentElement; p.querySelectorAll('.tab-content').forEach(el=>el.style.display='none'); p.querySelectorAll('.tab-content')[2].style.display='block'; this.parentElement.querySelectorAll('button').forEach(b=>b.style.background='transparent'); this.style.background='var(--bg-active)';">Trace</button>
            </div>
        </div>
        
        <div class="message-bubble tab-content" style="display: block; width: 100%;">
            <div class="markdown-body">{{ response_md|safe }}</div>
        </div>
        <div class="message-bubble tab-content" style="display: none; width: 100%;">
            <pre style="margin:0;">{{ jsonai|safe }}</pre>
        </div>
        <div class="message-bubble tab-content" style="display: none; font-family: var(--font-mono); font-size: 12px; width: 100%;">
            <div style="margin-bottom:8px; color: var(--text-tertiary);">⌚ Délai: <span style="color: var(--text-primary);">{{ latency }} ms</span></div>
            <div style="margin-bottom:8px; color: var(--text-tertiary);">🏭 Provider: <span style="color: var(--text-primary);">{{ model_name }}</span></div>
            <div style="color: var(--text-tertiary);">🫂 Consensus: <span style="color: var(--text-primary);">{{ consensus }}</span></div>
        </div>
    </div>
</div>
<script>if (typeof lucide !== 'undefined') { lucide.createIcons(); }</script>
"#,
    ext = "html"
)]
struct ChatResponseTemplate {
    mcp_feedback: String,
    prompt: String,
    model_name: String,
    response_md: String,
    jsonai: String,
    latency: String,
    consensus: String,
}

#[derive(Deserialize)]
struct ChatInput {
    provider: String,
    prompt: String,
    #[serde(default)]
    github_sources: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (log_tx, _) = tokio::sync::broadcast::channel::<String>(250);
    let broadcast_layer = logger::BroadcastLayer {
        sender: log_tx.clone(),
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .with(broadcast_layer)
        .init();

    tracing::info!("🚀 Booting R2D2-UI (Axum/HTMX Administration Console)...");

    // 1. Initialisation Physique du Cortex pour le Chat
    let mut reasoning_agent = ReasoningAgent::new();
    reasoning_agent
        .load()
        .await
        .expect("Erreur Fatale lors de l'allocation Tensorielle du Cortex Local");

    tracing::info!("🔌 Initialisation planifiée du Hub global MCP (Model Context Protocol)...");
    let mut envs = std::collections::HashMap::new();
    if let Ok(github_token) = std::env::var("GITHUB_PERSONAL_ACCESS_TOKEN") {
        if !github_token.is_empty() {
            envs.insert("GITHUB_PERSONAL_ACCESS_TOKEN".to_string(), github_token);
        }
    } else {
        // Fallback or via vault
        let vault_token =
            r2d2_cortex::security::vault::Vault::get_api_key("GITHUB_PERSONAL_ACCESS_TOKEN")
                .unwrap_or_default();
        if !vault_token.is_empty() {
            envs.insert("GITHUB_PERSONAL_ACCESS_TOKEN".to_string(), vault_token);
        }
    }

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://r2d2_admin:secure_r2d2_password_local@localhost:5433/r2d2_blackboard"
            .to_string()
    });
    let blackboard = Arc::new(PostgresBlackboard::new(&db_url).await?);

    // FIX: S'assurer que les tables (mcp_registry, model_registry) existent avant toute requête.
    if let Err(e) = blackboard.initialize_registry_tables().await {
        tracing::error!(
            "❌ Échec critique lors de l'initialisation du registre: {}",
            e
        );
    }

    match blackboard.get_all_mcp_tools().await {
        Ok(tools) => {
            let mut configs = Vec::new();
            for tool in tools {
                if tool.is_enabled {
                    let args_vec: Vec<String> =
                        serde_json::from_str(&tool.args_json).unwrap_or_default();
                    configs.push(r2d2_cortex::mcp_hub::McpServerConfig {
                        name: tool.name,
                        command: tool.command,
                        args: args_vec,
                        envs: envs.clone(),
                    });
                }
            }
            match r2d2_cortex::mcp_hub::McpHub::new(configs).await {
                Ok(hub) => {
                    let mcp_hub = std::sync::Arc::new(tokio::sync::Mutex::new(Some(hub)));
                    reasoning_agent.mcp_hub = mcp_hub;
                    tracing::info!(
                        "✅ MCP Hub instancié avec succès et rattaché au ReasoningAgent."
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "❌ Échec critique lors de l'instanciation de l'écosystème MCP : {}",
                        e
                    );
                }
            }
        }
        Err(e) => {
            tracing::error!(
                "❌ [DB-ERROR] Impossible de récupérer les outils MCP depuis le Blackboard : {:?}",
                e
            );
        }
    }

    // 2. Initialisation du Démon Circadien (L'Hyperviseur Background)
    tracing::info!("Démarrage de l'Hyperviseur R2D2 en arrière-plan...");
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://r2d2_admin:secure_r2d2_password_local@localhost:5433/r2d2_blackboard"
            .to_string()
    });
    let blackboard = Arc::new(PostgresBlackboard::new(&db_url).await?);

    // On réutilise ParadoxSolver et CortexRegistry global pour le Démon
    let mut reflex_judge =
        r2d2_paradox::reflex_judge::ReflexJudge::new().with_blackboard(blackboard.clone());
    let _ = reflex_judge.initialize().await;
    let reflex = Arc::new(tokio::sync::Mutex::new(reflex_judge));
    let solver = Arc::new(ParadoxSolver::new().with_reflex(reflex));
    let global_cortex = Arc::new(CortexRegistry::new());

    // Le Démon Circadien (Seuil = 0.85, 30sec = interval)
    let (mut daemon, circadian_rx) = CircadianDaemon::new(
        0.85,
        30,
        blackboard.clone(),
        global_cortex.clone(),
        solver.clone(),
    );
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // L'Orchestration Résiliente "Industrial-Grade" avec Exponential Backoff
    let daemon_blackboard = blackboard.clone();
    let daemon_cortex = global_cortex.clone();
    let daemon_solver = solver.clone();

    tokio::spawn(async move {
        let mut backoff = Duration::from_secs(1);
        loop {
            // Un canal fresh par daemon s'il crash
            let (fresh_daemon, _) = CircadianDaemon::new(
                0.85,
                30,
                daemon_blackboard.clone(),
                daemon_cortex.clone(),
                daemon_solver.clone(),
            );

            let handle = tokio::spawn({
                let rx = shutdown_rx.clone();
                async move { daemon.start_homeostasis_loop(rx).await }
            });

            match handle.await {
                Ok(Ok(_)) => break, // Normal Shutdown
                Ok(Err(e)) => tracing::error!("Le Démon Circadien a échoué: {}", e),
                Err(e) if e.is_panic() => {
                    tracing::error!(
                        "🚨 LE DÉMON CIRCADIEN A PANIQUÉ ! Auto-Réanimation en cours..."
                    );
                }
                Err(_) => tracing::error!("Tâche de supervision annulée brutalement."),
            }

            // On a paniqué, on reset le daemon principal et on backoff
            daemon = fresh_daemon;
            tokio::time::sleep(backoff).await;
            backoff = std::cmp::min(backoff * 2, Duration::from_secs(60));
        }
    });

    let mut sys = System::new_all();
    sys.refresh_all();

    let shared_state = AppState {
        agent: Arc::new(Mutex::new(reasoning_agent)),
        pending_debate_prompt: Arc::new(Mutex::new(None)),
        current_chat_session: Arc::new(Mutex::new(uuid::Uuid::new_v4().to_string())),
        log_tx: log_tx.clone(),
        sys: Arc::new(Mutex::new(sys)),
        start_time: Instant::now(),
        circadian_rx,
        blackboard: blackboard.clone(),
        hf_models_cache: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        chat_status: Arc::new(tokio::sync::RwLock::new(String::new())),
    };

    let cache_clone = shared_state.hf_models_cache.clone();
    tokio::spawn(async move {
        let models = list_local_hf_models().await;
        *cache_clone.write().await = models;
    });

    let api_routes = Router::new()
        .route("/widgets/status", get(get_status_widget))
        .route("/chat", post(handle_chat))
        .route("/chat/status", get(get_chat_status))
        .route("/chat/history/list", get(get_history_list))
        .route("/chat/history/:id", get(load_chat_history))
        .route("/chat/history/:id", delete(delete_chat_session))
        .route("/chat/history/:id/rename", post(rename_chat_session))
        .route("/chat/history/:id/pin", post(pin_chat_session))
        .route("/chat/history/:id/info", get(info_chat_session))
        .route("/chat/new", post(new_chat_session))
        .route("/chat/stream", get(handle_sse_stream))
        .route("/system/purge", post(system_purge))
        .route("/admin/vampire/queue/list", get(list_vampire_jobs))
        .route("/admin/vampire/queue/add", post(add_vampire_job))
        .route("/admin/vampire/queue/delete/:id", post(delete_vampire_job))
        .route("/admin/vampire/start", post(start_vampire))
        .route("/admin/assimilate", post(start_assimilation_all))
        .route("/admin/assimilate/:id", post(start_assimilation_id))
        .route("/admin/system/logs", get(stream_system_logs))
        .route("/admin/keys", post(set_admin_keys))
        .route("/chat/context/attach", post(attach_context))
        .route("/chat/context/remove", post(remove_context))
        .route("/mcp/add", post(add_mcp_tool))
        .route("/mcp/toggle/:id", post(toggle_mcp_tool))
        .route("/mcp/delete/:id", delete(delete_mcp_tool))
        .route("/store/download", post(store_download_model))
        .route("/store/scan", post(store_scan_hf))
        .route("/store/assign_role", post(assign_model_role))
        .route("/store/delete", post(delete_local_hf_model));

    let app = Router::new()
        .route("/", get(render_index))
        .route("/ui/dashboard", get(render_dashboard))
        .route("/ui/chat", get(render_chat))
        .route("/ui/cortex", get(render_cortex))
        .route("/ui/memory", get(render_memory))
        .route("/ui/store", get(render_store))
        .route("/ui/admin", get(render_admin))
        .nest("/api", api_routes)
        .nest_service(
            "/static",
            ServeDir::new(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static")),
        )
        .with_state(shared_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("📡 Server listening on http://0.0.0.0:3000");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn render_index() -> impl IntoResponse {
    let tmpl = IndexTemplate {
        status: "ACTIF (Air-Gapped)",
    };
    Html(tmpl.render().unwrap())
}

async fn render_dashboard(State(state): State<AppState>) -> impl IntoResponse {
    let mut sys = state.sys.lock().await;
    sys.refresh_memory();

    let total_ram = sys.total_memory() as f64 / 1_073_741_824.0;
    let used_ram = sys.used_memory() as f64 / 1_073_741_824.0;
    let ram_percent = if total_ram > 0.0 {
        (used_ram / total_ram * 100.0) as i32
    } else {
        0
    };
    let alert_threshold = ram_percent > 85;

    let core_count = sys.physical_core_count().unwrap_or(4);

    let uptime_secs = state.start_time.elapsed().as_secs();
    let uptime_formatted = format!(
        "{:02}:{:02}:{:02}",
        uptime_secs / 3600,
        (uptime_secs % 3600) / 60,
        uptime_secs % 60
    );

    let agent = state.agent.lock().await;
    let memory_vectors_count = agent.memory_vectors_count();
    let active_agents_count = if agent.is_active() { 3 } else { 0 };

    let vibe = state.circadian_rx.borrow().clone();

    Html(
        DashboardTemplate {
            total_ram_gb: format!("{:.1}", total_ram),
            used_ram_gb: format!("{:.1}", used_ram),
            ram_percent,
            active_agents_count,
            memory_vectors_count,
            alert_threshold,
            uptime_formatted,
            core_count,
            entropy: vibe.compute_entropy(),
            dissonance: vibe.dissonance,
            tension: vibe.tension,
        }
        .render()
        .unwrap(),
    )
}

async fn render_chat(State(state): State<AppState>) -> impl IntoResponse {
    let session_id = state.current_chat_session.lock().await.clone();
    let mut history_html = String::new();
    let mut active_sources_html = String::new();

    if let Some(session) = chat_history::load_session(&session_id) {
        // Generate existing attachment pills
        for repo in &session.github_sources {
            if repo.starts_with("github-async://") {
                let id_val = format!("github-async-{}", uuid::Uuid::new_v4().simple());
                let r = repo.replace("github-async://", "");
                active_sources_html.push_str(&format!(
                    "<div id=\"{}\" class=\"context-pill context-pill-async\">\
                        <i data-lucide=\"github\" class=\"icon-type\" style=\"width: 14px; height: 14px;\"></i>\
                        <span>{} (Async Queued)</span>\
                        <button type=\"button\" hx-post=\"/api/chat/context/remove\" hx-vals='{{\"repo\": \"{}\"}}' hx-target=\"#{}\" hx-swap=\"delete\">\
                            <i data-lucide=\"x\" style=\"width: 12px; height: 12px;\"></i>\
                        </button>\
                        <input type=\"hidden\" name=\"github_source_item\" value=\"{}\">\
                    </div>", id_val, r, repo, id_val, repo));
            } else {
                let id_val = format!("github-otf-{}", uuid::Uuid::new_v4().simple());
                active_sources_html.push_str(&format!(
                    "<div id=\"{}\" class=\"context-pill context-pill-otf\">\
                        <i data-lucide=\"github\" class=\"icon-type\" style=\"width: 14px; height: 14px;\"></i>\
                        <span>{} (Cognitive Tool)</span>\
                        <button type=\"button\" hx-post=\"/api/chat/context/remove\" hx-vals='{{\"repo\": \"{}\"}}' hx-target=\"#{}\" hx-swap=\"delete\">\
                            <i data-lucide=\"x\" style=\"width: 12px; height: 12px;\"></i>\
                        </button>\
                        <input type=\"hidden\" name=\"github_source_item\" value=\"{}\">\
                    </div>", id_val, repo, repo, id_val, repo));
            }
        }

        let mut i = 0;
        while i < session.turns.len() {
            let user_turn = &session.turns[i];
            if user_turn.role == "user" && i + 1 < session.turns.len() {
                let assistant_turn = &session.turns[i + 1];
                let prompt = user_turn.content.clone();
                let json_resp = assistant_turn.content.clone();

                let parsed: serde_json::Value = serde_json::from_str(&json_resp).unwrap_or(serde_json::json!({ "content": json_resp, "source": {"ParadoxEngine": "Unknown"} }));
                let raw_text = parsed["content"].as_str().unwrap_or(&json_resp).to_string();
                let model_name = parsed["source"]["ParadoxEngine"]
                    .as_str()
                    .unwrap_or("Paradox Local")
                    .to_string();
                let consensus = parsed["consensus"]
                    .as_str()
                    .unwrap_or("Unknown")
                    .to_string();
                let latency = parsed["id"]
                    .as_str()
                    .unwrap_or("")
                    .replace("paradox-multiapi-", "");

                let parser = pulldown_cmark::Parser::new(&raw_text);
                let mut html_output = String::new();
                pulldown_cmark::html::push_html(&mut html_output, parser);

                history_html.push_str(
                    &ChatResponseTemplate {
                        mcp_feedback: String::new(),
                        prompt,
                        model_name,
                        response_md: html_output,
                        jsonai: json_resp,
                        latency,
                        consensus,
                    }
                    .render()
                    .unwrap(),
                );
                i += 2;
            } else {
                i += 1;
            }
        }
    }
    if history_html.is_empty() {
        history_html = "<div class='message system'><i data-lucide='terminal'></i><div class='message-content'><strong>System Initialize</strong><p>Bienvenue Chef. Le Cortex est en écoute de stimuli.</p></div></div>".into();
    }
    let github_configured =
        r2d2_cortex::security::vault::Vault::get_api_key("GITHUB_PERSONAL_ACCESS_TOKEN").is_some();
    let queue = read_queue();
    let mut final_library_repos: Vec<String> = queue
        .into_iter()
        .map(|v| v.notebook)
        .filter(|n| n.contains("/"))
        .collect();

    // Hardcoded permanent repository for the project core architecture
    final_library_repos.push("JulienGioux/r2d2-core".to_string());

    final_library_repos.sort();
    final_library_repos.dedup();

    Html(
        ChatTemplate {
            history_html,
            github_configured,
            active_sources_html,
            library_repos: final_library_repos,
        }
        .render()
        .unwrap(),
    )
}
async fn render_cortex() -> impl IntoResponse {
    Html(CortexTemplate {}.render().unwrap())
}

async fn render_memory(State(state): State<AppState>) -> impl IntoResponse {
    let agent = state.agent.lock().await;
    let (total_vectors, sample_axioms) = if let Some(mem) = &agent.memory {
        (mem.len(), mem.get_sample_axioms(6))
    } else {
        (0, vec![])
    };

    Html(
        MemoryTemplate {
            total_vectors,
            sample_axioms,
        }
        .render()
        .unwrap(),
    )
}

async fn render_store(State(state): State<AppState>) -> impl IntoResponse {
    let t0 = std::time::Instant::now();
    let cached_models = state.hf_models_cache.read().await.clone();
    let t1 = std::time::Instant::now();
    tracing::info!(
        "⏱️ [STORE TELEMETRY] Lecture du Cache HD Models (RwLock): {:?}",
        t1.duration_since(t0)
    );

    let mappings = read_model_roles().await;
    let t2 = std::time::Instant::now();
    tracing::info!(
        "⏱️ [STORE TELEMETRY] Lecture Asynchrone (tokio::fs) models.json: {:?}",
        t2.duration_since(t1)
    );

    let mut local_models = vec![];
    for m in cached_models {
        let role = mappings
            .roles
            .get(&m)
            .cloned()
            .unwrap_or_else(|| "none".to_string());
        local_models.push((m, role));
    }

    let mut mcp_tools = vec![];
    let mut db_error = None;

    let t3 = std::time::Instant::now();
    match state.blackboard.get_all_mcp_tools().await {
        Ok(tools) => {
            for t in tools {
                mcp_tools.push(McpToolView {
                    id: t.id,
                    name: t.name,
                    description: "Serveur MCP distant configuré via Blackboard".to_string(),
                    command: format!("{} {}", t.command, t.args_json),
                    is_enabled: t.is_enabled,
                });
            }
        }
        Err(e) => {
            tracing::error!("Erreur accès Blackboard (MCP_Tools): {}", e);
            db_error = Some("La base de données PostgreSQL est actuellement inaccessible. Vérifiez que votre conteneur est lancé et disponible.".to_string());
        }
    }
    let t4 = std::time::Instant::now();
    tracing::info!(
        "⏱️ [STORE TELEMETRY] Requête PostgresBlackboard MCP_Tools: {:?}",
        t4.duration_since(t3)
    );

    let curated_models = vec![
        CuratedModel {
            id: "microsoft/Phi-3-mini-4k-instruct".into(),
            name: "Phi-3 Mini (4K)".into(),
            description: "Modèle compact ultra-performant idéal pour le code local.".into(),
        },
        CuratedModel {
            id: "deepseek-ai/deepseek-coder-1.3b-instruct".into(),
            name: "DeepSeek Coder 1.3B".into(),
            description: "Modèle de génération de code optimisé pour R2D2.".into(),
        },
        CuratedModel {
            id: "nomic-ai/nomic-embed-text-v1.5".into(),
            name: "Nomic Embed Text v1.5".into(),
            description: "Modèle Embedding Vectoriel Multilingue (8192 context).".into(),
        },
    ];

    let html_res = Html(
        StoreTemplate {
            local_models,
            mcp_tools,
            curated_models,
            db_error,
        }
        .render()
        .unwrap(),
    );

    let t5 = std::time::Instant::now();
    tracing::info!(
        "⏱️ [STORE TELEMETRY] Rendu Askama Template (html): {:?}",
        t5.duration_since(t4)
    );
    tracing::info!(
        "⏱️ [STORE TELEMETRY] TEMPS TOTAL: {:?}",
        t5.duration_since(t0)
    );

    html_res
}

#[derive(serde::Deserialize)]
struct AssignRolePayload {
    model: String,
    role: String,
}

async fn assign_model_role(Form(payload): Form<AssignRolePayload>) -> impl IntoResponse {
    let mut mapping = read_model_roles().await;
    if payload.role == "none" {
        mapping.roles.remove(&payload.model);
    } else {
        mapping
            .roles
            .insert(payload.model.clone(), payload.role.clone());
    }
    write_model_roles(&mapping).await;

    let role_display = match payload.role.as_str() {
        "reasoning" => "Main Reasoning",
        "vision" => "Vision Engine",
        "code" => "Code Assistant",
        _ => "Désactivé",
    };

    let color = if payload.role == "none" {
        "var(--text-tertiary)"
    } else {
        "var(--status-success)"
    };
    let icon = if payload.role == "none" {
        "power-off"
    } else {
        "check-circle-2"
    };

    Html(format!(
        "<div style='color: {}; font-size: 11px; margin-top: 4px;'><i data-lucide='{}' style='width: 12px; vertical-align: middle;'></i> Assigné : {}</div><script>lucide.createIcons();</script>",
        color, icon, role_display
    ))
}

async fn store_scan_hf(State(state): State<AppState>) -> impl IntoResponse {
    let cache_clone = state.hf_models_cache.clone();
    tokio::spawn(async move {
        let models = list_local_hf_models().await;
        *cache_clone.write().await = models;
    });
    Html("<div class='badge info' style='margin-top: 8px; margin-left: 12px; position: absolute; right: 260px; top: -4px;'><i data-lucide='loader-2' class='spin' style='width:14px'></i> Analyse de la matrice...</div><script>lucide.createIcons(); setTimeout(()=> htmx.ajax('GET', '/ui/store', {target: '#main-content', swap: 'innerHTML'}), 2000);</script>".to_string())
}

#[derive(serde::Deserialize)]
struct DeleteModelPayload {
    model: String,
}

async fn delete_local_hf_model(
    State(state): State<AppState>,
    Form(payload): Form<DeleteModelPayload>,
) -> impl IntoResponse {
    let model_name = payload.model;
    let cache_clone = state.hf_models_cache.clone();

    // Lancement de la suppression lourde et du rescan 100% en tâche de fond pour ne jamais bloquer l'UI
    tokio::spawn(async move {
        if !model_name.contains("..") && !model_name.is_empty() {
            let target_dir = format!("models--{}", model_name.replace("/", "--"));
            // 1. Suppression Physique
            let _ = tokio::task::spawn_blocking(move || {
                if let Some(home) = dirs::home_dir() {
                    let hf_hub = home.join(".cache/huggingface/hub");
                    let model_path = hf_hub.join(target_dir);
                    let _ = std::fs::remove_dir_all(&model_path);
                }
            })
            .await;

            // 2. Refresh du Cache
            let models = list_local_hf_models().await;
            *cache_clone.write().await = models;
        }
    });

    // On notifie l'utilisateur instantanément que l'opération est relayée au démon.
    Html("<div class='badge warning' style='margin-top:8px; width:100%; border-color: rgba(245,158,11,0.2) !important;'><i data-lucide='cpu' style='width:14px; margin-right:6px;'></i> Désinstallation programmée. Rafraîchissez dans quelques instants.</div><script>lucide.createIcons(); setTimeout(()=> htmx.ajax('GET', '/ui/store', {target: '#main-content', swap: 'innerHTML'}), 5000);</script>".to_string())
}

#[derive(serde::Deserialize)]
struct AddMcpPayload {
    name: String,
    command: String,
    args_json: String,
}

async fn add_mcp_tool(
    State(state): State<AppState>,
    Form(payload): Form<AddMcpPayload>,
) -> impl IntoResponse {
    let _ = state
        .blackboard
        .add_mcp_tool(&payload.name, &payload.command, &payload.args_json)
        .await;
    let success_msg = format!("<div class='mcp-card' style='border-color: var(--status-success);'><i data-lucide='check' style='color:var(--status-success)'></i> Configuré : {}</div><script>lucide.createIcons(); setTimeout(()=> htmx.ajax('GET', '/ui/store', {{target: '#main-content', swap: 'innerHTML'}}), 1500);</script>", payload.name);
    Html(success_msg)
}

async fn toggle_mcp_tool(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    // On inverse l'état existant
    if let Ok(tools) = state.blackboard.get_all_mcp_tools().await {
        if let Some(tool) = tools.iter().find(|t| t.id == id) {
            let _ = state
                .blackboard
                .enable_mcp_tool(&id, !tool.is_enabled)
                .await;
        }
    }
    Html(r#"<script>htmx.ajax('GET', '/ui/store', {target: '#main-content', swap: 'innerHTML'});</script>"#.to_string())
}

async fn delete_mcp_tool(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let _ = state.blackboard.delete_mcp_tool(&id).await;
    Html(r#"<script>htmx.ajax('GET', '/ui/store', {target: '#main-content', swap: 'innerHTML'});</script>"#.to_string())
}

// --- Administration (Vault) ---
async fn render_admin() -> impl IntoResponse {
    let masked = r2d2_cortex::security::vault::Vault::get_masked_keys();
    // Default providers expected in the Vault
    let providers = vec![
        "GEMINI_API_KEY",
        "MISTRAL_API_KEY",
        "GITHUB_PERSONAL_ACCESS_TOKEN",
        "HF_TOKEN",
    ];

    let mut keys = Vec::new();
    for p in providers {
        let val = masked
            .get(p)
            .cloned()
            .unwrap_or_else(|| "NON DÉFINIE".to_string());
        keys.push(KeyView {
            provider: p.to_string(),
            is_set: val != "NON DÉFINIE",
            masked_value: val,
        });
    }

    Html(AdminTemplate { keys }.render().unwrap())
}

async fn system_purge() -> impl IntoResponse {
    Html(r#"<div style='color: #2ecc71; padding: 12px; border: 1px solid #2ecc71; border-radius: 4px; background: rgba(46, 204, 113, 0.1);'><i data-lucide='check' style='display:inline-block; vertical-align:middle;'></i> VRAM Purgée (Mock). Cache nettoyé.</div><script>lucide.createIcons(); setTimeout(()=>document.querySelector(".dashboard-grid").innerHTML="", 3000);</script>"#.to_string())
}

async fn get_chat_status(State(state): State<AppState>) -> impl IntoResponse {
    let status = state.chat_status.read().await.clone();
    Html(status)
}

async fn handle_chat(
    State(state): State<AppState>,
    Form(input): Form<ChatInput>,
) -> impl IntoResponse {
    tracing::info!(
        "📥 [handle_chat] Received prompt: '{}', github_sources: '{}'",
        input.prompt,
        input.github_sources
    );
    // Handling the Consensus Stream Request locally
    if input.provider == "consensus" {
        *state.pending_debate_prompt.lock().await = Some(input.prompt.clone());
        let safe_prompt = input.prompt.replace("<", "&lt;").replace(">", "&gt;");
        let user_msg_html = format!(
            "<div class='message user'><div class='message-avatar'><i data-lucide='user' style='width:18px;'></i></div><div class='message-content'><div class='message-sender'>Vous</div><div class='message-bubble'>{}</div></div></div>", 
            safe_prompt
        );
        let html = format!(
            "{}\
            <div class='message system' hx-ext='sse' sse-connect='/api/chat/stream'>\
                <i data-lucide='brain-circuit'></i>\
                <div class='message-content' sse-swap='message' hx-swap='beforeend'>\
                   <strong style='color: var(--highlight-color);'>⚖️ Mode Débat (Triangulation LLM)</strong><br>\
                   <div style='font-style: italic; color: #888;'>Connexion au flux cognitif Synchrone établie...</div>\
                </div>\
            </div><script>var ch=document.getElementById('chat-history'); if(ch) ch.scrollTop = ch.scrollHeight; if(typeof lucide !== 'undefined') lucide.createIcons();</script>",
            user_msg_html
        );
        let mut resp = Html(html).into_response();
        resp.headers_mut()
            .insert("HX-Trigger", "chat-updated".parse().unwrap());
        return resp;
    }

    let session_id = state.current_chat_session.lock().await.clone();

    let mut cortex = state.agent.lock().await;

    // Isoler le contexte : on s'assure que le Cortex a bien chargé l'historique de cette session spécifique
    if let Some(session) = chat_history::load_session(&session_id) {
        let mut history = Vec::new();
        for turn in session.turns {
            history.push(r2d2_cortex::models::reasoning_agent::ChatMessage {
                role: if turn.role == "user" {
                    r2d2_cortex::models::reasoning_agent::MessageRole::User
                } else {
                    r2d2_cortex::models::reasoning_agent::MessageRole::Assistant
                },
                text: turn.content,
                function_name: None,
            });
        }
        cortex.set_history(history);
    } else {
        cortex.clear_history();
    }

    // 1. Initialiser le Provider (Gemini/Mistral/Local)
    cortex.set_provider(&input.provider);

    let final_prompt = input.prompt.clone();

    // Gestion des Sources GitHub assignées au prompt
    let mut otf_repos = Vec::new();
    if !input.github_sources.is_empty() {
        for ctx in input.github_sources.split(',') {
            if !ctx.trim().is_empty() {
                otf_repos.push(ctx.trim().to_string());
            }
        }
    }

    let mut mcp_feedback_html = String::new();

    // 2. Résolution cognitive RAG
    let mut current_prompt = final_prompt.clone();
    let json_resp;
    let mut is_function_result = false;
    let mut last_function_name = String::new();

    loop {
        // MAJ Statut : Inférence Cognitive ou Outil
        let status_msg = if is_function_result {
            format!("<div style='display:flex; align-items:center; gap:8px;'><div class='spinner'></div> <span class='pulsating-text'>Analyse du retour de l'outil {}...</span></div>", last_function_name)
        } else {
            "<div style='display:flex; align-items:center; gap:8px;'><div class='spinner'></div> <span class='pulsating-text'>Inférence Cognitive en cours...</span></div>".to_string()
        };
        *state.chat_status.write().await = status_msg;

        let thought_future = cortex.generate_thought_agentic(
            &current_prompt,
            &otf_repos,
            is_function_result,
            &last_function_name,
        );
        let timeout_duration = if otf_repos.is_empty() { 15 } else { 120 }; // Timeout étendu pour appel agentique potentiels via MCP
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_duration),
            thought_future,
        )
        .await;

        match result {
            Ok(Ok(r2d2_cortex::models::reasoning_agent::AgenticControlFlow::Completed(resp))) => {
                json_resp = resp;
                break;
            }
            Ok(Ok(
                r2d2_cortex::models::reasoning_agent::AgenticControlFlow::FunctionCallRequest {
                    name,
                    args,
                },
            )) => {
                tracing::info!(
                    "Agent initiated MCP tool call: {} with args {:?}",
                    name,
                    args
                );

                // MAJ Statut : Exécution MCP
                *state.chat_status.write().await = format!("<div style='display:flex; align-items:center; gap:8px;'><div class='spinner'></div> <span class='pulsating-text'>Exécution de l'outil {}...</span></div>", name);

                let mcp_lock = cortex.mcp_hub.lock().await;
                let mut init_failed = false;
                if mcp_lock.is_none() {
                    tracing::error!("Failed to use MCP Hub: Backend not initialized.");
                    current_prompt = "Erreur système interne: Le Hub MCP n'est pas instancié. L'appel outil a échoué.".to_string();
                    is_function_result = true;
                    last_function_name = name.clone();
                    mcp_feedback_html.push_str(&format!(
                        "<div style='font-size:0.8rem; color:#ef4444; border-left:2px solid #ef4444; padding-left:8px; margin-bottom:8px;'><i data-lucide='alert-circle' style='width:14px;'></i> Erreur d'Amorçage Outil `Hub-MCP::{}`: Daemon Not Instantiated</div>",
                        name
                    ));
                    init_failed = true;
                }

                if !init_failed {
                    if let Some(ref mcp) = *mcp_lock {
                        let parts: Vec<&str> = name.split("___").collect();
                        let (server_name, tool_name) = if parts.len() == 2 {
                            (parts[0], parts[1])
                        } else {
                            ("unknown", name.as_str())
                        };

                        let tool_future = mcp.call_tool(server_name, tool_name, args.clone());
                        let tool_result =
                            tokio::time::timeout(std::time::Duration::from_secs(10), tool_future)
                                .await;

                        match tool_result {
                            Ok(Ok(res)) => {
                                let mut res_str = res.to_string();
                                if res_str.len() > 250_000 {
                                    res_str = res_str.chars().take(250_000).collect();
                                    res_str.push_str("... [RESULT TRUNCATED BY R2D2]");
                                }
                                current_prompt = res_str.to_string();
                                is_function_result = true;
                                last_function_name = name.clone();
                                mcp_feedback_html.push_str(&format!(
                                    "<div style='font-size:0.8rem; color:#888; border-left:2px solid #3b82f6; padding-left:8px; margin-bottom:8px;'><i data-lucide='cpu' style='width:14px;'></i> Outil `{}::{}` exécuté.</div>",
                                    server_name, tool_name
                                ));
                            }
                            Ok(Err(e)) => {
                                current_prompt = format!("Tool {} execution failed: {}", name, e);
                                is_function_result = true;
                                last_function_name = name.clone();
                                mcp_feedback_html.push_str(&format!(
                                    "<div style='font-size:0.8rem; color:#ef4444; border-left:2px solid #ef4444; padding-left:8px; margin-bottom:8px;'><i data-lucide='alert-circle' style='width:14px;'></i> Erreur `{}`: {}</div>",
                                    name, e
                                ));
                            }
                            Err(_) => {
                                current_prompt = format!("System Error: Timeout réseau de 10s. Le sous-processus MCP ({}) ne répond pas et semble bufferiser la sortie ou être en boucle (Hang inter-processus).", server_name);
                                is_function_result = true;
                                last_function_name = name.clone();
                                mcp_feedback_html.push_str(&format!(
                                    "<div style='font-size:0.8rem; color:#ef4444; border-left:2px solid #ef4444; padding-left:8px; margin-bottom:8px;'><i data-lucide='alert-circle' style='width:14px;'></i> Timeout 10s: `{}` n'a pas répondu</div>",
                                    name
                                ));
                            }
                        }
                    } else {
                        current_prompt = format!(
                            "System Error: MCP Daemon unavailable. Tool {} aborted.",
                            name
                        );
                        is_function_result = true;
                        last_function_name = name.clone();
                    }
                }

                drop(mcp_lock);
                continue;
            }
            Ok(Err(e)) => {
                json_resp = serde_json::to_string(&serde_json::json!({
                    "content": format!("Erreur Cortex: {}", e),
                    "source": {"ParadoxEngine": "Error"},
                    "consensus": "Error",
                    "id": "paradox-multiapi-error"
                }))
                .unwrap();
                break;
            }
            Err(_) => {
                json_resp = serde_json::to_string(&serde_json::json!({
                    "content": "Défaillance de l'hyperviseur: Délai d'attente Cloud API expiré (> 15s). Cloud ou Proxy injoignable.",
                    "source": {"ParadoxEngine": "Timeout"},
                    "consensus": "Error",
                    "id": "paradox-timeout"
                })).unwrap();
                break;
            }
        };
    }

    // 3. Parsing du V3 JSONAI pour extraction du discours et des Traces
    let parsed: serde_json::Value = serde_json::from_str(&json_resp).unwrap_or(
        serde_json::json!({ "content": json_resp, "source": {"ParadoxEngine": "Unknown"} }),
    );
    let raw_text = parsed["content"].as_str().unwrap_or(&json_resp).to_string();

    let model_name = parsed["source"]["ParadoxEngine"]
        .as_str()
        .unwrap_or("Paradox Local")
        .to_string();
    let consensus = parsed["consensus"]
        .as_str()
        .unwrap_or("Unknown")
        .to_string();
    let latency = parsed["id"]
        .as_str()
        .unwrap_or("paradox-multiapi-0")
        .replace("paradox-multiapi-", "");

    // 4. Compilation Markdown vers HTML "Premium" Zero-Dependency (pulldown-cmark)
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);
    options.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    options.insert(pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION);
    let parser = pulldown_cmark::Parser::new_ext(&raw_text, options);
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, parser);

    let tmpl = ChatResponseTemplate {
        mcp_feedback: mcp_feedback_html,
        prompt: input.prompt.clone(),
        model_name,
        response_md: html_output,
        jsonai: json_resp.clone(),
        latency,
        consensus,
    };

    let session_id = state.current_chat_session.lock().await.clone();
    chat_history::save_turn(&session_id, &input.prompt, &json_resp, otf_repos.clone());

    let mut resp = Html(tmpl.render().unwrap()).into_response();
    resp.headers_mut()
        .insert("HX-Trigger", "chat-updated".parse().unwrap());
    resp
}

async fn get_history_list() -> impl IntoResponse {
    let sessions = chat_history::list_sessions();
    let mut html = String::new();
    for sess in sessions {
        let pin_icon = if sess.pinned {
            "<i data-lucide='pin' style='width: 14px; color: var(--accent-color); fill: var(--accent-color); margin-left: 4px;'></i>"
        } else {
            ""
        };

        html.push_str(&format!(
            "<div class='session-item' style='padding: 12px; border-bottom: 1px solid rgba(255,255,255,0.05); cursor: pointer; transition: background 0.2s; border-radius: 8px; position: relative;' onmouseover=\"this.style.background='rgba(255,255,255,0.1)'; this.querySelector('.session-actions').style.display='flex'\" onmouseout=\"this.style.background='transparent'; this.querySelector('.session-actions').style.display='none'\">\
                <div hx-get='/api/chat/history/{}' hx-target='#main-content' style='display: flex; align-items: center; gap: 8px; width: 100%;'>\
                    <i data-lucide='message-square' style='width: 16px; color: #888;'></i>\
                    <strong style='color: #e0e0e0; font-size: 0.85em; display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1;'>{}</strong>\
                    {}\
                </div>\
                <div class='session-actions' style='display: none; position: absolute; right: 8px; top: 50%; transform: translateY(-50%); gap: 4px; background: rgba(0,0,0,0.85); padding: 6px; border-radius: 6px; z-index: 10; border: 1px solid rgba(255,255,255,0.1);'>\
                    <button type='button' title='Info' hx-get='/api/chat/history/{}/info' hx-target='body' hx-swap='beforeend' style='background: transparent; border: none; color: #3498db; cursor: pointer; padding: 2px; transition: transform 0.2s;' onmouseover=\"this.style.transform='scale(1.2)'\" onmouseout=\"this.style.transform='scale(1)'\"><i data-lucide='info' style='width: 14px; height: 14px;'></i></button>\
                    <button type='button' title='Épingler' hx-post='/api/chat/history/{}/pin' style='background: transparent; border: none; color: #f39c12; cursor: pointer; padding: 2px; transition: transform 0.2s;' onmouseover=\"this.style.transform='scale(1.2)'\" onmouseout=\"this.style.transform='scale(1)'\"><i data-lucide='pin' style='width: 14px; height: 14px;'></i></button>\
                    <button type='button' title='Renommer' onclick=\"const newName = prompt('Nouveau nom ?', '{}'); if(newName && newName.trim() !== '') htmx.ajax('POST', '/api/chat/history/{}/rename', {{ values: {{ new_title: newName.trim() }} }})\" style='background: transparent; border: none; color: #9b59b6; cursor: pointer; padding: 2px; transition: transform 0.2s;' onmouseover=\"this.style.transform='scale(1.2)'\" onmouseout=\"this.style.transform='scale(1)'\"><i data-lucide='edit-2' style='width: 14px; height: 14px;'></i></button>\
                    <button type='button' title='Supprimer' hx-delete='/api/chat/history/{}' confirm='Supprimer définitivement cette conversation ?' style='background: transparent; border: none; color: #e74c3c; cursor: pointer; padding: 2px; transition: transform 0.2s;' onmouseover=\"this.style.transform='scale(1.2)'\" onmouseout=\"this.style.transform='scale(1)'\"><i data-lucide='trash' style='width: 14px; height: 14px;'></i></button>\
                </div>\
            </div>",
            sess.id, sess.title.replace("<", "&lt;").replace(">", "&gt;"), pin_icon, 
            sess.id, sess.id, sess.title.replace("\"", "&quot;").replace("'", "\\'"), sess.id, sess.id
        ));
    }
    if html.is_empty() {
        html = "<div style='padding: 16px; color: #888; text-align: center; font-size: 0.85em;'>Aucune conversation</div>".into();
    }
    html.push_str("<script>lucide.createIcons();</script>");
    Html(html)
}

#[derive(serde::Deserialize)]
struct RenamePayload {
    new_title: String,
}

async fn delete_chat_session(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    chat_history::delete_session(&id);
    let mut resp = Html("".to_string()).into_response();
    resp.headers_mut()
        .insert("HX-Trigger", "chat-updated".parse().unwrap());
    resp
}

async fn rename_chat_session(
    axum::extract::Path(id): axum::extract::Path<String>,
    Form(payload): Form<RenamePayload>,
) -> impl IntoResponse {
    chat_history::rename_session(&id, &payload.new_title);
    let mut resp = Html("".to_string()).into_response();
    resp.headers_mut()
        .insert("HX-Trigger", "chat-updated".parse().unwrap());
    resp
}

async fn pin_chat_session(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    chat_history::toggle_pin_session(&id);
    let mut resp = Html("".to_string()).into_response();
    resp.headers_mut()
        .insert("HX-Trigger", "chat-updated".parse().unwrap());
    resp
}

async fn info_chat_session(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let session = chat_history::load_session(&id);
    let html = if let Some(sess) = session {
        let date = chrono::DateTime::from_timestamp(sess.updated_at as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let turns_count = sess.turns.len() / 2; // Approximativement le nombre de messages user
        format!(
            "<div id='chat-info-modal' style='position: fixed; inset: 0; background: rgba(0,0,0,0.8); z-index: 9999; display: flex; align-items: center; justify-content: center; backdrop-filter: blur(5px);' onclick=\"if(event.target.id === 'chat-info-modal') this.remove();\">
                <div class='glass-card' style='padding: 32px; width: 450px; max-width: 90vw; position: relative;'>
                    <button onclick='document.getElementById(\"chat-info-modal\").remove()' style='position: absolute; right: 16px; top: 16px; background: transparent; color: #fff; border: none; font-size: 24px; cursor: pointer;'>&times;</button>
                    <h2 style='margin-top: 0; color: var(--accent-color); font-weight: 500;'><i data-lucide='info' style='display: inline-block; vertical-align: middle; margin-right: 8px;'></i> Metadata de la Session</h2>
                    <table style='width: 100%; border-collapse: collapse; margin-top: 24px;'>
                        <tr style='border-bottom: 1px solid rgba(255,255,255,0.1);'><td style='padding: 12px 0; color: #888;'>UUID</td><td style='padding: 12px 0; text-align: right; font-family: monospace;'>{}</td></tr>
                        <tr style='border-bottom: 1px solid rgba(255,255,255,0.1);'><td style='padding: 12px 0; color: #888;'>Création / MAJ</td><td style='padding: 12px 0; text-align: right;'>{}</td></tr>
                        <tr style='border-bottom: 1px solid rgba(255,255,255,0.1);'><td style='padding: 12px 0; color: #888;'>Tours d'Inférence</td><td style='padding: 12px 0; text-align: right;'>{} requêtes</td></tr>
                        <tr style='border-bottom: 1px solid rgba(255,255,255,0.1);'><td style='padding: 12px 0; color: #888;'>Mode</td><td style='padding: 12px 0; text-align: right;'>{}</td></tr>
                    </table>
                </div>
            </div>
            <script>lucide.createIcons();</script>",
            sess.id, date, turns_count, if sess.pinned { "<span style='color: #f39c12;'>📌 Épinglée</span>" } else { "Dynamique" }
        )
    } else {
        "<div id='chat-info-modal' style='position: fixed; inset: 0; background: rgba(0,0,0,0.8); z-index: 9999; display: flex; align-items: center; justify-content: center;' onclick='this.remove();'><div class='glass-card' style='padding: 24px; color: #e74c3c;'>Session Invalide</div></div>".to_string()
    };
    Html(html)
}

async fn new_chat_session(State(state): State<AppState>) -> impl IntoResponse {
    *state.current_chat_session.lock().await = uuid::Uuid::new_v4().to_string();
    state.agent.lock().await.clear_history();
    let history_html = "<div class='message system'><i data-lucide='terminal'></i><div class='message-content'><strong>System Initialize</strong><p>Bienvenue Chef. Le Cortex est en écoute de stimuli.</p></div></div>".into();
    let github_configured =
        r2d2_cortex::security::vault::Vault::get_api_key("GITHUB_PERSONAL_ACCESS_TOKEN").is_some();

    let queue = read_queue();
    let mut final_library_repos: Vec<String> = queue
        .into_iter()
        .map(|v| v.notebook)
        .filter(|n| n.contains("/"))
        .collect();

    // Hardcoded permanent repository for the project core architecture
    final_library_repos.push("JulienGioux/r2d2-core".to_string());

    final_library_repos.sort();
    final_library_repos.dedup();

    axum::response::Html(
        ChatTemplate {
            history_html,
            github_configured,
            active_sources_html: String::new(),
            library_repos: final_library_repos,
        }
        .render()
        .unwrap(),
    )
}

async fn load_chat_history(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    *state.current_chat_session.lock().await = id.clone();
    if let Some(session) = chat_history::load_session(&id) {
        let mut history = Vec::new();
        for turn in session.turns {
            history.push(r2d2_cortex::models::reasoning_agent::ChatMessage {
                role: if turn.role == "user" {
                    r2d2_cortex::models::reasoning_agent::MessageRole::User
                } else {
                    r2d2_cortex::models::reasoning_agent::MessageRole::Assistant
                },
                text: turn.content,
                function_name: None,
            });
        }
        state.agent.lock().await.set_history(history);
    } else {
        state.agent.lock().await.clear_history();
    }
    render_chat(State(state)).await
}

async fn get_status_widget() -> impl IntoResponse {
    use axum::response::Html;
    Html("<div class='status-indicator' style='display: flex; align-items: center; gap: 8px;'><span class='pulse-dot' style='width: 8px; height: 8px; background-color: #2ecc71; border-radius: 50%; box-shadow: 0 0 8px #2ecc71;'></span><span class='status-text' style='color: #2ecc71; font-weight: 500;'>Cortex: Online (Synced)</span></div>".to_string())
}

async fn handle_sse_stream(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let prompt_opt = state.pending_debate_prompt.lock().await.take();
    let prompt = prompt_opt.unwrap_or_else(|| "Erreur : Aucun prompt en attente.".to_string());

    let session_id = state.current_chat_session.lock().await.clone();
    let prompt_for_save = prompt.clone();

    let (tx, rx) = tokio::sync::mpsc::channel(10);
    let agent_arc = state.agent.clone();

    tokio::spawn(async move {
        let mut agent = agent_arc.lock().await;
        agent.set_provider("consensus");
        if let Err(e) = agent.run_debate(&prompt, tx.clone()).await {
            let _ = tx
                .send(DebateEvent::SystemEvent(format!(
                    "[ERREUR CASTRATHROPHIC CORTEX]: {}",
                    e
                )))
                .await;
        }
    });

    let stream = ReceiverStream::new(rx).map(move |event| {
        let html = match event {
            DebateEvent::SystemEvent(msg) => format!("<div style='color: #888; font-style: italic; margin-bottom: 8px;'>⚙️ Système : {}</div>", msg),
            DebateEvent::Turn { iteration, author, content } => {
                let parser = pulldown_cmark::Parser::new(&content);
                let mut md_html = String::new();
                pulldown_cmark::html::push_html(&mut md_html, parser);

                let (border_color, icon) = if author.contains("Gemini") { ("#10a37f", "🟢") } else { ("#f39c12", "🔵") };

                format!("<div style='border-left: 3px solid {}; padding-left: 12px; margin: 12px 0;'>\
                           <strong style='color: {};'>{} {} (Passe {}) :</strong>\
                           <div class='markdown-body' style='margin-top: 8px;'>{}</div>\
                         </div>", border_color, border_color, icon, author, iteration, md_html)
            },
            DebateEvent::FinalSynthesis(content) => {
                let json_struct = serde_json::json!({
                    "content": content,
                    "source": {"ParadoxEngine": "Consensus Cloud"},
                    "consensus": "Debated & Verified",
                    "id": format!("paradox-multiapi-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis())
                });

                // Dans le stream SSE (débat final), on recharge github_sources depuis la db s'ils existaient, ou on passe vide.
                // Au moment du get initial, l'historique a ete charge.
                let mut current_sources = Vec::new();
                if let Some(session) = crate::chat_history::load_session(&session_id) {
                    current_sources = session.github_sources.clone();
                }
                crate::chat_history::save_turn(&session_id, &prompt_for_save, &serde_json::to_string(&json_struct).unwrap(), current_sources);

                let parser = pulldown_cmark::Parser::new(&content);
                let mut md_html = String::new();
                pulldown_cmark::html::push_html(&mut md_html, parser);
                format!("<hr style='border-color: rgba(255,255,255,0.1); margin: 20px 0;'>\
                         <h3 style='color: var(--accent-color);'>🤝 Synthèse Absolue</h3>\
                         <div class='markdown-body'>{}</div>\
                         <script>if(typeof lucide !== 'undefined') lucide.createIcons(); var ch=document.getElementById('chat-history'); if(ch) ch.scrollTop = ch.scrollHeight;</script>\
                         <br><strong style='color: #2ecc71;'>[FIN DE LA TRIANGULATION - SOCKET CONSERVEE]</strong>", md_html)
            }
        };
        Ok(Event::default().data(html))
    }).chain(tokio_stream::pending());

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new())
}

async fn stream_system_logs(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.log_tx.subscribe();
    let s = stream! {
        while let Ok(msg) = rx.recv().await {
            yield Ok(Event::default().data(format!("<div hx-swap='beforeend'>{}</div>", msg)));
        }
    };
    Sse::new(s).keep_alive(axum::response::sse::KeepAlive::new())
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct VampireJob {
    id: String,
    theme: String,
    notebook: String,
    vampire_status: String,
    #[serde(default = "default_assimilation_status")]
    assimilation_status: String,
    #[serde(default = "default_provider")]
    provider: String,
}

fn default_assimilation_status() -> String {
    "NON ASSIMILÉ".to_string()
}

fn default_provider() -> String {
    "notebooklm".to_string()
}

#[derive(serde::Deserialize)]
struct AddJobPayload {
    theme: String,
    notebook: String,
}

fn read_queue() -> Vec<VampireJob> {
    std::fs::read_to_string("data/vampire_queue.json")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_queue(q: &Vec<VampireJob>) {
    std::fs::create_dir_all("data").unwrap_or_default();
    if let Ok(json) = serde_json::to_string_pretty(q) {
        let _ = std::fs::write("data/vampire_queue.json", json);
    }
}

async fn list_vampire_jobs() -> impl IntoResponse {
    let queue = read_queue();
    if queue.is_empty() {
        return axum::response::Html("<div style='color: #888; text-align: center; padding: 20px;'>Aucun apprentissage programmé dans la File d'Attente.</div>".to_string());
    }

    let mut html = String::from("<table style='width: 100%; text-align: left; border-collapse: collapse;'>
        <thead><tr style='border-bottom: 1px solid rgba(255,255,255,0.1); color: #888; font-size: 0.85rem;'>
            <th style='padding: 8px;'>Thème d'Apprentissage</th>
            <th style='padding: 8px;'>Notebook Cible</th>
            <th style='padding: 8px;'>Statut (Vampire)</th>
            <th style='padding: 8px;'>Statut (Forge)</th>
            <th style='padding: 8px; text-align: right;'>Actions</th>
        </tr></thead><tbody>");

    for job in queue {
        let v_color = match job.vampire_status.as_str() {
            "EN ATTENTE" => "#f39c12",
            "VAMPIRISÉ" => "#2ecc71",
            "ERREUR" => "#e74c3c",
            _ => "#3498db",
        };
        let a_color = match job.assimilation_status.as_str() {
            "NON ASSIMILÉ" => "#888",
            "EN COURS" => "#f39c12",
            "ASSIMILÉ" => "#9b59b6",
            "ERREUR" => "#e74c3c",
            _ => "#3498db",
        };

        let assimilate_btn = if job.vampire_status == "VAMPIRISÉ"
            && job.assimilation_status != "ASSIMILÉ"
        {
            format!("<button id='btn-assimilate-{}' class='btn-sm' hx-post='/api/admin/assimilate/{}' hx-swap='outerHTML' style='background: rgba(155,89,182,0.1); color: #9b59b6; border: 1px solid rgba(155,89,182,0.3); padding: 4px 8px; border-radius: 4px; cursor: pointer; transition: 0.2s;' onclick=\"this.innerHTML='<span class=\\'pulse-dot\\' style=\\'display:inline-block;width:6px;height:6px;background:#9b59b6;border-radius:50%;margin-right:6px;\\'></span> Forgeant...'; this.style.opacity='0.7';\">⚡ Forger (RAG)</button>", job.id, job.id)
        } else {
            String::new()
        };

        html.push_str(&format!(
            "<tr style='border-bottom: 1px solid rgba(255,255,255,0.05);'>
                <td style='padding: 12px 8px; font-weight: 500; font-size: 0.9rem;'>{}</td>
                <td style='padding: 12px 8px; color: #888; font-family: monospace; font-size: 0.85rem;'>{}</td>
                <td style='padding: 12px 8px;'>
                    <span style='background: {v_color}33; color: {v_color}; padding: 4px 8px; border-radius: 4px; font-size: 0.75rem; font-weight: bold;'>{}</span>
                </td>
                <td style='padding: 12px 8px;'>
                    <span style='background: {a_color}33; color: {a_color}; padding: 4px 8px; border-radius: 4px; font-size: 0.75rem; font-weight: bold;'>{}</span>
                </td>
                <td style='padding: 12px 8px; text-align: right; display: flex; gap: 6px; justify-content: flex-end;'>
                    {}
                    <button class='btn-sm' hx-post='/api/admin/vampire/queue/delete/{}' hx-target='#queue-list-container' style='background: rgba(231,76,60,0.1); color: #e74c3c; border: 1px solid rgba(231,76,60,0.3); padding: 4px 8px; border-radius: 4px; cursor: pointer; transition: 0.2s;'>
                        Suppr.
                    </button>
                </td>
            </tr>",
            job.theme, job.notebook, job.vampire_status, job.assimilation_status, assimilate_btn, job.id
        ));
    }
    html.push_str("</tbody></table>");
    axum::response::Html(html)
}

async fn add_vampire_job(Form(payload): Form<AddJobPayload>) -> impl IntoResponse {
    let mut queue = read_queue();
    queue.push(VampireJob {
        id: uuid::Uuid::new_v4().to_string(),
        theme: payload.theme,
        notebook: payload.notebook,
        vampire_status: "EN ATTENTE".to_string(),
        assimilation_status: "NON ASSIMILÉ".to_string(),
        provider: "notebooklm".to_string(),
    });
    write_queue(&queue);
    list_vampire_jobs().await
}

async fn delete_vampire_job(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let mut queue = read_queue();
    queue.retain(|j| j.id != id);
    write_queue(&queue);
    list_vampire_jobs().await
}

async fn start_vampire() -> impl IntoResponse {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    tokio::spawn(async {
        let mut child = Command::new("cargo")
            .args(["run", "--release", "-p", "r2d2-mcp", "--bin", "vampire"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Echec lancement vampire");

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let mut out_reader = BufReader::new(stdout).lines();
        let mut err_reader = BufReader::new(stderr).lines();

        let out_task = tokio::spawn(async move {
            while let Ok(Some(line)) = out_reader.next_line().await {
                tracing::info!("[VAMPIRE] {}", line);
            }
        });

        let err_task = tokio::spawn(async move {
            while let Ok(Some(line)) = err_reader.next_line().await {
                tracing::info!("[VAMPIRE] {}", line);
            }
        });

        let _ = tokio::join!(out_task, err_task);
        let _ = child.wait().await;
    });

    axum::response::Html(
        "<div style='color: #2ecc71;'>🩸 Éveil du Vampire initié ! Télémétrie en attente...</div>",
    )
}

async fn execute_assimilation(mission_id_arg: Option<String>) -> bool {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    let mut cmd = Command::new("cargo");
    cmd.args([
        "run",
        "--release",
        "-p",
        "r2d2-cortex",
        "--bin",
        "assimilate_knowledge",
    ]);
    if let Some(id) = &mission_id_arg {
        cmd.args(["--", "--mission", id]);
    }

    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Echec lancement assimilation");

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut out_reader = BufReader::new(stdout).lines();
    let mut err_reader = BufReader::new(stderr).lines();

    let out_task = tokio::spawn(async move {
        while let Ok(Some(line)) = out_reader.next_line().await {
            tracing::info!("[ASSIMILATEUR] {}", line);
        }
    });

    let err_task = tokio::spawn(async move {
        while let Ok(Some(line)) = err_reader.next_line().await {
            tracing::info!("[ASSIMILATEUR] {}", line);
        }
    });

    let _ = tokio::join!(out_task, err_task);
    let status = child.wait().await.unwrap();
    status.success()
}

async fn start_assimilation_all() -> impl IntoResponse {
    tokio::spawn(async {
        execute_assimilation(None).await;
    });
    axum::response::Html("<div style='color: #2ecc71;'>🧠 Auto-Assimilation globale initiée ! Surveillance via console...</div>")
}

async fn start_assimilation_id(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    // Marque en cours (optionnel, on attend la fin direct de tte facon)
    {
        let mut queue = read_queue();
        if let Some(job) = queue.iter_mut().find(|j| j.id == id) {
            job.assimilation_status = "EN COURS".to_string();
            write_queue(&queue);
        }
    }

    // Attente Synchrone du binaire (environ 5-15s en mode release deja packagé)
    let success = execute_assimilation(Some(id.clone())).await;

    if success {
        let mut queue = read_queue();
        if let Some(job) = queue.iter_mut().find(|j| j.id == id) {
            job.assimilation_status = "ASSIMILÉ".to_string();
            write_queue(&queue);
        }

        let success_html = "<button class='btn-sm' disabled style='background: rgba(46, 204, 113, 0.1); color: #2ecc71; border: 1px solid rgba(46, 204, 113, 0.3); padding: 4px 8px; border-radius: 4px; cursor: default;'>✅ Forgé</button><script>document.getElementById('queue-list-container').dispatchEvent(new Event('refresh'));</script>".to_string();
        axum::response::Html(success_html)
    } else {
        let err_html = "<button class='btn-sm' style='background: rgba(231, 76, 60, 0.1); color: #e74c3c; border: 1px solid rgba(231, 76, 60, 0.3); padding: 4px 8px; border-radius: 4px; cursor: default;'>❌ Échec</button><script>document.getElementById('queue-list-container').dispatchEvent(new Event('refresh'));</script>".to_string();
        axum::response::Html(err_html)
    }
}

#[derive(serde::Deserialize)]
struct KeyUpdateParams {
    provider: String,
    api_key: String,
}

async fn set_admin_keys(Form(params): Form<KeyUpdateParams>) -> axum::response::Response {
    r2d2_cortex::security::vault::Vault::set_api_key(&params.provider, &params.api_key);
    axum::response::Html(
        r#"<script>
        htmx.ajax('GET', '/ui/admin', {target: '#main-content', swap: 'innerHTML'});
    </script>"#
            .to_string(),
    )
    .into_response()
}

#[derive(serde::Deserialize)]
struct DownloadModelPayload {
    #[allow(dead_code)]
    #[allow(dead_code)]
    model_id: String,
    model_name: String,
}

async fn store_download_model(
    axum::Form(payload): axum::Form<DownloadModelPayload>,
) -> impl axum::response::IntoResponse {
    tracing::info!("Mocking download process for model: {}", payload.model_name);
    let html = "<button class='btn-primary' style='width: 100%; background: var(--status-success); color: #fff; cursor: default;' disabled><i data-lucide='check-circle' style='width: 16px;'></i> Allocation Initiée</button><script>if(typeof lucide !== 'undefined') lucide.createIcons();</script>".to_string();
    axum::response::Html(html)
}

#[derive(serde::Deserialize)]
struct AttachContextPayload {
    repo_url: String,
    analyze_mode: String,
}

async fn attach_context(
    State(state): State<AppState>,
    Form(payload): Form<AttachContextPayload>,
) -> impl IntoResponse {
    let mode = payload.analyze_mode.as_str();
    let repo = payload.repo_url.trim();
    let session_id = state.current_chat_session.lock().await.clone();

    if mode == "async" {
        let mut queue = read_queue();
        let target_theme = format!("Codebase: {}", repo);
        queue.push(VampireJob {
            id: uuid::Uuid::new_v4().to_string(),
            theme: target_theme.clone(),
            notebook: repo.to_string(),
            vampire_status: "EN ATTENTE".to_string(),
            assimilation_status: "NON ASSIMILÉ".to_string(),
            provider: "github".to_string(),
        });
        write_queue(&queue);

        // Persist local session Context
        let prefixed_repo = format!("github-async://{}", repo);
        chat_history::append_github_source(&session_id, &prefixed_repo);

        let id_val = format!("github-async-{}", uuid::Uuid::new_v4().simple());
        let html_snippet = format!(
            "<div id=\"{}\" class=\"context-pill context-pill-async\">\
                <i data-lucide=\"github\" class=\"icon-type\" style=\"width: 14px; height: 14px;\"></i>\
                <span>{} (Async Queued)</span>\
                <button type=\"button\" hx-post=\"/api/chat/context/remove\" hx-vals='{{\"repo\": \"{}\"}}' hx-target=\"#{}\" hx-swap=\"delete\">\
                    <i data-lucide=\"x\" style=\"width: 12px; height: 12px;\"></i>\
                </button>\
                <input type=\"hidden\" name=\"github_source_item\" value=\"{}\">\
            </div><script>if(typeof lucide !== 'undefined') lucide.createIcons();</script>",
            id_val, repo, prefixed_repo, id_val, prefixed_repo
        );
        axum::response::Html(html_snippet)
    } else {
        chat_history::append_github_source(&session_id, repo);
        let id_val = format!("github-otf-{}", uuid::Uuid::new_v4().simple());

        let html_snippet = format!(
            "<div id=\"{}\" class=\"context-pill context-pill-otf\">\
                <i data-lucide=\"github\" class=\"icon-type\" style=\"width: 14px; height: 14px;\"></i>\
                <span>{} (Cognitive Tool)</span>\
                <button type=\"button\" hx-post=\"/api/chat/context/remove\" hx-vals='{{\"repo\": \"{}\"}}' hx-target=\"#{}\" hx-swap=\"delete\">\
                    <i data-lucide=\"x\" style=\"width: 12px; height: 12px;\"></i>\
                </button>\
                <input type=\"hidden\" name=\"github_source_item\" value=\"{}\">\
            </div><script>if(typeof lucide !== 'undefined') lucide.createIcons();</script>",
            id_val, repo, repo, id_val, repo
        );
        axum::response::Html(html_snippet)
    }
}

#[derive(serde::Deserialize)]
struct RemoveContextPayload {
    repo: String,
}

async fn remove_context(
    State(state): State<AppState>,
    Form(payload): Form<RemoveContextPayload>,
) -> impl IntoResponse {
    let session_id = state.current_chat_session.lock().await.clone();
    chat_history::remove_github_source(&session_id, &payload.repo);
    axum::response::Html("") // Return empty block to swap out the pill
}
