use anyhow::Result;
use r2d2_sensory::stimulus::Stimulus;

/// Un utilitaire pour le chunking dynamique de textes pour le RAG
pub struct TextChunker;

impl TextChunker {
    /// Taille de bloc recommandée pour l'ingestion LLM: ~200 mots, Chevauchement: ~40 mots.
    pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut chunks = Vec::new();
        let mut i = 0;

        let max_chars = 1500; // Limite absolue "Token Explosion" du modèle MiniLM local (512 tokens)

        while i < words.len() {
            let mut end = (i + chunk_size).min(words.len());

            if end < words.len() {
                let mut search_idx = end;
                while search_idx > i && search_idx > end - overlap {
                    // Respecter les frontières sémantiques basiques (fin de phrase ou de ligne)
                    if words[search_idx - 1].ends_with('.') || words[search_idx - 1].ends_with('\n') {
                        end = search_idx;
                        break;
                    }
                    search_idx -= 1;
                }
            }

            let chunk_str = words[i..end].join(" ");

            if chunk_str.len() > max_chars {
                let chars: Vec<char> = chunk_str.chars().collect();
                let mut char_idx = 0;
                while char_idx < chars.len() {
                    let c_end = (char_idx + max_chars).min(chars.len());
                    let sub_chunk: String = chars[char_idx..c_end].iter().collect();
                    chunks.push(sub_chunk);
                    let char_overlap = max_chars / 4;
                    if c_end == chars.len() {
                        break;
                    }
                    char_idx = c_end - char_overlap;
                }
            } else {
                chunks.push(chunk_str);
            }

            if end == words.len() {
                break;
            }

            i = end - overlap;
        }

        chunks
    }
}
