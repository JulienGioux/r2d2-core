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
use crate::error::AppError;

pub async fn chimera_stream_handler(
    Query(req): Query<InferenceRequest>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, AppError> {
    tracing::info!(
        "🧠 Lancement Inférence QAT-Scratch sur le prompt : {}",
        req.prompt
    );

    // Chargement de l'agent en dehors du stream pour pouvoir remonter une HTTP 500 proprement au routeur Axum (captée par HTMX)
    let load_result = tokio::task::spawn_blocking(|| {
        use r2d2_cortex::agent::CognitiveAgent;
        use r2d2_cortex::models::chimera_agent::ChimeraAgent;
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut agent = ChimeraAgent::new();
            agent.load().await?;
            Ok::<_, r2d2_cortex::error::CortexError>(agent)
        })
    })
    .await
    .unwrap();

    let mut agent = match load_result {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("Erreur Cortex au chargement : {}", e);
            // Retourne une erreur stricte axum HTTP 500 Zéro-Information Leak (grâce au trait IntoResponse de AppError)
            return Err(anyhow::anyhow!("Erreur lors du chargement des tenseurs: {}", e).into());
        }
    };

    let stream = async_stream::stream! {
        let prompt_clone = req.prompt.clone();

        let result = tokio::task::spawn_blocking(move || {
            use r2d2_cortex::agent::CognitiveAgent;
            let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
            rt.block_on(async {
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

    Ok(Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new()))
}
