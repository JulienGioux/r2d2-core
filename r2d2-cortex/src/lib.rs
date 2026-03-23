//! # Brique 5 : Le Cortex Local (Bibliothèque d'Agents)
//!
//! Ce module orchestre le chargement dynamique et le déchargement des modèles 
//! IA locaux (GGUF via `candle`). Il agit comme un gestionnaire de RAM strict,
//! garantissant qu'un seul modèle lourd n'occupe la mémoire à un instant T.

pub mod agent;
pub mod registry;
pub mod models;

pub use agent::{CognitiveAgent, AgentError};
pub use registry::CortexRegistry;
