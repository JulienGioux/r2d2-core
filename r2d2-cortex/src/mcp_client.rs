use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use serde_json::{json, Value};
use anyhow::{Result, Context, anyhow};

#[derive(Debug)]
pub enum McpCommand {
    CallTool {
        name: String,
        arguments: Value,
        reply: oneshot::Sender<Result<Value>>,
    },
}

#[derive(Clone)]
pub struct McpClient {
    tx: mpsc::Sender<McpCommand>,
}

impl McpClient {
    pub async fn new(github_token: &str) -> Result<Self> {
        let (tx, mut rx) = mpsc::channel::<McpCommand>(32);

        // Détermination dynamique du moteur de conteneur ("podman" prioritaire, sinon "docker")
        let engine = if Command::new("podman")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
        {
            "podman"
        } else {
            "docker"
        };

        let mut child = Command::new(engine)
            .arg("run")
            .arg("-i")
            .arg("--rm")
            .arg("-e")
            .arg(format!("GITHUB_PERSONAL_ACCESS_TOKEN={}", github_token))
            .arg("ghcr.io/github/github-mcp-server:latest")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit()) // Route stderr to the main console for tracing
            .spawn()
            .context("Failed to spawn docker ghcr.io/github/github-mcp-server")?;

        let mut stdin = child.stdin.take().ok_or(anyhow!("Missing stdin"))?;
        let stdout = child.stdout.take().ok_or(anyhow!("Missing stdout"))?;
        let mut reader = BufReader::new(stdout).lines();

        // Perform MCP Handshake
        let init_req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "r2d2-cortex-mcp", "version": "1.0.0" }
            }
        });

        let mut init_str = serde_json::to_string(&init_req)?;
        init_str.push('\n');
        stdin.write_all(init_str.as_bytes()).await?;
        stdin.flush().await?;

        // Wait for initialize response with a strict 10s timeout
        let handshake_future = async {
            loop {
                if let Some(line) = reader.next_line().await? {
                    if let Ok(res) = serde_json::from_str::<Value>(&line) {
                        if res.get("id").and_then(|i| i.as_u64()) == Some(1) {
                            if res.get("error").is_some() {
                                return Err(anyhow!("MCP Initialization Error: {}", res));
                            }
                            
                            // Send initialized notification
                            let initialized_req = json!({
                                "jsonrpc": "2.0",
                                "method": "notifications/initialized"
                            });
                            let mut initd_str = serde_json::to_string(&initialized_req)?;
                            initd_str.push('\n');
                            stdin.write_all(initd_str.as_bytes()).await?;
                            stdin.flush().await?;
                            tracing::info!("🔌 [McpClient] Handshake 'initialize' completed with Daemon.");
                            break;
                        }
                    } else {
                        tracing::debug!("Ignoring unparseable line during init: {}", line);
                    }
                } else {
                    return Err(anyhow!("MCP Daemon exited before initialization"));
                }
            }
            Ok::<(), anyhow::Error>(())
        };

        match tokio::time::timeout(std::time::Duration::from_secs(10), handshake_future).await {
            Ok(Ok(_)) => {},
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(anyhow!("MCP Daemon Handshake Timeout: Container Engine ({}) failed to route STDIO to ghcr.io/github/github-mcp-server within 10s. Is the daemon running?", engine)),
        }

        let next_id = Arc::new(AtomicUsize::new(2));

        // Start Actor Loop
        tokio::spawn(async move {
            let mut pending_calls: HashMap<usize, oneshot::Sender<Result<Value>>> = HashMap::new();

            loop {
                tokio::select! {
                    cmd = rx.recv() => {
                        match cmd {
                            Some(McpCommand::CallTool { name, arguments, reply }) => {
                                let id = next_id.fetch_add(1, Ordering::SeqCst);
                                let req = json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "method": "tools/call",
                                    "params": {
                                        "name": name,
                                        "arguments": arguments
                                    }
                                });
                                let mut req_str = serde_json::to_string(&req).unwrap_or_default();
                                req_str.push('\n');

                                if let Err(e) = stdin.write_all(req_str.as_bytes()).await {
                                    tracing::error!("[McpClient] Pipe write error: {}", e);
                                    let _ = reply.send(Err(anyhow::anyhow!("Pipe broken: {}", e)));
                                    break;
                                }
                                let _ = stdin.flush().await;
                                pending_calls.insert(id, reply);
                            }
                            None => {
                                // Channel closed
                                tracing::info!("[McpClient] Channel closed, killing Daemon.");
                                let _ = child.kill().await;
                                break;
                            }
                        }
                    }
                    line_res = reader.next_line() => {
                        match line_res {
                            Ok(Some(line)) => {
                                if let Ok(val) = serde_json::from_str::<Value>(&line) {
                                    // is it a response to an id?
                                    if let Some(id_val) = val.get("id") {
                                        if let Some(id) = id_val.as_u64() {
                                            if let Some(reply) = pending_calls.remove(&(id as usize)) {
                                                if let Some(err) = val.get("error") {
                                                    let _ = reply.send(Err(anyhow::anyhow!("{}", err)));
                                                } else {
                                                    let content = val.get("result").unwrap_or(&json!({})).clone();
                                                    let _ = reply.send(Ok(content));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                tracing::warn!("[McpClient] Daemon stdout EOF");
                                break;
                            }
                            Err(e) => {
                                tracing::error!("[McpClient] Daemon read stream error: {}", e);
                                break;
                            }
                        }
                    }
                }
            }

            // RAII Cleanup
            tracing::info!("🛡️ [McpClient] Reaping MCP Daemon zombie process...");
            for (_, reply) in pending_calls {
                let _ = reply.send(Err(anyhow::anyhow!("Daemon crashed mid-request")));
            }
            let _ = child.kill().await;
            let _ = child.wait().await;
        });

        Ok(Self { tx })
    }

    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx.send(McpCommand::CallTool {
            name: name.to_string(),
            arguments,
            reply: reply_tx,
        }).await.map_err(|_| anyhow::anyhow!("McpDaemon thread is dead"))?;

        reply_rx.await.map_err(|_| anyhow::anyhow!("McpDaemon dropped the response"))?
    }
}
