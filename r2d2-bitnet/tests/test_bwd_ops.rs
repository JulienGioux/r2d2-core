#[cfg(feature = "cuda")]
use candle_core::{DType, Device, Tensor};
#[cfg(feature = "cuda")]
use r2d2_bitnet::custom_op_cuda::BitNetW1A8MatMulOp;

#[test]
#[cfg(feature = "cuda")]
fn test_bitnet_bwd_dx_matmul_cuda_accuracy() -> candle_core::Result<()> {
    // 1. Initialisation - Dimensions multiples de 16/32 pour éprouver la structure binaire
    let m = 32; // Ex: Batch * SeqLen = 32
    let n = 128; // Nombre de features de sortie
    let k = 256; // Nombre de features d'entrée (Multiple de 16 requis pour le TernaryBlock)

    let device = Device::new_cuda(0)?;

    // 2. Génération de W_quant synthétique et de son miroir F32 parfait
    let mut w_quant_vec = Vec::with_capacity(n * (k / 16));
    let mut w_f32_vec = Vec::with_capacity(n * k);

    for row in 0..n {
        for block in 0..(k / 16) {
            let mut m_pos = 0u16;
            let mut m_neg = 0u16;
            for bit in 0..16 {
                // Modulo deterministic logic to avoid rand crate dependency
                // 3 states: Positive, Negative, Zero
                let state = (bit + row * 16 + block) % 3;
                if state == 0 {
                    m_pos |= 1 << bit;
                    w_f32_vec.push(1.0f32);
                } else if state == 1 {
                    m_neg |= 1 << bit;
                    w_f32_vec.push(-1.0f32);
                } else {
                    w_f32_vec.push(0.0f32);
                }
            }
            // Packing little-endian u32: m_pos aux 16 bits inférieurs, m_neg aux 16 bits supérieurs
            let packed = (m_neg as u32) << 16 | (m_pos as u32);
            w_quant_vec.push(packed);
        }
    }

    let w_quant = Tensor::from_vec(w_quant_vec, (n, k / 16), &device)?;
    // Le miroir Float32 de W pour la validation arithmétique mathématique
    let w_f32 = Tensor::from_vec(w_f32_vec, (n, k), &device)?;

    // 3. Génération des entrées pures pour Ground Truth CPU/Native
    let grad_y_pure = Tensor::randn(0f32, 1f32, (m, n), &device)?;
    let act_pure = Tensor::randn(0f32, 1f32, (m, k), &device)?;

    // On forge le Ground Truth (Sûr et non affecté par nos tricks mémoires)
    // dX = dY * W^T
    let grad_x_gt = grad_y_pure.matmul(&w_f32)?;
    // grad_w = dY^T * X
    let grad_w_gt = grad_y_pure.t()?.contiguous()?.matmul(&act_pure)?;

    // 4. LE PIÈGE DE MÉMOIRE (Le VRAM Segfault Check)
    // On génère des tenseurs plus larges pour créer des offsets vicieux
    let pad_m = 5;
    let pad_n = 5;
    let pad_k = 5;

    // pad dY
    let grad_y_padded = Tensor::zeros((m + pad_m, n + pad_n), DType::F32, &device)?;
    // Copier grad_y_pure dans le sous-espace pour avoir EXACTEMENT les mêmes valeurs
    let mut grad_y_pad_vec = vec![0.0f32; (m + pad_m) * (n + pad_n)];
    let dy_pure_vec = grad_y_pure.flatten_all()?.to_vec1::<f32>()?;
    for i in 0..m {
        for j in 0..n {
            grad_y_pad_vec[(i + 2) * (n + pad_n) + (j + 2)] = dy_pure_vec[i * n + j];
        }
    }
    let grad_y_padded = Tensor::from_vec(grad_y_pad_vec, (m + pad_m, n + pad_n), &device)?;
    let grad_y = grad_y_padded.narrow(0, 2, m)?.narrow(1, 2, n)?;

    // pad X (act)
    let act_padded = Tensor::zeros((m + pad_m, k + pad_k), DType::F32, &device)?;
    let mut act_pad_vec = vec![0.0f32; (m + pad_m) * (k + pad_k)];
    let act_pure_vec = act_pure.flatten_all()?.to_vec1::<f32>()?;
    for i in 0..m {
        for j in 0..k {
            act_pad_vec[(i + 2) * (k + pad_k) + (j + 2)] = act_pure_vec[i * k + j];
        }
    }
    let act_padded = Tensor::from_vec(act_pad_vec, (m + pad_m, k + pad_k), &device)?;
    let act = act_padded.narrow(0, 2, m)?.narrow(1, 2, k)?;

    // 5. Instanciation du CustomOp PTX STE
    use candle_core::CustomOp2;
    let op = BitNetW1A8MatMulOp { m, n, k };

    let dummy_res = Tensor::zeros((m, n), DType::F32, &device)?;

    // 6. Exécution du Backward Pass (Notre nouveau noyau CUDA)
    let (grad_x_cuda, grad_w_cuda) = op.bwd(&act, &w_quant, &dummy_res, &grad_y)?;

    // 7. Validation Stricte
    let grad_x_cuda: Tensor =
        grad_x_cuda.expect("Le noyau BWD aurait dû retourner un tenseur grad_x");
    let grad_w_cuda: Tensor =
        grad_w_cuda.expect("Le noyau BWD aurait dû retourner un tenseur grad_w");

    // Comparaison grad_x (Délégation au Kernel PTX bitnet_bwd_dx_matmul)
    let diff_x = (grad_x_cuda.sub(&grad_x_gt)?)
        .abs()?
        .mean_all()?
        .to_scalar::<f32>()?;
    println!("PTX L1-Cache BWD vs Native MM (Grad_X Diff): {}", diff_x);
    assert!(
        diff_x < 1e-3,
        "FATAL: Le noyau PTX bitnet_bwd_dx_matmul (grad_x) dévie de la vérité terrain ! Diff = {}",
        diff_x
    );

    // Comparaison grad_w (Utilisation Native en Dual-STE)
    let diff_w = (grad_w_cuda.sub(&grad_w_gt)?)
        .abs()?
        .mean_all()?
        .to_scalar::<f32>()?;
    println!("Dual-STE BWD vs Native MM (Grad_W Diff): {}", diff_w);
    assert!(
        diff_w < 1e-3,
        "FATAL: La passe arrière en Dual-STE Matrix Multiply dévie de la vérité terrain ! Diff = {}", diff_w
    );

    Ok(())
}
