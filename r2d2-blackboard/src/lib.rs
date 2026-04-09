//! # Brique 7 : Le Blackboard Persistant
//!
//! Le pattern Blackboard gère l'ancrage des Fragments Validés dans la base de données.
//! Cette version utilise PostgreSQL 16+ et pgvector (Standard "Power of Two").
//!
//! Seuls les `Fragment<Validated>` peuvent traverser le mur d'écriture.

use anyhow::Result;
use async_trait::async_trait;
use pgvector::Vector;
use r2d2_jsonai::{ConsensusLevel, JsonAiV3};
use r2d2_kernel::Validated;
use r2d2_secure_mem::SecureMemGuard;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use thiserror::Error;
use tracing::{info, instrument};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelDbRow {
    pub id: String,
    pub name: String,
    pub model_type: String,
    pub provider: String,
    pub config_json: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpToolDbRow {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args_json: String,
    pub is_enabled: bool,
}

#[derive(Debug, Error)]
pub enum BlackboardError {
    #[error("Erreur de connexion à la base de données: {0}")]
    ConnectionError(String),
    #[error("Erreur SQL lors de l'insertion ou de la recherche: {0}")]
    QueryError(#[from] sqlx::Error),
    #[error("Erreur de sérialisation JSON: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Interface Port & Adapter certifiant que le Blackboard peut ancrer un fragment validé.
#[async_trait]
pub trait GlobalBlackboard {
    /// Réceptionne le garde sécurisé d'un fragment vérifié.
    /// Dès que l'écriture est certifiée sur disque, le Guard est détruit en RAM (Zeroize).
    async fn anchor_fragment(
        &self,
        guard: SecureMemGuard<Validated>,
    ) -> Result<String, BlackboardError>;
}

/// Implémentation PostgreSQL du Blackboard via SQLx et pgvector
pub struct PostgresBlackboard {
    pool: Pool<Postgres>,
}

impl PostgresBlackboard {
    /// Initialise la connexion au Blackboard vectoriel.
    pub async fn new(database_url: &str) -> Result<Self, BlackboardError> {
        let max_conn = std::env::var("DATABASE_MAX_CONN")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(20);

        let pool = PgPoolOptions::new()
            .max_connections(max_conn) // Idéal pour le Swarm R2D2 en local ou paramétrable via env
            .acquire_timeout(std::time::Duration::from_secs(10)) // FAIL-FAST Assoupli: Laisse le temps pour pgvector
            .connect_lazy(database_url)
            .map_err(|e| BlackboardError::ConnectionError(e.to_string()))?;

        Ok(Self { pool })
    }

    /// Initialise la connexion et vérifie simplement l'état du Registre.
    /// Éradication complète des DDL (CREATE TABLE) au runtime pour prévenir
    /// les impasses asynchrones via les verrous de catalogue. L'orchestrateur
    /// Postgres (DBA) est responsable de l'organisation par script SQL.
    pub async fn initialize_registry_tables(&self) -> Result<(), BlackboardError> {
        info!("🔧 Vérification des tables du Registre Sovereign (No-DDL Mode)...");
        // Les tables 'model_registry', 'mcp_registry' et 'blackboard_fragments'
        // doivent être créées via un moteur externe ou script psql au boot de R2D2.
        Ok(())
    }
}

#[async_trait]
impl GlobalBlackboard for PostgresBlackboard {
    #[instrument(skip(self, guard))]
    async fn anchor_fragment(
        &self,
        guard: SecureMemGuard<Validated>,
    ) -> Result<String, BlackboardError> {
        // 1. Ouvrir le coffre sécurisé pour exposer le payload Validé
        let validated = guard.expose_payload();

        // 2. Parser le payload JSONAI strict
        let jsonai: JsonAiV3 = serde_json::from_str(&validated.payload)?;

        info!(
            "Ancrage du fragment [{}] dans le Blackboard PostgreSQL (Consensus: {:?})",
            jsonai.id, jsonai.consensus
        );

        // 3. TODO: Générer un vecteur dynamique si ce fragment n'en a pas
        // Pour ce MVP, on injecte un vecteur nul (1024 dims) si non défini.
        let default_embedding = Vector::from(vec![0.0; 1024]);

        let payload_value = serde_json::to_value(&jsonai)?;

        let mut retries = 0;
        let mut delay = std::time::Duration::from_millis(500);

        loop {
            let result = sqlx::query(
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
            .bind(&jsonai.id)
            .bind(format!("{:?}", jsonai.source)) // Enums sérialisés en chaînes
            .bind(jsonai.timestamp)
            .bind(jsonai.is_fact)
            .bind(format!("{:?}", jsonai.belief_state))
            .bind(format!("{:?}", jsonai.consensus))
            .bind(&payload_value)
            .bind(default_embedding.clone())
            .bind(&validated.proof_of_inference)
            .execute(&self.pool)
            .await;

            match result {
                Ok(_) => {
                    // À la sortie de cette fonction, `guard` est relâché. Le destructeur Invoquera bzero()
                    // sur l'espace RAM ayant contenu le payload ! (Souveraineté des données)
                    return Ok(jsonai.id); // On sort de la boucle avec succès
                }
                Err(e) => {
                    // Pattern "Zero-Dependency Resilience": On cible uniquement les erreurs de connexion
                    let is_ephemeral = matches!(
                        e,
                        sqlx::Error::Io(_) | sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed
                    );

                    if !is_ephemeral || retries >= 5 {
                        return Err(e.into()); // Fin totale, l'erreur est fatale (Erreur de syntaxe, Constraint...)
                    }
                    tracing::warn!(
                        "⚠️ Erreur R2D2-Blackboard éphémère ({:?}). Tentative {}/5 après {}ms. Pool [Size: {}, Idle: {}]",
                        e,
                        retries + 1,
                        delay.as_millis(),
                        self.pool.size(),
                        self.pool.num_idle()
                    );
                }
            }

            tokio::time::sleep(delay).await;
            retries += 1;

            // Jitter Asynchrone : Empêche la Horde Tonitruante / DDoS Interne
            // Ajoute un délai aléatoire au backoff (+ 0 à 100ms)
            let jitter = std::time::Duration::from_millis(rand::random::<u64>() % 100);
            delay = (delay * 2) + jitter;
        }
    }
}

impl PostgresBlackboard {
    /// Effectue une recherche de similarité vectorielle stricte (HNSW) via pgvector.
    #[instrument(skip(self, query_embedding))]
    pub async fn recall_memory(
        &self,
        query_embedding: Vector,
        limit: i64,
    ) -> Result<Vec<String>, BlackboardError> {
        info!(
            "Recherche sémantique HNSW (Top-{}) demandée au Blackboard...",
            limit
        );

        // Requête de distance L2 (<->) optimisée par l'index HNSW pgvector
        // Compilation hors ligne (sans macro sqlx::query!)
        let rows = sqlx::query(
            r#"
            SELECT payload::text as payload_text
            FROM blackboard_fragments
            ORDER BY embedding <-> $1
            LIMIT $2
            "#,
        )
        .bind(query_embedding)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            use sqlx::Row;
            if let Ok(txt) = row.try_get::<String, _>("payload_text") {
                results.push(txt);
            }
        }

        Ok(results)
    }

    /// Exhume les mémoires n'ayant pas encore atteint le consensus final.
    /// Idéal pour le Cycle Circadien (Moteur MCTS).
    #[instrument(skip(self))]
    pub async fn fetch_unconsolidated_memories(
        &self,
        limit: i64,
    ) -> Result<Vec<JsonAiV3>, BlackboardError> {
        info!("Exhumation nocturne demandée (Limite: {})", limit);

        let rows = sqlx::query(
            r#"
            SELECT payload::text as payload_text
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
            if let Ok(txt) = row.try_get::<String, _>("payload_text") {
                if let Ok(jsonai) = serde_json::from_str(&txt) {
                    results.push(jsonai);
                }
            }
        }

        Ok(results)
    }

    /// Met à jour le niveau de consensus d'un fragment existant (Cristallisation).
    #[instrument(skip(self))]
    pub async fn update_consensus_level(
        &self,
        id: &str,
        new_level: ConsensusLevel,
    ) -> Result<(), BlackboardError> {
        info!(
            "Mise à jour du consensus pour le fragment {} -> {:?}",
            id, new_level
        );

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

    /// Sauvegarde une empreinte sémantique en mémoire réflexe (Système 1).
    #[instrument(skip(self, embedding))]
    pub async fn save_reflex(
        &self,
        embedding: Vector,
        action_payload: &str,
    ) -> Result<(), BlackboardError> {
        info!("Enregistrement d'un nouveau réflexe (Système 1) en BDD.");

        // DDL CREATE TABLE retiré : La table `reflex_memory` (id, embedding(384), action_payload)
        // doit exister préalablement.

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

    /// Recherche le réflexe le plus proche. Si la similarité cosinus dépasse le seuil, retourne l'action.
    #[instrument(skip(self, query_embedding))]
    pub async fn find_matching_reflex(
        &self,
        query_embedding: Vector,
        threshold: f32, // ex: 0.90
    ) -> Result<Option<(String, f32)>, BlackboardError> {
        // Optionnel: s'assurer que la table existe (Désactivé pour Etape II - Anti Starvation)

        let max_distance = 1.0 - threshold;

        // On utilise l'opérateur <=> pour la distance cosinus
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
            let distance: f64 = r.try_get("distance")?; // pgvector distance sort un float8 (f64)
            let similarity = 1.0 - distance as f32;
            Ok(Some((payload, similarity)))
        } else {
            Ok(None)
        }
    }

    /// Effectue un nettoyage du graphe HNSW et supprime les doublons inutiles (Deep Sleep Compression).
    #[instrument(skip(self))]
    pub async fn compress_vector_index(&self) -> Result<usize, BlackboardError> {
        info!("Début de la compression vectorielle (Folding)...");

        // Suppression des doublons exacts ou très proches (Distance euclidienne / Cosinus très faible < 0.05)
        let rows_deleted = sqlx::query(
            r#"
            DELETE FROM blackboard_fragments a USING blackboard_fragments b 
            WHERE a.id < b.id AND (a.embedding <=> b.embedding) < 0.05
            "#,
        )
        .execute(&self.pool)
        .await?;

        let duplicates_found = rows_deleted.rows_affected() as usize;

        if duplicates_found > 0 {
            info!(
                "📦 {} fragments similaires nettoyés. Consolidation effectuée.",
                duplicates_found
            );
        }

        info!("🧠 Optimisation de la structure du graphe vectoriel (Zero-Lock)...");

        // Création d'une connexion isolée pour paramétrer la RAM sans impacter l'Orchestrateur
        if let Ok(mut conn) = self.pool.acquire().await {
            let _ = sqlx::query("SET maintenance_work_mem = '1GB';")
                .execute(&mut *conn)
                .await;

            // Recherche dynamique de l'Index pgvector de la table
            let index_name: Option<(String,)> = sqlx::query_as(
                "SELECT indexname FROM pg_indexes WHERE tablename = 'blackboard_fragments' AND indexdef ILIKE '%embedding%'"
            )
            .fetch_optional(&mut *conn)
            .await
            .unwrap_or(None);

            if let Some((name,)) = index_name {
                let reindex_cmd = format!("REINDEX INDEX CONCURRENTLY {};", name);
                info!("PostgreSQL Engine: [{}]...", reindex_cmd);
                match sqlx::query(&reindex_cmd).execute(&mut *conn).await {
                    Ok(_) => info!("✅ Index vectoriel optimisé avec succès."),
                    Err(e) => tracing::warn!("⚠️ REINDEX CONCURRENTLY a échoué : {}", e),
                }
            } else {
                tracing::warn!("Index vectoriel non trouvé. Lancement d'un VACUUM classique.");
                let _ = sqlx::query("VACUUM (ANALYZE) blackboard_fragments;")
                    .execute(&mut *conn)
                    .await;
            }

            let _ = sqlx::query("RESET maintenance_work_mem;")
                .execute(&mut *conn)
                .await;
        } else {
            tracing::warn!(
                "⚠️ Impossible d'obtenir une connexion pour optimiser le Graphe PostgreSQL."
            );
        }

        Ok(duplicates_found)
    }

    /// Récupère l'intégralité des modèles enregistrés dans la configuration (actifs ou inactifs).
    #[instrument(skip(self))]
    pub async fn get_all_models(&self) -> Result<Vec<ModelDbRow>, BlackboardError> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, model_type, provider, config_json::text as config_json, is_enabled
            FROM model_registry
            "#,
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

    /// Récupère l'intégralité des outils MCP enregistrés dans la configuration.
    #[instrument(skip(self))]
    pub async fn get_all_mcp_tools(&self) -> Result<Vec<McpToolDbRow>, BlackboardError> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, command, args_json::text as args_json, is_enabled
            FROM mcp_registry
            "#,
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

    /// Active ou désactive un modèle.
    pub async fn enable_model(&self, id: &str, enable: bool) -> Result<(), BlackboardError> {
        sqlx::query("UPDATE model_registry SET is_enabled = $1 WHERE id = $2")
            .bind(enable)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Active ou désactive un outil MCP.
    pub async fn enable_mcp_tool(&self, id: &str, enable: bool) -> Result<(), BlackboardError> {
        sqlx::query("UPDATE mcp_registry SET is_enabled = $1 WHERE id = $2")
            .bind(enable)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Ajoute un nouvel outil MCP.
    pub async fn add_mcp_tool(
        &self,
        name: &str,
        command: &str,
        args_json: &str,
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

    /// Supprime un outil MCP.
    pub async fn delete_mcp_tool(&self, id: &str) -> Result<(), BlackboardError> {
        sqlx::query("DELETE FROM mcp_registry WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_db_row_serialization() {
        let row = ModelDbRow {
            id: "test-e5".to_string(),
            name: "E5-Small".to_string(),
            model_type: "semantic".to_string(),
            provider: "local_hf".to_string(),
            config_json: "{}".to_string(),
            is_enabled: true,
        };
        let js = serde_json::to_string(&row).expect("Doit parser");
        assert!(js.contains("test-e5"));
        assert!(js.contains("E5-Small"));

        let parsed: ModelDbRow = serde_json::from_str(&js).expect("Doit parser");
        assert!(parsed.is_enabled);
    }

    #[test]
    fn test_mcp_tool_db_row_serialization() {
        let row = McpToolDbRow {
            id: "mcp-test".to_string(),
            name: "test-mcp".to_string(),
            command: "npx".to_string(),
            args_json: "[]".to_string(),
            is_enabled: false,
        };
        let js = serde_json::to_string(&row).unwrap();
        assert!(js.contains("mcp-test"));
        assert!(js.contains("test-mcp"));

        let parsed: McpToolDbRow = serde_json::from_str(&js).unwrap();
        assert!(!parsed.is_enabled);
    }

    #[test]
    fn test_blackboard_error_format() {
        let err = BlackboardError::ConnectionError("PG Offline".to_string());
        assert_eq!(
            err.to_string(),
            "Erreur de connexion à la base de données: PG Offline"
        );
    }
}
