use r2d2_blackboard::PostgresBlackboard;
use std::sync::Arc;
use tracing::{info, instrument};

/// ============================================================================
/// 📦 FOLDING ENGINE (DÉDOUBLONNEMENT ET COMPRESSION HNSW)
/// ============================================================================
pub struct FoldingEngine {
    blackboard: Arc<PostgresBlackboard>,
}

impl FoldingEngine {
    pub fn new(blackboard: Arc<PostgresBlackboard>) -> Self {
        Self { blackboard }
    }

    /// Nettoie les vecteurs sémantiquement identiques et réindexe le graphe HNSW.
    #[instrument(skip_all, name = "FoldingEngine::compress_memory")]
    pub async fn compress_memory(&self) -> anyhow::Result<usize> {
        info!("🔍 Déclenchement du Folding Engine : Scan sémantique PostgreSQL...");

        let duplicates_found = self
            .blackboard
            .compress_vector_index()
            .await
            .map_err(|e| anyhow::anyhow!("Échec de compression vectorielle : {}", e))?;

        if duplicates_found > 0 {
            info!(
                "📦 {} fragments redondants détectés et purgés. Fusion Sémantique en cours...",
                duplicates_found
            );
            info!("🗑️ Élagage (Pruning) de l'index vectoriel terminé. Espace DB libéré.");
        } else {
            info!("✅ Aucun pliage nécessaire. Le graphe HNSW est optimal.");
        }

        Ok(duplicates_found)
    }
}
