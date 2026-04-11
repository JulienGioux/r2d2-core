# Livre 00 : R2D2 Constitution (La Topologie)
**Classification: STAFF ENGINEERING / CORE SYSTEM PROTOCOL**

## 1. VISION FRAME-KERNEL ET MONOLITHIQUE
Le projet **R2D2** fonctionne selon le paradigme du **Framekernel** (similaire au noyau d'OS Asterinas). L'évolution et la sécurité d'un écosystème IA fonctionnant Bare-Metal requièrent des garanties atomiques strictes. 

De ce fait :
- **Le Monorepo est Inviolable** : La division en registres (Multi-repo) est interdite. Les 25 crates actuelles partagent un unique cycle de vie Cargo, assurant la cohérence totale des types (Zero-Warning Policy locale).
- **Architecture Hexagonale Stricte** : La séparation ne se fait pas par les dossiers Git, mais par les frontières logiques "Noyau vs Ports vs Adaptateurs".

## 2. DÉMARCATION DU TCB (Trusted Computing Base)
Toute ligne de code R2D2 est considérée comme hostile à l'intégrité du système, sauf si elle appartient au **TCB**.

### Zone Rouge : Le TCB Privilégié
*La seule zone de la machine ayant autorité sur le Hardware.*
- **Loi de l'Unsafe** : Seul le TCB est autorisé à appeler ou instancier des blocs `unsafe` (FFI Cuda, pointeurs bruts, allocation Pinned Memory).
- **Crates concernées** :
  - `r2d2-cuda-core` (Inclusions PTX manuelles)
  - `r2d2-bitnet` (Moteur tensoriel et FWHT)
  - `r2d2-secure-mem` (Zeroization physique)

### Zone Verte : L'Espace Dé-privilégié (Safe Rust)
*Tout ce qui dépend du TCB ou orchestre la donnée externe.*
- **Loi de la Sécurité** : Les 22 autres crates (Serveurs MCP, UI HTMX, Scrapers web, JSONAI) s'exécutent en espace 100% Rust Safe. Toute tentative d'implémenter `unsafe` dans les adaptateurs externes sera rejetée par le compilateur via la consigne `#![forbid(unsafe_code)]` définie à la racine de la crate.

## 3. CARTOGRAPHIE DES 25 CRATES
- **Noyau Penseur :** `r2d2-kernel`, `r2d2-jsonai`, `r2d2-cortex`, `r2d2-mcp-core`.
- **TCB Mathématique :** `r2d2-secure-mem`, `r2d2-bitnet`, `r2d2-cuda-core`, `r2d2-inference-cpu`.
- **Transformation Cutanée (Ports) :** `r2d2-chunker`, `r2d2-paradox`, `r2d2-orchestrator`, `r2d2-tokenizer`.
- **Inbound/Outbound Adaptatifs :** `r2d2-blackboard` (PgVector), `r2d2-circadian` (Pruning Asynchrone), `r2d2-registry` (P2P Swarm), `r2d2-forge`.
- **Frontière Réseau Web / MCP :** `r2d2-ui`, `r2d2-mcp`, `r2d2-adapter-mcp`, `r2d2-adapter-candle`, `r2d2-vampire`, `r2d2-browser`, `r2d2-surfer`, `r2d2-bridge`.

## 4. ANALYSE DES RISQUES ET RÉSILIENCE STRATÉGIQUE (Issue du Livre Blanc Acte 7)

| Composant | Risque Identifié | Rempart Stratégique R2D2 |
| :--- | :--- | :--- |
| **IA Cloud Guests** | Manipulation sémantique, censure ou biais politique. | **Audit Bare-Metal :** Re-validation systématique par le Kernel local souverain via des preuves de raisonnement. |
| **Mémoire Vive** | Surcharge et ralentissement sémantique (Bruit). | **Dynamic Pruning :** Oubli sélectif basé sur l'utilité mathématique et la fréquence de rappel. |
| **Hardware** | Surchauffe, usure prématurée ou instabilité. | **Digital Twin :** Throttling prédictif basé sur la télémétrie réelle et l'ajustement des charges de travail. |
| **Indépendance** | Centralisation financière ou rachat hostile. | **Protocol Tax 1% :** Modèle de revenus décentralisé, auto-suffisant et géré par le code (Smart Contract). |
