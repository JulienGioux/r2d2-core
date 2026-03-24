//! # Brique 7 : Le Blackboard Persistant
//!
//! Le pattern Blackboard gère l'ancrage des Fragments Validés dans la base de données.
//! Cette version utilise PostgreSQL 16+ et pgvector (Standard "Power of Two").
//!
//! Seuls les `Fragment<Validated>` peuvent traverser le mur d'écriture.

use anyhow::Result;
use async_trait::async_trait;
use pgvector::Vector;
use r2d2_jsonai::JsonAiV3;
use r2d2_kernel::Validated;
use r2d2_secure_mem::SecureMemGuard;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use thiserror::Error;
use tracing::{info, instrument};

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
        let pool = PgPoolOptions::new()
            .max_connections(20) // Idéal pour le Swarm R2D2 en local.
            .connect_lazy(database_url)
            .map_err(|e| BlackboardError::ConnectionError(e.to_string()))?;

        Ok(Self { pool })
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

        // 4. Exécuter l'insertion stricte dans la base (Hybrid Indexing: JSONB + HNSW)
        sqlx::query(
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
        .bind(default_embedding)
        .bind(&validated.proof_of_inference)
        .execute(&self.pool)
        .await?;

        // À la sortie de cette fonction, `guard` est relâché. Le destructeur Invoquera bzero()
        // sur l'espace RAM ayant contenu le payload ! (Souveraineté des données)
        Ok(jsonai.id)
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
}
