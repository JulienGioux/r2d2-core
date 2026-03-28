use crate::agent::{AgentError, CognitiveAgent};
use anyhow::Result;
use async_trait::async_trait;
use tracing::{info, instrument};

// Importations Candle et Tokenizer
use crate::catalog::{CognitiveSense, CortexCatalog};
use candle_core::{Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hf_hub::{Repo, RepoType};
use tokenizers::Tokenizer;

/// Unité d'Extraction Sémantique Multilingue Légère (400 Mo).
/// Utilisée pour convertir le texte brut en tenseur HNSW (Vecteur).
/// Modèle: "intfloat/multilingual-e5-small".
pub struct MiniLmEmbedderAgent {
    name: String,
    device: Device,
    tokenizer: Option<Tokenizer>,
    model: Option<BertModel>,
}

impl MiniLmEmbedderAgent {
    pub fn new() -> Self {
        // En forçant le mode CPU, l'Architecte garantit que l'agent consommera la RAM classique
        // et n'entamera pas la VRAM, tout en restant extrêmement rapide de par sa taille native.
        let device = Device::Cpu;

        Self {
            name: "Multilingual-E5-Small".to_string(),
            device,
            tokenizer: None,
            model: None,
        }
    }
}

impl Default for MiniLmEmbedderAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CognitiveAgent for MiniLmEmbedderAgent {
    fn name(&self) -> &str {
        &self.name
    }

    #[instrument(skip(self))]
    async fn load(&mut self) -> Result<(), AgentError> {
        info!(
            "🔌 [CORTEX] Activation du téléchargement Auto/Local pour l'agent '{}'",
            self.name
        );

        let desc = CortexCatalog::get_default_descriptor(CognitiveSense::Semantic);

        let api =
            hf_hub::api::tokio::Api::new().map_err(|e| AgentError::LoadError(e.to_string()))?;
        let repo = api.repo(Repo::with_revision(
            desc.repo_id.to_string(),
            RepoType::Model,
            desc.revision.to_string(),
        ));

        // Téléchargement des 3 artefacts critiques
        info!("   [CORTEX] Résolution des poids Safetensors...");
        let model_file = repo
            .get(desc.weights_file)
            .await
            .map_err(|e| AgentError::LoadError(format!("Échec téléchargement weights: {}", e)))?;

        info!("   [CORTEX] Résolution du dictionnaire Tokenizer...");
        let tokenizer_file = repo
            .get(desc.tokenizer_file.unwrap())
            .await
            .map_err(|e| AgentError::LoadError(format!("Échec téléchargement tokenizer: {}", e)))?;

        let config_file = repo
            .get(desc.config_file.unwrap())
            .await
            .map_err(|e| AgentError::LoadError(format!("Échec téléchargement config: {}", e)))?;

        info!("   [CORTEX] Montage de la structure Tensorielle en RAM...");

        // Parser R2D2 du fichier JSON Tokenizer
        let tokenizer = Tokenizer::from_file(tokenizer_file)
            .map_err(|e| AgentError::LoadError(e.to_string()))?;

        // Chargement strict des Tenseurs Safetensors (Zero-Trust)
        let config_str = std::fs::read_to_string(config_file)
            .map_err(|e| AgentError::LoadError(e.to_string()))?;
        let config: Config = serde_json::from_str(&config_str)
            .map_err(|e| AgentError::LoadError(format!("JSON Error: {}", e)))?;

        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[model_file], DTYPE, &self.device) }
            .map_err(|e| AgentError::LoadError(e.to_string()))?;

        let model =
            BertModel::load(vb, &config).map_err(|e| AgentError::LoadError(e.to_string()))?;

        // Ancrage dans la structure (Agent Actif)
        self.tokenizer = Some(tokenizer);
        self.model = Some(model);

        info!("🛡️ [CORTEX] Agent '{}' Chargé & Opérationnel.", self.name);
        Ok(())
    }

    async fn unload(&mut self) -> Result<(), AgentError> {
        info!(
            "   [CORTEX] Drop inconditionnel des Tenseurs RAM pour '{}'.",
            self.name
        );
        self.model = None;
        self.tokenizer = None;
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.model.is_some() && self.tokenizer.is_some()
    }

    async fn generate_thought(&mut self, prompt: &str) -> Result<String, AgentError> {
        let vec_f32 = self.embed_raw(prompt, true).await?;
        // Format R2D2: On exporte sous format JSON array
        let str_export = serde_json::to_string(&vec_f32).unwrap();
        Ok(str_export)
    }
}

impl MiniLmEmbedderAgent {
    /// Méthode spécialisée pour la Brique VIII (RAG Zero-Copy).
    /// Permet de choisir le préfixe ('query: ' ou 'passage: ') selon E5.
    pub async fn embed_raw(&mut self, prompt: &str, is_query: bool) -> Result<Vec<f32>, AgentError> {
        if !self.is_active() {
            return Err(AgentError::NotActive);
        }

        let tokenizer = self.tokenizer.as_ref().unwrap();
        let model = self.model.as_ref().unwrap();

        let prefix = if is_query { "query: " } else { "passage: " };
        let e5_prompt = format!("{}{}", prefix, prompt);
        
        let tokens = tokenizer
            .encode(e5_prompt, true)
            .map_err(|e| AgentError::InferenceError(e.to_string()))?;

        let mut token_ids = tokens.get_ids().to_vec();
        
        // Axiome Frugalité & Sécurité : Les transformers E5 panic si token > 512 (Position Embeddings)
        if token_ids.len() > 512 {
            tracing::warn!("⚠️ Truncature sémantique automatique: le vecteur dépasse 512 tokens ({}). Le focus seul est conservé.", token_ids.len());
            let sep_token = token_ids.last().copied().unwrap_or(2); // 2 est souvent SEP en BERT
            token_ids.truncate(512);
            token_ids[511] = sep_token;
        }

        let token_tensor = Tensor::new(token_ids.as_slice(), &self.device)
            .map_err(|e| AgentError::InferenceError(e.to_string()))?
            .unsqueeze(0)
            .map_err(|e| AgentError::InferenceError(e.to_string()))?;

        let token_type_ids = token_tensor.zeros_like().unwrap();

        let embeddings = model
            .forward(&token_tensor, &token_type_ids, None)
            .map_err(|e| AgentError::InferenceError(e.to_string()))?;

        let cls_embedding = embeddings
            .i((0, 0, ..))
            .map_err(|e| AgentError::InferenceError(e.to_string()))?;

        let vec_f32: Vec<f32> = cls_embedding.to_vec1().unwrap();

        if vec_f32.len() < 1024 {
            // E5 sort 384 dimensions. Pgvector exige parfois 1024.
            // Mais pour notre RAG binaire, on veut garder le RAW 384 !
            // Donc si on est appelé par embed_raw, on retourne RAW (384).
            // Le padding à 1024 va se faire dans generate_thought si on veut, 
            // mais gardons le RAW réel pour être Bare-Metal et frugal (384).
        }
        
        Ok(vec_f32)
    }
}

