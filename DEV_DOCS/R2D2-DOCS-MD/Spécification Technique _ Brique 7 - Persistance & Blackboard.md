# 🗄️ BRIQUE 7 : PERSISTANCE & BLACKBOARD

## Architecture de Mémoire Hybride : Bus de Travail Temps Réel et Archive Sémantique ACID

**Version :** 1.3.0-PRO-SPEC

**Statut :** Spécification Maîtresse d'Ingénierie (Architecture de Données & Mémoire Vive)

**Composants :** Shared Memory Bus (Rust/POSIX), PostgreSQL 16+, pgvector (HNSW), Cache LRU Multi-niveaux, Zero-Copy Serialization

## 1. OBJECTIF FONCTIONNEL ET PHILOSOPHIE DE MÉMOIRE

La Brique 7 ne se contente pas de stocker des octets ; elle orchestre la dualité entre le "flux de conscience" immédiat et la "mémoire ancestrale" de R2D2. Elle doit résoudre dynamiquement le paradoxe entre la vitesse extrême requise par le débat entre agents (millisecondes) et la durabilité absolue nécessaire pour la connaissance certifiée (décennies).

1. **L'Échange Instantané (Blackboard)** : Un espace de travail en RAM à latence sub-microseconde. C'est le lieu de la "pensée brute" où les agents locaux s'affrontent et fusionnent leurs fragments JSONAI v3.1 en temps réel.
2. **La Recherche Sémantique Profonde (Intuition)** : Utilisation de l'indexation vectorielle pour retrouver des fragments passés par proximité conceptuelle. Si R2D2 a résolu un bug similaire il y a six mois, il doit s'en "souvenir" instantanément sans recherche par mot-clé.
3. **L'Ancrage Immuable (Sagesse)** : Un système de stockage ACID (Atomicité, Cohérence, Isolation, Durabilité) garantissant que chaque décision validée est gravée, traçable et protégée contre toute corruption, même en cas de coupure de courant brutale.

## 2. MODULE 7.1 : LE BLACKBOARD (BUS DE MÉMOIRE PARTAGÉE ZERO-COPY)

Dans une architecture à 64 vCPUs, le goulot d'étranglement traditionnel est la sérialisation (transformer un objet en JSON, puis inversement). Le Blackboard R2D2 élimine ce coût par une approche "Zero-Copy".

### 2.1. Architecture POSIX Shared Memory et Pointeurs Atomiques

* **Segment de RAM Projeté (mmap)** : Le Kernel Rust initialise un segment de mémoire partagée (jusqu'à 32 Go sur ton infrastructure Cloud). Ce segment est projeté directement dans l'espace d'adressage de chaque processus agent.
* **Sérialisation par Déplacement de Pointeur** : En utilisant des formats comme FlatBuffers ou Bincode, un agent écrit une donnée en RAM et envoie simplement l'offset mémoire (ex: 0x4F2A) aux autres agents via une file d'attente atomique. La lecture est immédiate : aucun octet n'est déplacé, aucun CPU n'est gaspillé en parsing.
* **Lock-Free Ring Buffer** : Pour éviter les verrous (mutex) qui figent les processeurs, le bus utilise des structures de données non-bloquantes. 10 agents peuvent lire simultanément sans jamais se ralentir les uns les autres.

### 2.2. Segmentation de la Conscience (Workspaces)

* **Zone de Spéculation (Volatile)** : Espace ultra-rapide pour les brouillons d'IA. Si un raisonnement est invalidé par le Paradox Engine (Brique 3), cette zone est "shreddée" (écrasement par des zéros) en un cycle d'horloge.
* **Zone de Consensus (Stage)** : Zone tampon où les fragments attendent le vote de la Ruche. C'est ici que s'opère la "fusion sémantique" entre les propositions de Gemini et de l'agent local.

## 3. MODULE 7.2 : PERSISTANCE RELATIONNELLE (POSTGRESQL 16+)

PostgreSQL n'est pas utilisé ici comme un simple entrepôt de données, mais comme un moteur de règles et de lignage.

### 3.1. Schéma JSONB et Indexation GIN (Generalized Inverted Index)

* **Table fragments** : Stocke l'intégralité du payload JSONAI. L'index GIN permet de répondre en quelques millisecondes à des requêtes complexes du type : *"Donne-moi tous les codes Rust produits par l'agent 'Codeur\_Senior' qui ont été validés avec un score de confiance > 0.95."*
* **Lignage de Vérité (Graph Tracking)** : Une table lineage enregistre les relations parents-enfants. Cela permet de reconstruire l'arbre de décision complet. Si une erreur est détectée, on peut remonter à l'agent source et aux données d'entrée qui ont provoqué la dérive.

### 3.2. Optimisation pour 64 vCPUs et I/O Haute Performance

* **Parallel Workers** : Configuration de Postgres pour utiliser jusqu'à 16 workers en parallèle pour les scans de table massifs.
* **Huge Pages & Shared Buffers** : Allocation de 25% de la RAM totale aux buffers partagés de Postgres, avec activation des "Huge Pages" (2Mo) pour réduire la surcharge du MMU (Memory Management Unit) lors des accès intensifs.

## 4. MODULE 7.3 : MOTEUR VECTORIEL ET INTUITION (PGVECTOR)

Le moteur vectoriel est ce qui permet à R2D2 de ne pas seulement "chercher", mais de "comprendre".

### 4.1. Indexation HNSW (Hierarchical Navigable Small World)

* **Embeddings Locaux (BGE-M3)** : Chaque fragment est transformé en un vecteur de 1024 dimensions par un modèle tournant localement sur ton CPU/GPU. Aucun vecteur ne transite par le Cloud.
* **Recherche Topologique** : L'index HNSW crée un graphe de connaissances. La recherche de similarité ne compare pas le texte, mais la "position mathématique" de l'idée.
* **Filtrage Hybride (The Power of Two)** : R2D2 combine la puissance du SQL et de l'IA. Exemple : *"Trouve des solutions similaires (Vector) mais uniquement celles écrites en Rust et certifiées avant le 1er mars (SQL)."*

## 5. MODULE 7.4 : PROTOCOLE "COW" (COPY-ON-WRITE) ET IMMUABILITÉ

Dans R2D2, la vérité est immuable. On ne modifie jamais un fait, on crée une nouvelle version qui le supplante.

* **Versioning Sémantique** : Toute modification d'un fragment génère un nouvel ID. Le fragment d'origine reste intact dans l'archive. Cela permet un "Time Travel" : tu peux demander à R2D2 : *"Montre-moi l'état de ton raisonnement tel qu'il était mardi à 14h02."*
* **Dédoublonnement Intelligent** : Si deux agents produisent exactement le même fragment, le Kernel ne stocke qu'une seule instance physique (Content-Addressable Storage), économisant ainsi ton stockage NVMe sur Infomaniak.

## 6. SÉCURITÉ, RÉSILIENCE ET "BIT-SHREDDING"

* **Chiffrement de Niveau Banque (LUKS + AES-GCM)** : Le volume de données est chiffré au repos. Même si le serveur Cloud est compromis physiquement, les données restent une suite d'octets aléatoires.
* **Zero-Knowledge Persistence** : Les fragments peuvent être chiffrés avec une clé dérivée du HardwareID de ton PC local. Le serveur Cloud stocke alors des "pensées" qu'il est physiquement incapable de lire.
* **Garbage Collection (Élagage)** : Selon les règles de la Brique 11, un worker Rust scanne périodiquement la base pour supprimer les pensées spéculatives qui n'ont jamais atteint le consensus, libérant de l'espace pour la connaissance utile.

## 7. CONFIGURATION CIBLE (SCALING RÉALISTE)

Sur ton environnement (Ryzen 7 local ou 64 vCPUs Cloud) :

| **Ressource** | **Allocation** | **Rôle Stratégique** |
| --- | --- | --- |
| **Shared Memory (RAM)** | 32 Go | Blackboard volatil : supporte 20+ agents en simultané. |
| **Postgres Cache** | 24 Go | Shared Buffers pour les index vectoriels critiques. |
| **Stockage NVMe** | 500 Go+ | Archive sémantique avec compression LZ4 (ZFS/XFS). |
| **Pool de Connexions** | 200 | Gestion des accès concurrents massifs via pgBouncer. |

## 8. NOTE POUR LES DÉVELOPPEURS SYSTÈME

La Brique 7 est le système circulatoire de R2D2. Une latence ici, et c'est tout l'essaim qui devient léthargique.

**Règle d'or :** Ne déplacez jamais la donnée vers le code, déplacez les pointeurs vers la donnée. Laissez PostgreSQL gérer les jointures et pgvector gérer les distances. Votre code Rust doit rester un chef d'orchestre "léger" qui manipule des offsets mémoire et des identifiants de fragments. Si vous commencez à parser des JSON de 5 Mo dans chaque agent, vous tuez la performance.