use pgvector::Vector;
use r2d2_jsonai::{ConsensusLevel, JsonAiV3};
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use crate::{BlackboardError, McpToolDbRow, ModelDbRow};

/// Structure encapsulant la requête d'ancrage (Typestate Pattern)
#[derive(Debug, Clone)]
pub struct AnchorRequest {
    pub id: String,
    pub source: String,
    pub timestamp: String,
    pub is_fact: bool,
    pub belief_state: String,
    pub consensus_level: String,
    pub payload_value: serde_json::Value,
    pub embedding: Vector,
    pub proof_of_inference: String,
}

/// Messages transitant vers l'Acteur isolé Blackboard
#[derive(Debug)]
pub enum BlackboardCommand {
    AnchorFragment {
        req: AnchorRequest,
        reply_to: oneshot::Sender<Result<String, BlackboardError>>,
    },
    RecallMemory {
        query_embedding: Vector,
        limit: i64,
        reply_to: oneshot::Sender<Result<Vec<String>, BlackboardError>>,
    },
    FetchUnconsolidated {
        limit: i64,
        reply_to: oneshot::Sender<Result<Vec<JsonAiV3>, BlackboardError>>,
    },
    UpdateConsensus {
        id: String,
        new_level: ConsensusLevel,
        reply_to: oneshot::Sender<Result<(), BlackboardError>>,
    },
    SaveReflex {
        embedding: Vector,
        action_payload: String,
        reply_to: oneshot::Sender<Result<(), BlackboardError>>,
    },
    FindMatchingReflex {
        query_embedding: Vector,
        threshold: f32,
        reply_to: oneshot::Sender<Result<Option<(String, f32)>, BlackboardError>>,
    },
    CompressVectorIndex {
        reply_to: oneshot::Sender<Result<usize, BlackboardError>>,
    },
    GetAllModels {
        reply_to: oneshot::Sender<Result<Vec<ModelDbRow>, BlackboardError>>,
    },
    GetAllMcpTools {
        reply_to: oneshot::Sender<Result<Vec<McpToolDbRow>, BlackboardError>>,
    },
    EnableModel {
        id: String,
        enable: bool,
        reply_to: oneshot::Sender<Result<(), BlackboardError>>,
    },
    EnableMcpTool {
        id: String,
        enable: bool,
        reply_to: oneshot::Sender<Result<(), BlackboardError>>,
    },
    AddMcpTool {
        name: String,
        command: String,
        args_json: String,
        reply_to: oneshot::Sender<Result<String, BlackboardError>>,
    },
    DeleteMcpTool {
        id: String,
        reply_to: oneshot::Sender<Result<(), BlackboardError>>,
    },
}

/// Moteur asynchrone strictement séquentiel pour garantir l'ordre causal I/O
pub struct BlackboardActor {
    pool: Pool<Postgres>,
    receiver: mpsc::Receiver<BlackboardCommand>,
}

impl BlackboardActor {
    pub fn new(pool: Pool<Postgres>, receiver: mpsc::Receiver<BlackboardCommand>) -> Self {
        Self { pool, receiver }
    }

    #[instrument(name = "BlackboardActor::run", skip(self))]
    pub async fn run(mut self) {
        info!("🧠 BlackboardActor Démarré. Boucle séquentielle active.");

        while let Some(msg) = self.receiver.recv().await {
            match msg {
                BlackboardCommand::AnchorFragment { req, reply_to } => {
                    let res = self.handle_anchor(req).await;
                    let _ = reply_to.send(res);
                }
                BlackboardCommand::RecallMemory {
                    query_embedding,
                    limit,
                    reply_to,
                } => {
                    let res = self.handle_recall(query_embedding, limit).await;
                    let _ = reply_to.send(res);
                }
                BlackboardCommand::FetchUnconsolidated { limit, reply_to } => {
                    let res = self.handle_fetch_unconsolidated(limit).await;
                    let _ = reply_to.send(res);
                }
                BlackboardCommand::UpdateConsensus {
                    id,
                    new_level,
                    reply_to,
                } => {
                    let res = self.handle_update_consensus(id, new_level).await;
                    let _ = reply_to.send(res);
                }
                BlackboardCommand::SaveReflex {
                    embedding,
                    action_payload,
                    reply_to,
                } => {
                    let res = self.handle_save_reflex(embedding, action_payload).await;
                    let _ = reply_to.send(res);
                }
                BlackboardCommand::FindMatchingReflex {
                    query_embedding,
                    threshold,
                    reply_to,
                } => {
                    let res = self
                        .handle_find_matching_reflex(query_embedding, threshold)
                        .await;
                    let _ = reply_to.send(res);
                }
                BlackboardCommand::CompressVectorIndex { reply_to } => {
                    let res = self.handle_compress_vector_index().await;
                    let _ = reply_to.send(res);
                }
                BlackboardCommand::GetAllModels { reply_to } => {
                    let res = self.handle_get_all_models().await;
                    let _ = reply_to.send(res);
                }
                BlackboardCommand::GetAllMcpTools { reply_to } => {
                    let res = self.handle_get_all_mcp_tools().await;
                    let _ = reply_to.send(res);
                }
                BlackboardCommand::EnableModel {
                    id,
                    enable,
                    reply_to,
                } => {
                    let res = self.handle_enable_model(id, enable).await;
                    let _ = reply_to.send(res);
                }
                BlackboardCommand::EnableMcpTool {
                    id,
                    enable,
                    reply_to,
                } => {
                    let res = self.handle_enable_mcp_tool(id, enable).await;
                    let _ = reply_to.send(res);
                }
                BlackboardCommand::AddMcpTool {
                    name,
                    command,
                    args_json,
                    reply_to,
                } => {
                    let res = self.handle_add_mcp_tool(name, command, args_json).await;
                    let _ = reply_to.send(res);
                }
                BlackboardCommand::DeleteMcpTool { id, reply_to } => {
                    let res = self.handle_delete_mcp_tool(id).await;
                    let _ = reply_to.send(res);
                }
            }
        }
        info!("🛑 BlackboardActor éteint proprement (Graceful Shutdown).");
        self.pool.close().await;
    }

    #[instrument(skip_all)]
    async fn handle_anchor(&self, req: AnchorRequest) -> Result<String, BlackboardError> {
        let mut retries = 0;
        let mut delay = Duration::from_millis(500);

        loop {
            let exec_future = sqlx::query(
                r#"
                INSERT INTO blackboard_fragments 
                (id, source, timestamp, is_fact, belief_state, consensus_level, payload, embedding, proof_of_inference)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT (id) DO UPDATE SET
                    consensus_level = EXCLUDED.consensus_level,
                    payload = EXCLUDED.payload,
                    proof_of_inference = EXCLUDED.proof_of_inference
                "#
            )
            .bind(&req.id)
            .bind(&req.source)
            .bind(&req.timestamp)
            .bind(req.is_fact)
            .bind(&req.belief_state)
            .bind(&req.consensus_level)
            .bind(&req.payload_value)
            .bind(req.embedding.clone())
            .bind(&req.proof_of_inference)
            .execute(&self.pool);

            // Bouclier Temporel (RustArch) : Empêche une transaction figée de bloquer toute la file séquentielle
            let timeout_result =
                tokio::time::timeout(Duration::from_millis(5000), exec_future).await;

            match timeout_result {
                Ok(Ok(_)) => return Ok(req.id),
                Ok(Err(e)) => {
                    let is_ephemeral = matches!(
                        e,
                        sqlx::Error::Io(_) | sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed
                    );
                    if !is_ephemeral || retries >= 5 {
                        error!("SQLx Error fatale dans l'Acteur: {:?}", e);
                        return Err(e.into());
                    }
                    warn!(
                        "R2D2-BlackActor éphémère ({:?}). Retries: {}/5",
                        e,
                        retries + 1
                    );
                }
                Err(_) => {
                    // Timeout
                    if retries >= 5 {
                        error!("TimeOut critique répété sur la Base de données Postgres.");
                        return Err(BlackboardError::ConnectionError(
                            "Postgres Timeout Asynchrone".to_string(),
                        ));
                    }
                    warn!("Timeout d'insertion atteint. Retries: {}/5", retries + 1);
                }
            }

            tokio::time::sleep(delay).await;
            retries += 1;
            let jitter = Duration::from_millis(rand::random::<u64>() % 100);
            delay = (delay * 2) + jitter;
        }
    }

    #[instrument(skip_all)]
    async fn handle_recall(
        &self,
        query_embedding: Vector,
        limit: i64,
    ) -> Result<Vec<String>, BlackboardError> {
        let rows = tokio::time::timeout(
            Duration::from_millis(15000), // Timeout généreux pour les scans HNSW
            sqlx::query(
                r#"
                SELECT payload::text as payload_text
                FROM blackboard_fragments
                ORDER BY embedding <-> $1
                LIMIT $2
                "#,
            )
            .bind(query_embedding)
            .bind(limit)
            .fetch_all(&self.pool),
        )
        .await
        .map_err(|_| {
            BlackboardError::ConnectionError("Timeout lecture rappel mémoire".to_string())
        })??;

        let mut results = Vec::new();
        for row in rows {
            use sqlx::Row;
            if let Ok(txt) = row.try_get::<String, _>("payload_text") {
                results.push(txt);
            }
        }
        Ok(results)
    }

    #[instrument(skip(self))]
    async fn handle_fetch_unconsolidated(
        &self,
        limit: i64,
    ) -> Result<Vec<JsonAiV3>, BlackboardError> {
        let rows = sqlx::query(
            r#"
            SELECT id, payload::text as payload_text
            FROM blackboard_fragments
            WHERE consensus_level != 'ConsensusReached'
            ORDER BY timestamp ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            use sqlx::Row;
            let db_id: String = match row.try_get("id") {
                Ok(id) => id,
                Err(_) => continue, // Rejet strict Typestate
            };
            if let Ok(txt) = row.try_get::<String, _>("payload_text") {
                // Remplacement du Zero-Trust Silencieux par un Match strict
                match serde_json::from_str::<JsonAiV3>(&txt) {
                    Ok(jsonai) => results.push(jsonai),
                    Err(e) => {
                        let trace_id = Uuid::new_v4();
                        error!(
                            trace_id = %trace_id,
                            fragment_id = %db_id,
                            error = %e,
                            "CORRUPTION SÉMANTIQUE (Payload invalide dans DB). Ligne exclue."
                        );
                        // On continue pour ne pas crasher tout le lot, mais l'erreur est désormais tracée !
                    }
                }
            }
        }
        Ok(results)
    }

    #[instrument(skip(self))]
    async fn handle_update_consensus(
        &self,
        id: String,
        new_level: ConsensusLevel,
    ) -> Result<(), BlackboardError> {
        sqlx::query(
            r#"
            UPDATE blackboard_fragments
            SET consensus_level = $1
            WHERE id = $2
            "#,
        )
        .bind(format!("{:?}", new_level))
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[instrument(skip(self, embedding))]
    async fn handle_save_reflex(
        &self,
        embedding: Vector,
        action_payload: String,
    ) -> Result<(), BlackboardError> {
        sqlx::query(
            r#"
            INSERT INTO reflex_memory (embedding, action_payload)
            VALUES ($1, $2)
            "#,
        )
        .bind(embedding)
        .bind(action_payload)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[instrument(skip(self, query_embedding))]
    async fn handle_find_matching_reflex(
        &self,
        query_embedding: Vector,
        threshold: f32,
    ) -> Result<Option<(String, f32)>, BlackboardError> {
        let max_distance = 1.0 - threshold;
        let row = sqlx::query(
            r#"
            SELECT action_payload, (embedding <=> $1) as distance
            FROM reflex_memory
            WHERE (embedding <=> $1) <= $2
            ORDER BY distance ASC
            LIMIT 1
            "#,
        )
        .bind(query_embedding)
        .bind(max_distance)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(r) = row {
            use sqlx::Row;
            let payload: String = r.try_get("action_payload")?;
            let distance: f64 = r.try_get("distance")?;
            let similarity = 1.0 - distance as f32;
            Ok(Some((payload, similarity)))
        } else {
            Ok(None)
        }
    }

    #[instrument(skip(self))]
    async fn handle_compress_vector_index(&self) -> Result<usize, BlackboardError> {
        // On effectue le REINDEX asynchrone hors de l'Acteur strict si besoin.
        // Faisons d'abord la suppression des doublons classiques :
        let rows_deleted = sqlx::query(
            r#"
            DELETE FROM blackboard_fragments a USING blackboard_fragments b 
            WHERE a.id < b.id AND (a.embedding <=> b.embedding) < 0.05
            "#,
        )
        .execute(&self.pool)
        .await?;

        let duplicates_found = rows_deleted.rows_affected() as usize;

        if let Ok(mut conn) = self.pool.acquire().await {
            let _ = sqlx::query("SET maintenance_work_mem = '1GB';")
                .execute(&mut *conn)
                .await;

            // On bypass la limitation du spawn_blocking en restant dans l'async,
            // assumant que ce hook est nocturne.
            let index_name: Option<(String,)> = sqlx::query_as(
                "SELECT indexname FROM pg_indexes WHERE tablename = 'blackboard_fragments' AND indexdef ILIKE '%embedding%'"
            )
            .fetch_optional(&mut *conn)
            .await
            .unwrap_or(None);

            if let Some((name,)) = index_name {
                let reindex_cmd = format!("REINDEX INDEX CONCURRENTLY {};", name);
                info!("PostgreSQL Engine: [{}]...", reindex_cmd);
                let _ = sqlx::query(&reindex_cmd).execute(&mut *conn).await;
            }

            let _ = sqlx::query("RESET maintenance_work_mem;")
                .execute(&mut *conn)
                .await;
        }

        Ok(duplicates_found)
    }

    #[instrument(skip(self))]
    async fn handle_get_all_models(&self) -> Result<Vec<ModelDbRow>, BlackboardError> {
        let rows = sqlx::query(
            r#"SELECT id, name, model_type, provider, config_json::text as config_json, is_enabled FROM model_registry"#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::new();
        for row in rows {
            use sqlx::Row;
            out.push(ModelDbRow {
                id: row.try_get("id").unwrap_or_default(),
                name: row.try_get("name").unwrap_or_default(),
                model_type: row.try_get("model_type").unwrap_or_default(),
                provider: row.try_get("provider").unwrap_or_default(),
                config_json: row
                    .try_get("config_json")
                    .unwrap_or_else(|_| "{}".to_string()),
                is_enabled: row.try_get("is_enabled").unwrap_or(true),
            });
        }
        Ok(out)
    }

    #[instrument(skip(self))]
    async fn handle_get_all_mcp_tools(&self) -> Result<Vec<McpToolDbRow>, BlackboardError> {
        let rows = sqlx::query(
            r#"SELECT id, name, command, args_json::text as args_json, is_enabled FROM mcp_registry"#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::new();
        for row in rows {
            use sqlx::Row;
            out.push(McpToolDbRow {
                id: row.try_get("id").unwrap_or_default(),
                name: row.try_get("name").unwrap_or_default(),
                command: row.try_get("command").unwrap_or_default(),
                args_json: row
                    .try_get("args_json")
                    .unwrap_or_else(|_| "[]".to_string()),
                is_enabled: row.try_get("is_enabled").unwrap_or(true),
            });
        }
        Ok(out)
    }

    async fn handle_enable_model(&self, id: String, enable: bool) -> Result<(), BlackboardError> {
        sqlx::query("UPDATE model_registry SET is_enabled = $1 WHERE id = $2")
            .bind(enable)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn handle_enable_mcp_tool(
        &self,
        id: String,
        enable: bool,
    ) -> Result<(), BlackboardError> {
        sqlx::query("UPDATE mcp_registry SET is_enabled = $1 WHERE id = $2")
            .bind(enable)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn handle_add_mcp_tool(
        &self,
        name: String,
        command: String,
        args_json: String,
    ) -> Result<String, BlackboardError> {
        let id = format!("mcp-{}", uuid::Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO mcp_registry (id, name, command, args_json, is_enabled) VALUES ($1, $2, $3, $4, true)"
        )
        .bind(&id)
        .bind(name)
        .bind(command)
        .bind(args_json)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    async fn handle_delete_mcp_tool(&self, id: String) -> Result<(), BlackboardError> {
        sqlx::query("DELETE FROM mcp_registry WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
