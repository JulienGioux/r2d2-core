use anyhow::{Context, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tracing::{error, info, instrument};

/// ============================================================================
/// 🔌 MCP UNIVERSAL CLIENT : CONNEXION AUX BINAIRES EXTERNES
/// ============================================================================
pub struct McpUniversalClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    request_id: u64,
    ready_rx: Option<tokio::sync::oneshot::Receiver<()>>,
}

impl McpUniversalClient {
    /// Lance un binaire externe (ex: `python -m mcp_server`, `npx ...`) et
    /// Attache les flux standard pour la communication JSON-RPC avec une carte d'environnement.
    #[instrument(skip_all, name = "McpUniversalClient::spawn")]
    pub async fn spawn(command: &str, args: &[&str], env_vars: Option<std::collections::HashMap<String, String>>) -> Result<Self> {
        info!("Lancement du serveur MCP externe : {} {:?}", command, args);

        let mut cmd = Command::new(command);
        cmd.args(args)
           .stdin(std::process::Stdio::piped())
           .stdout(std::process::Stdio::piped())
           .stderr(std::process::Stdio::piped()); // Capture asynchrone pour filtrage

        if let Some(envs) = env_vars {
            cmd.envs(envs);
        }

        let mut child = cmd.spawn()
            .with_context(|| format!("Impossible de démarrer le processus : {}", command))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Échec capture Stdin"))?;
        let stdout = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| anyhow::anyhow!("Échec capture Stdout"))?,
        );
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("Échec capture Stderr"))?;

        let (tx, rx) = tokio::sync::oneshot::channel();
        
        let mut tx_opt = Some(tx);
        tokio::spawn(async move {
            let stderr_reader = BufReader::new(stderr);
            let mut lines = stderr_reader.lines();
            let mut found_ready = false;
            while let Ok(Some(line)) = lines.next_line().await {
                if line.contains("Claude Code") || line.contains("Ready to receive") {
                    if !found_ready {
                        tracing::info!("✅ [MCP Serveur] R2D2 Sovereign Initialization Complete !");
                        if let Some(t) = tx_opt.take() {
                            let _ = t.send(());
                        }
                        found_ready = true;
                    }
                    continue; // Skip printing third-party brand logs
                }
                
                // Repipe the clean standard outputs to R2D2's tracing system
                tracing::info!("[MCP LOG] {}", line);
            }
        });

        Ok(Self {
            child,
            stdin,
            stdout,
            request_id: 1,
            ready_rx: Some(rx),
        })
    }

    /// Envoie une requête JSON-RPC v2 et attend la réponse synchrone.
    pub async fn send_request(&mut self, method: &str, params: Value) -> Result<Value> {
        let req_id = self.request_id;
        self.request_id += 1;

        let req = json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
            "params": params
        });

        let mut req_str = serde_json::to_string(&req)?;
        req_str.push_str("\r\n");

        self.stdin.write_all(req_str.as_bytes()).await?;
        self.stdin.flush().await?;

        // Attente de la réponse (Boucle pour ignorer les logs non-JSON dans STDOUT)
        loop {
            let mut line = String::new();
            if self.stdout.read_line(&mut line).await? == 0 {
                anyhow::bail!("Connexion MCP perdue (EOF)");
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with('{') {
                match serde_json::from_str::<Value>(trimmed) {
                    Ok(res) => {
                        // Est-ce une notification serveur ou la réponse à notre requête ?
                        if res.get("id").and_then(|v| v.as_u64()) == Some(req_id) {
                            if let Some(error) = res.get("error") {
                                error!("Erreur retournée par le Serveur MCP : {}", error);
                                anyhow::bail!("MCP Error: {}", error);
                            }
                            return Ok(res.get("result").cloned().unwrap_or(json!({})));
                        } else if res.get("id").is_none() {
                            // Ignorer les notifications asynchrones (progress, etc.) pour le moment
                            tracing::debug!("[MCP NOTIF] {}", trimmed);
                        }
                    }
                    Err(_) => {
                        // Faux positif, ce n'est pas un JSON valide, on log
                        tracing::debug!("[MCP STDOUT non-JSON] {}", trimmed);
                    }
                }
            } else {
                tracing::debug!("[MCP STDOUT brut] {}", trimmed);
            }
        }
    }

    /// Négocie la version du protocole et informe des capacités
    pub async fn initialize(&mut self) -> Result<Value> {
        if let Some(rx) = self.ready_rx.take() {
            info!("Attente du signal de préparation du serveur MCP (Node.js)...");
            // Un timeout optionnel pourrait être ajouté ici si on craint que le serveur ne se mette pas en route
            let _ = tokio::time::timeout(std::time::Duration::from_secs(10), rx).await;
        }

        info!("Envoi de l'Init MCP [2024-11-05]...");
        let result = self.send_request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    }
                },
                "clientInfo": {
                    "name": "r2d2-universal-client",
                    "version": "1.0.0"
                }
            }),
        )
        .await?;

        // L'implémentation standard MCP impose un client de notifier 'initialized' juste après le handshake
        let notif = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let mut notif_str = serde_json::to_string(&notif)?;
        notif_str.push_str("\r\n");
        self.stdin.write_all(notif_str.as_bytes()).await?;
        self.stdin.flush().await?;

        Ok(result)
    }

    /// Découvre les outils offerts par ce serveur externe
    pub async fn list_tools(&mut self) -> Result<Value> {
        self.send_request("tools/list", json!({})).await
    }

    /// Exécute un outil. ATTENTION : Toujours appeler `SemanticProxy::audit_tool_call`
    /// avant d'invoquer cette méthode.
    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value> {
        info!("Invocation MCP distante : tool='{}'", name);
        self.send_request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments
            }),
        )
        .await
    }

    /// Termine le sous-processus proprement
    pub async fn shutdown(mut self) -> Result<()> {
        info!("Extinction du Serveur MCP externe...");
        self.child.kill().await?;
        Ok(())
    }
}
