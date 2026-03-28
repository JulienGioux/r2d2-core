use std::fmt;

/// Les "Sens" ou facultés cognitives supportés par l'écosystème R2D2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CognitiveSense {
    /// Plongement vectoriel (Embeddings HNSW) pour le ParadoxEngine
    Semantic,
    /// Reconnaissance vocale (Transcription)
    Audio,
    /// Compréhension visuelle spatiale (VLM)
    Vision,
    /// Raisonnement logique et synthèse (LLM)
    Reasoning,
}

impl fmt::Display for CognitiveSense {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CognitiveSense::Semantic => write!(f, "Sémantique (Vectorisation)"),
            CognitiveSense::Audio => write!(f, "Auditif (Transcription)"),
            CognitiveSense::Vision => write!(f, "Visuel (VLM)"),
            CognitiveSense::Reasoning => write!(f, "Cognitif (Raisonnement LLM)"),
        }
    }
}

/// Descripteur absolu d'un modèle HuggingFace.
/// Implémente la doctrine "Zero-Trust" en forçant les dépôts statiques et le poids exigé.
#[derive(Debug, Clone)]
pub struct ModelDescriptor {
    pub repo_id: &'static str,
    pub revision: &'static str,
    pub weights_file: &'static str,
    pub tokenizer_file: Option<&'static str>,
    pub config_file: Option<&'static str>,
    pub auxiliary_repo: Option<&'static str>,
    pub auxiliary_files: Option<Vec<&'static str>>,
    pub required_ram_gb: f32, // Métrique stricte pour l'Hyperviseur
}

pub struct CortexCatalog;

impl CortexCatalog {
    /// Récupère l'empreinte génétique statique (Le Modèle) associée à un sens cognitif.
    /// Ces choix sont optimisés spécifiquement pour des environnements Frugaux (RAM \<= 8Go).
    pub fn get_default_descriptor(sense: CognitiveSense) -> ModelDescriptor {
        match sense {
            CognitiveSense::Semantic => ModelDescriptor {
                // intfloat/multilingual-e5-small : ~400 Mo, 384 dimensions. Adapté RAM 12Go.
                repo_id: "intfloat/multilingual-e5-small",
                revision: "main",
                weights_file: "model.safetensors",
                tokenizer_file: Some("tokenizer.json"),
                config_file: Some("config.json"),
                auxiliary_repo: None,
                auxiliary_files: None,
                required_ram_gb: 1.0,
            },
            CognitiveSense::Audio => ModelDescriptor {
                // openai/whisper-tiny : ~150 Mo. Tolérance matérielle absolue sur Laptop GTX.
                repo_id: "openai/whisper-tiny",
                revision: "main",
                weights_file: "model.safetensors",
                tokenizer_file: Some("tokenizer.json"),
                config_file: Some("config.json"),
                auxiliary_repo: None,
                auxiliary_files: None,
                required_ram_gb: 1.5,
            },
            CognitiveSense::Vision => ModelDescriptor {
                // vikhyatk/moondream2 : Petit modèle vision (1.8B) idéal pour l'inférence RAM-limited.
                repo_id: "vikhyatk/moondream2",
                revision: "main",
                weights_file: "model.safetensors",
                tokenizer_file: Some("tokenizer.json"),
                config_file: Some("config.json"),
                auxiliary_repo: None,
                auxiliary_files: None,
                required_ram_gb: 2.5,
            },
            CognitiveSense::Reasoning => ModelDescriptor {
                // Qwen2.5-1.5B-Instruct-GGUF : Ultra-rapide pour le Cortex central, au format GGUF.
                repo_id: "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
                revision: "main",
                // Note: En mode GGUF, config/tokenizer sont souvent inclus dans le poid.
                weights_file: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
                tokenizer_file: None,
                config_file: None,
                auxiliary_repo: None,
                auxiliary_files: None,
                required_ram_gb: 1.1,
            },
        }
    }
}

