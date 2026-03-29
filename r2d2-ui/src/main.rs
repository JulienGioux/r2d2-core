use axum::{
    extract::State,
    routing::{get, post, delete},
    Router, Form,
    response::{Html, IntoResponse},
};
use askama::Template;
use serde::Deserialize;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

use r2d2_cortex::agent::CognitiveAgent;
use r2d2_cortex::models::reasoning_agent::ReasoningAgent;
use axum::response::sse::{Event, Sse};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use std::convert::Infallible;
use r2d2_cortex::models::reasoning_agent::DebateEvent;
use async_stream::stream;
use sysinfo::System;
use std::time::Instant;

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
    pub active_agents_count: usize,
    pub memory_vectors_count: usize,
    pub alert_threshold: bool,
    pub uptime_formatted: String,
    pub core_count: usize,
}

#[derive(Template)]
#[template(path = "cortex.html")]
struct CortexTemplate {}

#[derive(Template)]
#[template(path = "chat.html")]
struct ChatTemplate {
    history_html: String,
    github_configured: bool,
    active_sources_html: String,
    library_repos: Vec<String>,
}

#[derive(Template)]
#[template(path = "memory.html")]
struct MemoryTemplate {
    pub total_vectors: usize,
    pub sample_axioms: Vec<(usize, String)>,
}

#[derive(Template)]
#[template(path = "admin.html")]
struct AdminTemplate {
    local_models: Vec<String>,
}

pub fn list_local_hf_models() -> Vec<String> {
    let mut models = vec![];
    if let Some(home) = dirs::home_dir() {
        let hf_hub = home.join(".cache/huggingface/hub");
        if let Ok(entries) = std::fs::read_dir(hf_hub) {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.starts_with("models--") {
                        models.push(name.replace("models--", "").replace("--", "/"));
                    }
                }
            }
        }
    }
    models.sort();
    models
}

#[derive(Template)]
#[template(source = r#"
<div class="message user">
    <i data-lucide="user"></i>
    <div class="message-content">
        <strong>Directif Utilisateur</strong>
        <p>{{ prompt }}</p>
    </div>
</div>
{{ mcp_feedback|safe }}
<div class="message system">
    <i data-lucide="brain-circuit" style="margin-top: 4px;"></i>
    <div class="message-content" style="width: 100%;">
        <div class="message-header" style="display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid rgba(255,255,255,0.1); padding-bottom: 8px; margin-bottom: 12px;">
            <strong style="color: var(--highlight-color); font-size: 0.9rem;">{{ model_name }}</strong>
            <div class="view-toggles" style="display: flex; gap: 4px;">
                <button type="button" class="btn-sm" onclick="showTab(this, 0)" style="background: var(--bg-tertiary); border: 1px solid var(--border-color); color: #fff; padding: 2px 8px; border-radius: 4px; font-size: 0.75rem; cursor: pointer; transition: all 0.2s;">Vue Synthétique (MD)</button>
                <button type="button" class="btn-sm" onclick="showTab(this, 1)" style="background: transparent; border: 1px solid var(--border-color); color: #888; padding: 2px 8px; border-radius: 4px; font-size: 0.75rem; cursor: pointer; transition: all 0.2s;">Matrice JSONAi</button>
                <button type="button" class="btn-sm" onclick="showTab(this, 2)" style="background: transparent; border: 1px solid var(--border-color); color: #888; padding: 2px 8px; border-radius: 4px; font-size: 0.75rem; cursor: pointer; transition: all 0.2s;">Trace Réseau</button>
            </div>
        </div>
        
        <div class="tab-content" style="display: block;">
            <div class="markdown-body" style="font-size: 0.95rem; line-height: 1.6;">{{ response_md|safe }}</div>
        </div>
        <div class="tab-content" style="display: none;">
            <pre style="background: rgba(0,0,0,0.5); padding: 12px; border-radius: 8px; overflow-x: auto; font-size: 0.8rem; color: #a5d6ff; margin: 0;">{{ jsonai|safe }}</pre>
        </div>
        <div class="tab-content" style="display: none;">
            <pre style="background: rgba(0,0,0,0.5); padding: 12px; border-radius: 8px; overflow-x: auto; font-size: 0.8rem; color: #ffcc00; margin: 0;">
⌚ Délai d'inférence cognitif : {{ latency }} ms
🏭 Provider Cloud Ciblé      : {{ model_name }}
🫂 Consensus d'Inférence     : {{ consensus }}
            </pre>
        </div>
    </div>
</div>
<script>if (typeof lucide !== 'undefined') { lucide.createIcons(); }</script>
"#, ext = "html")]
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
    let broadcast_layer = logger::BroadcastLayer { sender: log_tx.clone() };

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .with(broadcast_layer)
        .init();

    tracing::info!("🚀 Booting R2D2-UI (Axum/HTMX Administration Console)...");

    // 1. Initialisation Physique du Cortex
    let mut cortex = ReasoningAgent::new();
    cortex.load().await.expect("Erreur Fatale lors de l'allocation Tensorielle du Cortex");
    
    let mut sys = System::new_all();
    sys.refresh_all();
    
    let shared_state = AppState {
        agent: Arc::new(Mutex::new(cortex)),
        pending_debate_prompt: Arc::new(Mutex::new(None)),
        current_chat_session: Arc::new(Mutex::new(uuid::Uuid::new_v4().to_string())),
        log_tx: log_tx.clone(),
        sys: Arc::new(Mutex::new(sys)),
        start_time: Instant::now(),
    };

    let api_routes = Router::new()
        .route("/widgets/status", get(get_status_widget))
        .route("/chat", post(handle_chat))
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
        .route("/admin/keys", get(get_admin_keys))
        .route("/admin/keys", post(set_admin_keys))
        .route("/chat/context/attach", post(attach_context))
        .route("/chat/context/remove", post(remove_context));

    let app = Router::new()
        .route("/", get(render_index))
        .route("/ui/dashboard", get(render_dashboard))
        .route("/ui/chat", get(render_chat))
        .route("/ui/cortex", get(render_cortex))
        .route("/ui/memory", get(render_memory))
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
    let ram_percent = if total_ram > 0.0 { (used_ram / total_ram * 100.0) as i32 } else { 0 };
    let alert_threshold = ram_percent > 85;
    
    let core_count = sys.physical_core_count().unwrap_or(4);
    
    let uptime_secs = state.start_time.elapsed().as_secs();
    let uptime_formatted = format!("{:02}:{:02}:{:02}", uptime_secs / 3600, (uptime_secs % 3600) / 60, uptime_secs % 60);
    
    let agent = state.agent.lock().await;
    let memory_vectors_count = agent.memory_vectors_count();
    let active_agents_count = if agent.is_active() { 3 } else { 0 };

    Html(DashboardTemplate {
        total_ram_gb: format!("{:.1}", total_ram),
        used_ram_gb: format!("{:.1}", used_ram),
        ram_percent,
        active_agents_count,
        memory_vectors_count,
        alert_threshold,
        uptime_formatted,
        core_count,
    }.render().unwrap())
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
                let model_name = parsed["source"]["ParadoxEngine"].as_str().unwrap_or("Paradox Local").to_string();
                let consensus = parsed["consensus"].as_str().unwrap_or("Unknown").to_string();
                let latency = parsed["id"].as_str().unwrap_or("").replace("paradox-multiapi-", "");
                
                let parser = pulldown_cmark::Parser::new(&raw_text);
                let mut html_output = String::new();
                pulldown_cmark::html::push_html(&mut html_output, parser);
                
                history_html.push_str(&ChatResponseTemplate {
                    mcp_feedback: String::new(), prompt, model_name, response_md: html_output, jsonai: json_resp, latency, consensus,
                }.render().unwrap());
                i += 2;
            } else { i += 1; }
        }
    }
    if history_html.is_empty() {
        history_html = "<div class='message system'><i data-lucide='terminal'></i><div class='message-content'><strong>System Initialize</strong><p>Bienvenue Chef. Le Cortex est en écoute de stimuli.</p></div></div>".into();
    }
    let github_configured = r2d2_cortex::security::vault::Vault::get_api_key("GITHUB_PERSONAL_ACCESS_TOKEN").is_some();
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

    Html(ChatTemplate { history_html, github_configured, active_sources_html, library_repos: final_library_repos }.render().unwrap())
}
async fn render_cortex() -> impl IntoResponse { Html(CortexTemplate {}.render().unwrap()) }

async fn render_memory(State(state): State<AppState>) -> impl IntoResponse {
    let agent = state.agent.lock().await;
    let (total_vectors, sample_axioms) = if let Some(mem) = &agent.memory {
        (mem.len(), mem.get_sample_axioms(6))
    } else {
        (0, vec![])
    };

    Html(MemoryTemplate {
        total_vectors,
        sample_axioms,
    }.render().unwrap())
}

async fn render_admin() -> impl IntoResponse { Html(AdminTemplate { local_models: list_local_hf_models() }.render().unwrap()) }

async fn system_purge() -> impl IntoResponse {
    Html(r#"<div style='color: #2ecc71; padding: 12px; border: 1px solid #2ecc71; border-radius: 4px; background: rgba(46, 204, 113, 0.1);'><i data-lucide='check' style='display:inline-block; vertical-align:middle;'></i> VRAM Purgée (Mock). Cache nettoyé.</div><script>lucide.createIcons(); setTimeout(()=>document.querySelector(".dashboard-grid").innerHTML="", 3000);</script>"#.to_string())
}

async fn handle_chat(
    State(state): State<AppState>,
    Form(input): Form<ChatInput>
) -> impl IntoResponse {
    tracing::info!("📥 [handle_chat] Received prompt: '{}', github_sources: '{}'", input.prompt, input.github_sources);
    // Handling the Consensus Stream Request locally
    if input.provider == "consensus" {
        *state.pending_debate_prompt.lock().await = Some(input.prompt.clone());
        let safe_prompt = input.prompt.replace("<", "&lt;").replace(">", "&gt;");
        let user_msg_html = format!(
            "<div class='message user'><i data-lucide='user'></i><div class='message-content'><strong>Directif Utilisateur</strong><p>{}</p></div></div>", 
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
        resp.headers_mut().insert("HX-Trigger", "chat-updated".parse().unwrap());
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

    let mut final_prompt = input.prompt.clone();

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
    let mut json_resp = String::new();
    let mut is_function_result = false;
    let mut last_function_name = String::new();

    loop {
        let thought_future = cortex.generate_thought_agentic(&current_prompt, &otf_repos, is_function_result, &last_function_name);
        let timeout_duration = if otf_repos.is_empty() { 15 } else { 120 }; // Timeout étendu pour appel agentique potentiels via MCP
        let result = tokio::time::timeout(std::time::Duration::from_secs(timeout_duration), thought_future).await;
        
        match result {
            Ok(Ok(r2d2_cortex::models::reasoning_agent::AgenticControlFlow::Completed(resp))) => {
                json_resp = resp;
                break;
            },
            Ok(Ok(r2d2_cortex::models::reasoning_agent::AgenticControlFlow::FunctionCallRequest { name, args })) => {
                tracing::info!("Agent initiated MCP tool call: {} with args {:?}", name, args);
                
                // Lazy initialization du Daemon MCP (Actor Pattern)
                let github_token = r2d2_cortex::security::vault::Vault::get_api_key("GITHUB_PERSONAL_ACCESS_TOKEN").unwrap_or_default();
                let mut mcp_lock = cortex.mcp_client.lock().await;
                let mut init_failed = false;
                if mcp_lock.is_none() {
                    tracing::info!("Instantiating persistent MCP Daemon...");
                    match r2d2_cortex::mcp_client::McpClient::new(&github_token).await {
                        Ok(client) => {
                            *mcp_lock = Some(client);
                        },
                        Err(e) => {
                            tracing::error!("Failed to instantiate MCP Daemon: {}", e);
                            current_prompt = format!("Erreur système interne: impossible d'instancier le démon MCP. L'appel 'npx' a échoué (Timeout ou Proxy). Raison : {}", e);
                            is_function_result = true;
                            last_function_name = name.clone();
                            mcp_feedback_html.push_str(&format!(
                                "<div style='font-size:0.8rem; color:#ef4444; border-left:2px solid #ef4444; padding-left:8px; margin-bottom:8px;'><i data-lucide='alert-circle' style='width:14px;'></i> Erreur d'Amorçage Outil `github-mcp::{}`: Daemon Instantiation Failed</div>",
                                name
                            ));
                            init_failed = true;
                        }
                    }
                }

                if !init_failed {
                    if let Some(ref mcp) = *mcp_lock {
                        match mcp.call_tool(&name, args).await {
                            Ok(res) => {
                                let mut res_str = res.to_string();
                                if res_str.len() > 250_000 {
                                    res_str = res_str.chars().take(250_000).collect();
                                    res_str.push_str("... [RESULT TRUNCATED BY R2D2]");
                                }
                                current_prompt = format!("{}", res_str);
                                is_function_result = true;
                                last_function_name = name.clone();
                                mcp_feedback_html.push_str(&format!(
                                    "<div style='font-size:0.8rem; color:#888; border-left:2px solid #3b82f6; padding-left:8px; margin-bottom:8px;'><i data-lucide='cpu' style='width:14px;'></i> Outil `github-mcp::{}` exécuté.</div>",
                                    name
                                ));
                            },
                            Err(e) => {
                                current_prompt = format!("Tool {} execution failed: {}", name, e);
                                is_function_result = true;
                                last_function_name = name.clone();
                                mcp_feedback_html.push_str(&format!(
                                    "<div style='font-size:0.8rem; color:#ef4444; border-left:2px solid #ef4444; padding-left:8px; margin-bottom:8px;'><i data-lucide='alert-circle' style='width:14px;'></i> Erreur `github-mcp::{}`: {}</div>",
                                    name, e
                                ));
                            }
                        }
                    } else {
                        current_prompt = format!("System Error: MCP Daemon unavailable. Tool {} aborted.", name);
                        is_function_result = true;
                        last_function_name = name.clone();
                    }
                }
                
                drop(mcp_lock);
                continue;
            },
            Ok(Err(e)) => {
                json_resp = serde_json::to_string(&serde_json::json!({
                    "content": format!("Erreur Cortex: {}", e),
                    "source": {"ParadoxEngine": "Error"},
                    "consensus": "Error",
                    "id": "paradox-multiapi-error"
                })).unwrap();
                break;
            },
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
    let parsed: serde_json::Value = serde_json::from_str(&json_resp).unwrap_or(serde_json::json!({ "content": json_resp, "source": {"ParadoxEngine": "Unknown"} }));
    let raw_text = parsed["content"].as_str().unwrap_or(&json_resp).to_string();

    let model_name = parsed["source"]["ParadoxEngine"].as_str().unwrap_or("Paradox Local").to_string();
    let consensus = parsed["consensus"].as_str().unwrap_or("Unknown").to_string();
    let latency = parsed["id"].as_str().unwrap_or("paradox-multiapi-0").replace("paradox-multiapi-", "");

    // 4. Compilation Markdown vers HTML "Premium" Zero-Dependency (pulldown-cmark)
    let parser = pulldown_cmark::Parser::new(&raw_text);
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
    resp.headers_mut().insert("HX-Trigger", "chat-updated".parse().unwrap());
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

async fn delete_chat_session(axum::extract::Path(id): axum::extract::Path<String>) -> impl IntoResponse {
    chat_history::delete_session(&id);
    let mut resp = Html("".to_string()).into_response();
    resp.headers_mut().insert("HX-Trigger", "chat-updated".parse().unwrap());
    resp
}

async fn rename_chat_session(
    axum::extract::Path(id): axum::extract::Path<String>,
    Form(payload): Form<RenamePayload>
) -> impl IntoResponse {
    chat_history::rename_session(&id, &payload.new_title);
    let mut resp = Html("".to_string()).into_response();
    resp.headers_mut().insert("HX-Trigger", "chat-updated".parse().unwrap());
    resp
}

async fn pin_chat_session(axum::extract::Path(id): axum::extract::Path<String>) -> impl IntoResponse {
    chat_history::toggle_pin_session(&id);
    let mut resp = Html("".to_string()).into_response();
    resp.headers_mut().insert("HX-Trigger", "chat-updated".parse().unwrap());
    resp
}

async fn info_chat_session(axum::extract::Path(id): axum::extract::Path<String>) -> impl IntoResponse {
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
    let github_configured = r2d2_cortex::security::vault::Vault::get_api_key("GITHUB_PERSONAL_ACCESS_TOKEN").is_some();
    
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

    axum::response::Html(ChatTemplate { 
        history_html, 
        github_configured, 
        active_sources_html: String::new(),
        library_repos: final_library_repos
    }.render().unwrap())
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

async fn handle_sse_stream(State(state): State<AppState>) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
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
            let _ = tx.send(DebateEvent::SystemEvent(format!("[ERREUR CASTRATHROPHIC CORTEX]: {}", e))).await;
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

async fn stream_system_logs(State(state): State<AppState>) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
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
            _ => "#3498db"
        };
        let a_color = match job.assimilation_status.as_str() {
            "NON ASSIMILÉ" => "#888",
            "EN COURS" => "#f39c12",
            "ASSIMILÉ" => "#9b59b6",
            "ERREUR" => "#e74c3c",
            _ => "#3498db"
        };
        
        let assimilate_btn = if job.vampire_status == "VAMPIRISÉ" && job.assimilation_status != "ASSIMILÉ" {
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

async fn delete_vampire_job(axum::extract::Path(id): axum::extract::Path<String>) -> impl IntoResponse {
    let mut queue = read_queue();
    queue.retain(|j| j.id != id);
    write_queue(&queue);
    list_vampire_jobs().await
}

async fn start_vampire() -> impl IntoResponse {
    use tokio::process::Command;
    use tokio::io::{AsyncBufReadExt, BufReader};

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
    
    axum::response::Html("<div style='color: #2ecc71;'>🩸 Éveil du Vampire initié ! Télémétrie en attente...</div>")
}

async fn execute_assimilation(mission_id_arg: Option<String>) -> bool {
    use tokio::process::Command;
    use tokio::io::{AsyncBufReadExt, BufReader};

    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--release", "-p", "r2d2-cortex", "--bin", "assimilate_knowledge"]);
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

async fn start_assimilation_id(axum::extract::Path(id): axum::extract::Path<String>) -> impl IntoResponse {
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
        
        let success_html = format!("<button class='btn-sm' disabled style='background: rgba(46, 204, 113, 0.1); color: #2ecc71; border: 1px solid rgba(46, 204, 113, 0.3); padding: 4px 8px; border-radius: 4px; cursor: default;'>✅ Forgé</button><script>document.getElementById('queue-list-container').dispatchEvent(new Event('refresh'));</script>");
        axum::response::Html(success_html)
    } else {
        let err_html = format!("<button class='btn-sm' style='background: rgba(231, 76, 60, 0.1); color: #e74c3c; border: 1px solid rgba(231, 76, 60, 0.3); padding: 4px 8px; border-radius: 4px; cursor: default;'>❌ Échec</button><script>document.getElementById('queue-list-container').dispatchEvent(new Event('refresh'));</script>");
        axum::response::Html(err_html)
    }
}

#[derive(serde::Deserialize)]
struct KeyUpdateParams {
    provider: String,
    api_key: String,
}

async fn get_admin_keys() -> axum::response::Response {
    let keys = r2d2_cortex::security::vault::Vault::get_masked_keys();
    let gemini_key = keys.get("GEMINI_API_KEY").map(|k| k.as_str()).unwrap_or("NON DÉFINIE");
    let mistral_key = keys.get("MISTRAL_API_KEY").map(|k| k.as_str()).unwrap_or("NON DÉFINIE");
    let github_key = keys.get("GITHUB_PERSONAL_ACCESS_TOKEN").map(|k| k.as_str()).unwrap_or("NON DÉFINIE");
    let hf_key = keys.get("HF_TOKEN").map(|k| k.as_str()).unwrap_or("NON DÉFINIE");
    
    let keys_html = format!(r#"
        <div class="vault-keys" style="margin-bottom: 20px;">
            <div style="display: flex; justify-content: space-between; padding: 10px; background: rgba(0,0,0,0.2); border-radius: 4px; margin-bottom: 8px;">
                <strong style="color: #2ecc71;">GITHUB_PERSONAL_ACCESS_TOKEN</strong>
                <span style="font-family: monospace; color: #888;">{}</span>
            </div>
            <div style="display: flex; justify-content: space-between; padding: 10px; background: rgba(0,0,0,0.2); border-radius: 4px; margin-bottom: 8px;">
                <strong style="color: var(--accent-color);">GEMINI_API_KEY</strong>
                <span style="font-family: monospace; color: #888;">{}</span>
            </div>
            <div style="display: flex; justify-content: space-between; padding: 10px; background: rgba(0,0,0,0.2); border-radius: 4px; margin-bottom: 8px;">
                <strong style="color: #f39c12;">MISTRAL_API_KEY</strong>
                <span style="font-family: monospace; color: #888;">{}</span>
            </div>
            <div style="display: flex; justify-content: space-between; padding: 10px; background: rgba(0,0,0,0.2); border-radius: 4px; margin-bottom: 8px;">
                <strong style="color: #3498db;">HF_TOKEN</strong>
                <span style="font-family: monospace; color: #888;">{}</span>
            </div>
        </div>
    "#, github_key, gemini_key, mistral_key, hf_key);
    
    axum::response::Html(keys_html).into_response()
}

async fn set_admin_keys(Form(params): Form<KeyUpdateParams>) -> axum::response::Response {
    r2d2_cortex::security::vault::Vault::set_api_key(&params.provider, &params.api_key);
    get_admin_keys().await
}

#[derive(serde::Deserialize)]
struct AttachContextPayload {
    repo_url: String,
    analyze_mode: String,
}

async fn attach_context(
    State(state): State<AppState>,
    Form(payload): Form<AttachContextPayload>
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
    Form(payload): Form<RemoveContextPayload>
) -> impl IntoResponse {
    let session_id = state.current_chat_session.lock().await.clone();
    chat_history::remove_github_source(&session_id, &payload.repo);
    axum::response::Html("") // Return empty block to swap out the pill
}
