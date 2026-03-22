//! # Brique 0 : SecureMemGuard
//!
//! Cette crate implémente le niveau fondamental de sécurité "Zero-Trust" de la Ruche R2D2.
//! Elle fournit le wrapper `SecureMemGuard<T>` garantissant que toute donnée sensible
//! (fragments non validés, clés cryptographiques, poids d'inférence)
//! est physiquement écrasée (Zeroization) en mémoire dès que la variable sort du scope.
//!
//! Ceci empêche le *Memory Scraping* et l'exposition des canaux auxiliaires (Side Channel).

use secrecy::{ExposeSecret, SecretBox};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Erreurs liées à la gestion de la mémoire sécurisée.
#[derive(Debug, Error)]
pub enum SecureMemError {
    #[error("Tentative d'accès à une mémoire non initialisée ou révoquée")]
    RevokedAccess,
    #[error("Erreur de conversion mémoire sécurisée: {0}")]
    ConversionError(String),
}

/// Un garde mémoire sécurisé qui efface de la RAM son contenu à la destruction.
///
/// Ce conteneur "Zero-Trust" alloue l'information et s'assure qu'un appel
/// `explicit_bzero` (ou équivalent) est invoqué par le compilateur
/// lorsque la donnée n'est plus utile.
#[derive(Debug)]
pub struct SecureMemGuard<T>
where
    T: Zeroize + Clone,
{
    // Utilisé pour stocker des données dynamiques qui sont systématiquement nettoyées.
    inner: SecretBox<T>,
}

impl<T> SecureMemGuard<T>
where
    T: Zeroize + Clone,
{
    /// Alloue un nouveau bloc mémoire sécurisé.
    ///
    /// # Exemple
    ///
    /// ```rust
    /// use zeroize::ZeroizeOnDrop;
    /// use zeroize::Zeroize;
    /// use r2d2_secure_mem::SecureMemGuard;
    ///
    /// #[derive(Clone, Zeroize, ZeroizeOnDrop)]
    /// struct SecretData(String);
    ///
    /// let guard = SecureMemGuard::new(SecretData("Données Hautement Sensibles".to_string()));
    /// // À la fin du bloc, "Données Hautement Sensibles" est détruit physiquement de la RAM.
    /// ```
    pub fn new(data: T) -> Self {
        Self {
            inner: SecretBox::new(Box::new(data)),
        }
    }

    /// Expose temporairement la valeur pour un traitement immédiat.
    ///
    /// L'appelant doit faire passer cette référence à une fonction (ex: Paradox Engine).
    pub fn expose_payload(&self) -> &T {
        self.inner.expose_secret()
    }
}
