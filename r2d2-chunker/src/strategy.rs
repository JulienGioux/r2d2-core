use anyhow::Result;
use r2d2_sensory::stimulus::Stimulus;

/// Contrat d'ingénierie stricte pour tout extracteur de média.
/// Un Chunker prend un gros média de départ (le Raw Stimulus),
/// et le débite en un itérateur de sous-stimulus parfaits pour un Agent IA spécifique.
pub trait MediaChunker {
    /// Taille préférentielle d'un fragment exprimée d'une manière propre à chaque implémentation.
    /// Ex: "30_sec" pour Audio, "512_tokens" pour Texte, "1_keyframe_per_second" pour Vision.
    fn chunk_strategy_definition(&self) -> &str;

    /// Prend le stimulus colossal principal et le hache algorithmiquement
    /// en de multiples petits fragments prêts à l'ingestion par les Tenseurs.
    /// Pour des raisons de RAM (Frugalité), ce découpage devrait idéalement lazy-loop,
    /// mais pour la Brique 6, on renvoie un simple vecteur unifié de Stimuli.
    fn chunk(&self, parent_stimulus: &Stimulus) -> Result<Vec<Stimulus>>;
}
