use std::path::PathBuf;

/// Définit le type de stimulus capté par le R2D2 Sensory Gateway.
/// L'architecture Hexagonale permet de rajouter des sens physiques futuristes (ex: Tactile via robotique).
#[derive(Debug, Clone)]
pub enum StimulusType {
    /// Flux vocal ou enregistrement sonore
    Audio,
    /// Image statique ou Keyframe extraite
    Visual,
    /// Fichier texte, PDF ou Markdown
    Document,
    /// Flux vidéo continu
    VideoStream,
}

/// Conteneur générique sécurisé pour une donnée sensorielle brute.
#[derive(Debug, Clone)]
pub struct Stimulus {
    pub id: String,
    pub stimulus_type: StimulusType,
    pub payload_path: PathBuf,
    pub metadata: serde_json::Value,
}

impl Stimulus {
    pub fn new(id: impl Into<String>, stimulus_type: StimulusType, path: PathBuf) -> Self {
        Self {
            id: id.into(),
            stimulus_type,
            payload_path: path,
            metadata: serde_json::json!({}),
        }
    }
}
