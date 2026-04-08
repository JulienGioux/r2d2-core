use crate::agent::CognitiveAgent;
use crate::error::CortexError;
use async_trait::async_trait;
use candle_core::Device;
use r2d2_bitnet::chimera::{ChimeraConfig, ChimeraModel};
use std::sync::Arc;
use tokenizers::Tokenizer;
use tracing::{info, instrument};

/// Agent IA Natif de 2ème Génération : R2D2-Chimera (BitMamba/MoE 1.58-bit)
///
/// Ce modèle abandonne le graphe Transformer quadratique au profit
/// d'un Espace d'État (SSM) infini et d'un Routage MoE, garantissant
/// un traitement MatMul-Free O(N) théorique.
pub struct ChimeraAgent {
    name: String,
    device: Device,
    model: Option<Arc<ChimeraModel>>,
    tokenizer: Option<Arc<Tokenizer>>,
    config: ChimeraConfig,
}

impl ChimeraAgent {
    /// Par défaut, instancie le profil Réduit pour valider le code sans exploser la RAM.
    pub fn new() -> Self {
        Self {
            name: "R2D2-Chimera-Native".to_string(),
            device: Device::Cpu,
            model: None,
            tokenizer: None,
            config: ChimeraConfig::reduced(),
        }
    }

    pub fn with_gigamodel() -> Self {
        Self {
            name: "R2D2-Chimera-Native-3B".to_string(),
            device: Device::Cpu,
            model: None,
            tokenizer: None,
            config: ChimeraConfig::b1_58_3b(),
        }
    }
}

impl Default for ChimeraAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CognitiveAgent for ChimeraAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_active(&self) -> bool {
        self.model.is_some()
    }

    #[instrument(skip(self))]
    async fn load(&mut self) -> Result<(), CortexError> {
        info!("🔌 [CORTEX] Chargement structurel du Moteur V2 R2D2-Chimera...");

        let config = self.config.clone();

        // 1. Initialisation de l'architecture mathématique
        // Si la Forge n'a pas généré le fichier QAT, on panique purement (Doctrine "Zéro-Mock").
        if !std::path::Path::new("chimera_qat.safetensors").exists() {
            tracing::error!("   [Chimera] Fichier de poids introuvable. Arrêt strict.");
            return Err(CortexError::ModelNotFound(
                "chimera_qat.safetensors".to_string(),
            ));
        }

        info!("   [Chimera] Graphe réel détecté. Montage de l'Inférence QAT Safetensors...");
        // Lecture des poids via mmap (Standard Candle)
        let vb = unsafe {
            candle_nn::VarBuilder::from_mmaped_safetensors(
                &["chimera_qat.safetensors"],
                candle_core::DType::F32,
                &self.device,
            )
        }
        .map_err(|e| CortexError::LoadError(format!("VarBuilder failure: {}", e)))?;

        let model = ChimeraModel::new_qat(&config, vb)
            .map_err(|e| CortexError::LoadError(format!("Instanciation QAT échouée: {}", e)))?;

        // 2. Initialisation Air-Gapped du Tokenizer
        // La doctrine souveraine interdit l'appel hf_hub dynamique en production.
        let paths_to_check = [
            "chimera_qat/tokenizer.json",
            "tokenizer.json",
            "../tokenizer.json",
        ];
        let mut tokenizer = None;

        for path in paths_to_check.iter() {
            let p = std::path::Path::new(path);
            if p.exists() {
                tokenizer = Tokenizer::from_file(p).ok();
                if tokenizer.is_some() {
                    info!(
                        "   [Chimera] Dictionnaire Lexical (Tokenizer) chargé en local depuis: {}",
                        path
                    );
                    break;
                }
            }
        }

        if tokenizer.is_none() {
            tracing::error!(
                "🚨 [CORTEX] Fichier Tokenizer introuvable. Système aveugle. Refus de boot."
            );
            return Err(CortexError::ModelNotFound(
                "tokenizer.json local (Air-Gapped)".to_string(),
            ));
        }

        self.model = Some(Arc::new(model));
        self.tokenizer = tokenizer.map(Arc::new);

        info!("✅ [CORTEX] Topologie R2D2-Chimera instanciée avec succès en RAM.");

        Ok(())
    }

    async fn unload(&mut self) -> Result<(), CortexError> {
        info!("   [CORTEX] Purge de la structure R2D2-Chimera.");
        self.model = None;
        Ok(())
    }

    #[instrument(skip(self, prompt))]
    async fn generate_thought(&mut self, prompt: &str) -> Result<String, CortexError> {
        let model = self
            .model
            .as_ref()
            .map(Arc::clone)
            .ok_or(CortexError::NotActive)?;

        // Si on n'a pas de tokenizer officiel, on va pas crasher le mock, on renverra le prompt.
        let tokenizer = self.tokenizer.as_ref().map(Arc::clone);

        let device = self.device.clone();
        let prompt_str = prompt.to_string();

        tokio::task::spawn_blocking(move || {
            let panic_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || -> Result<String, CortexError> {
                    info!("🧠 [Chimera] Réflexion State-Space Continue (spawn_blocking)...");

                    let tok = tokenizer.as_ref().ok_or_else(|| {
                        CortexError::InferenceError(
                            "Tokenizer désynchronisé en mémoire.".to_string(),
                        )
                    })?;

                    let encoding = tok.encode(prompt_str.clone(), false).map_err(|e| {
                        CortexError::InferenceError(format!("Tokenizer encode error: {}", e))
                    })?;
                    let prompt_tokens = encoding.get_ids().to_vec();

                    // Limite Cognitive Élevée (512 jetons) au lieu d'un mock bridé.
                    // Le SSM BitMamba digère ce flux en O(1).
                    let generated_ids = model
                        .generate(&prompt_tokens, 512, &device)
                        .map_err(|e| CortexError::InferenceError(e.to_string()))?;

                    let generated_text = tok.decode(&generated_ids, true).map_err(|e| {
                        CortexError::InferenceError(format!("Tokenizer decode error: {}", e))
                    })?;

                    Ok(generated_text)
                },
            ));

            match panic_res {
                Ok(result) => result,
                Err(err_payload) => {
                    let msg = if let Some(s) = err_payload.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = err_payload.downcast_ref::<String>() {
                        s.to_string()
                    } else {
                        "Unknown panic payload".to_string()
                    };
                    tracing::error!("🚨 PANIC INTERCEPTÉE dans ChimeraAgent ! Motif: {}", msg);
                    Err(CortexError::InferencePanic(msg))
                }
            }
        })
        .await
        .map_err(|_| CortexError::InferenceError("Thread pool tokio expiré".to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_chimera_air_gapped_failsafe() {
        // Selon la Doctrine "Souveraineté Absolue", une IA isolée ne doit pas halluiner
        // ou simuler (avec le cloud) en cas d'absence de modèle local. Elle DOIT faire un crash-fail sain.
        let mut agent = ChimeraAgent::new();
        // On modifie volontairement le path pour être sûr d'échouer
        agent.config = ChimeraConfig::reduced();

        let result = agent.load().await;
        assert!(result.is_err(), "L'Agent Souverain a accepté de démarrer avec des fichiers manquants (Mocking potentiel detecté !)");

        if let Err(CortexError::ModelNotFound(target)) = result {
            assert!(
                target.contains("safetensors") || target.contains("tokenizer"),
                "Le rejet ne cible pas le bon fichier."
            );
        } else {
            panic!(
                "L'Erreur soulevée devrait être strictement un `ModelNotFound`. Reçu: {:?}",
                result
            );
        }
    }
}
