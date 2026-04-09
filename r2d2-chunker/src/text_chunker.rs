use anyhow::Result;
use r2d2_tokenizer::R2Tokenizer;

/// Un utilitaire pour le chunking dynamique de textes pour le RAG
pub struct TextChunker;

impl TextChunker {
    /// Utilise l'architecture avancée "Token-Aware" pour découper sans perdre de signal sémantique
    /// Les chunks font `chunk_token_size` (exemple: 256) avec un chevauchement `overlap_tokens` (exemple: 40)
    pub fn chunk_text(
        tokenizer: &R2Tokenizer,
        text: &str,
        chunk_token_size: usize,
        overlap_tokens: usize,
    ) -> Result<Vec<String>> {
        let tokens = tokenizer.encode(text)?;
        let mut chunks = Vec::new();
        let mut i = 0;

        while i < tokens.len() {
            let prev_i = i;
            let mut end_idx = (i + chunk_token_size).min(tokens.len());

            // Sécurité Strict UTF-8: on ne coupe pas au milieu d'un mot asymétrique.
            if end_idx < tokens.len() {
                end_idx = tokenizer.safe_word_boundary(text, end_idx)?;
            }

            let chunk_tokens = &tokens[i..end_idx];
            let decoded_chunk = tokenizer.decode(chunk_tokens, true)?;
            chunks.push(decoded_chunk);

            if end_idx >= tokens.len() {
                break;
            }

            // Calcul de l'overlapping sécurisé
            let overlap_start = end_idx.saturating_sub(overlap_tokens);

            // Re-alignement sur une safe word boundary pour l'overlap aussi !
            i = tokenizer.safe_word_boundary(text, overlap_start)?;

            // Fallback pour éviter les boucles infinies si un mot géant dépasse l'overlap
            if i <= prev_i {
                i = end_idx; // On sacrifie l'overlap au profit de l'avancement
            }
        }

        Ok(chunks)
    }
}
