use anyhow::Result;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use axum::extract::ws::{Message, WebSocket};
use futures::{sink::SinkExt, stream::StreamExt};
use tokio::sync::mpsc;

pub struct TerminalSession {
    pub master: Box<dyn portable_pty::MasterPty + Send>,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
}

pub fn spawn_terminal(workspace_id: Option<&str>) -> Result<(TerminalSession, bool, Option<String>)> {
    let pty_system = NativePtySystem::default();
    
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let mut cmd_builder;
    let mut ws_startup_script = None;
    let mut is_newly_booted = false;

    if let Some(id) = workspace_id {
        // Spin up the container if it's not running
        let session_opt = crate::chat_history::load_session(id);
        let ws_config = session_opt
            .as_ref()
            .and_then(|s| s.workspace_config.clone())
            .unwrap_or_default();
            
        ws_startup_script = ws_config.startup_script.clone();
            
        let mut workdir = "/root".to_string();
        if let Some(s) = &session_opt {
            if let Some(repo_name) = s.github_sources.first() {
                let parts: Vec<&str> = repo_name.split('/').collect();
                let folder = if parts.len() == 2 { parts[1] } else { repo_name };
                workdir = format!("/{}", folder);
            }
        }
            
        let container_name = format!("r2d2-workspace-{}", id);
        let (_workspace, is_new) = r2d2_cortex::workspace::PodmanWorkspace::new(
            &container_name,
            Some(&ws_config.base_image),
            ws_config.startup_script.as_deref()
        );
        is_newly_booted = is_new;

        let bash_entrypoint = format!("cd {} 2>/dev/null || cd /root; exec /bin/bash", workdir);

        cmd_builder = CommandBuilder::new("podman");
        cmd_builder.args(["exec", "-it", &container_name, "bash", "-c", &bash_entrypoint]);
    } else {
        return Err(anyhow::anyhow!("Sécurité R2D2: Accès au shell local interdict. Une session Workspace est obligatoire."));
    }
    for (key, value) in std::env::vars() {
        cmd_builder.env(key, value);
    }
    cmd_builder.env("TERM", "xterm-256color");
    cmd_builder.env("COLORTERM", "truecolor");

    let child = pair.slave.spawn_command(cmd_builder)?;

    Ok((TerminalSession {
        master: pair.master,
        child,
    }, is_newly_booted, ws_startup_script))
}

pub async fn handle_terminal_socket(socket: WebSocket, session_id: String) {
    println!(">>> HANDLE TERMINAL SOCKET INVOCATION for session_id: {}", session_id);
    let workspace_id = if session_id == "local" || session_id.is_empty() { None } else { Some(session_id.as_str()) };

    
    let (mut term_session, is_newly_booted, ws_startup_script) = match spawn_terminal(workspace_id) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to spawn terminal: {:?}", e);
            return;
        }
    };

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let reader = Arc::new(Mutex::new(term_session.master.try_clone_reader().unwrap()));
    let writer = Arc::new(Mutex::new(term_session.master.take_writer().unwrap()));

    if is_newly_booted {
        if let Some(script) = ws_startup_script {
            if !script.is_empty() {
                let writer_clone = writer.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(2500));
                    let cmd = format!("echo -e \"\\n\\033[1;36m[R2D2 Cortex]\\033[0m Lancement de la configuration automatique du Workspace...\\n\"; {}\r", script);
                    if let Ok(mut w) = writer_clone.lock() {
                        let _ = w.write_all(cmd.as_bytes());
                    }
                });
            }
        }
    }

    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(32);
    let reader_clone = reader.clone();
    
    std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        if let Ok(mut r) = reader_clone.lock() {
            loop {
                match r.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.blocking_send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    });

    loop {
        tokio::select! {
            Some(data) = rx.recv() => {
                if ws_sender.send(Message::Binary(data)).await.is_err() { break; }
            }
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(resize_cmd) = serde_json::from_str::<serde_json::Value>(&text) {
                            if resize_cmd.get("type").and_then(|t| t.as_str()) == Some("resize") {
                                if let (Some(cols), Some(rows)) = (
                                    resize_cmd.get("cols").and_then(|c| c.as_u64()),
                                    resize_cmd.get("rows").and_then(|r| r.as_u64())
                                ) {
                                    let _ = term_session.master.resize(PtySize {
                                        rows: rows as u16,
                                        cols: cols as u16,
                                        pixel_width: 0,
                                        pixel_height: 0,
                                    });
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Binary(bin))) => {
                        let w = writer.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            if let Ok(mut locked) = w.lock() {
                                let _ = locked.write_all(&bin);
                                let _ = locked.flush();
                            }
                        }).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
    
    let _ = term_session.child.kill();
}

pub async fn handle_ai_terminal_socket(socket: WebSocket, session_id: String, mut terminal_rx: tokio::sync::broadcast::Receiver<(String, String)>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    
    let init_msg = format!("\x1b[36m[R2D2 Engine] System Sensor Link Attached - Session: {}\x1b[0m\r\n\x1b[90mAwaiting autonomous agent activity...\x1b[0m\r\n", session_id);
    let _ = ws_sender.send(Message::Binary(init_msg.into_bytes())).await;
    
    loop {
        tokio::select! {
            Ok((target_session, msg)) = terminal_rx.recv() => {
                if target_session == session_id {
                    if ws_sender.send(Message::Binary(msg.into_bytes())).await.is_err() { break; }
                }
            }
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_terminal_local() {
        let mut session = spawn_terminal(None).unwrap();
        let mut reader = session.master.try_clone_reader().unwrap();
        let mut writer = session.master.take_writer().unwrap();

        writeln!(writer, "echo 'hello cortex'").unwrap();
        drop(writer);

        let mut buf = [0u8; 1024];
        let n = reader.read(&mut buf).unwrap();
        let output = String::from_utf8_lossy(&buf[..n]);

        assert!(output.len() > 0);
        let _ = session.child.kill();
    }
}
