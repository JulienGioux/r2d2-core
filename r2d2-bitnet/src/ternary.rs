use std::fmt;

/// ⚡ `TernaryBlock16` : Structure R2D2-BitNet (1.58-bit)
///
/// Implémente l'encodage "Dual-Mask" exigé par la Brique 4.
/// Permet de stocker 16 poids ternaires `{-1, 0, 1}` dans un seul u32 (4 octets).
///
/// L'optimisation Zero-Branch repose sur cette structure où :
/// - `1`    -> (m_pos: 1, m_neg: 0)
/// - `-1`   -> (m_pos: 0, m_neg: 1)
/// - `0`    -> (m_pos: 0, m_neg: 0) -> "Silence Synaptique" (Zéro-Coût)
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)] // Alignement garanti pour l'interopérabilité mémoire (FFI / CUDA)
pub struct TernaryBlock16 {
    pub m_pos: u16,
    pub m_neg: u16,
}

impl TernaryBlock16 {
    /// Initialise un bloc composé uniquement de silences synaptiques (zéros).
    #[inline(always)]
    pub fn empty() -> Self {
        Self { m_pos: 0, m_neg: 0 }
    }

    /// Construit un bloc Dual-Mask à partir d'un fragment de 16 valeurs `i8`.
    /// 
    /// # Panics
    /// Panique en mode debug si le slice ne contient pas exactement 16 éléments,
    /// ou si une valeur est en dehors de l'espace dimensionnel `{-1, 0, 1}`.
    pub fn from_i8_slice(weights: &[i8]) -> Self {
        debug_assert_eq!(weights.len(), 16, "Un TernaryBlock nécessite exactement 16 poids");

        let mut m_pos = 0u16;
        let mut m_neg = 0u16;

        for (i, &w) in weights.iter().enumerate() {
            match w {
                1 => m_pos |= 1 << i,
                -1 => m_neg |= 1 << i,
                0 => {} // Silence = Aucune affectation
                _ => debug_assert!(false, "Valeur non-ternaire détectée : {}", w),
            }
        }

        Self { m_pos, m_neg }
    }

    /// Audit de cohérence O(1) : Un masque de position ne doit jamais heurter un masque négatif.
    /// Renvoie `true` si la topologie du bloc est exempte de corruption bit-à-bit.
    #[inline(always)]
    pub fn is_valid(&self) -> bool {
        // Validation d'État Pure : Le AND logique garantit l'absence de superposition.
        (self.m_pos & self.m_neg) == 0
    }
}

impl fmt::Debug for TernaryBlock16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TernaryBlock16 {{ pos: {:016b}, neg: {:016b}, valid: {} }}",
            self.m_pos,
            self.m_neg,
            self.is_valid()
        )
    }
}

// -----------------------------------------------------------------------------
// SECURE AUDIT TESTS (CI: Zero-Warnings)
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_ternary_block() {
        let weights: [i8; 16] = [
            1, 0, -1, 1, 0, 0, -1, 1,
            -1, 0, 1, 0, -1, 1, 0, -1
        ];

        let block = TernaryBlock16::from_i8_slice(&weights);
        assert!(block.is_valid());

        // Bit positionnel 0 (poids = 1) -> m_pos bit 0 est à 1
        assert_eq!(block.m_pos & 1, 1);
        assert_eq!(block.m_neg & 1, 0);

        // Bit positionnel 2 (poids = -1) -> m_neg bit 2 est à 1
        assert_eq!(block.m_neg & (1 << 2), 1 << 2);
        assert_eq!(block.m_pos & (1 << 2), 0);
    }
}
