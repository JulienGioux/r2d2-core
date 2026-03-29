use candle_core::{Result, Tensor};
use tracing::instrument;

/// 🎭 Calcule la Cross-Entropy Loss avec un masque de pondération (Masked Loss).
///
/// Idéal pour l'apprentissage sur JSONAI v5.1 :
/// Les tokens de structuration (ex: `{`, `"consensus"`, `[`) ont un mask de `0.0`.
/// Les tokens sémantiques purs (contenu des Belief States) ont un mask de `1.0`.
/// Cela focalise l'attention du gradient exclusivement sur l'Intelligence et non la syntaxe.
#[instrument(skip_all, name = "masked_cross_entropy")]
pub fn masked_cross_entropy(logits: &Tensor, targets: &Tensor, mask: &Tensor) -> Result<Tensor> {
    // 1. Calcul du log_softmax sur les prédictions (Dimension du Vocabulaire : typiquement -1)
    // Nous le ferons de toute façon plus bas après un reshape.

    // 2. Extraire la probabilité correspondant au 'target' réel
    // Dimensions attendues :
    // logits: [Batch, SeqLen, VocabSize]
    // targets: [Batch, SeqLen]
    // log_probs_gathered: [Batch, SeqLen]
    // Note : candle_nn::loss::cross_entropy utilise une méthode interne, ici on l'implémente manuellement
    // via gather ou bien avec la fonction de perte native de Candle puis on la pondère avant le "mean".

    // Pour simplifier et optimiser avec le mask batch par batch :
    // Candle v0.8.2 ne propose pas de gather tensor vs tensor facilement sur la dernière dim.
    // Mais on peut faire :
    // let loss = candle_nn::loss::cross_entropy(logits.flatten(0, 1)?, targets.flatten(0, 1)?)?;
    // Cette fonction native fait le `.mean_all()`.
    // Puisque nous voulons appliquer un mask, nous devrions soit le passer dans une CustomOp, soit reconstruire le Loss.

    // SOLUTION ÉLÉGANTE (Industrial-Grade) :
    // Si la Masked Loss est trop verbeuse à implémenter pure mathématiquement sans primitives avancées,
    // l'Architecte utilise la fonction d'erreur native et multiplie les logits synthétiques ?
    // En fait, dans `candle`, on a `gather`.

    // Reshape pour l'opération :
    let b_sz = targets.dim(0)?;
    let seq_len = targets.dim(1)?;
    let vocab_size = logits.dim(2)?;

    let logits_2d = logits.reshape((b_sz * seq_len, vocab_size))?;
    let targets_1d = targets.flatten_all()?;
    let mask_1d = mask.flatten_all()?;

    // arg 1 = dimension (vocab)
    // targets_1d.unsqueeze(1) pour faire [B*S, 1]
    let t_unsqueeze = targets_1d.unsqueeze(1)?;
    let log_p = candle_nn::ops::log_softmax(&logits_2d, 1)?;

    // gather : sur la dim 1, avec les cibles
    let gathered = log_p.gather(&t_unsqueeze, 1)?.squeeze(1)?;

    // Loss point par point = -gathered
    let unweighted_loss = gathered.neg()?;

    // On pondère par le mask (0.0 pour syntaxe JSON, 1.0 pour raisonnement)
    let weighted_loss = unweighted_loss.broadcast_mul(&mask_1d)?;

    // Moyenne uniquement sur les tokens actifs (mask > 0)
    let sum_loss = weighted_loss.sum_all()?;
    let sum_mask = mask_1d.sum_all()?;

    // Division finale (évite la division par zéro s'il n'y a aucun token actif)
    let eps = Tensor::new(1e-5f32, logits.device())?;
    let denom = sum_mask.broadcast_add(&eps)?;

    sum_loss.broadcast_div(&denom)
}
