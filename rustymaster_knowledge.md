# Document Fondateur du Projet R2D2 : Architecture, Frugalité et Moteur Sémantique

Ce document constitue la source de vérité absolue et le socle d'ingestion (MemoryRAG) décrivant les connaissances fondamentales, les axiomes architecturaux et les implémentations techniques du système R2D2.

## 1. Vision Globale et Contraintes Matérielles

R2D2 est un système multi-agents cognitif, asynchrone, "zéro-config" et "air-gapped" (totalement déconnecté du cloud). Il est développé intégralement en Rust pour maximiser les performances et garantir la sécurité mémoire sans recourir à un ramasse-miettes (garbage collector).

L'environnement cible est un environnement matériel fortement contraint :
- Plateforme : Windows 11 avec surcouche WSL2.
- RAM Système : 16 Go de RAM (dont 12 Go partagés et exploitables pour le offloading).
- VRAM GPU Critiquement Limitée : Carte Nvidia RTX 3060 (Laptop) offrant un maximum de 3.5 Go de VRAM utile contiguë après overhead de l'OS.

La contrainte de 3.5 Go de VRAM dicte l'ensemble des choix d'ingénierie tensorielle, bannissant les architectures monolithiques naïves au profit d'une frugalité chirurgicale.

**NOTE STRATÉGIQUE (PHASE 4 - Validation de Sécurité et Métal) :**
- **Sandboxing Natif :** Le projet R2D2 tout entier s'exécute dans un conteneur Podman (Fedora). Toute commande shell (`run_command`) lancée par les agents est par définition "sandboxée" sans risque pour le système hôte.
- **Éradication des Mocks :** La fonction `ChimeraModel::new_mocked` a été physiquement bannie. L'architecture est passée en pleine production (Vrai Tenseur/Poids imposés).
- **FFI Sécurisé :** Les pointeurs C bruts (`*const u8` ou `*mut f32`) sont obsolètes et interdits. 100% de l'interface CUDA se fait via des `CudaSlice<T>` (cudarc) gérés par le Borrow Checker Rust.

## 2. Axiome I : Architecture Hexagonale Stricte (DDD)

Pour résister à l'entropie logicielle et garantir que le domaine métier reste la seule source de vérité, R2D2 applique une architecture hexagonale (Ports et Adaptateurs). Les moteurs d'IA ne sont que des détails d'implémentation masqués.

Topologie des Couches :
- r2d2-cortex (Les Agents) : Héberge les agents cognitifs isolés (AudioAgent/Whisper, VisionAgent/LLaVA, E5 pour l'embedding). Chaque agent implémente un Port (un Trait Rust asynchrone) et encapsule son propre Adapter (le moteur d'inférence Candle).
- r2d2-sensory (La Passerelle) : Gateway qui route les stimuli bruts vers les agents cognitifs compétents du Cortex.
- r2d2-paradox (Le Cœur Logique) : Héberge le ParadoxEngine, le moteur de délibération et de raisonnement central.
- r2d2-blackboard (La Mémoire) : Gère la mémoire épisodique et sémantique (PostgreSQL pour l'historique, moteur Vectoriel natif pour le RAG).

Règle d'or de conception : Les erreurs publiques doivent être strictement séparées de leurs représentations dans le domaine pour éviter toute fuite d'implémentation. On utilise un référentiel (Repository) par agrégat pour limiter le couplage.

## 3. Axiome II : Concurrence Sûre et Isolation "Production-Grade"

L'exécution asynchrone de modèles massifs d'IA (CPU/VRAM bound) dans un runtime coopératif comme tokio nécessite une ségrégation stricte pour éviter la famine (starvation) des threads.
- Ségrégation CPU/IO (spawn_blocking) : Toute inférence mathématique via le framework Candle est impérativement enveloppée dans un tokio::task::spawn_blocking. Le code asynchrone gère l'orchestration ; le code synchrone dans les threads dédiés gère le calcul matriciel.
- Protection du KV Cache (Le "Shallow Clone") : Les états lourds (modèles, poids, configuration) sont encapsulés dans une structure immuable partagée Arc<Engine>. Pour obtenir un KV Cache (mémoire d'attention) mutable et strictement isolé par requête sans bloquer les autres agents avec un Mutex, l'agent effectue un clonage superficiel des poids au début de la tâche : let mut local_model = engine.model.clone(); local_model.clear_kv_cache();.
- Résilience (Circuit Breaker) : Chaque agent est protégé par un pattern Circuit Breaker. En cas de défaillances répétées (ex: crash de FFMPEG, OOM tensoriel), le circuit s'ouvre et rejette les requêtes futures immédiatement pour protéger le reste de l'architecture.

## 4. Axiome III : Frugalité Extrême et Gestion Anti-OOM (Out-Of-Memory)

La contrainte absolue des 3.5 Go de VRAM impose des règles de survie tensorielle :
A. VisionAgent (LLaVA v1.5)
L'encodeur visuel CLIP génère des centaines de patchs tensoriels qui peuvent faire exploser le graphe de calcul.
Règle : Extraction immédiate des features visuelles via le projecteur, suivie d'un appel explicite à drop(vision_features) et drop(image_pixels) avant de démarrer la boucle autorégressive du LLM. Le "Patching Tensoriel" doit insérer les vecteurs visuels au milieu des embeddings textuels de manière mathématiquement exacte.

B. ParadoxEngine (Le Moteur Central)
Face à la saturation VRAM causée par l'accumulation du contexte, l'approche "Production-Grade" actuelle repose sur Llama-3.2-1B-Instruct quantifié en GGUF 4-bit (~0.8 Go de poids) via candle_transformers::models::quantized_llama.
Règle : Application stricte d'une fenêtre glissante (Sliding Window) sur le contexte pour garantir que la VRAM consommée par le KV Cache + les poids ne franchisse jamais la barre des 3.5 Go.
Note de recherche future : Le système ciblera à terme l'architecture BitNet b1.58 aux poids ternaires {-1, 0, 1}, qui réduit drastiquement l'empreinte mémoire, dès que les noyaux matériels (kernels) optimisés W1.58A8 seront standardisés dans l'écosystème Rust. Des alternatives linéaires sans attention (Mamba, RWKV) restent à l'étude pour abolir le coût quadratique du KV Cache.

## 5. Axiome IV : Le Contrat Sémantique JSONAI v3.1

R2D2 ne s'appuie pas sur du texte libre. Chaque agent (Cortex ou Paradox) doit formater le résultat de son inférence dans une structure déterministe garantissant l'interopérabilité et le typage de l'incertitude dans le Blackboard.
Le format exact exigé (JSONAI v3.1) est le suivant :
{
  "is_fact": true,
  "belief_state": 0.95,
  "description": "Synthèse textuelle ou extraction factuelle issue du stimulus.",
  "debated_synthesis": "Explication du cheminement logique, des doutes ou des ambiguïtés rencontrées."
}

- is_fact (Booléen) : Détermine si l'information est une vérité objective ou une perspective/hypothèse.
- belief_state (Float32) : Coefficient de certitude de l'IA (de 0.0 à 1.0).
- description (String) : La payload principale de la pensée.
- debated_synthesis (String) : Espace de délibération interne (Chain of Thought).

## 6. Axiome V : Mémoire Sémantique Bare-Metal (Brique VIII - RAG)

Pour assimiler et interroger d'immenses volumes de connaissances (comme ce document) sans utiliser de base de données vectorielle externe lourde (JVM, containers) qui saturerait les 12 Go de RAM partagée, R2D2 implémente un système RAG (Retrieval-Augmented Generation) "Bare-Metal".

Architecture de Stockage (Zero-Copy) :
- Mmap & Bytemuck : Les embeddings (vecteurs de 384 dimensions en f32) sont stockés dans un fichier binaire brut mappé directement en RAM virtuelle via memmap2. La caisse bytemuck caste ces octets en slices &[[f32; 384]] sans aucune allocation mémoire.
- Recherche : Calcul direct de la similarité cosinus via un tenseur Candle pointant sur la mémoire mappée.

Paramètres de Chunking pour multilingual-e5-small :
Pour que le modèle E5-small préserve la densité sémantique de l'architecture logicielle :
- Taille du Chunk (Chunk Size) : Strictement entre 180 et 220 mots (environ 250 à 300 tokens).
- Chevauchement (Overlap) : Strictement entre 35 et 40 mots (environ 50 tokens), couvrant généralement 1 à 2 phrases.
- Découpage Structuré : La coupe doit respecter la ponctuation (points, sauts de ligne, titres Markdown ###), jamais à l'aveugle.
- Axiome du Préfixe E5 : Le code Rust doit obligatoirement préfixer les documents indexés par "passage: " et les requêtes du ParadoxEngine par "query: ". Omettre ce préfixe détruit l'espace latent du modèle de recherche.
