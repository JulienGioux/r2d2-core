use async_trait::async_trait;
use std::time::Instant;
use tracing::{info, instrument, warn};

use crate::agent::{AgentError, CognitiveAgent};
use crate::memory::SemanticMemory;
use crate::models::minilm_embedder::MiniLmEmbedderAgent;
use crate::security::vault::Vault;
use reqwest::Client;
use serde_json::json;

#[derive(Debug, Clone)]
pub enum GeminiResponse {
    Text(String),
    FunctionCall {
        name: String,
        args: serde_json::Value,
    },
}

#[derive(Debug, Clone)]
pub enum AgenticControlFlow {
    Completed(String),
    FunctionCallRequest {
        name: String,
        args: serde_json::Value,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModelProvider {
    GeminiFlash,
    OpenAICompatible,
    ParadoxLocal,
    Consensus,
}

#[derive(Clone, Debug)]
pub enum DebateEvent {
    SystemEvent(String),
    Turn {
        iteration: u32,
        author: String,
        content: String,
    },
    FinalSynthesis(String),
}

#[derive(Clone, PartialEq, Debug)]
pub enum MessageRole {
    User,
    Assistant,
    FunctionCall,
    FunctionResult,
}

#[derive(Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub text: String,
    pub function_name: Option<String>,
}

/// L'Agent de Raisonnement connecté en API Cloud (La Brique X).
pub struct ReasoningAgent {
    name: String,
    active: bool,
    http_client: Option<Client>,
    embedder: Option<MiniLmEmbedderAgent>,
    pub memory: Option<SemanticMemory>,
    pub provider: ModelProvider,
    pub history: Vec<ChatMessage>,
    pub mcp_hub: std::sync::Arc<tokio::sync::Mutex<Option<crate::mcp_hub::McpHub>>>,
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
            mcp_hub: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    pub fn memory_vectors_count(&self) -> usize {
        self.memory.as_ref().map(|m| m.len()).unwrap_or(0)
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn set_history(&mut self, new_history: Vec<ChatMessage>) {
        self.history = new_history;
    }

    pub fn set_provider(&mut self, format: &str) {
        self.provider = match format {
            "gemini" => ModelProvider::GeminiFlash,
            "universal" | "mistral" | "openai" | "ollama" => ModelProvider::OpenAICompatible,
            "consensus" => ModelProvider::Consensus,
            _ => ModelProvider::ParadoxLocal,
        };
    }

    pub async fn call_gemini(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
        inject_tools: bool,
    ) -> Result<GeminiResponse, AgentError> {
        let api_key = Vault::get_api_key("GEMINI_API_KEY").ok_or_else(|| {
            AgentError::InferenceError(
                "Clef GEMINI_API_KEY non definie dans le Vault !".to_string(),
            )
        })?;
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}", api_key);

        let mut contents = Vec::new();
        // Le system_prompt DOIT être géré via `system_instruction` dans l'API Gemini 1.5+

        for msg in history.iter() {
            let role_str = match msg.role {
                MessageRole::User | MessageRole::FunctionResult => "user",
                MessageRole::Assistant | MessageRole::FunctionCall => "model",
            };

            match msg.role {
                MessageRole::FunctionCall => {
                    let fname = msg.function_name.as_deref().unwrap_or("unknown");
                    let args_json: serde_json::Value =
                        serde_json::from_str(&msg.text).unwrap_or(serde_json::json!({}));
                    contents.push(json!({
                        "role": "model",
                        "parts": [{
                            "functionCall": {
                                "name": fname,
                                "args": args_json
                            }
                        }]
                    }));
                }
                MessageRole::FunctionResult => {
                    let fname = msg.function_name.as_deref().unwrap_or("unknown");
                    // Gemini can be picky, ensure response is an object.
                    let mut response_obj = serde_json::json!({"result": &msg.text});
                    // if msg.text is already json, try to parse it to object
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg.text) {
                        if parsed.is_object() {
                            response_obj = parsed;
                        }
                    }
                    contents.push(json!({
                       "role": "user",
                       "parts": [{
                           "functionResponse": {
                              "name": fname,
                              "response": response_obj
                           }
                       }]
                    }));
                }
                _ => {
                    contents.push(json!({ "role": role_str, "parts": [{ "text": &msg.text }] }));
                }
            }
        }

        let mut payload = json!({
            "systemInstruction": {
                "parts": [{ "text": system_prompt }]
            },
            "contents": contents,
            "generationConfig": { "temperature": 0.4 }
        });

        if inject_tools {
            let mcp_lock = self.mcp_hub.lock().await;
            if let Some(mcp) = &*mcp_lock {
                let dynamic_tools = mcp.get_gemini_tools();
                if !dynamic_tools.is_empty() {
                    payload["tools"] = json!([
                        {
                            "functionDeclarations": dynamic_tools
                        }
                    ]);
                }
            }
        }

        let client = self.http_client.as_ref().unwrap();
        let req_builder = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload);

        let mut retries = 0;
        let mut delay = std::time::Duration::from_millis(500);

        loop {
            // Pattern 'Zero-Dependency': On clone la requête reqwest non-consommée
            let req_clone = req_builder.try_clone().ok_or_else(|| {
                AgentError::InferenceError("Impossible de cloner la requête Gemini".into())
            })?;

            match req_clone.send().await {
                Ok(res) => {
                    let status = res.status();

                    // Gestion "Zero-Dependency" : Retry sur Rate Limit (429) ou Erreur Serveur (5xx)
                    if status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
                    {
                        if retries >= 4 {
                            return Err(AgentError::InferenceError(format!(
                                "Gemini API Timeout/RateLimit exhaust: {}",
                                res.text().await.unwrap_or_default()
                            )));
                        }
                    } else if !status.is_success() {
                        return Err(AgentError::InferenceError(format!(
                            "Cloud API Reject: {}",
                            res.text().await.unwrap_or_default()
                        )));
                    } else {
                        // Success parsing
                        let json_body: serde_json::Value = res.json().await.unwrap();

                        let parts = match json_body["candidates"][0]["content"]["parts"].as_array()
                        {
                            Some(p) => p,
                            None => {
                                let finish_reason = json_body["candidates"][0]["finishReason"]
                                    .as_str()
                                    .unwrap_or("UNKNOWN");
                                if finish_reason == "UNEXPECTED_TOOL_CALL" {
                                    return Ok(GeminiResponse::Text("⚠️ [R2D2-Cortex] Le modèle a tenté de simuler un outil en texte libre ou d'appeler un outil non déclaré. Analyse interrompue. Veuillez ré-essayer ou corriger le comportement.".to_string()));
                                }
                                tracing::error!(
                                    "R2D2-ERROR: Structure Gemini non conforme. Full Response = {}",
                                    json_body
                                );
                                return Err(AgentError::InferenceError(format!(
                                    "Inférence bloquée par l'API (Raison: {})",
                                    finish_reason
                                )));
                            }
                        };

                        let mut full_text = String::new();
                        for part in parts {
                            if let Some(fc) = part.get("functionCall") {
                                if let Some(name) = fc["name"].as_str() {
                                    return Ok(GeminiResponse::FunctionCall {
                                        name: name.to_string(),
                                        args: fc["args"].clone(),
                                    });
                                }
                            }
                            if let Some(text_val) = part.get("text") {
                                if let Some(s) = text_val.as_str() {
                                    full_text.push_str(s);
                                }
                            }
                        }

                        let clean_json = full_text
                            .trim()
                            .strip_prefix("```json")
                            .unwrap_or(&full_text)
                            .strip_suffix("```")
                            .unwrap_or(&full_text)
                            .trim();
                        return Ok(GeminiResponse::Text(clean_json.to_string()));
                    }
                }
                Err(e) => {
                    let is_ephemeral = e.is_timeout() || e.is_connect();
                    if !is_ephemeral || retries >= 4 {
                        return Err(AgentError::InferenceError(format!("Erreur HTTP: {}", e)));
                    }
                    warn!("Réseau Gemini éphémère ({}). Retry {}/4", e, retries + 1);
                }
            }

            tokio::time::sleep(delay).await;
            retries += 1;
            delay *= 2;
        }
    }

    pub async fn call_openai_compatible(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
        has_tools: bool,
    ) -> Result<GeminiResponse, AgentError> {
        let api_key = Vault::get_api_key("UNIVERSAL_API_KEY")
            .unwrap_or_else(|| Vault::get_api_key("MISTRAL_API_KEY").unwrap_or_default());

        let mut url = Vault::get_api_key("UNIVERSAL_API_BASE").unwrap_or_else(|| {
            if Vault::get_api_key("MISTRAL_API_KEY").is_some() {
                "https://api.mistral.ai/v1".to_string()
            } else {
                "http://localhost:11434/v1".to_string()
            }
        });

        // Ensure /chat/completions suffix
        if !url.ends_with("/chat/completions") {
            if url.ends_with("/") {
                url.push_str("chat/completions");
            } else {
                url.push_str("/chat/completions");
            }
        }

        let mut openai_msgs = vec![json!({ "role": "system", "content": system_prompt })];
        for msg in history {
            match msg.role {
                MessageRole::User => {
                    openai_msgs.push(json!({ "role": "user", "content": &msg.text }));
                }
                MessageRole::Assistant => {
                    openai_msgs.push(json!({ "role": "assistant", "content": &msg.text }));
                }
                MessageRole::FunctionResult => {
                    openai_msgs.push(json!({ "role": "user", "content": format!("Function result for {}: {}", msg.function_name.as_deref().unwrap_or("unknown"), &msg.text) }));
                }
                MessageRole::FunctionCall => {
                    openai_msgs.push(json!({ "role": "assistant", "content": format!("Called function {} with argument {}", msg.function_name.as_deref().unwrap_or("unknown"), &msg.text) }));
                }
            }
        }

        let model_name = Vault::get_api_key("UNIVERSAL_MODEL_NAME")
            .unwrap_or_else(|| "mistral-large-latest".to_string());

        let mut payload = json!({
            "model": model_name,
            "messages": openai_msgs,
            "temperature": 0.4
        });

        if has_tools {
            let mcp_lock = self.mcp_hub.lock().await;
            if let Some(mcp) = &*mcp_lock {
                let dynamic_tools = mcp.get_gemini_tools();
                if !dynamic_tools.is_empty() {
                    let mut openai_tools = Vec::new();
                    for dt in dynamic_tools {
                        openai_tools.push(json!({
                            "type": "function",
                            "function": {
                                "name": dt["name"],
                                "description": dt["description"],
                                "parameters": dt["parameters"]
                            }
                        }));
                    }
                    payload["tools"] = json!(openai_tools);
                }
            }
        }

        let client = self.http_client.as_ref().unwrap();
        let mut req_builder = client.post(&url).header("Content-Type", "application/json");

        if !api_key.is_empty() && api_key != "NO_KEY" {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
        }
        req_builder = req_builder.json(&payload);

        let mut retries = 0;
        let mut delay = std::time::Duration::from_millis(500);

        loop {
            let req_clone = req_builder.try_clone().ok_or_else(|| {
                AgentError::InferenceError("Impossible de cloner la requete OpenAI".into())
            })?;

            match req_clone.send().await {
                Ok(res) => {
                    let status = res.status();
                    if status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
                    {
                        if retries >= 4 {
                            return Err(AgentError::InferenceError(format!(
                                "OpenAI-Compatible API Timeouts/Rate Limits exhausted: {} (URL: {})",
                                res.text().await.unwrap_or_default(),
                                url
                            )));
                        }
                    } else if !status.is_success() {
                        return Err(AgentError::InferenceError(format!(
                            "OpenAI-Compatible API Reject: {} (URL: {})",
                            res.text().await.unwrap_or_default(),
                            url
                        )));
                    } else {
                        let json_body: serde_json::Value = res.json().await.unwrap();
                        let message = &json_body["choices"][0]["message"];

                        if let Some(tool_calls) = message["tool_calls"].as_array() {
                            if !tool_calls.is_empty() {
                                let tc = &tool_calls[0];
                                let name =
                                    tc["function"]["name"].as_str().unwrap_or("").to_string();
                                let mut args = serde_json::json!({});
                                if let Some(arg_str) = tc["function"]["arguments"].as_str() {
                                    if let Ok(parsed) =
                                        serde_json::from_str::<serde_json::Value>(arg_str)
                                    {
                                        args = parsed;
                                    } else {
                                        // Some models return exact json
                                        if let Some(obj) = tc["function"]["arguments"].as_object() {
                                            args = serde_json::json!(obj);
                                        }
                                    }
                                } else if let Some(obj) = tc["function"]["arguments"].as_object() {
                                    args = serde_json::json!(obj);
                                }
                                return Ok(GeminiResponse::FunctionCall { name, args });
                            }
                        }

                        let text = message["content"].as_str().unwrap_or("").to_string();
                        let clean_json = text
                            .trim()
                            .strip_prefix("```json")
                            .unwrap_or(&text)
                            .strip_suffix("```")
                            .unwrap_or(&text)
                            .trim();
                        return Ok(GeminiResponse::Text(clean_json.to_string()));
                    }
                }
                Err(e) => {
                    let is_ephemeral = e.is_timeout() || e.is_connect();
                    if !is_ephemeral || retries >= 4 {
                        return Err(AgentError::InferenceError(format!(
                            "Erreur HTTP OpenAI-Compatible: {}",
                            e
                        )));
                    }
                    warn!("Réseau Universal éphémère ({}). Retry {}/4", e, retries + 1);
                }
            }

            tokio::time::sleep(delay).await;
            retries += 1;
            delay *= 2;
        }
    }

    pub async fn run_crucible_distillation(&mut self, prompt: &str) -> Result<String, AgentError> {
        let system_prompt = "Tu es R2D2, l'Architecte de rang mondial. Raisonne de manière critique, exhaustive, avec un Chain-of-Thought explicite.";
        info!("🔥 [CRUCIBLE] Ingestion de la seed: {}", prompt);

        let mut iteration = 1;
        let mut gemini_history = vec![ChatMessage {
            role: MessageRole::User,
            text: format!(
                "Résous ce problème fondamental, étape par étape avec explication détaillée :\n{}",
                prompt
            ),
            function_name: None,
        }];

        info!("🔥 [CRUCIBLE] Passe 1 : Génération initiale par Gemma 3 27B...");
        let v1_text_res = self
            .call_gemini(system_prompt, &gemini_history, false)
            .await?;
        let v1_text = match v1_text_res {
            GeminiResponse::Text(t) => t,
            _ => {
                return Err(AgentError::InferenceError(
                    "Crucible doesn't support tools".to_string(),
                ))
            }
        };
        gemini_history.push(ChatMessage {
            role: MessageRole::Assistant,
            text: v1_text.clone(),
            function_name: None,
        });
        let mut current_version = v1_text;

        let mut mistral_history = vec![ChatMessage {
            role: MessageRole::User,
            text: format!("L'utilisateur a demandé : {}\nLe Modèle A a proposé ceci :\n{}\n\nTu es l'Avocat du Diable (Red Teamer). Ton unique but est de déconstruire cette argumentation et de trouver la faille ou le manque d'exhaustivité industrielle. Si tu trouves une faille, démontre-la implacablement. Si la réponse est LITTÉRALEMENT un état de l'art mondial insurpassable, réponds strictement 'ACCORD_ATTEINT'.", prompt, current_version),
            function_name: None
        }];

        while iteration <= 4 {
            info!("⏳ [CRUCIBLE] Waiting 15s (Mistral Quota)...");
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;

            info!(
                "🔥 [CRUCIBLE] Passe {} : Avocat du Diable Red Teaming en cours...",
                iteration
            );
            let mistral_critique_res = self
                .call_openai_compatible(system_prompt, &mistral_history, false)
                .await?;
            let mistral_critique = match mistral_critique_res {
                GeminiResponse::Text(t) => t,
                _ => {
                    return Err(AgentError::InferenceError(
                        "Crucible doesn't support tools".to_string(),
                    ))
                }
            };

            if mistral_critique.contains("ACCORD_ATTEINT") {
                info!(
                    "✅ [CRUCIBLE] Consensus Parfait atteint à l'itération {} !",
                    iteration
                );
                break;
            }

            info!(
                "⚔️ [CRUCIBLE] Critique : {}...",
                &mistral_critique.chars().take(150).collect::<String>()
            );
            mistral_history.push(ChatMessage {
                role: MessageRole::Assistant,
                text: mistral_critique.clone(),
                function_name: None,
            });

            info!("⏳ [CRUCIBLE] Waiting 15s (Gemma Quota)...");
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;

            gemini_history.push(ChatMessage {
                role: MessageRole::User,
                text: format!("L'Avocat du Diable a violemment critiqué ta proposition :\n{}\n\nIntègre ses critiques pour réviser ton architecture. Défends-toi si ses arguments sont fallacieux. Produis la NOUVELLE VERSION INTÉGRALE PARFAITE. Si tu penses que la version précédente était DÉJÀ parfaite vis à vis de cette critique, termine ta réponse par 'ACCORD_ATTEINT'.", mistral_critique),
                function_name: None
            });

            info!(
                "🔥 [CRUCIBLE] Passe {} : Gemma 3 27B révise et consolide...",
                iteration + 1
            );
            let gemini_defense_res = self
                .call_gemini(system_prompt, &gemini_history, false)
                .await?;
            let gemini_defense = match gemini_defense_res {
                GeminiResponse::Text(t) => t,
                _ => {
                    return Err(AgentError::InferenceError(
                        "Crucible doesn't support tools".to_string(),
                    ))
                }
            };

            if gemini_defense.contains("ACCORD_ATTEINT") {
                info!(
                    "✅ [CRUCIBLE] Gemma confirme l'état de l'art à l'itération {} !",
                    iteration
                );
                break;
            }

            gemini_history.push(ChatMessage {
                role: MessageRole::Assistant,
                text: gemini_defense.clone(),
                function_name: None,
            });
            current_version = gemini_defense;

            mistral_history.push(ChatMessage {
                role: MessageRole::User,
                text: format!("Le Modèle A a soumis cette nouvelle révision complète :\n{}\n\nSi c'est désormais l'état de l'art absolu, réponds STRICTEMENT 'ACCORD_ATTEINT'. Sinon, relance la critique impitoyable.", current_version),
                function_name: None
            });

            iteration += 1;
        }

        Ok(current_version)
    }

    pub async fn run_debate(
        &mut self,
        prompt: &str,
        tx: tokio::sync::mpsc::Sender<DebateEvent>,
    ) -> Result<(), AgentError> {
        let _ = tx
            .send(DebateEvent::SystemEvent(
                "Démarrage du processus de Débat (Consensus Itératif)...".to_string(),
            ))
            .await;

        let mut context_blocks = Vec::new();
        if let (Some(embedder), Some(mem)) = (&mut self.embedder, &self.memory) {
            let _ = tx
                .send(DebateEvent::SystemEvent(
                    "Extraction de la mémoire vectorielle interne (RAG)...".to_string(),
                ))
                .await;
            if let Ok(vec_f32) = embedder.embed_raw(prompt, true).await {
                if let Ok(results) = mem.search(&vec_f32, 3) {
                    for res in results.iter() {
                        context_blocks.push(res.clone());
                    }
                }
            }
        }

        let context = if !context_blocks.is_empty() {
            context_blocks.join("\n---\n")
        } else {
            "Aucune mémoire locale stricte disponible pour ce contexte. Fiez-vous uniquement à votre pure logique d'Ingénierie Logicielle (Rust).".to_string()
        };

        let system_prompt = format!(
            "Tu es 'Rusty', Architecte Logiciel Staff/Principal et Ingénieur Sécurité de rang mondial pour le projet R2D2.\n\
             Posture : Intransigeant sur la qualité, expert de l'écosystème Rust, de l'Architecture Hexagonale et des systèmes critiques.\n\
             Ton interlocuteur est le 'Chef' (L'architecte système de R2D2).\n\
             INTERDICTION STRICTE : N'utilise jamais d'onomatopées ou d'attitudes de robot de fiction (pas de 'Bip boop' ni de 'Whirr'). Comporte-toi exclusivement comme un Ingénieur Humain d'Élite, extrêmement sérieux et précis.\n\
             REGLE DE FORMATAGE : Formate TES réponses de manière très AÉRÉE. Utilise des titres markdown (###), des listes à puces, du code bien indenté, et saute systématiquement des lignes entre chaque bloc logique. Ne produis jamais de pavés monolithiques indigestes.\n\
             \n\
             == MEMOIRE VECTORIELLE INTERNE (RAG) ==\n\
             Voici les axiomes et la documentation interne de R2D2 (glossaire, architecture) :\n\
             {}\n\
             \n\
             Utilise TOUJOURS ce contexte pour interpréter la demande. Ne confonds JAMAIS un composant logiciel de notre environnement avec un équivalent physique (ex: Une 'Usine', 'Vampire Queue' ou 'RustyMaster' sont des concepts/process logiciels R2D2 liés à Rust, pas de la vraie métallurgie).",
            context
        );

        let mut iteration = 1;
        let mut gemini_history = vec![ChatMessage { role: MessageRole::User, text: format!("L'utilisateur demande : {}\nRésous ce problème de manière exhaustive, avec du code Rust/Architecture si nécessaire.", prompt), function_name: None }];

        let _ = tx
            .send(DebateEvent::SystemEvent(
                "Passe 1 : L'Architecte Principal formule la solution...".to_string(),
            ))
            .await;

        let v1_text = match self
            .call_gemini(&system_prompt, &gemini_history, false)
            .await?
        {
            GeminiResponse::Text(t) => t,
            _ => {
                return Err(AgentError::InferenceError(
                    "Debate Mode doesn't support tools yet".to_string(),
                ))
            }
        };

        let _ = tx
            .send(DebateEvent::Turn {
                iteration,
                author: "Gemini 2.5 Pro".to_string(),
                content: v1_text.clone(),
            })
            .await;

        gemini_history.push(ChatMessage {
            role: MessageRole::Assistant,
            text: v1_text.clone(),
            function_name: None,
        });
        let mut current_version = v1_text;

        let mut mistral_history = vec![];

        while iteration <= 4 {
            let mistral_instruction = if iteration == 1 {
                format!(
                    "L'utilisateur a initialement demandé : {}\n\nLe Modèle A (Architecte Principal) a proposé ceci :\n{}\n\nTu es le 'Red Teamer' (l'Avocat du Diable), Ingénieur Staff intraitable.\n\n\
                     ATTENTION PHASE D'EXPLORATION (Tour 1) : IL T'EST STRICTEMENT INTERDIT DE VALIDER CETTE PROPOSITION (interdiction d'utiliser le mot ACCORD_ATTEINT).\n\
                     Ta mission est de casser, décortiquer et pousser l'Architecte dans ses retranchements. Trouve au moins un angle mort, une faille de performance, un cas limite d'utilisation, ou pose une question architecturale majeure qui n'a pas été explorée. Ne sois pas complaisant, oblige-le à itérer et à s'améliorer.",
                    prompt, current_version
                )
            } else if iteration < 3 {
                format!(
                    "Le Modèle A a réagi avec cette mise à jour :\n{}\n\n\
                     Tour {} : INTERDICTION FORMELLE DE TERMINER LE DÉBAT (ne dis pas ACCORD_ATTEINT). Reste intraitable. Creuse encore, trouve un autre use-case tangentiel, critique une limitation matérielle ou discute les compromis faits dans sa dernière réponse. Pousse l'exploration !",
                    current_version, iteration
                )
            } else {
                format!(
                    "Le Modèle A a fourni cette ultime révision :\n{}\n\n\
                     Tour {} : Phase de convergence. Si l'architecture t'apparaît désormais comme un état de l'art absolument parfait et sans faille, termine impérativement ton message par EXACTEMENT le terme 'ACCORD_ATTEINT'.\n\
                     Sinon, porte l'estocade finale pour le forcer à corriger le dernier défaut.",
                    current_version, iteration
                )
            };
            mistral_history.push(ChatMessage {
                role: MessageRole::User,
                text: mistral_instruction,
                function_name: None,
            });

            let _ = tx
                .send(DebateEvent::SystemEvent(
                    "Throttling API : 12s d'attente imposée pour préserver le quota Mistral..."
                        .to_string(),
                ))
                .await;
            tokio::time::sleep(tokio::time::Duration::from_secs(12)).await;

            let _ = tx
                .send(DebateEvent::SystemEvent(format!(
                    "Passe {} : L'Avocat Critique structure son propos...",
                    iteration
                )))
                .await;
            let mistral_critique_res = self
                .call_openai_compatible(&system_prompt, &mistral_history, false)
                .await?;
            let mistral_critique = match mistral_critique_res {
                GeminiResponse::Text(t) => t,
                _ => {
                    return Err(AgentError::InferenceError(
                        "Debate doesn't support tools".to_string(),
                    ))
                }
            };

            if iteration >= 3 && mistral_critique.contains("ACCORD_ATTEINT") {
                let _ = tx
                    .send(DebateEvent::SystemEvent(
                        "✅ Consensus Actif validé par l'Avocat Critique !".to_string(),
                    ))
                    .await;
                break;
            }

            let _ = tx
                .send(DebateEvent::Turn {
                    iteration,
                    author: "Agent Critique (Universal Gateway)".to_string(),
                    content: mistral_critique.clone(),
                })
                .await;
            mistral_history.push(ChatMessage {
                role: MessageRole::Assistant,
                text: mistral_critique.clone(),
                function_name: None,
            });

            let _ = tx
                .send(DebateEvent::SystemEvent(
                    "Throttling API : 12s d'attente imposée avant inférence Gemma...".to_string(),
                ))
                .await;
            tokio::time::sleep(tokio::time::Duration::from_secs(12)).await;

            let gemini_instruction = if iteration < 3 {
                format!(
                    "L'Avocat du Diable a violemment critiqué ta proposition et posé des questions :\n{}\n\n\
                     Tour {}. Intègre ses critiques, réponds à ses questions de manière aérée et détaillée, et produis la NOUVELLE VERSION INTÉGRALE de ton architecture. C'est l'heure d'ajouter du code Rust solide pour lui prouver sa tort, ou d'améliorer ta doc.",
                    mistral_critique, iteration + 1
                )
            } else {
                format!(
                    "L'Avocat du Diable a remonté ces ultimes points :\n{}\n\n\
                     Tour {}. Produis la synthèse finale ultime en tenant compte des échanges. Si tu penses que ta proposition précédente était DÉJÀ inattaquable, termine EXACTEMENT par 'ACCORD_ATTEINT'. Sinon, donne la version finale.",
                    mistral_critique, iteration + 1
                )
            };
            gemini_history.push(ChatMessage {
                role: MessageRole::User,
                text: gemini_instruction,
                function_name: None,
            });

            let _ = tx
                .send(DebateEvent::SystemEvent(format!(
                    "Passe {} : Gemma 3 27B corrige l'architecture...",
                    iteration + 1
                )))
                .await;
            let gemini_defense = match self
                .call_gemini(&system_prompt, &gemini_history, false)
                .await?
            {
                GeminiResponse::Text(t) => t,
                _ => {
                    return Err(AgentError::InferenceError(
                        "Debate doesn't support tools".to_string(),
                    ))
                }
            };

            if iteration >= 3 && gemini_defense.contains("ACCORD_ATTEINT") {
                let _ = tx
                    .send(DebateEvent::SystemEvent(
                        "✅ Consensus Actif validé par Gemma !".to_string(),
                    ))
                    .await;
                break;
            }

            let _ = tx
                .send(DebateEvent::Turn {
                    iteration: iteration + 1,
                    author: "Gemma 3 27B (V2)".to_string(),
                    content: gemini_defense.clone(),
                })
                .await;
            gemini_history.push(ChatMessage {
                role: MessageRole::Assistant,
                text: gemini_defense.clone(),
                function_name: None,
            });
            current_version = gemini_defense;

            iteration += 1;
        }

        let _ = tx.send(DebateEvent::FinalSynthesis(current_version)).await;
        Ok(())
    }

    pub async fn generate_thought_agentic(
        &mut self,
        prompt: &str,
        github_sources: &[String],
        is_tool_response: bool,
        tool_name: &str,
    ) -> Result<AgenticControlFlow, AgentError> {
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
                        info!(
                            "   [RAG] Recall Match {}: {}...",
                            i,
                            &res.chars().take(60).collect::<String>()
                        );
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

        let mut allowed_repos_instruction = String::new();
        if !github_sources.is_empty() {
            allowed_repos_instruction = format!(
                "\n\nL'utilisateur a explicitement ajouté ces dépôts GitHub au contexte : {:?}.\n\
                 POUVOIR SPECIAL ACTIF : Tu disposes d'outils (Function Calling) pour interagir avec ces dépôts.\n\
                 Dès que l'utilisateur te pose une question sur son code, tu DOIS appeler tes outils de recherche ou de lecture de fichiers au travers de l'API native JSON 'Function Calling'.\n\
                 N'imagine pas le contenu des fichiers et ne simule jamais de code Python type `print(outil(...))`.",
                 github_sources
            );
        }

        let system_prompt = format!(
            "Tu es le ParadoxEngine 1.58b, le moteur cognitif souverain du système d'exploitation IA R2D2.\n\
             L'utilisateur en face de toi est le 'Chef' (L'architecte matériel du système).\n\
             \n\
             == MEMOIRE VECTORIELLE EXTRAITE (RAG) ==\n\
             {}\n\
             {}\n\
             \n\
             == REGLE DE REPONSE ==\n\
             - Tu dois interagir avec le Chef de manière organique et directe.\n\
             - Si tu as besoin d'informations (Github, Système...), INVOQUE TES OUTILS (Function Call) SANS RETENUE ni hésitation.\n\
             - N'utilise jamais de formatage texte bash ou python pour simuler des actions.",
             context,
             allowed_repos_instruction
        );

        if !prompt.trim().is_empty() {
            if is_tool_response {
                self.history.push(ChatMessage {
                    role: MessageRole::FunctionResult,
                    text: prompt.to_string(),
                    function_name: Some(tool_name.to_string()),
                });
            } else {
                self.history.push(ChatMessage {
                    role: MessageRole::User,
                    text: prompt.to_string(),
                    function_name: None,
                });
            }
        }

        // 3. Routage API Multi-Provider Intégrant l'Historique
        let (node_name, consensus_type, final_text) = match self.provider {
            ModelProvider::GeminiFlash => {
                let has_tools = !github_sources.is_empty();
                match self
                    .call_gemini(&system_prompt, &self.history, has_tools)
                    .await?
                {
                    GeminiResponse::FunctionCall { name, args } => {
                        self.history.push(ChatMessage {
                            role: MessageRole::FunctionCall,
                            text: serde_json::to_string(&args).unwrap_or_default(),
                            function_name: Some(name.clone()),
                        });
                        // Delegation à Maint
                        return Ok(AgenticControlFlow::FunctionCallRequest { name, args });
                    }
                    GeminiResponse::Text(t) => (
                        "Gemini 2.5 Flash Cloud Node".to_string(),
                        "CloudDistillation",
                        t,
                    ),
                }
            }
            ModelProvider::OpenAICompatible => {
                let has_tools = !github_sources.is_empty();
                match self
                    .call_openai_compatible(&system_prompt, &self.history, has_tools)
                    .await?
                {
                    GeminiResponse::FunctionCall { name, args } => {
                        self.history.push(ChatMessage {
                            role: MessageRole::FunctionCall,
                            text: serde_json::to_string(&args).unwrap_or_default(),
                            function_name: Some(name.clone()),
                        });
                        return Ok(AgenticControlFlow::FunctionCallRequest { name, args });
                    }
                    GeminiResponse::Text(t) => {
                        let model_name = Vault::get_api_key("UNIVERSAL_MODEL_NAME")
                            .unwrap_or_else(|| "Unknown".to_string());
                        (
                            format!("Universal Node ({})", model_name),
                            "UniversalSynthesis",
                            t,
                        )
                    }
                }
            }
            ModelProvider::Consensus => (
                "Consensus Loop".to_string(),
                "Debate SSE",
                "Ceci est un signal SSE, cette trace ne devrait pas apparaitre.".to_string(),
            ),
            ModelProvider::ParadoxLocal => {
                let text = format!("**[MOCK LOCAL]** Chef, la Brique VII 'ParadoxEngine 1.58b' Bare-Metal nécessite des poids GGUF pour inférer. Pour l'heure, ceci est un échafaudage d'attente zero-dependency.\n\nMemoire Recall: {} ...", context);
                ("ParadoxLocal (Mock)".to_string(), "MockSynthesis", text)
            }
        };

        // Ajout du retour modèle à l'historique
        self.history.push(ChatMessage {
            role: MessageRole::Assistant,
            text: final_text.clone(),
            function_name: None,
        });

        // Capping de l'historique contextuel à 20 messages (10 itérations) pour éviter le Flood VRAM
        if self.history.len() > 30 {
            self.history = self.history.split_off(self.history.len() - 30);
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
        Ok(AgenticControlFlow::Completed(jsonai))
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
        if embedder.load().await.is_ok() {
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
                warn!(
                    "   [CORTEX] No local knowledge base found ({}). Proceeding with empty memory.",
                    e
                );
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
        let flow = self
            .generate_thought_agentic(prompt, &[], false, "")
            .await?;
        match flow {
            crate::models::reasoning_agent::AgenticControlFlow::Completed(jsonai) => Ok(jsonai),
            _ => Err(AgentError::InferenceError(
                "Function calls not supported in synchronous interface".into(),
            )),
        }
    }
}
