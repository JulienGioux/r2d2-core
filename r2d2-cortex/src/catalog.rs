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
                // intfloat/multilingual-e5-small : ~130 Mo, 384 dimensions. Le standard absolu.
                repo_id: "intfloat/multilingual-e5-small",
                revision: "main",
                weights_file: "model.safetensors",
                tokenizer_file: Some("tokenizer.json"),
                config_file: Some("config.json"),
                auxiliary_repo: None,
                auxiliary_files: None,
                required_ram_gb: 0.15,
            },
            CognitiveSense::Audio => ModelDescriptor {
                // openai/whisper-tiny : ~75 Mo. Transcrit le flux à la volée. Frugalité totale.
                repo_id: "openai/whisper-tiny",
                revision: "main",
                weights_file: "model.safetensors",
                tokenizer_file: Some("tokenizer.json"),
                config_file: Some("config.json"),
                auxiliary_repo: None,
                auxiliary_files: None,
                required_ram_gb: 0.2,
            },
            CognitiveSense::Vision => ModelDescriptor {
                // llava-1.5-7b-hf quantifié ou llava-phi. On s'appuie sur la structure LLaVA standard.
                // Note : Pour une RAM de 8Go, on privilégiera des poids GGUF q4k si disponibles, ou des variantes réduites.
                repo_id: "llava-hf/llava-1.5-7b-hf",
                revision: "main",
                weights_file: "model.safetensors.index.json", // Load shard index ou gguf
                tokenizer_file: Some("tokenizer.json"),
                config_file: Some("config.json"),
                auxiliary_repo: None,
                auxiliary_files: Some(vec!["preprocessor_config.json"]),
                required_ram_gb: 4.5,
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
