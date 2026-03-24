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
}

impl McpUniversalClient {
    /// Lance un binaire externe (ex: `python -m mcp_server`, `npx ...`) et
    /// attache les flux standard pour la communication JSON-RPC.
    #[instrument(skip_all, name = "McpUniversalClient::spawn")]
    pub async fn spawn(command: &str, args: &[&str]) -> Result<Self> {
        info!("Lancement du serveur MCP externe : {} {:?}", command, args);

        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit()) // Les logs d'erreur ressortent naturellement
            .spawn()
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

        Ok(Self {
            child,
            stdin,
            stdout,
            request_id: 1,
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
        req_str.push('\n');

        self.stdin.write_all(req_str.as_bytes()).await?;
        self.stdin.flush().await?;

        // Attente de la réponse
        let mut line = String::new();
        self.stdout.read_line(&mut line).await?;
        
        if line.is_empty() {
             anyhow::bail!("Connexion MCP perdue (EOF)");
        }

        let res: Value = serde_json::from_str(&line)
            .with_context(|| format!("Parsing JSON-RPC échoué pour la réponse: {}", line))?;

        if let Some(error) = res.get("error") {
            error!("Erreur retournée par le Serveur MCP : {}", error);
            anyhow::bail!("MCP Error: {}", error);
        }

        Ok(res.get("result").cloned().unwrap_or(json!({})))
    }

    /// Négocie la version du protocole et informe des capacités
    pub async fn initialize(&mut self) -> Result<Value> {
        info!("Envoi de l'Init MCP [2024-11-05]...");
        self.send_request(
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
        .await
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
