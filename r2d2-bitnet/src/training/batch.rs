/// Structure de donnée pour transférer la data depuis l'I/O (Dataloader) vers le moteur Math (BitNet).
/// Zéro Appel GPU / FFI : Uniquement des vecteurs purs alloués sur CPU.
pub struct TrainingBatch {
    /// Tenseur des séquences d'entrée (Tokens).
    pub input_tokens: Vec<u32>,
    /// Tenseur Cible (Next Token Prediction).
    pub target_tokens: Vec<u32>,
    /// Tenseur de Masquage (0.0 pour JSON, 1.0 pour Sémantique).
    pub loss_mask: Vec<f32>,
    /// Numéro dynamique du batch
    pub batch_idx: usize,
}

impl TrainingBatch {
    /// Crée un nouveau TrainingBatch pré-alloué pour éviter les réallocations OS.
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self {
            input_tokens: Vec::with_capacity(capacity),
            target_tokens: Vec::with_capacity(capacity),
            loss_mask: Vec::with_capacity(capacity),
            batch_idx: 0,
        }
    }

    /// Vide les vecteurs tout en conservant leur capacité mémoire (Object Pool).
    pub fn clear(&mut self) {
        self.input_tokens.clear();
        self.target_tokens.clear();
        self.loss_mask.clear();
        self.batch_idx = 0;
    }
}

/// État de la boucle d'entrainement.
#[derive(Debug, Clone, PartialEq)]
pub enum TrainingState {
    /// L'Apprentissage tourne correctement.
    Active { loss: f32, epoch: usize },
    /// Le système a terminé l'analyse du dataset.
    Finished { final_loss: f32 },
    /// Circuit Breaker ! Divergence identifiée (ex: division par zéro).
    CircuitOpen { reason: String },
}
