use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

// Wrapper local ("Newtype") pour capturer n'importe quelle erreur Anyhow ou standard
pub struct AppError(pub anyhow::Error);

// Permet l'utilisation de l'opérateur `?`
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

// Implémentation du contrat Axum pour transformer l'erreur en réponse HTTP
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("Erreur interne IHM: {:?}", self.0);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Une erreur interne critique est survenue dans l'interface.",
        )
            .into_response()
    }
}
