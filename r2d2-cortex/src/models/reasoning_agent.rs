use std::sync::Arc;
use tokio::sync::Mutex;
use candle_core::{Device, DType};
use async_trait::async_trait;
use tracing::{info, warn, instrument};
use std::time::Instant;

use crate::agent::{AgentError, CognitiveAgent};
use crate::models::minilm_embedder::MiniLmEmbedderAgent;
use crate::memory::SemanticMemory;
use reqwest::Client;
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModelProvider {
    GeminiFlash,
    MistralLarge,
    ParadoxLocal,
    Consensus,
}

#[derive(Clone, Debug)]
pub enum DebateEvent {
    SystemEvent(String),
    Turn { iteration: u32, author: String, content: String },
    FinalSynthesis(String),
}

#[derive(Clone)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub text: String,
}

/// L'Agent de Raisonnement connecté en API Cloud (La Brique X).
pub struct ReasoningAgent {
    name: String,
    active: bool,
    http_client: Option<Client>,
    embedder: Option<MiniLmEmbedderAgent>,
    memory: Option<SemanticMemory>,
    pub provider: ModelProvider,
    pub history: Vec<ChatMessage>,
}

impl Default for ReasoningAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl ReasoningAgent {
    pub fn new() -> Self {
        Self {
            name: "Paradox-MultiAPI Router".to_string(),
            active: false,
            http_client: None,
            embedder: None,
            memory: None,
            provider: ModelProvider::GeminiFlash,
            history: Vec::new(),
        }
    }
    
    pub fn set_provider(&mut self, format: &str) {
        self.provider = match format {
            "gemini" => ModelProvider::GeminiFlash,
            "mistral" => ModelProvider::MistralLarge,
            "consensus" => ModelProvider::Consensus,
            _ => ModelProvider::ParadoxLocal,
        };
    }

    pub async fn call_gemini(&self, system_prompt: &str, history: &[ChatMessage]) -> Result<String, AgentError> {
        let api_key = std::env::var("GEMINI_API_KEY")
            .map_err(|_| AgentError::InferenceError("Clef GEMINI_API_KEY non definie !".to_string()))?;
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}", api_key);
        
        let mut contents = Vec::new();
        for (i, msg) in history.iter().enumerate() {
            let role_str = match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "model",
            };
            
            // Injection du System Prompt dans le premier message "user" pour éviter 
            // le crash HTTP "system_instruction not supported" sur les modèles Gemma.
            let final_text = if i == 0 {
                format!("[CONTEXTE SYSTEME INAMOVIBLE] : {}\n\n[REQUETE DU CHEF] : {}", system_prompt, msg.text)
            } else {
                msg.text.clone()
            };
            
            contents.push(json!({ "role": role_str, "parts": [{ "text": final_text }] }));
        }
        
        let payload = json!({
            "contents": contents,
            "generationConfig": { "temperature": 0.4 }
        });
        
        let client = self.http_client.as_ref().unwrap();
        let res = client.post(&url).header("Content-Type", "application/json").json(&payload).send().await
            .map_err(|e| AgentError::InferenceError(format!("Erreur HTTP: {}", e)))?;
        if !res.status().is_success() {
            return Err(AgentError::InferenceError(format!("Cloud API Reject: {}", res.text().await.unwrap_or_default())));
        }
        let json_body: serde_json::Value = res.json().await.unwrap();
        let text = json_body["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("").to_string();
        
        let clean_json = text.trim().strip_prefix("```json").unwrap_or(&text).strip_suffix("```").unwrap_or(&text).trim();
        Ok(clean_json.to_string())
    }

    pub async fn call_mistral(&self, system_prompt: &str, history: &[ChatMessage]) -> Result<String, AgentError> {
        let api_key = std::env::var("MISTRAL_API_KEY")
            .map_err(|_| AgentError::InferenceError("Clef MISTRAL_API_KEY non definie !".to_string()))?;
        let url = "https://api.mistral.ai/v1/chat/completions";
        
        let mut mistral_msgs = vec![json!({ "role": "system", "content": system_prompt })];
        for msg in history {
            let role_str = match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
            };
            mistral_msgs.push(json!({ "role": role_str, "content": &msg.text }));
        }

        let payload = json!({
            "model": "mistral-large-latest",
            "messages": mistral_msgs
        });
        
        let client = self.http_client.as_ref().unwrap();
        let res = client.post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload).send().await
            .map_err(|e| AgentError::InferenceError(format!("Erreur HTTP: {}", e)))?;
        if !res.status().is_success() {
            return Err(AgentError::InferenceError(format!("Cloud API Reject: {}", res.text().await.unwrap_or_default())));
        }
        let json_body: serde_json::Value = res.json().await.unwrap();
        let text = json_body["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
        
        let clean_json = text.trim().strip_prefix("```json").unwrap_or(&text).strip_suffix("```").unwrap_or(&text).trim();
        Ok(clean_json.to_string())
    }

    pub async fn run_crucible_distillation(&mut self, prompt: &str) -> Result<String, AgentError> {
        let system_prompt = "Tu es R2D2, l'Architecte de rang mondial. Raisonne de manière critique, exhaustive, avec un Chain-of-Thought explicite.";
        info!("🔥 [CRUCIBLE] Ingestion de la seed: {}", prompt);
        
        let mut iteration = 1;
        let mut gemini_history = vec![ChatMessage { role: MessageRole::User, text: format!("Résous ce problème fondamental, étape par étape avec explication détaillée :\n{}", prompt) }];
        
        info!("🔥 [CRUCIBLE] Passe 1 : Génération initiale par Gemma 3 27B...");
        let v1_text = self.call_gemini(system_prompt, &gemini_history).await?;
        gemini_history.push(ChatMessage { role: MessageRole::Assistant, text: v1_text.clone() });
        let mut current_version = v1_text;
        
        let mut mistral_history = vec![ChatMessage { 
            role: MessageRole::User, 
            text: format!("L'utilisateur a demandé : {}\nLe Modèle A a proposé ceci :\n{}\n\nTu es l'Avocat du Diable (Red Teamer). Ton unique but est de déconstruire cette argumentation et de trouver la faille ou le manque d'exhaustivité industrielle. Si tu trouves une faille, démontre-la implacablement. Si la réponse est LITTÉRALEMENT un état de l'art mondial insurpassable, réponds strictement 'ACCORD_ATTEINT'.", prompt, current_version)
        }];

        while iteration <= 4 {
            info!("⏳ [CRUCIBLE] Waiting 15s (Mistral Quota)...");
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;

            info!("🔥 [CRUCIBLE] Passe {} : Mistral Red Teaming en cours...", iteration);
            let mistral_critique = self.call_mistral(system_prompt, &mistral_history).await?;
            
            if mistral_critique.contains("ACCORD_ATTEINT") {
                info!("✅ [CRUCIBLE] Consensus Parfait atteint à l'itération {} !", iteration);
                break;
            }

            info!("⚔️ [CRUCIBLE] Critique de Mistral : {}...", &mistral_critique.chars().take(150).collect::<String>());
            mistral_history.push(ChatMessage { role: MessageRole::Assistant, text: mistral_critique.clone() });
            
            info!("⏳ [CRUCIBLE] Waiting 15s (Gemma Quota)...");
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;

            gemini_history.push(ChatMessage {
                role: MessageRole::User,
                text: format!("L'Avocat du Diable a violemment critiqué ta proposition :\n{}\n\nIntègre ses critiques pour réviser ton architecture. Défends-toi si ses arguments sont fallacieux. Produis la NOUVELLE VERSION INTÉGRALE PARFAITE. Si tu penses que la version précédente était DÉJÀ parfaite vis à vis de cette critique, termine ta réponse par 'ACCORD_ATTEINT'.", mistral_critique)
            });

            info!("🔥 [CRUCIBLE] Passe {} : Gemma 3 27B révise et consolide...", iteration + 1);
            let gemini_defense = self.call_gemini(system_prompt, &gemini_history).await?;
            
            if gemini_defense.contains("ACCORD_ATTEINT") {
                info!("✅ [CRUCIBLE] Gemma confirme l'état de l'art à l'itération {} !", iteration);
                break;
            }

            gemini_history.push(ChatMessage { role: MessageRole::Assistant, text: gemini_defense.clone() });
            current_version = gemini_defense;

            mistral_history.push(ChatMessage {
                role: MessageRole::User,
                text: format!("Le Modèle A a soumis cette nouvelle révision complète :\n{}\n\nSi c'est désormais l'état de l'art absolu, réponds STRICTEMENT 'ACCORD_ATTEINT'. Sinon, relance la critique impitoyable.", current_version)
            });

            iteration += 1;
        }

        Ok(current_version)
    }

    pub async fn run_debate(&mut self, prompt: &str, tx: tokio::sync::mpsc::Sender<DebateEvent>) -> Result<(), AgentError> {
        let system_prompt = "Tu es R2D2, un Architecte IA de rang mondial. Raisonne de manière critique, industrielle, exhaustive.";
        let _ = tx.send(DebateEvent::SystemEvent("Démarrage du processus de Débat (Consensus Itératif)...".to_string())).await;
        
        let mut iteration = 1;
        let mut gemini_history = vec![ChatMessage { role: MessageRole::User, text: format!("L'utilisateur demande : {}\nRésous ce problème de manière complète.", prompt) }];
        
        let _ = tx.send(DebateEvent::SystemEvent("Passe 1 : Gemma 3 27B formule l'architecture initiale...".to_string())).await;
        let v1_text = self.call_gemini(system_prompt, &gemini_history).await?;
        
        let _ = tx.send(DebateEvent::Turn { iteration, author: "Gemma 3 27B".to_string(), content: v1_text.clone() }).await;
        
        gemini_history.push(ChatMessage { role: MessageRole::Assistant, text: v1_text.clone() });
        let mut current_version = v1_text;
        
        let mut mistral_history = vec![ChatMessage { 
            role: MessageRole::User, 
            text: format!("L'utilisateur a demandé : {}\nLe Modèle A a proposé ceci :\n{}\n\nEs-tu d'accord avec cette approche ? Critique, amende, ou valide en rédigeant ton analyse détaillée. Si et SEULEMENT SI la proposition de A est un état de l'art absolument parfait et ne nécessite pas la moindre virgule de modification, termine ton message par le mot exact 'ACCORD_ATTEINT'.", prompt, current_version)
        }];

        while iteration <= 4 {
            let _ = tx.send(DebateEvent::SystemEvent("Throttling API : 12s d'attente imposée pour préserver le quota Mistral...".to_string())).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(12)).await;

            let _ = tx.send(DebateEvent::SystemEvent(format!("Passe {} : Mistral Large exerce la Critique...", iteration))).await;
            let mistral_critique = self.call_mistral(system_prompt, &mistral_history).await?;
            
            if mistral_critique.contains("ACCORD_ATTEINT") {
                let _ = tx.send(DebateEvent::SystemEvent("✅ Consensus Actif validé par Mistral Large !".to_string())).await;
                break;
            }

            let _ = tx.send(DebateEvent::Turn { iteration, author: "Mistral Large (Critique)".to_string(), content: mistral_critique.clone() }).await;
            mistral_history.push(ChatMessage { role: MessageRole::Assistant, text: mistral_critique.clone() });
            
            let _ = tx.send(DebateEvent::SystemEvent("Throttling API : 12s d'attente imposée avant inférence Gemma...".to_string())).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(12)).await;

            gemini_history.push(ChatMessage {
                role: MessageRole::User,
                text: format!("Le Modèle B a critiqué ta proposition :\n{}\n\nIntègre ses critiques pour réviser l'architecture et rédige la NOUVELLE VERSION INTÉGRALE de ta réponse. Si tu considères que ta proposition précédente était déjà absolument parfaite face à cette critique, termine ta réponse par STRICTEMENT 'ACCORD_ATTEINT'.", mistral_critique)
            });

            let _ = tx.send(DebateEvent::SystemEvent(format!("Passe {} : Gemma 3 27B corrige l'architecture...", iteration + 1))).await;
            let gemini_defense = self.call_gemini(system_prompt, &gemini_history).await?;
            
            if gemini_defense.contains("ACCORD_ATTEINT") {
                let _ = tx.send(DebateEvent::SystemEvent("✅ Consensus Actif validé par Gemma !".to_string())).await;
                break;
            }

            let _ = tx.send(DebateEvent::Turn { iteration: iteration + 1, author: "Gemma 3 27B (V2)".to_string(), content: gemini_defense.clone() }).await;
            gemini_history.push(ChatMessage { role: MessageRole::Assistant, text: gemini_defense.clone() });
            current_version = gemini_defense;

            mistral_history.push(ChatMessage {
                role: MessageRole::User,
                text: format!("Le Modèle A a mis à jour sa proposition :\n{}\n\nSi c'est parfait, termine ta réponse par 'ACCORD_ATTEINT'. Sinon, relance la critique détaillée.", current_version)
            });

            iteration += 1;
        }

        let _ = tx.send(DebateEvent::FinalSynthesis(current_version)).await;
        Ok(())
    }
}

#[async_trait]
impl CognitiveAgent for ReasoningAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_active(&self) -> bool {
        self.active
    }

    #[instrument(skip(self))]
    async fn load(&mut self) -> Result<(), AgentError> {
        info!("🔌 [ReasoningAgent] Booting ParadoxEngine (Cloud API Router Mode)...");
        
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .map_err(|e| AgentError::LoadError(format!("Reqwest build failed: {}", e)))?;
            
        self.http_client = Some(client);

        // Brique VIII : Chargement de la Mémoire Sémantique Zéro-Copy et de son Embedder
        info!("   [CORTEX] Booting RAG subsystem (MiniLM + Mmap)...");
        let mut embedder = MiniLmEmbedderAgent::new();
        if let Ok(_) = embedder.load().await {
            self.embedder = Some(embedder);
        } else {
            warn!("   [CORTEX] Failed to load embedder, RAG will be disabled.");
        }

        match SemanticMemory::load("knowledge.bin", "knowledge_meta.json") {
            Ok(mem) => {
                info!("   [CORTEX] 🗄️ Knowledge Base Mapped (Zero-Copy)!");
                self.memory = Some(mem);
            }
            Err(e) => {
                warn!("   [CORTEX] No local knowledge base found ({}). Proceeding with empty memory.", e);
            }
        }

        self.active = true;
        
        info!("✅ [ReasoningAgent] ParadoxEngine is UP and Air-Gapped.");
        Ok(())
    }

    async fn unload(&mut self) -> Result<(), AgentError> {
        warn!("🔻 [ReasoningAgent] Deactivating ParadoxEngine Router...");
        self.http_client = None;
        if let Some(mut embedder) = self.embedder.take() {
            let _ = embedder.unload().await;
        }
        self.memory = None;
        self.active = false;
        Ok(())
    }

    #[instrument(skip_all, name = "ReasoningAgent::generate_thought")]
    async fn generate_thought(&mut self, prompt: &str) -> Result<String, AgentError> {
        if !self.is_active() || self.http_client.is_none() {
            return Err(AgentError::NotActive);
        }

        let start = Instant::now();
        info!("🧠 [ReasoningAgent] ParadoxEngine sends query to Cloud Architect...");

        // 1. Extraction RAG Locale (Mémoire Vectorielle Infaillible)
        let mut context_blocks = Vec::new();
        if let (Some(embedder), Some(mem)) = (&mut self.embedder, &self.memory) {
            info!("   [RAG] Searching semantic memory for query...");
            if let Ok(vec_f32) = embedder.embed_raw(prompt, true).await {
                if let Ok(results) = mem.search(&vec_f32, 3) {
                    for (i, res) in results.iter().enumerate() {
                        info!("   [RAG] Recall Match {}: {}...", i, &res.chars().take(60).collect::<String>());
                        context_blocks.push(res.clone());
                    }
                }
            }
        }

        let context = if !context_blocks.is_empty() {
            context_blocks.join("\n---\n")
        } else {
            "Aucune mémoire locale stricte disponible. Fiez-vous uniquement à votre logique interne.".to_string()
        };

        // 2. Construction du System Prompt "Maître"
        let system_prompt = format!(
            "Tu es le ParadoxEngine 1.58b, le moteur cognitif souverain du système d'exploitation IA R2D2.\n\
             L'utilisateur en face de toi est le 'Chef' (L'architecte matériel du système).\n\
             \n\
             == MEMOIRE VECTORIELLE EXTRAITE (RAG) ==\n\
             Voici les axiomes et faits locaux exacts concernant ce système ou sa demande :\n\
             {}\n\
             \n\
             == REGLE DE REPONSE CRITIQUE ==\n\
             Tu dois interagir avec le Chef de manière organique, en utilisant la mémoire vectorielle ci-dessus si pertinente.\n\
             Réponds directement, de manière claire et assertive, au format texte simple. NE PAS ENCAPSULER LA RÉPONSE DANS DU JSON. Le système R2D2 (mon routeur Rust) s'occupera lui-même de t'encapsuler dans le format JSONAI V3 strict.",
             context
        );

        // 3. Gestion de l'Historique de Conversation (Memoire Court Terme)
        self.history.push(ChatMessage { role: MessageRole::User, text: prompt.to_string() });

        // 4. Routage API Multi-Provider Intégrant l'Historique
        let (node_name, consensus_type, final_text) = match self.provider {
            ModelProvider::GeminiFlash => {
                let text = self.call_gemini(&system_prompt, &self.history).await?;
                ("Gemma 3 27B Cloud Node", "CloudDistillation", text)
            },
            ModelProvider::MistralLarge => {
                let text = self.call_mistral(&system_prompt, &self.history).await?;
                ("Mistral Large Cloud Node", "CloudDistillation", text)
            },
            ModelProvider::Consensus => {
                ("Consensus Loop", "Debate SSE", "Ceci est un signal SSE, cette trace ne devrait pas apparaitre.".to_string())
            },
            ModelProvider::ParadoxLocal => {
                let text = format!("**[MOCK LOCAL]** Chef, la Brique VII 'ParadoxEngine 1.58b' Bare-Metal nécessite des poids GGUF pour inférer. Pour l'heure, ceci est un échafaudage d'attente zero-dependency.\n\nMemoire Recall: {}", context);
                ("ParadoxLocal (Mock)", "MockSynthesis", text)
            }
        };

        // Ajout du retour modèle à l'historique
        self.history.push(ChatMessage { role: MessageRole::Assistant, text: final_text.clone() });

        // Capping de l'historique contextuel à 20 messages (10 itérations) pour éviter le Flood VRAM
        if self.history.len() > 20 {
            self.history = self.history.split_off(self.history.len() - 20);
        }

        // 5. Encapsulation Finale dans le Standard R2D2 JSONAi V3
        let jsonai = format!(
            r#"{{
            "id": "paradox-multiapi-{}",
            "source": {{ "ParadoxEngine": "{}" }},
            "timestamp": "2026-03-25T00:00:00Z",
            "is_fact": true,
            "belief_state": 0.99,
            "consensus": "{}",
            "content": {},
            "ontological_tags": ["Reasoning", "Abstract", "Router", "MemoryRAG"],
            "dependencies": []
        }}"#,
            start.elapsed().as_millis(),
            node_name,
            consensus_type,
            serde_json::to_string(&final_text).unwrap()
        );

        info!("Inférence Cloud accomplie en {:?}", start.elapsed());
        Ok(jsonai)
    }
}
