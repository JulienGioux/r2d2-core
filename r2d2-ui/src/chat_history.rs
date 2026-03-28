use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

fn default_pinned() -> bool { false }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub updated_at: u64,
    #[serde(default = "default_pinned")]
    pub pinned: bool,
    pub turns: Vec<ChatTurn>,
}

pub fn save_turn(session_id: &str, user_msg: &str, assistant_msg: &str) {
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
            turns: vec![],
        })
    } else {
        ChatSession {
            id: session_id.to_string(),
            title: user_msg.chars().take(28).collect::<String>() + "...",
            updated_at: 0,
            pinned: false,
            turns: vec![],
        }
    };

    session.updated_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    session.turns.push(ChatTurn { role: "user".into(), content: user_msg.into() });
    session.turns.push(ChatTurn { role: "assistant".into(), content: assistant_msg.into() });

    if let Ok(json) = serde_json::to_string_pretty(&session) {
        let _ = fs::write(file_path, json);
    }
}

pub fn list_sessions() -> Vec<ChatSession> {
    let dir = PathBuf::from("data/chats");
    let mut sessions = vec![];
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().extension().map(|s| s == "json").unwrap_or(false) {
                if let Ok(data) = fs::read_to_string(entry.path()) {
                    if let Ok(sess) = serde_json::from_str::<ChatSession>(&data) {
                        sessions.push(sess);
                    }
                }
            }
        }
    }
    sessions.sort_by(|a, b| {
        b.pinned.cmp(&a.pinned).then_with(|| b.updated_at.cmp(&a.updated_at))
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
