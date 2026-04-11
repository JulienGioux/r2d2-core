# Livre 04 : Les Plateformes et I/O (L'Asynchronisme Rust)
**Classification: ADAPTER PATTERN / EDGE INBOUND OUTBOUND**

Ce document fige les règles d'or et anti-patterns critiques concernant l'asynchronisme en Rust et la gestion de l'infrastructure R2D2. L'abus du mot-clé `async` partout génère des `Futures` de l'ordre de plusieurs dizaines de kilo-octets. Le code asynchrone doit être formellement limité aux points de friction réseau, disque, ou attente I/O (FFmpeg ingestion). Le cœur logique de décision interne des agents doit être composé de fonctions synchrones pures.

## 1. LA DOCTRINE "ANTI-STARVATION" (SÉGRÉGATION STRICTE)
La mort silencieuse ("Deadlock") et la perte de latence massive ("Thread Starvation") sont causées par une utilisation dilettante de `async/await` en écosystème IA lourd. Tokio ordonnance les tâches par le biais de "Work Stealing" purement coopératif. L'inférence tensorielle n'est pas coopérative, elle monopolise intensivement le CPU. 

**Loi Stricte :** Ne JAMAIS lancer une inférence de modèle ML directement sur le thread d'une tâche asynchrone. Tout I/O disque lourd (chargement de poids), toute extraction PTX/CUDA FFI via Tensor, ou tout batch long imposant plus de `1ms` de travail exclusif au CPU **DOIT** impérativement être relégué/confiné dans un `tokio::task::spawn_blocking` (thread pool CPU dédié). Un code Rust qui appelle du `.await` sur un traitement FFI lourd est un `commit` immédiatement rejeté.

## 2. SURROGATE WEB ET ACTOR PATTERN (LE PIÈGE DU MUTEX)
L'UI Administrateur `r2d2-ui` (pilotée par HTMX) et nos Chrome Headless de récupération Sémantique (`r2d2-vampire`) exigent de muter l'état global.
- Un `Mutex` verrouillé bloquant un scope contenant un `.await` (le fléau "Lock-Across-Await") va systématiquement forcer le Runtime de Tokio à corrompre le multiplexage, détruisant tout le modèle de performance asynchrone R2D2.

**Loi Stricte :** Les interfaces (Axum) ne détiennent pas elles-mêmes les données. R2D2 impose **l'Actor Pattern** pour le Surrogate Web. Les Axum Handlers envoient des messages internes asynchrones via les `mpsc` non bornés (Channels). Le processus Acteur unique ciblé consomme le canal, accède à la mémoire en mode Synchrone, et répond. 

## 3. ZERO-JS HTMX FRONTEND
Pour `r2d2-ui`, aucun moteur JavaScript client complexe (React, Vue) n'est autorisé.
L'hyper-media asynchrone suffit au fonctionnement des interfaces administrateurs (Boutons de déploiement d'agents, visualisation des Logs Tensoriels). Axum (Serveur) manipule les templates Askama, et transmet les fragments HTML via les Events Server-Sent ou WebSockets, garantissant à l'écosystème R2D2 qu'aucune dépendance fragile Node.js n'invadera le TCB.

## 4. RÈGLES MATÉRIELLES SUPPLÉMENTAIRES DE L'INFRASTRUCTURE

### 4.1 Pression Arrière (Backpressure) & Bounded Channels
Un flux continu de données sensorielles (vidéo, audio continu) peut noyer un agent lent et engendrer des Out Of Memory silencieux. Toujours utiliser des canaux de communication **bornés** (`mpsc::channel(N)`) lors des transferts inter-agents. L'orchestrateur doit rejeter la tâche ou ralentir le producteur (backpressure) si la file est saturée.

### 4.2 Bulkhead Pattern & Isolation d'Agents
Les agents cognitifs doivent être isolés pour prévenir les défaillances en cascade (Panic). Utiliser des **Circuit Breakers** pour dégrader gracieusement le service.

### 4.3 Cadrage Strict (Zéro Padding Artificiel)
(Ex: AudioChunker) Ne jamais introduire de vide numérique (silence absolu `0.0`) pour combler artificiellement des buffers d'inférence temporels (ex: 30s imposés par Whisper). `candle` gère structurellement la projection de signaux arbitraires tronqués. Combler avec de la donnée morte incite les transformers d'attention à projeter des hallucinations bouclées.

### 4.4 Isolation Mutuelle du KV Cache (Clone Shallow)
Partager une structure de modèle ML via `Arc` interdit strictement son emprunt mutable. Ne **JAMAIS** utiliser de `std::sync::Mutex` sur un modèle entier, sous peine de sérialisation complète du Thread Pool. À l'intérieur de `spawn_blocking`, opérer un clone direct (ex: `let mut local_model = engine.model.clone();`). Sous `candle`, cela clone de façon ultra-peu coûteuse les C-pointeurs (Shallow copy) tout en isolant formellement l'instance et son KV Cache interne pour ce thread.

### 4.5 Routage Dynamique des Filtres Spatiaux 
L'ingestion sensorielle (ex: Mel-Filters) ne doit jamais supposer une géométrie fixe. Elle doit lire dynamiquement la configuration du modèle distant et router le téléchargement tensoriel du dictionnaire approprié, garantissant un back-end évolutif sans intervention de l'Architecte.

### 4.6 Hardware Digital Twin (Sensations Somatiques)
R2D2 possède une conscience de son support matériel. Il surveille en temps réel la télémétrie thermique et électrique (via NVML et lm_sensors). Si une surchauffe est prédite, l'IA réduit dynamiquement son débit de tokens ou bascule ses calculs critiques vers des cœurs CPU plus économes (Throttling prédictif).

### 4.7 Isolation d'Exécution et Sandboxing (Podman)
L'intégralité du socle d'I/O et de l'environnement d'exécution (serveur RPC, base de données, inférence) est circonscrit au sein d'un **conteneur Podman (image Fedora)**. Le système ne présuppose aucune confiance envers le shell hôte natif (Windows/WSL).
- **Conséquence de sécurité :** Même en cas de compromission LLM conduisant à la génération d'une commande système malveillante via l'outil `run_command`, la charge active reste confinée dans le namespace isolé du container (Syscalls bridés, PID et Networking isolés), ne menaçant jamais le Trusted Computing Base (TCB) de l'OS maître.
