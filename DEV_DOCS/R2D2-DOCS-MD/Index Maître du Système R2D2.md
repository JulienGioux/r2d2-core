# 🗺️ CARTE DE LA RUCHE : INDEX MAÎTRE R2D2

**État du Projet :** Phase de Conception Finale / Pré-Forge

**Dernière Mise à Jour :** Mars 2026

## 📄 1. DOCUMENTS STRATÉGIQUES (VUE GLOBALE)

| **Document** | **Chemin** | **Description** |
| --- | --- | --- |
| **Livre Blanc v8.2** | white\_paper\_r2d2\_v8\_executive.md | Le manifeste complet (Technique, Éthique, Économique). À envoyer aux investisseurs/presse. |
| **Audit de Cohérence** | atc\_r2d2\_audit.md | Analyse des risques (KV-Cache, Latence API) et solutions de viabilité. |

## 🧩 2. LES BRIQUES FONDAMENTALES (SPÉCIFICATIONS)

Ces documents définissent le "comment" de chaque module.

### A. Le Corps (Hardware & Inférence)

* **Brique 0** : r2d2-specs/brique\_0\_hyperviseur.md (Isolation & SecureMemGuard)
* **Brique 4** : r2d2-specs/brique\_4\_inference\_cpu.md (Optimisation AVX-512)
* **Brique 5** : r2d2-specs/brique\_5\_inference\_gpu.md (Optimisation CUDA/L40s)
* **Brique 12** : r2d2-specs/brique\_12\_hardware\_twin.md (Télémétrie & Survie matérielle)

### B. L'Esprit (Logique & Sémantique)

* **Brique 1** : r2d2-specs/brique\_1\_jsonai\_protocole.md (Standard JSONAI v3.1 Pro)
* **Brique 2** : r2d2-specs/brique\_2\_kernel\_logique.md (Typestate Pattern Rust)
* **Brique 3** : r2d2-specs/brique\_3\_paradox\_engine.md (Détecteur de contradictions)
* **Brique 8** : r2d2-specs/brique\_8\_cycle\_circadien.md (Rêve, Sensations & Sommeil)
* **Brique 10** : r2d2-specs/brique\_10\_immunite\_cognitive.md (Chaos Monkey & Sécurité)

### C. Le Système Nerveux (Réseau & Mémoire)

* **Brique 6** : r2d2-specs/brique\_6\_swarm\_network.md (P2P libp2p & QUIC)
* **Brique 7** : r2d2-specs/brique\_7\_persistance\_blackboard.md (Postgres, pgvector & RAM partagée)
* **Brique 9** : r2d2-specs/brique\_9\_mcp\_native.md (Gateway d'action MCP)
* **Brique 11** : r2d2-specs/brique\_11\_pruning\_dynamique.md (Hygiène de la mémoire/Oubli)

## 💰 3. ÉCONOMIE ET VIABILITÉ

* **Brique 13** : r2d2-specs/brique\_13\_economie\_flux.md (Taxe de protocole 1% & Micro-paiements)
* **Analyse Crypto** : r2d2-specs/analyse\_crypto\_bitnet.md (Synergie Blockchain & Inférence Utile)

## 🛠️ 4. OUTILS DE FORGE (PRÊT À L'EMPLOI)

* **Setup Clé en main** : forge\_setup.sh (Script Bash Fedora WSL2 pour installer CUDA et Rust)
* **Diagnostic Matériel** : r2d2\_preflight.sh (Audit profond CPU/GPU et génération de r2d2\_forge.toml)
* **Analyse Hardware** : strategie\_hardware\_realiste.md (Pourquoi ton PC est suffisant pour le BitNet)

## 🚀 PROCHAINE ÉTAPE RECOMMANDÉE

1. **Exécuter forge\_setup.sh** dans ton Fedora WSL2.
2. **Lancer le Diagnostic** pour obtenir ton premier fichier r2d2\_forge.toml.
3. **Coder le squelette de la Brique 2** (Le Kernel Rust).