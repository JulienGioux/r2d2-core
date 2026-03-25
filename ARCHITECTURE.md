# 🛡️ Doctrine Architecturale : R2D2 Multi-Agent Asynchrone (RustyMaster Standards)

Ce document fige les règles d'or et anti-patterns critiques pour l'évolution de la base de code R2D2, guidés par les standards industriels Rust (édition 2026).

## 1. Ségrégation Stricte : Thread Pool vs Async (Le Piège Mortel)
**Alerte** : L'inférence `candle` monopolise intensivement le CPU. 
**Règle** : L'écosystème `tokio` utilise un ordonnancement coopératif. Ne **JAMAIS** lancer une inférence de modèle ML directement sur le thread d'une tâche asynchrone (risque d'affamer le runtime/Thread Starvation).
**Solution** : Envelopper systématiquement l'inférence (telle que Whisper ou LLaVA) dans `tokio::task::spawn_blocking` ou un thread pool CPU dédié ségrégé du runtime I/O.

## 2. Pression Arrière (Backpressure) & Bounded Channels
**Alerte** : Un flux continu de données sensorielles (vidéo, audio continu) peut noyer un agent lent et engendrer des Out Of Memory silencieux.
**Règle** : Toujours utiliser des canaux de communication **bornés** (`mpsc::channel(N)`) lors des transferts inter-agents. L'orchestrateur doit rejeter la tâche ou ralentir le producteur (backpressure) si la file est saturée.

## 3. Bulkhead Pattern & Isolation d'Agents
**Règle** : Les agents cognitifs (`AudioAgent`, `VisionAgent`) doivent être isolés pour prévenir les défaillances en cascade. Si un agent est bloqué sur une hallucination ou crashe (Panic), cela ne doit pas faire tomber le `SensoryGateway`. Utiliser des **Circuit Breakers** pour dégrader gracieusement le service.

## 4. Prévention du Stack Bloat Asynchrone
**Règle** : L'abus du mot-clé `async` partout génère des `Futures` de l'ordre de plusieurs dizaines de kilo-octets. Le code asynchrone doit être formellement limité aux points de friction réseau, disque, ou attente I/O (FFmpeg ingestion). Le cœur logique de décision interne des agents doit être composé de fonctions **synchrones** pures.

## 5. Cadrage strict des signaux bruts (Zéro Padding Artificiel)
**Règle (AudioChunker)** : Ne jamais introduire de vide numérique (silence absolu `0.0`) pour combler artificiellement des buffers d'inférence temporels (ex: 30s imposés par Whisper). `candle` gère structurellement la projection de signaux arbitraires tronqués via les vecteurs de position dynamiques. Combler avec de la donnée morte incite les transformers d'attention à projeter des hallucinations bouclées.
