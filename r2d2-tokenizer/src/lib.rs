/// Crate Sémantique: R2D2-Tokenizer
///
/// Gère la projection de texte vers l'espace vectoriel discret (Tokenization BPE).
/// Intègre `Aho-Corasick` pour le lexing haute-performance et `tokenizers` pour la quantification texte.
use aho_corasick::{AhoCorasick, MatchKind};
use anyhow::Result;
use std::sync::Arc;
use tokenizers::Tokenizer;

pub struct LexerAutomaton {
    ac: AhoCorasick,
}

impl LexerAutomaton {
    pub fn new(patterns: Vec<&str>) -> Self {
        // MATCHKIND::LeftmostLongest est OBLIGATOIRE comme validé par IndicSuperTokenizer
        let ac = AhoCorasick::builder()
            .match_kind(MatchKind::LeftmostLongest)
            .build(patterns.clone())
            .expect("Echec de compilation DFA Aho-Corasick");

        Self { ac }
    }

    /// Extrait linéairement toutes les balises d'un texte `O(N)`
    pub fn extract_tags<'a>(&self, haystack: &'a str) -> Vec<(&'a str, usize, usize)> {
        let mut results = Vec::new();
        for mat in self.ac.find_iter(haystack) {
            results.push((&haystack[mat.start()..mat.end()], mat.start(), mat.end()));
        }
        results
    }
}

/// Enveloppe "Sovereign Shield" pour le Tokenizer BPE
#[derive(Clone)]
pub struct R2Tokenizer {
    inner: Arc<Tokenizer>,
}

impl R2Tokenizer {
    /// Initialise le tokenizer natif
    pub fn new(model_id: &str) -> Result<Self> {
        // En vrai: on téléchargerait via hf_hub, mais pour l'instant on utilise un fallback ou un model local
        let tokenizer = Tokenizer::from_pretrained(model_id, None)
            .map_err(|e| anyhow::anyhow!("Echec du chargement du tokenizer {}: {}", model_id, e))?;
        Ok(Self {
            inner: Arc::new(tokenizer),
        })
    }

    /// Encode un texte en une séquence de tokens
    pub fn encode(&self, text: &str) -> Result<Vec<u32>> {
        let encoding = self
            .inner
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("Echec d'encodage: {}", e))?;
        Ok(encoding.get_ids().to_vec())
    }

    /// Decode une séquence de tokens
    pub fn decode(&self, ids: &[u32], skip_special_tokens: bool) -> Result<String> {
        let decoded = self
            .inner
            .decode(ids, skip_special_tokens)
            .map_err(|e| anyhow::anyhow!("Echec de décodage: {}", e))?;
        Ok(decoded)
    }

    /// Trouve la limite de coupe sémantique ("word boundary") la plus proche
    /// en se basant sur les word_ids internes du BPE pour éviter de casser la séquence.
    pub fn safe_word_boundary(&self, text: &str, target_token_idx: usize) -> Result<usize> {
        let encoding = self
            .inner
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("Echec d'encodage BPE: {}", e))?;

        let word_ids = encoding.get_word_ids();
        let tokens = encoding.get_ids();

        if target_token_idx >= tokens.len() {
            return Ok(tokens.len());
        }

        // On recule jusqu'à ce que le word_id change
        // Cela garantit qu'on ne coupe pas un mot au milieu (évitant l'amnésie UTF-8 et la casse BPE)
        let mut idx = target_token_idx;
        let current_word = word_ids[idx];

        while idx > 0 {
            if word_ids[idx] != current_word {
                // On est tombé sur une frontière de mot !
                break;
            }
            idx -= 1;
        }

        Ok(idx)
    }
}
