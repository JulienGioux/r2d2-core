use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

fn default_pinned() -> bool {
    false
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ContextResource {
    pub res_type: String, // "github", "file", "folder", "web"
    pub path: String,     // repo name, or physical path, or URL
    pub priority: String, // "grey" (MCP), "green" (RAG), "gold" (System Prompt)
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct WorkspaceConfig {
    pub name: String,
    pub base_image: String,
    pub startup_script: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DebateConfig {
    pub architect_provider: String,
    pub red_teamer_provider: String,
    pub max_rounds: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub updated_at: u64,
    #[serde(default = "default_pinned")]
    pub pinned: bool,
    #[serde(default)]
    pub github_sources: Vec<String>,
    #[serde(default)]
    pub context_resources: Vec<ContextResource>,
    pub debate_config: Option<DebateConfig>,
    pub workspace_config: Option<WorkspaceConfig>,
    pub turns: Vec<ChatTurn>,
}

pub fn save_turn(
    session_id: &str,
    user_msg: &str,
    assistant_msg: &str,
    current_sources: Vec<String>,
) {
    let dir = PathBuf::from("data/chats");
    let _ = fs::create_dir_all(&dir);
    let file_path = dir.join(format!("{}.json", session_id));

    let mut session = if file_path.exists() {
        let data = fs::read_to_string(&file_path).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or_else(|_| ChatSession {
            id: session_id.to_string(),
            title: user_msg.chars().take(28).collect::<String>() + "...",
            updated_at: 0,
            pinned: false,
            github_sources: current_sources.clone(),
            context_resources: vec![],
            debate_config: None,
            workspace_config: None,
            turns: vec![],
        })
    } else {
        ChatSession {
            id: session_id.to_string(),
            title: user_msg.chars().take(28).collect::<String>() + "...",
            updated_at: 0,
            pinned: false,
            github_sources: current_sources.clone(),
            context_resources: vec![],
            debate_config: None,
            workspace_config: None,
            turns: vec![],
        }
    };

    session.updated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    session.github_sources = current_sources;
    session.turns.push(ChatTurn {
        role: "user".into(),
        content: user_msg.into(),
    });
    session.turns.push(ChatTurn {
        role: "assistant".into(),
        content: assistant_msg.into(),
    });

    if let Ok(json) = serde_json::to_string_pretty(&session) {
        let _ = fs::write(file_path, json);
    }
}

pub fn list_sessions() -> Vec<ChatSession> {
    let dir = PathBuf::from("data/chats");
    let mut sessions = vec![];
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry
                .path()
                .extension()
                .map(|s| s == "json")
                .unwrap_or(false)
            {
                if let Ok(data) = fs::read_to_string(entry.path()) {
                    if let Ok(sess) = serde_json::from_str::<ChatSession>(&data) {
                        sessions.push(sess);
                    }
                }
            }
        }
    }
    sessions.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });
    sessions
}

pub fn load_session(session_id: &str) -> Option<ChatSession> {
    let file_path = PathBuf::from("data/chats").join(format!("{}.json", session_id));
    let data = fs::read_to_string(file_path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn delete_session(session_id: &str) {
    let file_path = PathBuf::from("data/chats").join(format!("{}.json", session_id));
    let _ = fs::remove_file(file_path);
}

pub fn rename_session(session_id: &str, new_title: &str) {
    if let Some(mut session) = load_session(session_id) {
        session.title = new_title.to_string();
        if let Ok(json) = serde_json::to_string_pretty(&session) {
            let file_path = PathBuf::from("data/chats").join(format!("{}.json", session_id));
            let _ = fs::write(file_path, json);
        }
    }
}

pub fn toggle_pin_session(session_id: &str) {
    if let Some(mut session) = load_session(session_id) {
        session.pinned = !session.pinned;
        if let Ok(json) = serde_json::to_string_pretty(&session) {
            let file_path = PathBuf::from("data/chats").join(format!("{}.json", session_id));
            let _ = fs::write(file_path, json);
        }
    }
}

pub fn add_context_resource(session_id: &str, res: ContextResource) {
    if let Some(mut session) = load_session(session_id) {
        // Remove existing resource with same path if any
        session.context_resources.retain(|r| r.path != res.path);
        session.context_resources.push(res);
        let file_path = PathBuf::from("data/chats").join(format!("{}.json", session_id));
        if let Ok(json) = serde_json::to_string_pretty(&session) {
            let _ = fs::write(file_path, json);
        }
    }
}

pub fn remove_context_resource(session_id: &str, path: &str) {
    if let Some(mut session) = load_session(session_id) {
        session.context_resources.retain(|r| r.path != path);
        let file_path = PathBuf::from("data/chats").join(format!("{}.json", session_id));
        if let Ok(json) = serde_json::to_string_pretty(&session) {
            let _ = fs::write(file_path, json);
        }
    }
}

pub fn save_session(session_id: &str, session: ChatSession) {
    let dir = PathBuf::from("data/chats");
    let _ = fs::create_dir_all(&dir);
    let file_path = dir.join(format!("{}.json", session_id));
    if let Ok(data) = serde_json::to_string_pretty(&session) {
        let _ = fs::write(file_path, data);
    }
}

pub fn update_workspace_config(session_id: &str, config: WorkspaceConfig) {
    if let Some(mut session) = load_session(session_id) {
        session.workspace_config = Some(config);
        save_session(session_id, session);
    }
}

pub fn update_debate_config(session_id: &str, config: DebateConfig) {
    if let Some(mut session) = load_session(session_id) {
        session.debate_config = Some(config);
        save_session(session_id, session);
    }
}
