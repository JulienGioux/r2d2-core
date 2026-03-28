use axum::{
    extract::State,
    routing::{get, post},
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
use tower_http::trace::TraceLayer;
use axum::response::sse::{Event, Sse};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use std::convert::Infallible;
use r2d2_cortex::models::reasoning_agent::DebateEvent;

#[derive(Clone)]
struct AppState {
    agent: Arc<Mutex<ReasoningAgent>>,
    pending_debate_prompt: Arc<Mutex<Option<String>>>,
}

#[derive(Template)]
#[template(path = "base.html")]
struct IndexTemplate {
    status: &'static str,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {}

#[derive(Template)]
#[template(path = "cortex.html")]
struct CortexTemplate {}

#[derive(Template)]
#[template(path = "memory.html")]
struct MemoryTemplate {}

#[derive(Template)]
#[template(source = r#"
<div class="message user">
    <i data-lucide="user"></i>
    <div class="message-content">
        <strong>Directif Utilisateur</strong>
        <p>{{ prompt }}</p>
    </div>
</div>
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("🚀 Booting R2D2-UI (Axum/HTMX Administration Console)...");

    // 1. Initialisation Physique du Cortex
    let mut cortex = ReasoningAgent::new();
    cortex.load().await.expect("Erreur Fatale lors de l'allocation Tensorielle du Cortex");
    
    let shared_state = AppState {
        agent: Arc::new(Mutex::new(cortex)),
        pending_debate_prompt: Arc::new(Mutex::new(None)),
    };

    let api_routes = Router::new()
        .route("/widgets/status", get(get_status_widget))
        .route("/chat", post(handle_chat))
        .route("/chat/stream", get(handle_sse_stream));

    let app = Router::new()
        .route("/", get(render_index))
        .route("/ui/dashboard", get(render_dashboard))
        .route("/ui/cortex", get(render_cortex))
        .route("/ui/memory", get(render_memory))
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

async fn render_dashboard() -> impl IntoResponse { Html(DashboardTemplate {}.render().unwrap()) }
async fn render_cortex() -> impl IntoResponse { Html(CortexTemplate {}.render().unwrap()) }
async fn render_memory() -> impl IntoResponse { Html(MemoryTemplate {}.render().unwrap()) }

async fn handle_chat(
    State(state): State<AppState>,
    Form(input): Form<ChatInput>
) -> impl IntoResponse {
    // Handling the Consensus Stream Request locally
    if input.provider == "consensus" {
        *state.pending_debate_prompt.lock().await = Some(input.prompt.clone());
        let html = format!(
            "<div class='message system' hx-ext='sse' sse-connect='/api/chat/stream'>\
                <i data-lucide='brain-circuit'></i>\
                <div class='message-content' sse-swap='message' hx-swap='beforeend'>\
                   <strong style='color: var(--highlight-color);'>⚖️ Mode Débat (Triangulation LLM)</strong><br>\
                   <div style='font-style: italic; color: #888;'>Connexion au flux cognitif Synchrone établie...</div>\
                </div>\
            </div><script>if(typeof lucide !== 'undefined') lucide.createIcons();</script>"
        );
        return Html(html).into_response();
    }

    let mut cortex = state.agent.lock().await;

    // 1. Initialiser le Provider (Gemini/Mistral/Local)
    cortex.set_provider(&input.provider);

    // 2. Résolution cognitive RAG
    let json_resp = cortex.generate_thought(&input.prompt).await.unwrap_or_else(|e| format!("{{ \"content\": \"Erreur Cortex: {}\", \"source\": {{\"ParadoxEngine\": \"Error\"}} }}", e));
    
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
        prompt: input.prompt,
        model_name,
        response_md: html_output,
        jsonai: json_resp,
        latency,
        consensus,
    };
    
    Html(tmpl.render().unwrap()).into_response()
}

async fn get_status_widget() -> impl IntoResponse {
    use axum::response::Html;
    Html("<div class='status-indicator' style='display: flex; align-items: center; gap: 8px;'><span class='pulse-dot' style='width: 8px; height: 8px; background-color: #2ecc71; border-radius: 50%; box-shadow: 0 0 8px #2ecc71;'></span><span class='status-text' style='color: #2ecc71; font-weight: 500;'>Cortex: Online (Synced)</span></div>".to_string())
}

async fn handle_sse_stream(State(state): State<AppState>) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let prompt_opt = state.pending_debate_prompt.lock().await.take();
    let prompt = prompt_opt.unwrap_or_else(|| "Erreur : Aucun prompt en attente.".to_string());
    
    let (tx, rx) = tokio::sync::mpsc::channel(10);
    let agent_arc = state.agent.clone();
    
    tokio::spawn(async move {
        let mut agent = agent_arc.lock().await;
        agent.set_provider("consensus");
        if let Err(e) = agent.run_debate(&prompt, tx.clone()).await {
            let _ = tx.send(DebateEvent::SystemEvent(format!("[ERREUR CASTRATHROPHIC CORTEX]: {}", e))).await;
        }
    });

    let stream = ReceiverStream::new(rx).map(|event| {
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
                let parser = pulldown_cmark::Parser::new(&content);
                let mut md_html = String::new();
                pulldown_cmark::html::push_html(&mut md_html, parser);
                format!("<hr style='border-color: rgba(255,255,255,0.1); margin: 20px 0;'>\
                         <h3 style='color: var(--accent-color);'>🤝 Synthèse Absolue</h3>\
                         <div class='markdown-body'>{}</div>\
                         <script>if(typeof lucide !== 'undefined') lucide.createIcons();</script>\
                         <br><strong style='color: #2ecc71;'>[FIN DE LA TRIANGULATION - SOCKET CONSERVEE]</strong>", md_html)
            }
        };
        Ok(Event::default().data(html))
    }).chain(tokio_stream::pending());

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new())
}
