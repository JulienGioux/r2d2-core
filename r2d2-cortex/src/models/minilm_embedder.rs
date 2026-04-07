use crate::agent::CognitiveAgent;
use crate::error::CortexError;
use async_trait::async_trait;
use tracing::{info, instrument};

// Importations Candle et Tokenizer
use candle_core::{Device, IndexOp, Tensor};

use crate::store::{get_model_store, BertTopology, ModelStore};
use std::sync::Arc;
/// Unité d'Extraction Sémantique Multilingue Légère (400 Mo).
/// Utilisée pour convertir le texte brut en tenseur HNSW (Vecteur).
/// Modèle: "intfloat/multilingual-e5-small".
pub struct MiniLmEmbedderAgent {
    name: String,
    device: Device,
    store: Arc<ModelStore>,
    topology: Option<BertTopology>,
}

impl MiniLmEmbedderAgent {
    pub fn new() -> Self {
        // En forçant le mode CPU, l'Architecte garantit que l'agent consommera la RAM classique
        // et n'entamera pas la VRAM, tout en restant extrêmement rapide de par sa taille native.
        let device = Device::Cpu;

        Self {
            name: "Multilingual-E5-Small".to_string(),
            device,
            store: get_model_store(),
            topology: None,
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
    async fn load(&mut self) -> Result<(), CortexError> {
        info!(
            "🔌 [CORTEX] Extraction des tenseurs inertes depuis le Store pour '{}'",
            self.name
        );

        // Checkout de la topologie partagée via le ModelStore
        let topo = self
            .store
            .checkout_bert(&self.name)
            .await
            .map_err(|e| CortexError::LoadError(e.to_string()))?;

        // Ancrage local (l'Agent devient opérationnel mais n'own plus les tenseurs)
        self.topology = Some(topo);

        info!(
            "🛡️ [CORTEX] Agent '{}' Chargé & Opérationnel via Store.",
            self.name
        );
        Ok(())
    }

    async fn unload(&mut self) -> Result<(), CortexError> {
        info!(
            "   [CORTEX] Drop inconditionnel des vues RAM pour '{}'.",
            self.name
        );
        self.topology = None;
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.topology.is_some()
    }

    async fn generate_thought(&mut self, prompt: &str) -> Result<String, CortexError> {
        let vec_f32 = self.embed_raw(prompt, true).await?;
        // Format R2D2: On exporte sous format JSON array
        let str_export = serde_json::to_string(&vec_f32).unwrap();
        Ok(str_export)
    }
}

impl MiniLmEmbedderAgent {
    /// Méthode spécialisée pour la Brique VIII (RAG Zero-Copy).
    /// Permet de choisir le préfixe ('query: ' ou 'passage: ') selon E5.
    pub async fn embed_raw(
        &mut self,
        prompt: &str,
        is_query: bool,
    ) -> Result<Vec<f32>, CortexError> {
        if !self.is_active() {
            return Err(CortexError::NotActive);
        }

        let topology = self.topology.as_ref().unwrap();
        let tokenizer = &topology.tokenizer;
        let model = &topology.model;

        let prefix = if is_query { "query: " } else { "passage: " };
        let e5_prompt = format!("{}{}", prefix, prompt);

        let tokens = tokenizer
            .encode(e5_prompt, true)
            .map_err(|e| CortexError::InferenceError(e.to_string()))?;

        let mut token_ids = tokens.get_ids().to_vec();

        // Axiome Frugalité & Sécurité : Les transformers E5 panic si token > 512 (Position Embeddings)
        if token_ids.len() > 512 {
            tracing::warn!("⚠️ Truncature sémantique automatique: le vecteur dépasse 512 tokens ({}). Le focus seul est conservé.", token_ids.len());
            let sep_token = token_ids.last().copied().unwrap_or(2); // 2 est souvent SEP en BERT
            token_ids.truncate(512);
            token_ids[511] = sep_token;
        }

        let panic_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> Result<Vec<f32>, CortexError> {
                let token_tensor = Tensor::new(token_ids.as_slice(), &self.device)
                    .map_err(|e| CortexError::InferenceError(e.to_string()))?
                    .unsqueeze(0)
                    .map_err(|e| CortexError::InferenceError(e.to_string()))?;

                let token_type_ids = token_tensor.zeros_like().unwrap();

                let embeddings = model
                    .forward(&token_tensor, &token_type_ids, None)
                    .map_err(|e| CortexError::InferenceError(e.to_string()))?;

                let cls_embedding = embeddings
                    .i((0, 0, ..))
                    .map_err(|e| CortexError::InferenceError(e.to_string()))?;

                Ok(cls_embedding.to_vec1().unwrap())
            },
        ));

        let vec_f32 = match panic_res {
            Ok(result) => result?,
            Err(err_payload) => {
                let msg = if let Some(s) = err_payload.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = err_payload.downcast_ref::<String>() {
                    s.to_string()
                } else {
                    "Unknown panic payload".to_string()
                };
                tracing::error!("🚨 PANIC INTERCEPTÉE dans MiniLmEmbedder ! Motif: {}", msg);
                return Err(CortexError::InferencePanic(msg));
            }
        };

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
