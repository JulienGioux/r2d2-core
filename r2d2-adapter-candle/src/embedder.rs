use r2d2_kernel::ports::{DomainError, EmbeddingVector, TextEmbedder};
use r2d2_registry::manifest::ModelManifest;
use std::future::Future;
use std::pin::Pin;

use candle_core::{Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use tokenizers::Tokenizer;

pub struct CandleEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    dimension: usize,
    device: Device,
}

impl CandleEmbedder {
    pub fn new(manifest: &ModelManifest) -> Result<Self, anyhow::Error> {
        let dimension = manifest.topology.vector_dimension.unwrap_or(384);

        // Map Domain Type to Physical Device
        let device = match manifest.topology.device {
            r2d2_registry::types::TargetDevice::Cpu => Device::Cpu,
            r2d2_registry::types::TargetDevice::Gpu(id) => Device::new_cuda(id)?,
        };

        let storage = &manifest.storage;
        let weights_path = storage
            .weights_path
            .as_ref()
            .expect("Weights path required");
        let tokenizer_path = storage
            .tokenizer_path
            .as_ref()
            .expect("Tokenizer path required");
        let config_path = storage.config_path.as_ref().expect("Config path required");

        // Load Tokenizer
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Tokenizer Error: {}", e))?;

        // Extract Configuration Dynamically
        let config_str = std::fs::read_to_string(config_path)?;
        let config: Config = serde_json::from_str(&config_str)?;

        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device) }?;
        let model = BertModel::load(vb, &config)?;

        Ok(Self {
            model,
            tokenizer,
            dimension,
            device,
        })
    }
}

impl TextEmbedder for CandleEmbedder {
    fn embed_text<'a>(
        &'a self,
        text: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<EmbeddingVector, DomainError>> + Send + 'a>> {
        Box::pin(async move {
            let tokens = self
                .tokenizer
                .encode(text, true)
                .map_err(|e| DomainError::EncodingError(e.to_string()))?;

            let mut token_ids = tokens.get_ids().to_vec();
            if token_ids.len() > 512 {
                let sep_token = token_ids.last().copied().unwrap_or(2);
                token_ids.truncate(512);
                token_ids[511] = sep_token;
            }

            let token_tensor = Tensor::new(token_ids.as_slice(), &self.device)
                .map_err(|e| DomainError::InferenceError(e.to_string()))?
                .unsqueeze(0)
                .map_err(|e| DomainError::InferenceError(e.to_string()))?;

            let token_type_ids = token_tensor
                .zeros_like()
                .map_err(|e| DomainError::InferenceError(e.to_string()))?;

            let embeddings = self
                .model
                .forward(&token_tensor, &token_type_ids, None)
                .map_err(|e| DomainError::InferenceError(e.to_string()))?;

            let cls_embedding = embeddings
                .i((0, 0, ..))
                .map_err(|e| DomainError::InferenceError(e.to_string()))?;

            let vec_data = cls_embedding
                .to_vec1()
                .map_err(|e| DomainError::InferenceError(e.to_string()))?;

            // Zero-padding logic implemented here to meet expected dimensionality
            let mut final_data = vec_data;
            if final_data.len() < self.dimension {
                final_data.resize(self.dimension, 0.0);
            }

            Ok(EmbeddingVector {
                data: final_data,
                dimension: self.dimension,
            })
        })
    }

    fn expected_dimension(&self) -> usize {
        self.dimension
    }
}
