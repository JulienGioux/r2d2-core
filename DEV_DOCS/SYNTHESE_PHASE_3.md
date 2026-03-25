# Point d'Étape R2D2 : Les Sens et le Cycle Circadien (Brique 8 & V)
*Version 0.3.0 - Fin de la Phase 3 (Synthèse Sensorielle)*

## 1. Ce qui a été accompli (Staff-Level Quality)

Nous avons complété l'implémentation de l'Homéostasie et du Système Nerveux Périphérique avec un standard de sécurité absolu (Zero-Warning) et en respectant l'isolement Air-Gapped.

### 🌙 Crate `r2d2-circadian` (Brique 8 - MCTS & Sommeil)
- Implémentation du **CircadianDaemon** : Polling de l'entropie et déclenchement des cycles de sommeil.
- Moteur **MCTS (Dream Simulator)** : L'Agent BitNet 1.58b exhale les fragments "DEBATED" du Blackboard, rejoue les inférences pour trouver des consensus, puis fait valider le résultat par le `ParadoxEngine`.
- Intégration transparente avec PostgreSQL (`fetch_unconsolidated_memories` / `update_consensus_level`).

### 👁️ Crate `r2d2-sensory` (Brique V - Ingestion Multimodale)
- Architecture Hexagonale stricte via le **SensoryGateway**.
- Typage fort des `Stimulus` interceptés (flux continus ou fichiers isolés). 
- Isolation parfaite entre l'extraction des payloads et le moteur cognitif.

### 🧠 Crate `r2d2-cortex` (Évolution Multimodale)
- **AudioAgent (Whisper)** : Standardisation sur l'ingestion de fichiers **OGG Vorbis** via l'architecture `CognitiveAgent`. 
- **VisionAgent (LLaVA)** : Interface locale Candle prête pour l'analyse de Keyframes Visuelles.
- Les neuro-agents convertissent les signaux en `Fragment<Signal>` formatés au standard `JsonAiV3`. Le compilateur Rust force ces stimuli à traverser le Pare-feu Axiomatique.

---

## 2. Ce qu'il manque (État Actuel)

Bien que le compilateur valide le système (les traits polymorphiques fonctionnent), l'interface de pilotage (Tooling) n'expose pas encore ces capacités.
- **Passerelle MCP** : Le serveur ne permet pas encore aux clients (comme l'IDE Cursor ou des agents distants) de solliciter explicitement l' `AudioAgent` ou le `VisionAgent`.

---

## 3. Les Prochaines Étapes : Lancement de la Phase 4

### Objectif Principal : `r2d2-mcp` (Exposition des Capteurs)
Nous devons écrire les outils MCP pour l'intégration des sens afin de boucler le flux d'entrée.
1. **Tool `ingest_audio`** : Permettra de fournir un chemin absolu vers un fichier vocal `.ogg`.
2. **Tool `ingest_visual`** : Fournira une image/vidéo au `SensoryGateway`.
