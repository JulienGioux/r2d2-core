use axum::{
    extract::Query,
    response::sse::{Event, Sse},
};
use serde::Deserialize;
use std::convert::Infallible;

#[derive(Deserialize)]
pub struct InferenceRequest {
    pub prompt: String,
}

/// Endpoint Axum SSE dédié à l'Inférence brute du Moteur Chimera 1.58-bit.
/// Permet l'affichage fluide HTMX (Zero-JS) via Server-Sent Events.
pub async fn chimera_stream_handler(
    Query(req): Query<InferenceRequest>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    tracing::info!(
        "🧠 Lancement Inférence QAT-Scratch sur le prompt : {}",
        req.prompt
    );

    let stream = async_stream::stream! {
        // Exécution bloquante off-loadée pour ne pas geler l'Event Loop
        let prompt_clone = req.prompt.clone();
        let result = tokio::task::spawn_blocking(move || {
            // Dans une version finale, ce moteur serait persistant en mémoire dans l'AppState.
            // Pour l'intégration V1.1, on instancie l'Agent R2D2 natif isolément.
            use r2d2_cortex::models::chimera_agent::ChimeraAgent;
            use r2d2_cortex::agent::CognitiveAgent;

            // Exécution synchrone dans un thread de fond bloquant (Runtime local au thread)
            let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
            rt.block_on(async {
                let mut agent = ChimeraAgent::new();
                if let Err(e) = agent.load().await {
                    return format!("Erreur Cortex au chargement : {}", e);
                }
                match agent.generate_thought(&prompt_clone).await {
                    Ok(text) => text,
                    Err(e) => format!("Erreur Cortex à l'inférence : {}", e),
                }
            })
        })
        .await
        .unwrap_or_else(|e| format!("Erreur critique du Thread Inférentiel : {}", e));

        // Proxy "Streaming Matérialisé" : On décompose la réponse globale en streaming HTMX
        let tokens: Vec<&str> = result.split_whitespace().collect();
        for token in tokens {
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            yield Ok(Event::default().data(format!("{} ", token)));
        }

        yield Ok(Event::default().data("[DONE]"));
    };

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new())
}
