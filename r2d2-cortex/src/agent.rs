use anyhow::Result;
use async_trait::async_trait;
use thiserror::Error;
use std::path::PathBuf;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Erreur de chargement du tenseur en mémoire : {0}")]
    LoadError(String),
    #[error("Erreur d'inférence (Moteur Candle) : {0}")]
    InferenceError(String),
    #[error("Fichier de poids introuvable : {0}")]
    WeightsNotFound(PathBuf),
    #[error("Le modèle est déjà déchargé ou inactif.")]
    NotActive,
}

/// Contrat strict imposé à chaque Agent Local (Plug & Play)
/// 
/// Tout agent (Qwen, Llama, MiniLM) doit se plier à cette interface.
/// L'Architecte exige une soumission absolue de l'IA aux cycles de vie OS.
#[async_trait]
pub trait CognitiveAgent: Send + Sync {
    /// Retourne le nom officiel de l'agent.
    fn name(&self) -> &str;

    /// Charge les poids du modèle (ex: .gguf) directement en RAM via `candle`.
    /// Cette opération alloue la mémoire massive et initie le Tokenizer.
    async fn load(&mut self) -> Result<(), AgentError>;

    /// Force explicitement le *Drop* des tenseurs en RAM.
    /// Renvoie la mémoire disponible au système hôte (Zero-Trust hardware).
    async fn unload(&mut self) -> Result<(), AgentError>;

    /// Informe si l'agent est actuellement en RAM et prêt à travailler.
    fn is_active(&self) -> bool;

    /// Soumet une séquence au Cortex pour générer une pensée brute.
    /// Ne peut être appelé que si `is_active()` est vrai.
    async fn generate_thought(&mut self, prompt: &str) -> Result<String, AgentError>;
}
