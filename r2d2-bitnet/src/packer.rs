
use crate::ternary::TernaryBlock16;
use candle_core::{Result, Tensor};
use tracing::{info, instrument};

/// 🧊 `Packer` : Cristallise les poids F32 d'entraînement vers Int8 compressé.
///
/// Ce module traite l'export "Wake Up" post-entraînement : il quantifie la matrice autograd via AbsMean,
/// sérialise les `{-1, 0, 1}` en un `Vec<i8>`, puis empaquette par groupe de 16 poids
/// dans notre `TernaryBlock16`, permettant un facteur de compression mémoire de 16x par rapport au F32.
pub struct BitNetPacker;

impl BitNetPacker {
    /// Compresse mathématiquement un tenseur linéaire F32 entrainé en Blocs Ternaires purifiés.
    #[instrument(skip_all, name = "pack_tensor_to_blocks")]
    pub fn pack_tensor(tensor: &Tensor) -> Result<Vec<TernaryBlock16>> {
        let span = tracing::info_span!("quantize_and_pack");
        let _enter = span.enter();

        // 1. Appliquer le STE Forward (Quantification) manuellement pour obtenir des valeurs i8 exactes {-1, 0, 1}
        let w_q = crate::quantization::absmean_quantize_weights(tensor)?;

        // 2. Aplatir le tenseur (Flatten)
        let flat_wq = w_q.flatten_all()?;
        let num_elements = flat_wq.elem_count();
        
        // Assert : La taille totale doit être un multiple de 16 pour permettre un packing propre.
        // Sinon, l'architecture a été mal dimensionnée.
        debug_assert_eq!(
            num_elements % 16,
            0,
            "L'architecture réseau doit avoir des dimensions multiples de 16 !"
        );

        // EXTRACTION ZÉRO-COST: On ramène les flottants quantifiés dans un `Vec<f32>` classique CPU
        let flat_f32_vec = flat_wq.to_vec1::<f32>()?;
        
        // 3. Empaqueter massivement par chunks de 16 sans allocations superflues
        let num_blocks = num_elements / 16;
        let mut blocks = Vec::with_capacity(num_blocks);

        for chunk_idx in 0..num_blocks {
            let offset = chunk_idx * 16;
            let mut chunk_i8 = [0i8; 16];
            for i in 0..16 {
                // Cast natif garanti sans arrondi dangereux car les valeurs sont exactement -1.0, 0.0, 1.0
                chunk_i8[i] = flat_f32_vec[offset + i] as i8;
            }
            
            blocks.push(TernaryBlock16::from_i8_slice(&chunk_i8));
        }

        info!("🧊 Tenseur cristallisé: {} valeurs -> {} TernaryBlock16", num_elements, num_blocks);
        Ok(blocks)
    }
}
