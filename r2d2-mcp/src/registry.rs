use serde_json::json;
use std::collections::HashMap;

/// ============================================================================
/// 📖 TOOL DISCOVERY REGISTRY : MAPPAGE DES CAPACITÉS DYNAMIQUES
/// ============================================================================
/// Représente la définition standard d'un Outil au sens du Model Context Protocol (MCP).
#[derive(Debug, Clone)]
pub struct McpToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub requires_hitl: bool,
}

pub struct ToolRegistry {
    tools: HashMap<String, McpToolDef>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        registry.register_core_tools();
        registry
    }

    /// Enregistre les capacités natives d'introspection et d'action du Kernel
    fn register_core_tools(&mut self) {
        self.register(McpToolDef {
            name: "anchor_thought".to_string(),
            description: "Force le R2D2 Kernel à analyser une proposition via le ParadoxSolver et à l'ancrer dans sa matrice.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "Le texte brut de la pensée." },
                    "agent_name": { "type": "string", "description": "L'IA d'origine." }
                },
                "required": ["content", "agent_name"]
            }),
            requires_hitl: false,
        });

        self.register(McpToolDef {
            name: "recall_memory".to_string(),
            description: "Recherche sémantique vectorielle (HNSW) dans le Blackboard R2D2."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Question ou mots-clés." }
                },
                "required": ["query"]
            }),
            requires_hitl: false,
        });

        // Brique 8: Cycle Circadien
        self.register(McpToolDef {
            name: "read_dreams".to_string(),
            description:
                "Lire les déductions logiques forgées par le Moteur Circadien pendant la nuit."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "number", "description": "Nombre de rêves à remonter" }
                },
                "required": ["limit"]
            }),
            requires_hitl: false,
        });

        // Nouvel outil testant le HITL
        self.register(McpToolDef {
            name: "delete_memory_cluster".to_string(),
            description: "[DANGEREUX] Purge un cluster sémantique spécifique de la mémoire vectorielle locale.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "cluster_id": { "type": "string", "description": "UUID du cluster à irradier." }
                },
                "required": ["cluster_id"]
            }),
            requires_hitl: true, // Le Proxy HITL va automatiquement s'enclencher dû au pattern 'delete_'
        });

        // Brique V: Synthèse Sensorielle
        self.register(McpToolDef {
            name: "ingest_audio".to_string(),
            description: "Soumettre un fichier audio (.ogg Vorbis privilégié) au système nerveux périphérique de R2D2 pour transcription locale via Whisper.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "audio_path": { "type": "string", "description": "Chemin absolu vers l'archive audio locale." }
                },
                "required": ["audio_path"]
            }),
            requires_hitl: false,
        });

        self.register(McpToolDef {
            name: "ingest_visual".to_string(),
            description: "Soumettre une image ou keyframe au R2D2 Sensory Gateway pour analyse sémantique détaillée via le neuro-agent LLaVA.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "image_path": { "type": "string", "description": "Chemin absolu vers le fichier visuel local." }
                },
                "required": ["image_path"]
            }),
            requires_hitl: false,
        });
        self.register(McpToolDef {
            name: "read_git_status".to_string(),
            description:
                "Analyse l'état actuel du dépôt Git (fichiers non-suivis, modifiés, stagés)."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
            }),
            requires_hitl: false,
        });

        self.register(McpToolDef {
            name: "read_git_log".to_string(),
            description: "Lit les derniers commits du dépôt Git.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "number", "description": "Nombre de commits à lire (défaut: 5)" }
                }
            }),
            requires_hitl: false,
        });
    }

    pub fn register(&mut self, tool: McpToolDef) {
        self.tools.insert(tool.name.clone(), tool);
    }

    /// Exporte la liste complète au format JSON exigé par le protocole MCP (tools/list)
    pub fn export_mcp_format(&self) -> serde_json::Value {
        let tools_array: Vec<serde_json::Value> = self
            .tools
            .values()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema
                })
            })
            .collect();

        json!({ "tools": tools_array })
    }

    pub fn exists(&self, tool_name: &str) -> bool {
        self.tools.contains_key(tool_name)
    }
}
