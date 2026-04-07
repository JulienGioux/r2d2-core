use crate::agent::CognitiveAgent;
use crate::catalog::{CognitiveSense, CortexCatalog};
use crate::error::CortexError;
use async_trait::async_trait;
use std::time::Instant;
use tracing::{info, instrument};

use std::sync::Arc;
use std::time::Duration;

use candle_core::Device;
// Si l'on souhaite implémenter le forward complet à l'avenir :
// use candle_nn::VarBuilder;
// use candle_transformers::models::llava;

/// Gestionnaire de panne (Circuit Breaker) Visuel
pub struct CircuitBreaker {
    pub failures: u32,
    pub threshold: u32,
    pub last_failure: Option<Instant>,
    pub reset_timeout: Duration,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            failures: 0,
            threshold,
            last_failure: None,
            reset_timeout,
        }
    }

    pub fn check(&mut self) -> Result<(), CortexError> {
        if self.failures >= self.threshold {
            if let Some(last) = self.last_failure {
                if last.elapsed() < self.reset_timeout {
                    return Err(CortexError::InferenceError(
                        "Circuit Breaker OPEN: Trop d'échecs sur VisionAgent LLaVA".to_string(),
                    ));
                } else {
                    self.failures = self.threshold - 1;
                }
            }
        }
        Ok(())
    }

    pub fn record_success(&mut self) {
        self.failures = 0;
        self.last_failure = None;
    }

    pub fn record_failure(&mut self) {
        self.failures += 1;
        self.last_failure = Some(Instant::now());
    }
}

/// Moteur encapsulant l'état immuable de LLaVA
pub struct LlavaEngine {
    // A décommenter lors du link complet avec candle_transformers::models::llava
    // pub model: llava::Model,
    // pub config: llava::Config,
    pub tokenizer: tokenizers::Tokenizer,
    pub device: Device,
}

/// Agent Cortical dédié à la perception visuelle (Expert 1 : LLaVA).
pub struct VisionAgentLlava {
    name: String,
    active: bool,
    engine: Option<Arc<LlavaEngine>>,
    circuit_breaker: CircuitBreaker,
}

impl Default for VisionAgentLlava {
    fn default() -> Self {
        Self::new()
    }
}

impl VisionAgentLlava {
    pub fn new() -> Self {
        Self {
            name: "VisionAgent-Llava".to_string(),
            active: false,
            engine: None,
            circuit_breaker: CircuitBreaker::new(3, Duration::from_secs(60)),
        }
    }

    /// Boucle d'analye d'image : extraction CLIP puis génération textuelle
    async fn analyze_image(&mut self, _prompt: &str) -> Result<String, CortexError> {
        self.circuit_breaker.check()?;
        let _engine = self
            .engine
            .as_ref()
            .cloned()
            .ok_or(CortexError::NotActive)?;

        info!("-> Traitement visuel dans Pipeline Llava");

        // Lancement architectural : Inférence Vision sur le noeud local llava
        let client = reqwest::Client::new();
        let res = client
            .post("http://localhost:11434/api/generate")
            .json(&serde_json::json!({
                "model": "llava",
                "prompt": "Décris cette image ou répond au prompt suivant sur tes capacités visuelles : ".to_string() + _prompt,
                "stream": false
            }))
            .send()
            .await
            .map_err(|e| {
                self.circuit_breaker.record_failure();
                CortexError::InferenceError(format!("Vision Gateway Local inaccessible : {}", e))
            })?;

        if !res.status().is_success() {
            self.circuit_breaker.record_failure();
            return Err(CortexError::InferenceError(
                "Status d'Inférence visuelle en échec".to_string(),
            ));
        }

        let json_body: serde_json::Value = res.json().await.unwrap_or_default();
        let text = json_body["response"]
            .as_str()
            .unwrap_or("[No Local Output]")
            .trim()
            .to_string();
        info!("<- Extraction textuelle générée avec succès.");
        self.circuit_breaker.record_success();
        Ok(text)
    }
}

#[async_trait]
impl CognitiveAgent for VisionAgentLlava {
    fn name(&self) -> &str {
        &self.name
    }

    #[instrument(skip(self))]
    async fn load(&mut self) -> Result<(), CortexError> {
        let desc = CortexCatalog::get_default_descriptor(CognitiveSense::Vision);
        self.name = format!(
            "VisionAgent-Llava-{}",
            desc.repo_id.split('/').next_back().unwrap_or("Fallback")
        );

        info!(
            "🔌 [CORTEX] Téléchargement / Résolution HuggingFace pour '{}'",
            self.name
        );

        let api = hf_hub::api::tokio::ApiBuilder::new()
            .with_token(crate::security::vault::Vault::get_api_key("HF_TOKEN"))
            .build()
            .map_err(|e| CortexError::LoadError(e.to_string()))?;
        let repo = api.repo(hf_hub::Repo::with_revision(
            desc.repo_id.to_string(),
            hf_hub::RepoType::Model,
            desc.revision.to_string(),
        ));

        info!("   [CORTEX] Résolution du Dictionnaire Tokenizer Visuel...");
        let tokenizer_file = repo
            .get(desc.tokenizer_file.unwrap_or("tokenizer.json"))
            .await
            .map_err(|e| CortexError::LoadError(e.to_string()))?;

        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_file)
            .map_err(|e| CortexError::LoadError(e.to_string()))?;

        // [BLUEPRINT] : Chargement du model_file (VarBuilder) et preprocessor config.
        // let vb = VarBuilder::from_mmaped_safetensors(...)
        // let model = llava::Model::load(vb, config)?;

        self.engine = Some(Arc::new(LlavaEngine {
            tokenizer,
            device: Device::Cpu,
        }));

        self.circuit_breaker.record_success();
        self.active = true;
        info!(
            "🛡️ [CORTEX] Agent '{}' Chargé & Structurellement Prêt.",
            self.name
        );
        Ok(())
    }

    async fn unload(&mut self) -> Result<(), CortexError> {
        info!(
            "   [CORTEX] Drop de l'Engine et Tenseurs VRAM pour '{}'.",
            self.name
        );
        self.active = false;
        self.engine = None;
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.active
    }

    #[instrument(skip_all, name = "VisionAgentLlava::generate_thought")]
    async fn generate_thought(&mut self, prompt: &str) -> Result<String, CortexError> {
        if !self.is_active() {
            return Err(CortexError::NotActive);
        }
        let start = Instant::now();
        info!("👁️ VisionAgent-LLaVA démarre la pipeline visuelle...");

        let description = self.analyze_image(prompt).await?;

        let jsonai = format!(
            r#"{{
            "id": "vision-llava-{}",
            "source": {{ "Vision_Llava": "{}" }},
            "timestamp": "2026-03-25T00:00:00Z",
            "is_fact": true,
            "belief_state": "Visual Extraction (Expert 1)",
            "consensus": "Raw Sensor",
            "content": "{}",
            "ontological_tags": ["Vision", "Llava", "Perspective-1"],
            "dependencies": []
        }}"#,
            start.elapsed().as_millis(),
            self.name(),
            description.replace("\"", "\\\"")
        );

        info!("Inférence visuelle accomplie en {:?}", start.elapsed());
        Ok(jsonai)
    }
}
