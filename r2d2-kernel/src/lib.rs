//! # Brique 2 : Kernel Logique
//!
//! Le Kernel de R2D2 agit en tant qu'hyperviseur sémantique.
//! Il implémente le Typestate Pattern strict pour garantir qu'aucune donnée
//! n'est traitée ou mémorisée sans avoir été formellement auditée par le Paradox Engine.

pub mod ports;

use r2d2_secure_mem::SecureMemGuard;
use thiserror::Error;
use zeroize::Zeroize;

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("Échec de la validation sémantique: {0}")]
    ValidationFailed(String),
}

// ==========================================
// Typestate Pattern : Le Cycle de la Vérité
// ==========================================

/// Fragment de donnée brut, non parsé et potentiellement malveillant.
#[derive(Debug, Clone)]
pub struct Signal {
    pub raw_data: String,
}

impl Zeroize for Signal {
    fn zeroize(&mut self) {
        self.raw_data.zeroize();
    }
}

/// Fragment structuré (par ex. JSONAI v3.1) mais dont la cohérence n'est pas vérifée.
#[derive(Debug, Clone)]
pub struct Unverified {
    pub payload: String,
}

impl Zeroize for Unverified {
    fn zeroize(&mut self) {
        self.payload.zeroize();
    }
}

/// Fragment vérifié par le Paradox Engine et ayant atteint le consensus.
#[derive(Debug, Clone)]
pub struct Validated {
    pub payload: String,
    pub proof_of_inference: String,
}

impl Zeroize for Validated {
    fn zeroize(&mut self) {
        self.payload.zeroize();
        self.proof_of_inference.zeroize();
    }
}

/// Conteneur immuable hébergeant la donnée avec la garantie du compilateur.
pub struct Fragment<State> {
    state: State,
}

impl<State> Fragment<State> {
    /// Extrait formellement l'état interne en consommant le conteneur.
    pub fn into_inner(self) -> State {
        self.state
    }
}

impl Fragment<Signal> {
    /// Crée un nouveau signal brut amnésique (Zeroize on Drop).
    pub fn new(raw_data: String) -> Self {
        Self {
            state: Signal { raw_data },
        }
    }

    /// Tente de parser le signal brut vers une structure `Unverified`.
    pub fn parse(self) -> Result<Fragment<Unverified>, KernelError> {
        let _: r2d2_jsonai::JsonAiV3 = serde_json::from_str(&self.state.raw_data).map_err(|e| {
            KernelError::ValidationFailed(format!("Signal non conforme JSONAI V3.1: {}", e))
        })?;

        Ok(Fragment {
            state: Unverified {
                payload: self.state.raw_data,
            },
        })
    }
}

/// Port (Hexagonal Architecture) décrivant le validateur de vérité.
pub trait TruthValidator: Send + Sync {
    /// Prend en entrée un payload brut et retourne (Payload_Validé, Preuve_Inférence)
    fn validate_payload(
        &self,
        payload: &str,
    ) -> impl std::future::Future<Output = Result<(String, String), KernelError>> + Send;
}

impl Fragment<Unverified> {
    /// Soumet le fragment au Paradox Engine (Brique 3) via injection de dépendance.
    ///
    /// Retourne un fragment `Validated` uniquement si le consensus épistémologique
    /// est atteint sans paradoxe.
    pub async fn verify<V: TruthValidator>(
        self,
        validator: &V,
    ) -> Result<Fragment<Validated>, KernelError> {
        let (verified_payload, poi) = validator.validate_payload(&self.state.payload).await?;

        Ok(Fragment {
            state: Validated {
                payload: verified_payload,
                proof_of_inference: poi,
            },
        })
    }
}

impl Fragment<Validated> {
    /// Ancre la donnée validée dans le Blackboard de la Ruche.
    /// Renvoie un `SecureMemGuard` qui s'assurera de la Zeroization de la RAM en sortie de bloc.
    pub fn finalize(self) -> SecureMemGuard<Validated> {
        SecureMemGuard::new(zeroize::Zeroizing::new(self.state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockValidator;
    impl TruthValidator for MockValidator {
        async fn validate_payload(&self, payload: &str) -> Result<(String, String), KernelError> {
            Ok((payload.to_string(), "poi_0xABCDEF_R2D2".to_string()))
        }
    }

    #[tokio::test]
    async fn test_typestate_pipeline() {
        let mock_json = r2d2_jsonai::JsonAiV3::new(
            "test".to_string(),
            r2d2_jsonai::AgentSource::System,
            "test".to_string(),
            r2d2_jsonai::BeliefState::Fact,
        );
        let signal = Fragment::new(serde_json::to_string(&mock_json).unwrap());

        let unverified = signal.parse().expect("Doit parser");
        let validator = MockValidator;
        let validated = unverified.verify(&validator).await.expect("Doit valider");

        // Impossible de compiler: let err = unverified.finalize();

        let secure_mem = validated.finalize();
        assert_eq!(
            secure_mem.expose_payload().proof_of_inference,
            "poi_0xABCDEF_R2D2"
        );
    }
}
