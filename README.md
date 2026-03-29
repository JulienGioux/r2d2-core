<div align="center">

# 🛡️ R2D2 : L'ÉCOSystème Cognitif Souverain

[![CI](https://github.com/r2d2-forge/r2d2-core/actions/workflows/ci.yml/badge.svg)](https://github.com/r2d2-forge/r2d2-core/actions/workflows/ci.yml)
[![Rust Version](https://img.shields.io/badge/rust-1.80.0%2B-blue.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-green.svg)](#license)
[![Status](https://img.shields.io/badge/status-Industrial--Grade-red.svg)](#status)

**Architecture d'un Essaim Décentralisé, Ternaire et Auto-Financé**

[Architecture & Livre Blanc](./DEV_DOCS/LIVRE_BLANC.md) • [Contribuer](./CONTRIBUTING.md) • [Roadmap](./DEV_DOCS/ROADMAP.md)

</div>

---

## 🚀 Le Projet R2D2

R2D2 n'est pas une simple itération de LLM. C'est un **Système d'Exploitation Cognitif (COS)** bâti sur trois piliers intransigeants :

1. **La Vérité comme Infrastructure :** Fini le probabilisme et les hallucinations. Le "Paradox Engine" valide sémantiquement chaque fragment via le standard `JSONAI v3.1`. Rien n'est exécuté sans preuve.
2. **La Rupture de l'Inférence Ternaire (BitNet b1.58) :** Destructuration du *Memory Wall*. Remplace les multiplications flottantes par de simples accumulations. Fait tourner des essaims de 15 agents critiques simultanément sur des machines grand public (L40s, RTX 3060).
3. **Souveraineté et Sécurité "Zero-Trust" :** Écrit en Rust. `SecureMemGuard` et *Zeroization* physique de la RAM (SIMD AVX-512) après chaque inférence pour bloquer le Memory Scraping.

Si vous cherchez un "wrapper API" vite fait, vous êtes au mauvais endroit. Si vous voulez forger la nouvelle épine dorsale inviolable de l'I.A. souveraine, **bienvenue dans la Ruche.**

---

## 🛠️ Déploiement Multi-Plateforme (Industrial-Grade)

Le système R2D2 est agnostique au système d'exploitation, mais exige un environnement Rust sécurisé et l'accès aux cycles CPU/VRAM. L'empreinte mémoire du moteur 1.58-bit (BitNet) permet un déploiement sur des machines Edge classiques en "Zero-Config".

### Prérequis Globaux
- **Rust Toolchain** (Édition 2021) avec `cargo`
- **Git** & **CMake** (pour la compilation des stubs C++ Whisper si Audio activé)

---

### 🐧 Linux : Déploiement Natif (Debian / Ubuntu)

Sur un environnement bare-metal Debian/Ubuntu, la compilation nécessite les librairies SSL et les headers de base.

```bash
# 1. Installer les dépendances de compilation critiques
sudo apt update && sudo apt install -y build-essential pkg-config libssl-dev cmake

# 2. Cloner la ruche R2D2 Sovereign
git clone https://github.com/r2d2-forge/r2d2-core.git
cd r2d2-core

# 3. Validation de l'intégrité algébrique et compilation stricte
cargo test --workspace --release
cargo run -p r2d2-mcp --release
```

---

### 📦 Linux : Déploiement Isolé Fedora / Podman (Red Hat)

Pour garantir l'immuabilité "Zero-Trust" de l'hôte, le déploiement via **Podman** (Rootless) est le standard militaro-industriel recommandé sous Fedora/RHEL.

```bash
# 1. Création de l'image isolée basée sur UBI/Fedora
cat <<EOF > Containerfile
FROM fedora:39
RUN dnf install -y gcc gcc-c++ cmake openssl-devel cargo
WORKDIR /usr/src/r2d2
COPY . .
RUN cargo build --release --workspace
CMD ["cargo", "run", "--release", "-p", "r2d2-mcp"]
EOF

# 2. Build et lancement Sans-Privilège (Rootless)
podman build -t r2d2-core-secure .
podman run -d --name r2d2-swarm --network host r2d2-core-secure
```
*Note : Assurez-vous d'avoir instancié les drivers `/dev/dri` si l'accélération matérielle est visée hors BitNet.*

---

### 🏁 Windows : WSL2 & Natif (DirectX 12)

Le système R2D2 s'exécute de façon translucide sur Windows grâce à **WSL2** ou nativement via *MSVC*.

**Méthode WSL2 (Recommandée pour la Gateway MCP) :**
```bash
# Depuis un shell Ubuntu/Fedora Remix (WSL2)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
git clone https://github.com/r2d2-forge/r2d2-core.git
cd r2d2-core
cargo check --workspace
```
*L'interconnexion au client UI Windows s'opère par Stdio via le Model Context Protocol (MCP).*

**Méthode Windows Native (PowerShell/MSVC) :**
```powershell
# Prérequis : Visual Studio C++ Build Tools
git clone https://github.com/r2d2-forge/r2d2-core.git
cd r2d2-core
cargo run --release -p r2d2-mcp
```

---

### 🍏 macOS : Apple Silicon (M1/M2/M3)

Le moteur de R2D2 utilise Candle et supporte nativement l'API `Metal` de macOS, transformant les puces Apple Silicon en redoutables forges d'inférence.

```bash
# 1. Installation de la Toolchain via Homebrew
brew install rust cmake

# 2. Clonage et exécution (avec l'accélération backend Metal)
git clone https://github.com/r2d2-forge/r2d2-core.git
cd r2d2-core
cargo run --release -p r2d2-mcp --features metal
```

---

## 🦾 R2D2-BitNet : Moteur Cognitif 1.58-bit

Ce dépôt inclut désormais la brique fondatrice `r2d2-bitnet`, notre implémentation locale du modèle de langage ternaire (AbsMean Quantization Poids/Activations). 

Pour lancer un prompt sur le Moteur CPU Souverain (0 MatMul, 100% Additions Logic-Only) :
```bash
cargo test -p r2d2-bitnet
```
*Le backend télécharge de façon atomique les SafeTensors "1bitLLM/bitnet_b1_58-3B", les convertit en masques d'états `{-1, 0, 1}` via hard-thresholding strict, et exécute la boucle autorégressive sans toucher aux FPU !*

**Architecture d'Entraînement "Air-Gapped"** :
L'entraînement du modèle 1.58b évolue dans `r2d2-cortex`. Le pipeline d'ingestion (Dataloader) est par design garanti **Zero-Allocation (Object Pool MPSC)** post-démarrage. Le streaming asynchrone est protégé contre les vecteurs d'attaque OOM et les corruptions d'apprentissage liées aux chevauchements de frontières UTF-8 du dictionnaire (LLaMA-3 / 128k).

---

## 🧠 Doctrine d'Ingénierie (Staff-Level Requirement)

Notre code est critique (Défense, Finance, Systèmes Souverains). Lisez attentivement le [CONTRIBUTING.md](./CONTRIBUTING.md) avant votre première PR. Les règles d'or :

1. **Typestate Pattern Absolu :** Un état invalide `Unverified` ne doit *jamais* compiler avec une fonction nécessitant un état `Validated`.
2. **0 `unwrap()` / 0 `panic!()` :** Toute erreur est traitée, tracée (`tracing`) et renvoyée formellement via `thiserror` et `anyhow`.
3. **Zero-Trust Memory :** Si vous manipulez des poids de modèles ou des données utilisateurs, elles doivent transiter par les sandbox de la Brique 0 (Hyperviseur).

---

## 🗺️ Architecture de Haut Niveau

L'essaim R2D2 est un agencement modulaire strict de 14 Briques. L'intégration récente du Cortex (Tensor/Local) et de la Gateway MCP rend le système autonome et sécure.

```mermaid
graph TD;
    LLM[Agent Externe / Claude / Gemini] -->|Stdio JSON-RPC| Brique9(Gateway MCP);
    Brique9 -->|Ingestion| Brique1(JSONAI v3.1);
    Brique1 -->|Unverified| Brique3(Paradox Engine);
    Brique3 <-->|Vectors 1024D (E5-Large-Instruct)| Brique4(Cortex Local);
    Brique3 <-->|Audio (Whisper-Large-v3-Turbo)| Brique4;
    Brique3 -->|Consensus| Brique2(Kernel & Blackboard);
    Brique2 -->|Validated| Brique7(Vector DB PostgreSQL HNSW);
```

---

## 🔌 Intégration LLM / Éditeur (Guide MCP)

Vous pouvez lier **n'importe quel agent d'IA** (Claude Desktop, Cursor, Gemini) à R2D2 via le protocole ouvert MCP (Model Context Protocol). L'Essaim tournant sous WSL, le pont Windows/Fedora est natif, étanche, et sans latence HTTP !

Pour donner la mémoire absolue à votre IA, ajoutez ceci à votre fichier `mcp_config.json` (Ex: `%APPDATA%/Claude/claude_desktop_config.json` ou `~/.gemini/mcp_config.json`) :

```json
{
  "mcpServers": {
    "r2d2-blackboard": {
      "command": "wsl.exe",
      "args": [
        "--",
        "bash",
        "-c",
        "cd /mnt/d/VOTRE_CHEMIN/R2D2 && source ~/.cargo/env && RUST_LOG=info cargo run -q -p r2d2-mcp"
      ]
    }
  }
}
```

Redémarrez votre éditeur, et voici les pouvoirs débloqués :
- `anchor_thought` : Fait glisser la réflexion de l'IA vers le moteur tensoriel E5 puis dans PostgreSQL.
- `recall_memory` : Effectue un *Similarity Cosine Search* dans le Blackboard à vitesse maximale pour exhumer toute architecture passée.

---

## 📜 Licence

Ce projet est distribué sous double licence MIT et Apache 2.0 au choix. Voir les fichiers `LICENSE-MIT` et `LICENSE-APACHE`.
L'infrastructure de financement décentralisée par preuve d'inférence (PoI Tax 1%) est intégrée au protocole.

<div align="center">
  <i>Document certifié par l'Essaim R2D2 - Épuration Sémantique Validée</i>
</div>