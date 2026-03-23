use crate::agent::{AgentError, CognitiveAgent};
use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn, instrument};

/// Gestionnaire souverain de l'allocation des Agents IA en RAM.
///
/// Le Registre applique la doctrine de l'"Unicité d'Inférence".
/// Pour préserver les capacités du Hardware (PC "sur les genoux"),
/// le CortexRegistry garantit qu'un seul LLM lourd n'est actif à la fois.
pub struct CortexRegistry {
    /// Dépôt des agents déclarés (qu'ils soient en RAM ou dormants sur le disque).
    agents: RwLock<HashMap<String, Box<dyn CognitiveAgent>>>,
}

impl CortexRegistry {
    /// Initialise une bibliothèque vide de Cortex.
    pub fn new() -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
        }
    }

    /// Ajoute dynamiquement un nouvel agent (Plugin) à la bibliothèque.
    /// L'agent est stocké à l'état dormant (non chargé en RAM).
    pub async fn register_agent(&self, agent: Box<dyn CognitiveAgent>) {
        let name = agent.name().to_string();
        self.agents.write().await.insert(name.clone(), agent);
        info!("Agent '{}' inséré dans le Registre Cortex (Dormant).", name);
    }

    /// Réveille un agent spécifique (le charge en RAM)
    /// et désactive OBLIGATOIREMENT les autres pour purger la mémoire.
    #[instrument(skip(self))]
    pub async fn activate(&self, target_name: &str) -> Result<(), AgentError> {
        let mut registry = self.agents.write().await;

        if !registry.contains_key(target_name) {
            warn!("Agent {} introuvable dans la bibliothèque.", target_name);
            return Err(AgentError::LoadError(format!("Agent inconnu: {}", target_name)));
        }

        // 1. Purge sécurisée de tous les autres agents actifs
        info!("CortexRegistry : Phase de Purge Mémoire...");
        for (name, agent) in registry.iter_mut() {
            if name != target_name && agent.is_active() {
                info!("Déchargement forcé de l'agent '{}' pour libérer la VRAM/RAM.", name);
                agent.unload().await?;
            }
        }

        // 2. Chargement de l'agent cible
        if let Some(target_agent) = registry.get_mut(target_name) {
            if !target_agent.is_active() {
                info!("Chargement lourd des tenseurs de '{}'...", target_name);
                target_agent.load().await?;
                info!("Agent '{}' 100% On-Line en RAM.", target_name);
            } else {
                info!("L'agent '{}' était déjà réveillé.", target_name);
            }
        }

        Ok(())
    }

    /// Exécute une instruction via l'agent actuellement actif.
    pub async fn interact_with(&self, target_name: &str, prompt: &str) -> Result<String, AgentError> {
        let mut registry = self.agents.write().await;
        
        if let Some(agent) = registry.get_mut(target_name) {
            if !agent.is_active() {
                return Err(AgentError::NotActive);
            }
            // L'agent génère le texte mathématiquement
            agent.generate_thought(prompt).await
        } else {
            Err(AgentError::LoadError("Agent inconnu".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    /// Agent factice utilisé pour valider la mécanique stricte de la RAM
    /// sans saturer la CI de GitHub Actions avec de vrais poids Tensoriels.
    struct MockAgent {
        name: String,
        active: bool,
    }

    impl MockAgent {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                active: false,
            }
        }
    }

    #[async_trait]
    impl CognitiveAgent for MockAgent {
        fn name(&self) -> &str {
            &self.name
        }

        async fn load(&mut self) -> Result<(), AgentError> {
            self.active = true;
            Ok(())
        }

        async fn unload(&mut self) -> Result<(), AgentError> {
            self.active = false;
            Ok(())
        }

        fn is_active(&self) -> bool {
            self.active
        }

        async fn generate_thought(&mut self, prompt: &str) -> Result<String, AgentError> {
            if !self.active {
                return Err(AgentError::NotActive);
            }
            Ok(format!("Mocked Thought: {}", prompt))
        }
    }

    #[tokio::test]
    async fn test_uniqueness_of_inference() {
        let registry = CortexRegistry::new();
        
        let agent_a = Box::new(MockAgent::new("Agent-A"));
        let agent_b = Box::new(MockAgent::new("Agent-B"));

        registry.register_agent(agent_a).await;
        registry.register_agent(agent_b).await;

        // Activation de A
        assert!(registry.activate("Agent-A").await.is_ok());
        
        {
            let agents = registry.agents.read().await;
            assert!(agents.get("Agent-A").unwrap().is_active());
            assert!(!agents.get("Agent-B").unwrap().is_active());
        }

        // Activation de B (Doit purger A de la RAM !)
        assert!(registry.activate("Agent-B").await.is_ok());

        {
            let agents = registry.agents.read().await;
            assert!(!agents.get("Agent-A").unwrap().is_active());
            assert!(agents.get("Agent-B").unwrap().is_active());
        }
    }

    #[tokio::test]
    async fn test_interaction_routing() {
        let registry = CortexRegistry::new();
        let agent = Box::new(MockAgent::new("MockingBird"));
        registry.register_agent(agent).await;

        // Échec si inactif
        assert!(registry.interact_with("MockingBird", "Hello").await.is_err());

        // Succès si actif
        registry.activate("MockingBird").await.unwrap();
        let thought = registry.interact_with("MockingBird", "Alpha").await.unwrap();
        assert_eq!(thought, "Mocked Thought: Alpha");
    }
}