//! # Brique Principale de Gestion des Topologies (Model Store Zero-Trust)
//!
//! Ce module héberge le `ModelStore`, le registre souverain des poids mathématiques `Arc<T>`.
//! Suivant la doctrine R2D2, **aucun Agent Cognitif n'est persistant**. L'Store s'assure
//! que seuls les poids (inertes et mathématiques) résident en mémoire, permettant
//! l'instanciation pure et jetable ("Zeroization") des Agents à la volée.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;

use anyhow::Result;
use tracing::{info, instrument};

use candle_core::Device;
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hf_hub::{api::tokio::ApiBuilder, Repo, RepoType};
use tokenizers::Tokenizer;

use crate::catalog::{CognitiveSense, CortexCatalog};

/// Instance globale unique du Store, pour assurer l'unicité des tenseurs en RAM.
pub static GLOBAL_MODEL_STORE: OnceLock<Arc<ModelStore>> = OnceLock::new();

/// Point d'accès universel au magasin de vecteurs.
pub fn get_model_store() -> Arc<ModelStore> {
    GLOBAL_MODEL_STORE
        .get_or_init(|| Arc::new(ModelStore::new()))
        .clone()
}

/// Structure inerte contenant les tenseurs mmapés d'un modèle BERT.
/// Tout état dynamique est formellement banni de cette structure.
#[derive(Clone)]
pub struct BertTopology {
    pub model: Arc<BertModel>,
    pub tokenizer: Arc<Tokenizer>,
}

/// Registre Cerveau. Partage ses poids inaltérables avec les agents.
pub struct ModelStore {
    bert_topologies: RwLock<HashMap<String, BertTopology>>,
    device: Device,
}

impl ModelStore {
    /// Initialise un nouveau ModelStore verrouillé sur le hardware demandé (CPU pour Frugalité extreme ou Cuda).
    pub fn new() -> Self {
        Self {
            bert_topologies: RwLock::new(HashMap::new()),
            device: Device::Cpu, // Frugalité par défaut
        }
    }

    /// Extrait (ou charge si c'est la première fois) les poids inertes d'un modèle Sémantique (Bert).
    #[instrument(skip(self))]
    pub async fn checkout_bert(&self, id: &str) -> Result<BertTopology> {
        {
            let lock = self.bert_topologies.read().await;
            if let Some(topology) = lock.get(id) {
                return Ok(topology.clone());
            }
        }

        info!("🛡️ [MODEL-STORE] Poids inertes manquants pour '{}'. Déclenchement de la Forge (HF Hub)...", id);

        // Récupération via le Catalogue Interne pour obtenir les liens précis (Zero-Config)
        // Idéalement on devrait lire la configuration de la BDD, mais on fallback sur le Catalog par défaut.
        let desc = CortexCatalog::get_default_descriptor(CognitiveSense::Semantic);

        // TODO: Retirer Vault si c'est public, ou bien le garder pour HuggingFace PRO.
        let token = crate::security::vault::Vault::get_api_key("HF_TOKEN");

        let api = ApiBuilder::new().with_token(token).build()?;

        let repo = api.repo(Repo::with_revision(
            desc.repo_id.to_string(),
            RepoType::Model,
            desc.revision.to_string(),
        ));

        info!("   [MODEL-STORE] Résolution des poids Safetensors...");
        let model_file = repo.get(desc.weights_file).await?;

        info!("   [MODEL-STORE] Résolution du dictionnaire Tokenizer...");
        let tokenizer_file = repo.get(desc.tokenizer_file.unwrap()).await?;
        let config_file = repo.get(desc.config_file.unwrap()).await?;

        info!(
            "   [MODEL-STORE] Montage de la structure Tensorielle sur {:?}...",
            self.device
        );

        let tokenizer = Tokenizer::from_file(tokenizer_file)
            .map_err(|e| anyhow::anyhow!("Erreur Tokenizer: {}", e))?;

        let config_str = std::fs::read_to_string(config_file)?;
        let config: Config = serde_json::from_str(&config_str)?;

        let vb =
            unsafe { VarBuilder::from_mmaped_safetensors(&[model_file], DTYPE, &self.device) }?;

        let model = BertModel::load(vb, &config)?;

        let topology = BertTopology {
            model: Arc::new(model),
            tokenizer: Arc::new(tokenizer),
        };

        {
            let mut lock = self.bert_topologies.write().await;
            lock.insert(id.to_string(), topology.clone());
        }

        info!(
            "✅ [MODEL-STORE] Topologie '{}' verrouillée et prête à l'emploi.",
            id
        );

        Ok(topology)
    }
}

impl Default for ModelStore {
    fn default() -> Self {
        Self::new()
    }
}
