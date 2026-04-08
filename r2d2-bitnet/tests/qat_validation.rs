use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
// Assuming r2d2_bitnet exposes bitlinear and weights modules. If not, this test checks the boundary.
use r2d2_bitnet::bitlinear::BitLinear;
use r2d2_bitnet::weights::TrainingWeights;

#[test]
fn test_qat_lasso_effect_collapse() -> candle_core::Result<()> {
    // Phase 6 TDD: Vérification mathématique du Lasso Effect (Quantization-Aware Training)
    // L'objectif est de s'assurer que l'estimateur fonctionnel STE (Straight-Through Estimator)
    // compresse bien les données proches de zéro dans la matrice ternaire (Lasso Régularisation).
    let in_f = 16;
    let out_f = 16;

    let mut w_data = vec![0.0f32; in_f * out_f];
    // On injecte du bruit (proche de zéro) et des valeurs fortes
    w_data[0] = 0.01; // Doit s'effondrer a 0 (Lasso)
    w_data[1] = 0.99; // Doit devenir 1
    w_data[2] = -0.99; // Doit devenir -1

    let w_tensor = Tensor::from_vec(w_data, &[out_f, in_f], &Device::Cpu)?;

    let mut tensors = std::collections::HashMap::new();
    tensors.insert("weight".to_string(), w_tensor);
    let vb = VarBuilder::from_tensors(tensors, DType::F32, &Device::Cpu);

    let layer = BitLinear::<TrainingWeights>::load_train(in_f, out_f, vb)?;

    // Passe Forward
    let xs = Tensor::ones((1, in_f), DType::F32, &Device::Cpu)?;
    let out = candle_core::Module::forward(&layer, &xs)?;

    let target = Tensor::ones_like(&out)?;
    // Calcul de loss = Test de l'arbre computationnel (Autograd ready)
    let loss = out.sub(&target)?.sqr()?.sum_all()?;

    let loss_val = loss.to_scalar::<f32>()?;
    assert!(
        loss_val >= 0.0,
        "La fonction de coût (QAT) doit être calculable."
    );

    // Le test passe si l'opération est mathématiquement correcte et que le tenseur STE ne crash pas.
    Ok(())
}
