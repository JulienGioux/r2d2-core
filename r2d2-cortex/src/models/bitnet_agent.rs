use crate::agent::{AgentError, CognitiveAgent};
use async_trait::async_trait;
use candle_core::{DType, Device};
use candle_nn::VarBuilder;
use r2d2_bitnet::model::{BitNetConfig, BitNetModel};
use r2d2_bitnet::InferenceWeights;
use std::sync::Arc;
use tokenizers::Tokenizer;
use tracing::{info, instrument};

/// Agent IA Natif : R2D2-BitNet (1.58-bit)
///
/// Contrairement aux modèles externes (GGUF, Llama.cpp), cet agent
/// s'exécute silencieusement, sans MatMul, directement dans le CPU local
/// grâce à l'architecture Ternaire MathMul-Free développée "from scratch".
pub struct BitNetAgent {
    name: String,
    device: Device,
    model: Option<Arc<BitNetModel<InferenceWeights>>>,
    tokenizer: Option<Arc<Tokenizer>>,
}

impl BitNetAgent {
    pub fn new() -> Self {
        Self {
            name: "R2D2-BitNet-Native".to_string(),
            device: Device::Cpu,
            model: None,
            tokenizer: None,
        }
    }
}

impl Default for BitNetAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CognitiveAgent for BitNetAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_active(&self) -> bool {
        self.model.is_some()
    }

    #[instrument(skip(self))]
    async fn load(&mut self) -> Result<(), AgentError> {
        info!("🔌 [CORTEX] Chargement structurel du modèle natif R2D2-BitNet (1.58-bit)...");

        let config = BitNetConfig::default();

        info!("   [BitNet] Résolution des poids via HuggingFace Hub...");
        let api_result = hf_hub::api::tokio::ApiBuilder::new()
            .with_token(crate::security::vault::Vault::get_api_key("HF_TOKEN"))
            .build();

        let (vb, tokenizer) = match api_result {
            Ok(api) => {
                let repo = api.model("1bitLLM/bitnet_b1_58-3B".to_string());

                let tok_res = repo.get("tokenizer.json").await;
                let tokenizer = if let Ok(tok_path) = tok_res {
                    Tokenizer::from_file(&tok_path).ok()
                } else {
                    None
                };

                match repo.get("model.safetensors").await {
                    Ok(weights_filename) => {
                        info!("✅ Poids BitNet localisés : {:?}", weights_filename);
                        let vb_res = unsafe {
                            VarBuilder::from_mmaped_safetensors(
                                &[weights_filename],
                                DType::F32,
                                &self.device,
                            )
                        };
                        match vb_res {
                            Ok(vb) => (vb, tokenizer),
                            Err(e) => {
                                return Err(AgentError::LoadError(format!(
                                    "Erreur Mmap Safetensors : {}",
                                    e
                                )));
                            }
                        }
                    }
                    Err(e) => {
                        return Err(AgentError::LoadError(format!(
                            "Impossible de télécharger les poids depuis HuggingFace : {}",
                            e
                        )));
                    }
                }
            }
            Err(e) => {
                return Err(AgentError::LoadError(format!(
                    "API HF inaccessible : {}",
                    e
                )));
            }
        };

        let model = BitNetModel::<InferenceWeights>::load_inference(vb, &config)
            .map_err(|e| AgentError::LoadError(format!("Erreur d'ancrage BitNet: {}", e)))?;

        self.model = Some(Arc::new(model));
        self.tokenizer = tokenizer.map(Arc::new);
        info!("✅ [CORTEX] Topologie R2D2-BitNet instanciée avec succès en RAM (0 TFLOPS MatMul).");

        Ok(())
    }

    async fn unload(&mut self) -> Result<(), AgentError> {
        info!("   [CORTEX] Purge de la structure R2D2-BitNet.");
        self.model = None;
        Ok(())
    }

    #[instrument(skip(self, prompt))]
    async fn generate_thought(&mut self, prompt: &str) -> Result<String, AgentError> {
        let model = self
            .model
            .as_ref()
            .map(Arc::clone)
            .ok_or(AgentError::NotActive)?;
        let tokenizer = self
            .tokenizer
            .as_ref()
            .map(Arc::clone)
            .ok_or(AgentError::NotActive)?;
        let device = self.device.clone();

        let prompt_str = prompt.to_string();

        tokio::task::spawn_blocking(move || {
            info!("🧠 [BitNet] Réflexion Autorégressive sur le prompt (spawn_blocking)...");

            let encoding = tokenizer.encode(prompt_str, false).map_err(|e| {
                AgentError::InferenceError(format!("Tokenizer encode error: {}", e))
            })?;

            let prompt_tokens: Vec<u32> = encoding.get_ids().to_vec();

            // Limite de 512 tokens générés
            let generated_ids = model
                .generate(&prompt_tokens, 512, &device)
                .map_err(|e| AgentError::InferenceError(e.to_string()))?;

            // Reconstruction textuelle réelle via BPE tokenizer
            let generated_text = tokenizer.decode(&generated_ids, true).map_err(|e| {
                AgentError::InferenceError(format!("Tokenizer decode error: {}", e))
            })?;

            Ok(generated_text)
        })
        .await
        .map_err(|e| AgentError::InferenceError(format!("Thread local panic: {}", e)))?
    }
}
