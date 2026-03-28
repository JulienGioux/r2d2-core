use anyhow::{Context, Result};
use bytemuck::try_cast_slice;
use memmap2::Mmap;
use serde::Deserialize;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

#[derive(Deserialize)]
struct ChunkMeta {
    id: usize,
    content: String,
}

/// Noyau de Recherche Sémantique Zero-Copy (Bare-Metal RAG)
/// Fonctionne sur un Mmap ultra-léger et un registre de texte.
pub struct SemanticMemory {
    /// Mmap partagé du fichier binaire de Tenseurs
    mmap: Arc<Mmap>,
    /// Metadonnées chargées en RAM (Minimaliste)
    metadata: Vec<ChunkMeta>,
}

impl SemanticMemory {
    /// Charge la mémoire sémantique
    pub fn load<P: AsRef<Path>>(bin_path: P, meta_path: P) -> Result<Self> {
        let bin_file = File::open(bin_path).context("Failed to open knowledge.bin")?;
        let mmap = unsafe { Mmap::map(&bin_file).context("Failed to mmap knowledge.bin")? };
        
        let meta_file = File::open(meta_path).context("Failed to open knowledge_meta.json")?;
        let metadata: Vec<ChunkMeta> = serde_json::from_reader(meta_file)
            .context("Failed to parse knowledge_meta.json")?;

        Ok(Self {
            mmap: Arc::new(mmap),
            metadata,
        })
    }

    /// Recherche les K chunks les plus pertinents via Similarité Cosinus
    /// La requête doit avoir été vectorisée (embed) par MiniLmEmbedder.
    pub fn search(&self, query_vec: &[f32], top_k: usize) -> Result<Vec<String>> {
        if query_vec.len() != 384 {
            anyhow::bail!("Semantic query memory mismatch. Expected 384 dimensions, got {}", query_vec.len());
        }

        // Zero copy cast : Les octets du fichier mappé vers des f32
        let db_vecs: &[f32] = try_cast_slice(&self.mmap).map_err(|e| anyhow::anyhow!("Bytemuck cast fail: {:?}", e))?;
        
        // Norme de la requête
        let q_norm = query_vec.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-8);

        let mut scores: Vec<(usize, f32)> = Vec::with_capacity(self.metadata.len());

        // Parcourir chaque chunk (vecteur de 384 dims)
        for chunk_id in 0..self.metadata.len() {
            let offset = chunk_id * 384;
            let vec_slice = &db_vecs[offset..offset + 384];
            
            let mut dot_product = 0.0;
            let mut v_norm_sq = 0.0;
            
            // Calcul Dot Product & L2 Norm simultané
            for i in 0..384 {
                dot_product += query_vec[i] * vec_slice[i];
                v_norm_sq += vec_slice[i] * vec_slice[i];
            }
            
            let v_norm = v_norm_sq.sqrt().max(1e-8);
            let cosine_sim = dot_product / (q_norm * v_norm);
            
            scores.push((chunk_id, cosine_sim));
        }

        // Trier par similarité décroissante
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut results = Vec::new();
        for (id, score) in scores.iter().take(top_k) {
            tracing::debug!("RAG Match ID {} avec score: {:.4}", id, score);
            if let Some(meta) = self.metadata.iter().find(|m| m.id == *id) {
                results.push(meta.content.clone());
            }
        }

        Ok(results)
    }
}
