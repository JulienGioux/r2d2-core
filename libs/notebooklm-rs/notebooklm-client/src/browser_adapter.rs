use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use notebooklm_core::domain::RpcDomain;
use notebooklm_core::errors::{NotebookError, Result};
use notebooklm_core::ports::NotebookProvider;
use notebooklm_core::types::{ArtifactId, ArtifactStatus, ArtifactType, NotebookId, SourceId};

use crate::actor::{BrowserCommand, ChromiumActor};

#[derive(Clone)]
pub struct NotebookClient {
    sender: mpsc::Sender<BrowserCommand>,
}

impl NotebookClient {
    /// Initialise le client avec un canal borné pour protéger le runtime
    pub fn new(capacity: usize) -> Self {
        let sender = ChromiumActor::spawn(capacity);
        Self { sender }
    }

    /// Exécute un script via l'acteur sans bloquer le Worker thread courant
    async fn execute_script(&self, script: String) -> Result<Value> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(BrowserCommand::ExecuteScript { script, respond_to: tx })
            .await
            .map_err(|_| NotebookError::InfrastructureError("Acteur Chromium mort".to_string()))?;

        rx.await
            .map_err(|_| NotebookError::InfrastructureError("Acteur Chromium n'a pas répondu".to_string()))?
    }

    /// Exécute une RPC standard sur NotebookLM
    pub async fn execute_rpc(&self, rpc_id: &str, path: &str, params: Value) -> Result<Value> {
        let payload = RpcDomain::build_batchexecute_body(rpc_id, params)?;

        let script = format!(
            r#"
            (async () => {{
                try {{
                    const token = window.WIZ_global_data?.SNlM0e;
                    if (!token) throw new Error("Missing CSRF Token (SNlM0e)");

                    let formData = new URLSearchParams();
                    formData.append("f.req", {payload:?});
                    formData.append("at", token);

                    const url = `https://notebooklm.google.com/u/0/_/NotebookUi/data/batchexecute?rpcids={rpc_id}&source-path={path}&f.sid=${{window.WIZ_global_data?.FdrFJe || ""}}&rt=c`;
                    const response = await fetch(url, {{
                        method: "POST",
                        body: formData,
                        headers: {{ "Content-Type": "application/x-www-form-urlencoded;charset=UTF-8" }}
                    }});

                    if (!response.ok) throw new Error(`HTTP Error ${{response.status}}`);

                    const text = await response.text();
                    const lines = text.split('\n');
                    let found = null;

                    const crawl = (arr) => {{
                        if (!Array.isArray(arr)) return;
                        if (arr[0] === "wrb.fr") {{
                            found = arr[2];
                            return;
                        }}
                        for (let item of arr) crawl(item);
                    }};

                    for (const line of lines) {{
                        if (line.trim() === "" || line.startsWith(")]}}'")) continue;
                        try {{
                            const json = JSON.parse(line);
                            crawl(json);
                            if (found) break;
                        }} catch(e) {{}}
                    }}

                    if (found) {{
                        return JSON.parse(found);
                    }} else {{
                        return {{"error": "RPC Result Not Found in parsing"}};
                    }}
                }} catch (e) {{
                    return {{"error": e.toString()}};
                }}
            }})()
            "#,
            payload = payload,
            rpc_id = rpc_id,
            path = path
        );

        let res = self.execute_script(script).await?;
        
        // Custom error check
        if let Some(err) = res.get("error") {
            if err.as_str() == Some("RPC Result Not Found in parsing") {
                return Ok(serde_json::json!({ "status": "success_no_payload" }));
            }
            return Err(NotebookError::InfrastructureError(format!("JS Error: {}", err)));
        }

        Ok(res)
    }
}

// Implémentation du Port Domaine par l'Adaptateur Client
#[async_trait]
impl NotebookProvider for NotebookClient {
    async fn list_notebooks(&self) -> Result<Vec<(NotebookId, String)>> {
        let res = self.execute_rpc("gArtLc", "/notebooks", serde_json::json!([])).await?;
        
        let mut notebooks = Vec::new();
        if let Some(arr) = res.as_array() {
            if let Some(first) = arr.first() {
                if let Some(list) = first.as_array() {
                    for item in list {
                        if let Some(n_arr) = item.as_array() {
                            if n_arr.len() > 2 {
                                let title = n_arr[1].as_str().unwrap_or("Untitled").to_string();
                                let id = n_arr[2].as_str().unwrap_or("").to_string();
                                if !id.is_empty() {
                                    notebooks.push((NotebookId(id), title));
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(notebooks)
    }

    async fn get_notebook(&self, notebook_id: &NotebookId) -> Result<serde_json::Value> {
        let params = serde_json::json!([notebook_id.0, serde_json::Value::Null, [2], serde_json::Value::Null, 0]);
        self.execute_rpc("bQjNpf", &format!("/notebook/{}", notebook_id.0), params).await
    }

    async fn create_notebook(&self, title: &str) -> Result<NotebookId> {
        let params = serde_json::json!([title]);
        let root_res = self.execute_rpc("yixI3b", "/notebooks", params).await?;
        
        let id_str = root_res
            .as_array()
            .and_then(|arr| arr.get(2))
            .and_then(|id| id.as_str())
            .ok_or_else(|| NotebookError::PayloadParsingError("ID introuvable dans create_notebook".to_string()))?;
            
        Ok(NotebookId(id_str.to_string()))
    }

    async fn rename_notebook(&self, notebook_id: &NotebookId, new_title: &str) -> Result<()> {
        let params = serde_json::json!([notebook_id.0, new_title, serde_json::Value::Null, serde_json::Value::Null, serde_json::Value::Null]);
        self.execute_rpc("nTYDyd", &format!("/notebook/{}", notebook_id.0), params).await?;
        Ok(())
    }

    async fn delete_notebook(&self, notebook_id: &NotebookId) -> Result<()> {
        let params = serde_json::json!([notebook_id.0]);
        self.execute_rpc("pXwihb", "/notebook", params).await?;
        Ok(())
    }

    // --- Sources ---
    async fn list_sources(&self, notebook_id: &NotebookId) -> Result<Vec<SourceId>> {
        let params = serde_json::json!([[notebook_id.0]]);
        let res = self.execute_rpc("Eihx2", &format!("/notebook/{}", notebook_id.0), params).await?;
        
        let mut sources = Vec::new();
        // Parsing simplifié: TODO: adapter aux tableaux retournés 
        if let Some(arr) = res.as_array() {
            if let Some(first) = arr.first().and_then(|f| f.as_array()) {
                if let Some(source_list) = first.first().and_then(|l| l.as_array()) {
                    for src in source_list {
                        if let Some(id) = src.as_array().and_then(|s| s.get(2)).and_then(|i| i.as_str()) {
                            sources.push(SourceId(id.to_string()));
                        }
                    }
                }
            }
        }
        Ok(sources)
    }

    async fn add_source_text(&self, notebook_id: &NotebookId, title: &str, content: &str) -> Result<SourceId> {
        let params = serde_json::json!([
            [
                [
                    serde_json::Value::Null,
                    [title, content],
                    serde_json::Value::Null,
                    serde_json::Value::Null,
                    serde_json::Value::Null,
                    serde_json::Value::Null,
                    serde_json::Value::Null,
                    serde_json::Value::Null
                ]
            ],
            notebook_id.0,
            [2],
            serde_json::Value::Null,
            serde_json::Value::Null
        ]);
        
        let root_res = self.execute_rpc("x05U3", &format!("/notebook/{}", notebook_id.0), params).await?;
        
        let source_id = root_res
            .as_array()
            .and_then(|a| a.first())
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|a| a.as_str())
            .ok_or_else(|| NotebookError::PayloadParsingError("Impossible d'extraire source_id".to_string()))?;
            
        Ok(SourceId(source_id.to_string()))
    }

    async fn delete_source(&self, notebook_id: &NotebookId, source_id: &SourceId) -> Result<()> {
        let params = serde_json::json!([[notebook_id.0], [source_id.0]]);
        self.execute_rpc("v7MIfc", &format!("/notebook/{}", notebook_id.0), params).await?;
        Ok(())
    }

    // --- Artefacts ---
    async fn list_artifacts(&self, notebook_id: &NotebookId) -> Result<Vec<(ArtifactId, String, ArtifactStatus)>> {
        let params = serde_json::json!([[2], notebook_id.0, "NOT artifact.status = \"ARTIFACT_STATUS_SUGGESTED\""]);
        let _res = self.execute_rpc("s0O65d", &format!("/notebook/{}", notebook_id.0), params).await?;
        
        // Mock parsing pour la compilation
        // Il faudrat affiner le parsing d'IDs complexes ici
        Ok(vec![])
    }

    async fn create_artifact(
        &self,
        notebook_id: &NotebookId,
        artifact_type: ArtifactType,
        source_ids: Option<Vec<SourceId>>,
    ) -> Result<ArtifactId> {
        let sources_block = if let Some(sids) = source_ids {
            // Transforme [Source1, Source2] en [[[Source1]], [[Source2]]]
            sids.into_iter().map(|s| serde_json::json!([[s.0]])).collect::<Vec<_>>()
        } else {
            // Vide mais array
            vec![]
        };

        let params = serde_json::json!([
            [2],
            notebook_id.0,
            [
                serde_json::Value::Null,
                serde_json::Value::Null,
                artifact_type as u8,
                sources_block,
                serde_json::Value::Null,
                serde_json::Value::Null,
                serde_json::Value::Null,
                serde_json::Value::Null,
                serde_json::Value::Null,
                [
                    serde_json::Value::Null,
                    [
                        1, 
                        serde_json::Value::Null,
                        serde_json::Value::Null,
                        serde_json::Value::Null,
                        serde_json::Value::Null,
                        serde_json::Value::Null,
                        [serde_json::Value::Null, serde_json::Value::Null]
                    ]
                ]
            ]
        ]);
        
        let root_res = self.execute_rpc("R7cb6c", &format!("/notebook/{}", notebook_id.0), params).await?;
        
        let id_val = root_res
            .as_array()
            .and_then(|a| a.first())
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|a| a.as_str())
            .ok_or_else(|| NotebookError::PayloadParsingError("Id introuvable pour l'artefact".into()))?;
            
        Ok(ArtifactId(id_val.to_string()))
    }

    async fn delete_artifact(&self, notebook_id: &NotebookId, artifact_id: &ArtifactId) -> Result<()> {
        let params = serde_json::json!([[2], artifact_id.0]);
        self.execute_rpc("StQWmb", &format!("/notebook/{}", notebook_id.0), params).await?;
        Ok(())
    }

    async fn fetch_interactive_data(&self, notebook_id: &NotebookId, artifact_id: &ArtifactId) -> Result<serde_json::Value> {
        // Paramètre EXACT : "[artifact_id]" corrigé grâce à l'itération précédente
        let params = serde_json::json!([artifact_id.0]);
        let res = self.execute_rpc("v9rmvd", &format!("/notebook/{}", notebook_id.0), params).await?;
        
        crate::endpoints::artifacts::find_html(&res)
            .and_then(|html| {
                let decoded = html_escape::decode_html_entities(&html).to_string();
                serde_json::from_str(&decoded).ok()
            })
            .ok_or_else(|| NotebookError::PayloadParsingError("Aucun document interactif trouvé (data-app-data)".to_string()))
    }
}
