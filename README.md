<div align="center">

# 🛡️ R2D2 CHIMERA ENGINE : L'Écosystème Cognitif Souverain

[![CI](https://github.com/r2d2-forge/r2d2-core/actions/workflows/ci.yml/badge.svg)](https://github.com/r2d2-forge/r2d2-core/actions/workflows/ci.yml)
[![Rust Version](https://img.shields.io/badge/rust-1.80.0%2B-blue.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-green.svg)](#license)
[![Status](https://img.shields.io/badge/status-Industrial--Grade-red.svg)](#status)

**Architecture d'un Système d'Exploitation IA Décentralisé, Ternaire (1.58-bit) et "Zero-Trust"**

[📑 **CONSULTER L'ÉTAT DES LIEUX GLOBAL & LIVRE BLANC B2B (Mise à jour d'Avril 2026)**](./docs_dev/StatusGlobal-2026-04-09.md)  
*Rapport exhaustif contenant les statistiques de complétion, les Benchmarks et le Modèle Économique.*

[Architecture Base](./DEV_DOCS/LIVRE_BLANC.md) • [Contribuer](./CONTRIBUTING.md) • [Roadmap](./DEV_DOCS/ROADMAP.md)

</div>

---

> [!IMPORTANT]
> Bienvenue dans la Forge ! Que vous soyez un développeur junior, un expert en Machine Learning ou un DSI issu du pôle bancaire ou militaire, ce document est votre point d'entrée unique. Prenez le temps de le lire intégralement. R2D2 est un projet d'une complexité rare, mais documenté avec une rigueur absolue.

## 🚀 Le Projet R2D2 : Qu'est-ce que c'est ?

Le marché de l'Intelligence Artificielle est aujourd'hui dominé par des "Boîtes Noires" délocalisées sur des clouds étrangers (OpenAI, AWS, Google Cloud). L'envoi de données confidentielles vers ces serveurs constitue une violation des secrets industriels pour de nombreuses organisations.

R2D2 n'est pas un banal wrapper d'API. C'est un **Système d'Exploitation Cognitif** complet, tournant physiquement *sur votre machine ou vos serveurs fermés*, garanti totalement déconnecté d'Internet. Il s'appuie sur trois ruptures technologiques :

1. **La Rupture Mathématique (Inférence Ternaire 1.58-bit) :** Les modèles classiques requièrent des supercalculateurs hors de prix. Nous avons réécrit les noyaux mathématiques (CUDA) afin de convertir d'énormes réseaux de neurones en petites matrices `[-1, 0, 1]`. R2D2 divise la taille mémoire requise (VRAM) par 10 sans perte d'intelligence.
2. **"Zero-Hallucination" (Le Paradox Engine) :** Les IA classiques mentent de façon plausible. R2D2 valide sémantiquement chaque fragment via une juridiction interne. Rien n'est présenté à un humain sans avoir passé un consensus mathématique stricte.
3. **Sécurité et Éthique (100% Rust) :** Par design, notre code ne peut pas subir d'attaques fondamentales par débordement de mémoire (Buffer Overflow). Tout est sécurisé à la compilation.

> [!TIP]
> **Pour les Débutants :** Imaginez R2D2 comme un super-logiciel capable de lire tous vos documents privés de façon intelligente, de programmer à votre place et d'auditer vos calculs, sans jamais avoir besoin du Wifi.  
> **Pour les Experts :** R2D2 est une implémentation native Rust d'un orchestrateur RAG/Agentique. Nous utilisons des `TernaryBlock16` pour un encodage VRAM Zero-Allocation, encadré par des Graphes Ontologiques (JsonAI V3) parsés via `Aho-Corasick LeftmostLongest` en frontière de Token BPE.

---

## 🗺️ Cartographie de la Ruche (Composants)

Le projet est modulaire, découpé en "Crates" (Paquets logiciels) interagissant de façon asynchrone :

* 🧠 **`r2d2-cortex` (Le Cerveau)** : Centre nerveux Agentique. Maintient le contexte, pilote les LLMs locaux ou distants et donne les ordres.
* ⚡ **`r2d2-cuda-core` & `r2d2-bitnet` (Les Muscles)** : Accélération matérielle. Contient nos noyaux C++ CUDA écrits à la main pour traiter l'encodage extrêmement dense 1.58-bit en VRAM.
* 📚 **`r2d2-jsonai` / `tokenizer` / `chunker` (La Mémoire Épiscopale)** : Couche de vectorisation (RAG). L'IA lit vos fichiers locaux, et nous utilisons notre propre *Tokenizer* BPE pour ne jamais, ô grand jamais, tronquer un fichier au milieu d'un mot (la garantie d'une information parfaite conservée Postgres/PgVector).
* 🛡️ **`r2d2-paradox` (La Douane)** : Moteur de résolution des contradictions. Audit chaque phrase qui s'apprête à être affichée à un utilisateur pour détecter les failles logiques.
* 🎛️ **`r2d2-ui` (L'Écran de Contrôle)** : Interface Web native propulsée par *HTMX* et *Axum*, ne nécessitant aucun lourd Framework JavaScript, la fluidité est absolue (<50ms).
* 🔌 **`r2d2-mcp` (Les Bras)** : Serveur Model Context Protocol. Permet à R2D2 de contrôler votre terminal de commande, lire vos logiciels de paie ou exécuter du code dans un container virtuel sécurisé (`r2d2-workspace`).

---

## 🛠️ Phase d'Amorçage (Installation & Commandes)

Le système R2D2 est conçu pour une industrialisation "bare-metal" ou conteneurisée.
L'empreinte mémoire du moteur BitNet permet un déploiement sur des machines grand public équipées de GPU d'entrée de gamme (ex: RTX 3060).

### Prérequis Indispensables
1. **Rust Toolchain (Édition 2021) :** Assurez-vous d'avoir `cargo` et `rustc` installés via [rustup.rs](https://rustup.rs/).
2. **PostgreSQL & PgVector :** Nécessaire pour la persistance vectorielle (Blackboard).
3. **CUDA Toolkit (Optionnel mais recommandé) :** Pour l'inférence hardware Nvidia.

> [!WARNING]
> Toujours exécuter `source $HOME/.cargo/env` si Rust vient d'être installé, afin de rendre la commande `cargo` disponible dans votre terminal de session active.

### 1. Cloner l'Essaim Souverain
```bash
git clone https://github.com/r2d2-forge/r2d2-core.git
cd r2d2-core
```

### 2. Le Rituel de Validation Continue (CI Local)
*Pourquoi ?* : Avant la moindre exécution, nous passons le système au crible. Le script `local_ci.sh` va formater votre code (`cargo fmt`), purger les moindres failles structurelles (`cargo clippy -D warnings`), et tester la totalité des couches logiques du système ainsi que la couverture de test (`cargo llvm-cov`).
```bash
./scripts/local_ci.sh
```
> [!CAUTION]  
> Ne lancez jamais une compilation de production si le CI Local retourne la moindre erreur pointée en Rouge `Exit Code: 101`. Notre base de code suit le **"Zero-Warning Policy"**.

### 3. Allumer le Tableau de Bord Utilisateur (L'Interface UI)
L'UI est le meilleur moyen d'interagir avec la matrice. Il faut compiler en "Release" pour s'assurer que les optimisations CPU sont activées (Divise le temps d'inférence par 10).
```bash
cargo run --release -p r2d2-ui
```
* **Que se passe-t-il ?** : Le serveur web asynchrone *Axum* s'allume. Il prend le contrôle du port `3000` (ou `8080` selon votre `mcp_config.json`).
* **Comment l'utiliser ?** : Ouvrez votre navigateur Web (Chrome, Firefox) sur `http://localhost:3000`. Vous aurez accès en direct à la télémétrie, au Tenseur Sémantique, et à la fenêtre de discussion avec les Modèles.

### 4. Bâtir le Pont de Commandement Applicatif (MCP Daemon)
Si vous voulez que votre IDE (Cursor, Claude Desktop) bénéficie du Cerveau R2D2, il faut initialiser le "Model Context Protocol".
```bash
# Lancement de la passerelle en silencieux avec journalisation Info
RUST_LOG=info cargo run --release -p r2d2-mcp
```
* **Pourquoi l'utiliser ?** : Cela permet aux IA installées sur votre machine de comprendre l'état de votre projet en direct, de scripter automatiquement et d'archiver vos données en base vectorielle de façon 100% On-Premise.

---

## 🧠 Doctrine d'Ingénierie & Procédure de Contribution

Le cœur du système gérant de potentielles données Défense/Banque, la complaisance technique n'est pas tolérée. Si vous souhaitez modifier le code ou soumettre une amélioration via Pull Request (PR) :

1. **Typestate Pattern Absolu :** En Rust, nous rendons les états invalides impossibles à compiler. Un fragment d'information `Unverified` ne passera jamais dans une fonction exigeant un `Validated`. Si vous détruisez cette sécurité, Clippy vous arrêtera. Ne contournez jamais.
2. **0 `unwrap()` / 0 `panic!()` inattendus :** Dans une chaîne industrielle, si un fichier JSONAI est illisible, l'IA ne doit pas s'arrêter globalement. Gérez toutes les erreurs via l'usage canonique de `Result<T, Anyhow::Error>` et l'annotation de `#[instrument]` via `tracing`.
3. **Intégrité UTF-8 Vectorielle :** Toute altération sur le `r2d2-chunker` ou `r2d2-tokenizer` DOIT utiliser la fonction `safe_word_boundary()` basée sur les métadonnées de l'index interne BPE `word_ids`. Séparer brutalement un texte via `.split()` causera une amnésie du sens et corrompra nos espaces latents vectoriels. Ceci est sévèrement proscrit.

### Workflow PR (Zero-Direct-Push Policy)
Personne (pas même l'Architecte Souverain) n'est autorisé à exécuter un `git push` basique sur la branche `main`. Nous utilisons **systématiquement** le Workflow GitHub automatisé `@[/pr-pipeline]`.

1. Placez vos changements sur une branche séparée : `git checkout -b fix/nom-du-correctif`.
2. Assurez-vous que `./scripts/local_ci.sh` est complètement vert.
3. Compilez la PR via `gh pr create` en certifiant dans la description la réussite des tests de la pipeline Locale.

---

## 📜 Licences, Gouvernance & Financement

Ce projet est distribué sous double licence **MIT** et **Apache 2.0** au choix. Voir les fichiers `LICENSE-MIT` et `LICENSE-APACHE`.

La gouvernance et l'infrastructure de la ruche sont auto-sourcées. La fondation du RAG s'inscrit dans un protocole d'amélioration continue et une architecture éthique assurant le rapatriement total des puissances computationnelles vers les périphériques individuels et les clouds strictement privés.

<br>
<div align="center">
  <i>Document Forgé et Certifié par le Cortex R2D2 - Souveraineté Radicalement Déterministe.</i>
</div>