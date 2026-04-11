use crate::core::McpTool;
use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{error, info};

// 1. Le NewType Sécurisé
#[derive(Clone)]
pub struct GithubToken(pub SecretString);

// 2. Le Port (Trait) pour l'Architecture Hexagonale
#[async_trait]
pub trait GithubGateway: Send + Sync {
    async fn search_code(&self, query: &str) -> Result<String, anyhow::Error>;
}

// 3. L'Adaptateur Physique (qui possède reqwest et le secret)
pub struct ReqwestGithubClient {
    client: reqwest::Client,
    token: GithubToken,
}

impl ReqwestGithubClient {
    pub fn new(token: GithubToken) -> anyhow::Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/vnd.github.v3+json"),
        );
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static("R2D2-Vampire/1.0"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            // L'utilisation de rustls-tls est garantie par le `Cargo.toml` (`default-features=false`)
            .build()?;

        Ok(Self { client, token })
    }
}

#[async_trait]
impl GithubGateway for ReqwestGithubClient {
    async fn search_code(&self, query: &str) -> Result<String, anyhow::Error> {
        let url = format!(
            "https://api.github.com/search/code?q={}",
            urlencoding::encode(query)
        );

        let req = self
            .client
            .get(&url)
            .bearer_auth(self.token.0.expose_secret());

        let res = req.send().await?;

        if res.status().is_success() {
            let body = res.text().await?;
            // Pour l'exfiltration, on peut simplifier en renvoyant le JSON brut ou l'extraire
            // Ici on retourne directement la string de reponse GitHub brutes.
            Ok(body)
        } else {
            let status = res.status();
            let err_txt = res.text().await.unwrap_or_default();
            error!("Erreur Github API [{}]: {}", status, err_txt);
            anyhow::bail!("Erreur API Github: {}", status)
        }
    }
}

// 4. L'Outil MCP (Totalement agnostique du réseau et de l'environnement)
pub struct GithubTool {
    gateway: Arc<dyn GithubGateway>,
}

impl GithubTool {
    pub fn new(gateway: Arc<dyn GithubGateway>) -> Self {
        Self { gateway }
    }
}

#[async_trait]
impl McpTool for GithubTool {
    fn name(&self) -> String {
        "search_code".to_string()
    }

    fn description(&self) -> String {
        "Recherche du code sur Github via l'API REST de maniere souveraine (Zero-Trust)."
            .to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "q": {
                    "type": "string",
                    "description": "La requete de recherche Github. ex: 'repo:MonOrg/MonRepo pattern'"
                }
            },
            "required": ["q"]
        })
    }

    async fn call(&self, arguments: Value) -> Result<Value, anyhow::Error> {
        let query = arguments.get("q").and_then(|v| v.as_str()).unwrap_or("");

        if query.is_empty() {
            anyhow::bail!("Parametre 'q' manquant ou vide.");
        }

        info!("🦇 Vampirisation Github en cours pour: '{}'", query);

        let result = self.gateway.search_code(query).await?;

        // On pourrait parser les Resultats Github pour extraire le base64 decodé,
        // mais l'API de base Search renvoie juste des localisations de fichiers.
        // C'est un point de depart solide qui respecte le contrat MCP.

        Ok(json!(result))
    }
}
