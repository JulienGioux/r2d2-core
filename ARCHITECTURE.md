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

## 6. Isolation Mutuelle du KV Cache (Pattern Clone Shallow)
**Alerte** : Partager une structure de modèle ML (`Whisper` par ex.) via `Arc` interdit strictement son emprunt mutable (erreur `cannot borrow data in an Arc as mutable`), indispensable au remplissage du KV Cache lors de la génération.
**Règle** : Ne **JAMAIS** utiliser de `std::sync::Mutex` sur un modèle entier, sous peine de sérialisation complète du Thread Pool et de pollution du cache entre requêtes concurrentes. À l'intérieur de `spawn_blocking`, opérer un clone `let mut local_model = engine.model.clone();`. Sous `candle`, cela clone de façon ultra-peu coûteuse les C-pointeurs (Tenseurs/Weights) tout en isolant formellement l'instance et son KV Cache interne pour ce thread actif.

## 7. Routage Dynamique des Filtres Spatiaux (Mel-Filters)
**Règle** : L'ingestion audio ne doit jamais supposer une géométrie fixe de 80 bins. Elle doit lire dynamiquement le `config.num_mel_bins` du modèle distant (ex: 128 bins pour `whisper-large-v3-turbo`) et router le téléchargement tensoriel du dictionnaire approprié (`melfilters128.bytes`) sur HuggingFace, garantissant un back-end évolutif sans intervention de l'Architecte.
