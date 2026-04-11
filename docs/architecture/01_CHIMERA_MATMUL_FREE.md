# Livre 01 : Chimera MatMul-Free (Le Métal et les Mathématiques)
**Classification: CORE SYSTEM / TENSORIAL OPTIMIZATION**

La frugalité énergétique de R2D2 repose sur un postulat mathématique fondamental : **la multiplication flottante est une hérésie sur Hardware Edge**.

## 1. INFÉRENCE TERNAIRE (BITNET 1.58-BIT) : LA RUPTURE MATÉRIELLE

Le goulot d'étranglement de l'IA moderne n'est pas la puissance de calcul brute, mais le mouvement des données entre la mémoire et le processeur (le fameux "Memory Wall"). R2D2 exploite la technologie BitNet b1.58 pour briser cette limite physique.

### 1.1 L'Algorithme de Zéro Multiplication (MatMul-Free)

Les réseaux de neurones de la famille R2D2 s'interdisent le calcul dense FP16 dans leurs couches linéaires. Les poids ternaires $\*{-1, 0, 1}$ transforment les produits matriciels complexes en simples accumulations dirigées par masques.

- **Impact CPU (AVX-512 & AMX) :** Le Kernel Rust pilote des masques de bits pour additionner ou soustraire les activations sans jamais solliciter les unités de multiplication flottante (FPU). Cela réduit la consommation énergétique de 70% et la latence d'inférence d'un facteur 10 sur des processeurs comme le Ryzen 7.
- **Impact GPU (VRAM Optimized) :** La bande passante mémoire nécessaire est réduite de 90%. Une RTX 3060 (12 Go) peut désormais héberger des modèles dont l'équivalent FP16 exigerait 120 Go de VRAM, rendant les modèles de 70B+ paramètres accessibles au grand public.

### 1.2 Quantification Adaptive du KV-Cache

Pour maximiser l'efficacité du multi-agent, R2D2 utilise une quantification dynamique de la mémoire de travail (KV-Cache). Les tâches créatives utilisent une précision de 4-bit, tandis que les audits de code critiques ou les calculs mathématiques remontent automatiquement en FP16 pour préserver une précision absolue.

## 2. TOPOLOGIE DE LA CHIMÈRE V2

Le pipeline tensoriel du modèle fonctionne comme un entonnoir de traitement segmenté. Chaque "Brique" adresse un coût computationnel précis.

### Brique 1 : Le Stabilisateur Quantique (Hadamard V2)
- **Localisation :** `r2d2-bitnet/src/hadamard.rs`
- **Mécanique :** Fast Walsh-Hadamard Transform (FWHT).
- **Rôle :** Aplatir l'immense disparité des valeurs en sortie du bloc précédent (Outliers) vers un format uniforme. Ceci permet à l'échelon suivant de ternariser `{-1, 0, 1}` sans écraser la moindre nuance clé. Le calcul du noyau `FWHT` reste MatMul-Free (O(N log N) additions et soustractions).

### Brique 2 : L'Épine Dorsale Continue (Le "BitMamba-3 MIMO")
L'Attention Quadratique standard (Transformers) détruit silencieusement la VRAM (OOM) au fil de la discussion. 
- **La Règle Mamba :** Il est strictement proscrit de réintroduire des convolutions causales séquentielles (type CNN temporel) sur le CPU.
- **Mécanique :** Un Espace d'État (SSM) discret à transition linéaire temporelle. L'inférence utilise une formulation **MIMO** (Multi-Input, Multi-Output) compilée en PTX (CUDA) garantissant une utilisation maximale des Tensor Cores disponibles (sm_86+). Ses filtres `A, B, C` seront convertis (quantifiés) selon la norme 1.58b.
- **Rôle :** Maintien du "flux de conscience temporel". Ingérer infiniment le texte et l'historique de l'IDE sans que la couche *Key-Value Cache* (KVCache) ne croisse. La taille de l'état caché (KVCache State) reste mathématiquement fixe, quelle que soit la longueur du contexte (1M+ tokens).

### Brique 3 : Le Cœur Strict (Le "BitTransformer")
- **Localisation :** `r2d2-bitnet/src/attention.rs` & `r2d2-bitnet/src/transformer.rs`
- **Rôle :** Le mécanisme de `BitSelfAttention` originel garantit un recueil parfait et inviolable des règles fixes pour ce qui ne souffre d'aucune "compression de mémoire".
- **Cas d'usage Actif :** Seul le "System Prompt", le "Fail-Safe R2D2", et les "Workflows immédiats" passeront par ce circuit inviolable.

### Brique 4 : L'Adaptation (La "Mixture of Experts" - BitMoE)
- **Localisation :** `r2d2-bitnet/src/moe.rs`
- **Mécanique :** Un routeur probabiliste d'entrée de bloc, paramétré par `top_k = N` experts activés.
- **Rôle :** Empêcher le chargement mort du modèle. Si R2D2 ingère de la documentation Python, l'expert "Backend C++" n'est pas chargé des disques / VRAM.

## 3. PARAMÈTRES DE L'INTERFAÇAGE (ZERO-COPY) ET INTERDICTION CPU
Le `r2d2-bitnet` n'autorisera aucune régression de convolution sur le Host. Tout chemin critique doit :
- Passer par les kernels matériels CUDA.
- Utiliser l'AWQ adaptatif (fallback `r2d2-inference-cpu` avec support AVX-512 VNNI).

Pour respecter la doctrine de non-duplication RAM :
- Le module s'enclavera sous la dépendance `candle_core`.
- Tous les modules d'optimisations mathématiques bas-niveau (Hadamard FWHT) devront attaquer l'opérateur "inplace" du tenseur Candle, émulant une architecture C/SIMD native sans sortie du graphe computationnel Rust.

## 4. LOI FFI : SÉCURITÉ MÉMOIRE ABSOLUE (`CudaSlice<T>`) ET DESTRUCTION DES "MOCKS"
Afin de garantir une stabilité logicielle inébranlable (Zéro Segfault/Access Violation), la doctrine de l'écosystème R2D2 proscrit formellement deux éléments majeurs :
- **Les Pointeurs C Bruts** : Plus aucune allocation ou interface Rust-CUDA n'utilise de pointeurs instables (`*const u8` ou `*mut f32`). L'intégralité du passage par FFI (Foreign Function Interface) s'effectue exclusivement via les primitives sécurisées de la crate `cudarc` (ex: `driver::CudaSlice<T>`). La gestion de la mémoire vidéo (VRAM) est ainsi 100% déléguée au Borrow Checker Rust (protection RAII).
- **Les Fallbacks Imaginaires (new_mocked)** : La phase de prototypage (Mocks logiciels) est officiellement close. Le système interdit le démarrage de l'inférence via un faux modèle (ex. `ChimeraModel::new_mocked`). Le code Rust manipule les poids canoniques quantifiés concrets dès le *cold-start*. Le "Zéro-Trust" s'applique aussi à l'inférence factice.
