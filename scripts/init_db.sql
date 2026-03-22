-- Initialisation de la Brique 7 : Blackboard Persistant (R2D2)
-- Ce script est joué automatiquement par l'image PostgreSQL au 1er lancement.

-- 1. Activer l'extension Vectorielle
CREATE EXTENSION IF NOT EXISTS vector;

-- 2. Table Centrale du Blackboard (Hybridation Sémantique & Relationnelle)
CREATE TABLE IF NOT EXISTS blackboard_fragments (
    id VARCHAR(255) PRIMARY KEY,
    source VARCHAR(100) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    is_fact BOOLEAN NOT NULL,
    belief_state VARCHAR(50) NOT NULL,
    consensus_level VARCHAR(50) NOT NULL,
    
    -- Le payload complet JSONAI v3.1 pour requêtes complexes
    payload JSONB NOT NULL,
    
    -- L'intuition vectorielle (Embedding de 1024 dimensions type BGE-M3 ou similaire)
    embedding vector(1024),
    
    -- Le lignage cryptographique validé par la Brique 3
    proof_of_inference VARCHAR(255) NOT NULL
);

-- 3. Indexation Structurelle Extreme (GIN) sur le Payload
CREATE INDEX IF NOT EXISTS idx_blackboard_payload_gin 
ON blackboard_fragments USING GIN (payload jsonb_path_ops);

-- 4. Indexation de Similarité Sémantique (HNSW) sur l'Embedding
-- HNSW (Hierarchical Navigable Small World) permet des requêtes K-NN ultra-rapides.
CREATE INDEX IF NOT EXISTS idx_blackboard_embedding_hnsw 
ON blackboard_fragments USING hnsw (embedding vector_cosine_ops);

-- 5. Table de Lignage (Graphe Raisonnement)
CREATE TABLE IF NOT EXISTS fragment_lineage (
    parent_id VARCHAR(255) REFERENCES blackboard_fragments(id) ON DELETE CASCADE,
    child_id VARCHAR(255) REFERENCES blackboard_fragments(id) ON DELETE CASCADE,
    relation_type VARCHAR(100) NOT NULL, -- ex: "Requires", "Entails"
    PRIMARY KEY (parent_id, child_id)
);