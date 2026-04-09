//! # Brique 0 : SecureMemGuard
//!
//! Cette crate implémente le niveau fondamental de sécurité "Zero-Trust" de la Ruche R2D2.
//! Elle fournit le wrapper `SecureMemGuard<T, State>` garantissant que toute donnée sensible
//! (fragments non validés, clés cryptographiques, poids d'inférence)
//! est physiquement écrasée (Zeroization) en mémoire dès que la variable sort du scope.
//!
//! Ceci empêche le *Memory Scraping* et l'exposition des canaux auxiliaires (Side Channel).

use secrecy::{ExposeSecret, SecretBox};
use std::marker::PhantomData;
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

/// État typé indiquant que le conteneur mémoire est actif (lisible).
pub struct Active;

/// État typé indiquant que le conteneur mémoire est révoqué (indisponible).
pub struct Revoked;

/// Erreurs liées à la gestion de la mémoire sécurisée.
#[derive(Debug, Error)]
pub enum SecureMemError {
    #[error("Erreur de conversion mémoire sécurisée: {0}")]
    ConversionError(String),
}

/// Un garde mémoire sécurisé qui efface de la RAM son contenu à la destruction.
///
/// Ce conteneur "Zero-Trust" alloue l'information et s'assure qu'un appel
/// `explicit_bzero` (ou équivalent) est invoqué par le compilateur
/// lorsque la donnée n'est plus utile.
pub struct SecureMemGuard<T, State = Active>
where
    T: Zeroize,
{
    // Utilisé pour stocker des données dynamiques qui sont systématiquement nettoyées.
    inner: SecretBox<T>,
    _state: PhantomData<State>,
}

impl<T, State> std::fmt::Debug for SecureMemGuard<T, State>
where
    T: Zeroize,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecureMemGuard(<REDACTED>)")
    }
}

impl<T> SecureMemGuard<T, Active>
where
    T: Zeroize,
{
    /// Alloue un nouveau bloc mémoire sécurisé.
    /// L'appelant doit fournir une donnée enrobée dans un `Zeroizing<T>` afin
    /// d'éviter que la variable d'origine ne laisse une copie fantôme sur la pile.
    pub fn new(mut data: Zeroizing<T>) -> Self {
        // Safe conversion if moving into an allocation
        // But since we can't easily Box a Zeroizing without consuming inner
        // And zeroing strings is UB.
        // What we actually want is to zeroize `data` on drop, but we want to steal its value.
        // Zeroizing doesn't allow stealing without `Default` or `unsafe read`.
        // Let's use `unsafe { std::ptr::read(&*data) }` and then zero memory.
        let val_ptr = unsafe {
            let ptr = &mut *data as *mut T;
            let val = std::ptr::read(ptr);
            // manually zero the stack
            std::ptr::write_bytes(ptr, 0, 1);
            // Now forget data so it doesn't double-drop the (now zeroed) string pointers
            std::mem::forget(data);
            Box::new(val)
        };

        Self {
            inner: SecretBox::new(val_ptr),
            _state: PhantomData,
        }
    }

    /// Expose temporairement la valeur pour un traitement immédiat.
    pub fn expose_payload(&self) -> &T {
        self.inner.expose_secret()
    }

    /// Révoque de manière irrévocable l'accès à la mémoire sécurisée.
    /// Consomme l'accès actif, rendant toute exposition ultérieure impossible.
    pub fn revoke(self) -> SecureMemGuard<T, Revoked> {
        SecureMemGuard {
            inner: self.inner,
            _state: PhantomData,
        }
    }
}
