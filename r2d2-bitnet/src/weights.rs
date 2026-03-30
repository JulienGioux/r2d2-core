use crate::ternary::TernaryBlock16;
/// Trait marqueur pour le pattern Type-State de l'architecture BitLinear.
/// Il permet à rustc de monomorphiser `BitLinear` en deux implémentations distinctes
/// garantissant un coût d'abstraction nul au runtime ("Zero-Cost Abstraction").
pub trait WeightProvider: Send + Sync {}

/// État d'Inférence : Les poids sont cristalisés sous forme de blocs ternaires hyper-compressés (`TernaryBlock16`).
/// Utilisé pour l'exécution CPU "Zero-Branch" ultra-rapide.
pub struct InferenceWeights {
    pub blocks: Vec<TernaryBlock16>,
}

impl WeightProvider for InferenceWeights {}

/// État d'Entraînement : Les poids latents sont maintenus en virgule flottante (`Var` f32).
/// Utilisé pour la rétropropagation du gradient via l'estimateur STE (Straight-Through Estimator).
pub struct TrainingWeights {
    // La variable de poids stockée pour l'Autograd.
    pub var: candle_core::Var,
}

impl WeightProvider for TrainingWeights {}
