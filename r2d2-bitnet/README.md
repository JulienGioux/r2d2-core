# 🧠 R2D2-BitNet (Native AI Engine)

`r2d2-bitnet` est le moteur neuronal souverain du projet R2D2, implémentant une architecture d'Intelligence Artificielle **MathMul-Free** basée sur le paradigme 1.58-bit (Ternary LLM). 

L'intégralité du traitement tensoriel s'effectue sans aucune unité arithmétique de multiplication flottante (FPU), permettant à l'Agent Cognitif de fonctionner avec un rendement énergétique et mémoire optimal (Edge AI).

## 🚀 Architecture Topologique (Le Moteur "Chimera V2")

Le moteur a été repensé "from scratch" en Rust (Zero-Trust Memory) pour fusionner 3 technologies ultra-performantes, sans FPU, en exploitant l'auto-vectorisation (AVX-512) et le Multi-threading CPU via `Rayon` :

- **`ssm.rs` (BitMamba)** : Oubliez la complexité quadratique ($O(N^2)$) des Transformers d'Attention. BitMamba utilise une méthode par Espaces d'États avec projection Poids-Ternaires. Complètement indépendant en mémoire, aucune allocation superflue (Zéro OOM).
- **`moe.rs` (Sparse Mixture of Experts Asynchrone VRAM)** : Le Scatter/Gather ultime, migré à 100% sur Carte Graphique "Zero-Bus-Bottleneck". L'extraction des jetons (`index_select`), le routage Top-1 / Top-K, et la recombinaison finale (`index_add`) restent intégralement confinés sous la bande passante locale du GPU (>500 Go/s), éradiquant totalement le piège classique des goulots PCI-E (Transferts inutiles Host-to-Device).
- **`hadamard.rs` (Stabilisateur Quantique)** : Intégration de la Fast Walsh-Hadamard Transform (FWHT), agissant comme un dôme lisseur sur les activations aberrantes ("Outliers"), indispensable pour que BitMamba conserve son exactitude en ternaire sans crash de dérivation.
- **`custom_op_cuda.rs` (Sovereign PTX Bridge & Warp-Reduction)** : Accélération CUDA asynchrone pour les Tenseurs Quantifiés et le Routage SparseMoE. Implémente la `Warp-Reduction` (synchronisation via l'instruction matérielle `__shfl_down_sync`) pour garantir la coalescence des lectures mémoire.
- **`BitLinear`** (Legacy Support) : Couche linéaire dense avec packaging scalaire ternaire.

## 💡 Le Paradigme 1.58-bit (AbsMean Quantization)

Contrairement aux solutions d'Inférence externes, `r2d2-bitnet` ingère les tenseurs de modèles de fondation (ex: 1bitLLM, GGUF, Safetensors) et applique à la volée l'algorithme de quantification rigoureux **AbsMean** décrit par Microsoft Research :

1. **γ (Gamma)** est calculé comme la moyenne de la valeur absolue de la matrice de poids.
2. Une échelle (`Scale`) inversement proportionnelle à `γ` est définie.
3. Chaque poids scalaire continu est divisé par `γ`, arrondi mathématiquement (`Round`), puis durement clipsé sur l'espace fini `{-1, 0, 1}`.

Ce processus déterministe garantit que n'importe quelle matrice "Float" peut être assimilée et transformée en structure ternaire sans altérer la cohérence du routage synaptique initial.

## 🛡️ Intégration Cortex (Plug & Play)

Le moteur `r2d2-bitnet` est instancié depuis la passerelle globale via le port d'architecture Hexagonale `CognitiveAgent`.  
L'implémentation `BitNetAgent` (contenue dans `r2d2-cortex`) gère le cycle de vie OS du réseau :
- Extraction des poids depuis le `HuggingFace Hub`.
- Projection et packaging RAM via `candle`.
- Interception de la séquence de tokens pour inférence **Autorégressive**.

## 🧪 Tests Algébriques

La crate embarque un panel de tests unitaires formels visant à éprouver la viabilité opératoire des tenseurs ternaires et la solidité de l'algorithme _Zero-Branch_ `FMA` Logic-Only :

```bash
cargo test -p r2d2-bitnet
```

L'Architecte impose la directive "Zéro OOM" (Out Of Memory). Le déchargement dynamique RAM est asynchrone et impératif.
