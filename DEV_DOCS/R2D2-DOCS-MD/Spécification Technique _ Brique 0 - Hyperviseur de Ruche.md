# 🧱 BRIQUE 0 : HYPERVISEUR DE RUCHE & SÉCURITÉ PHYSIQUE

## Gestionnaire de Ressources Dynamiques, Isolation Multi-Agent et Forteresse RAM

**Version :** 1.0.0-PRO-SPEC

**Statut :** Spécification Maîtresse

**Hardware Cible :** x86\_64 (AVX-512), NVIDIA (Ampere/Ada/sm\_86+), AMD (via HIP)

## 1. RÉSUMÉ EXÉCUTIF

La Brique 0 agit comme la couche d'abstraction entre le matériel brut (Bare Metal ou Cloud) et les agents cognitifs. Elle orchestre la cohabitation de plusieurs profils d'IA (Architecte, Dev, Sec, etc.) sur une même machine en garantissant :

1. **L'isolation sémantique** (Bacs à sable de réflexion).
2. **L'allocation de puissance** (Orchestration des 64 vCPUs et 48 Go de VRAM).
3. **L'effacement physique** (Protocole de Zeroization anti-scraping).

## 2. MODULE 0.1 : LE HIVE-MANAGER (HYPERVISEUR)

Ce module gère le cycle de vie des instances d'agents locaux.

### 2.1. Spawning et Isolation (Linux Primitives)

* **Cgroups v2 :** Chaque agent est confiné dans un groupe de contrôle. On limite strictement son quota de CPU (ex: Agent Dev = 20 vCPUs max) et de RAM système.
* **Namespaces (nix-rust) :** Utilisation des namespaces Linux pour isoler le réseau et le système de fichiers. Un agent "Bac à sable" ne doit pas voir les fichiers de l'agent "Consensus" sans autorisation.
* **Affinité CPU :** Liaison (Pinning) des processus aux cœurs physiques du Ryzen/vCPU pour minimiser les changements de contexte (Context Swapping) et maximiser le cache L3.

### 2.2. Mode Dynamique de Ressources (The Balancer)

* **Monitoring :** Un thread superviseur surveille la charge toutes les 50ms.
* **Mode Automatique :** Si l'Agent "Architecte" émet une requête critique, l'hyperviseur augmente son poids dans le scheduler CPU et réduit temporairement la priorité des agents de "Sommeil".
* **Mode Manuel :** Possibilité via le swarmctl de figer des ressources (ex: "Je veux que l'Agent Sécurité ait toujours 4 cœurs dédiés et 4 Go de VRAM réservés").

## 3. MODULE 0.2 : GESTION DE LA VRAM ET DU GPU (SLICING)

La RTX 3060 ou la L40s doit être partagée sans collision.

### 3.1. Partage de Poids (Weight Sharing)

* **Immutable Core :** Les poids du modèle BitNet (1.58-bit) sont chargés une seule fois en mémoire GPU en lecture seule (Read-Only).
* **Context Isolation :** Chaque instance possède son propre buffer d'activation (KV Cache) dans des segments VRAM distincts.
* **NVIDIA MPS (Multi-Process Service) :** Activation de MPS pour permettre l'exécution simultanée de plusieurs kernels CUDA sur le même GPU, augmentant l'occupation des SM (Streaming Multiprocessors).

## 4. MODULE 0.3 : LA FORTERESSE RAM (ZEROIZATION & RAII)

La donnée sensible ne doit jamais survivre à son utilité.

### 4.1. Protocole SecureMemGuard<T>

* **Implémentation Rust :** Wrapper utilisant le trait Drop.
* **Destruction Vectorisée :** Utilisation d'instructions SIMD (AVX-512 \_mm512\_stream\_ps) pour écraser des blocs de 64 octets avec des zéros en un temps record.
* **Barrière Anti-Optimisation :** std::ptr::write\_volatile combiné à compiler\_fence(Ordering::SeqCst) pour garantir que le compilateur n'ignore pas l'effacement.

### 4.2. Protection des Registres

* À la fin de chaque cycle d'inférence, une fonction de "Scrubbing" est appelée pour remettre à zéro les registres ZMM (CPU) et les registres GPU afin d'éviter les attaques par canal auxiliaire.

## 5. MODULE 0.4 : LE BLACKBOARD (WORKSPACE COMMUN)

C'est l'espace où les agents déposent leurs propositions pour le consensus.

### 5.1. Zero-Copy Shared Memory (IPC)

* **mmap partagé :** Utilisation de fichiers en mémoire vive (tmpfs ou shm) pour que les agents puissent lire les propositions des autres sans copier les données (économie massive de RAM sur l'instance 128 Go).
* **Verrous Lock-Free :** Utilisation de primitives atomiques pour les notifications de mise à jour du Blackboard afin d'éviter les blocages (Deadlocks) entre les 15 agents concurrents.

## 6. PORTABILITÉ ET DÉPLOIEMENT (DOCKER/BARE-METAL)

### 6.1. Hardware Discovery

Au lancement, le Kernel exécute un test de capacités :

1. **CPU :** Check cpuid pour AVX-512 ou AVX2.
2. **GPU :** Check cudaGetDeviceProperties. Si AMD détecté, bascule vers le runtime HIP.
3. **Dispatch :** Chargement dynamique des bibliothèques (.so / .dll) de calcul optimisées.

### 6.2. Containerization (Distroless)

* **Image de Base :** gcr.io/distroless/static-debian12.
* **Configuration :** L'image ne contient ni shell ni gestionnaire de paquets. Le binaire R2D2 est l'UID 1 (PID 1), agissant comme son propre init system.

## 7. INVARIANTS À TESTER (QA)

1. **Test d'Isolation :** L'Agent A ne doit pas pouvoir pwrite dans le segment mémoire de l'Agent B.
2. **Test de Charge :** Lancer 10 instances saturant 100% du GPU ; le Kernel doit rester réactif via ses cœurs réservés.
3. **Test d'Effacement :** Scanner la RAM après la fermeture d'un agent pour vérifier l'absence de chaînes de caractères JSONAI.