# 🐘 Déploiement : R2D2 Blackboard (Brique 7)

La ligne directrice exigée par la doctrine R2D2 et NotebookLM est l'utilisation stricte de **PostgreSQL 16+ avec l'extension `pgvector`**.

## Architecture Hybride (Power of Two)
- `JSONB` indexé en **GIN** pour l'intégralité du payload sémantique JSONAI v3.1.
- `vector(1024)` indexé en **HNSW** (Hierarchical Navigable Small World) pour l'intuition et la proximité de graphe.

---

## 🚀 Lancement Zéro-Config (Docker)

Un `docker-compose.yml` est fourni à la racine du projet. Il embarque directement l'image officielle PostgreSQL configurée avec l'extension `pgvector` (`ankane/pgvector:v0.7.0`).

### 1. Démarrer le serveur
```bash
docker-compose up -d
```
> Le port `5432` sera exposé. Le script `scripts/init_db.sql` s'exécutera automatiquement au 1er lancement pour créer les tables de Fact et le Lignage (Lineage Engine).

### 2. Arrêter le serveur (sans perte de données)
```bash
docker-compose stop
```
La persistance est assurée par le volume Docker `r2d2_pgdata`.

### 3. Destruction TOTALE de la base (Wipe)
Si vous souhaitez purger tous les Axiomes R2D2 et relancer la forge sémantique depuis Zéro :
```bash
docker-compose down -v
```

---

## 🔧 Variables d'Environnement
Pour que le serveur MCP et le Kernel puissent interagir avec le Blackboard, l'URL suivante est attendue par défaut :
`DATABASE_URL=postgres://r2d2_admin:secure_r2d2_password_local@localhost:5432/r2d2_blackboard`