# 🛠️ BRIQUE 9 : GATEWAY MCP NATIVE & ORCHESTRATION

## Interface d'Action, Pilotage d'Outils et Pont d'Interopérabilité

**Version :** 1.2.0-PRO-SPEC

**Statut :** Spécification Maîtresse (Interface d'Exécution & Souveraineté)

**Composants :** MCP Host/Server Engine, Tool Discovery Registry, Semantic Permission Proxy, OCI-compliant Sandbox Executor, Multi-Transport Bridge (stdio/SSE)

## 1. OBJECTIF FONCTIONNEL : L'IA AGISSANTE

La Brique 9 constitue le système moteur de R2D2. Elle implémente le standard **MCP (Model Context Protocol)** pour exposer les ressources locales et distantes aux agents de la ruche. Alors que les autres briques gèrent la "pensée" (inférence, logique, paradoxes), la Brique 9 gère l'**acte**. Elle transforme une intention de haut niveau ("Optimise la base de données") en une séquence d'opérations atomiques sécurisées ("Lecture du schéma", "Analyse des index", "Exécution du CREATE INDEX").

### **1.1. Les Piliers de l'Action Souveraine**

1. **Auditabilité Radicale** : Chaque appel d'outil est encapsulé dans un fragment JSONAI (Brique 1), laissant une trace indélébile dans le lignage sémantique. On ne demande pas seulement "Qu'as-tu fait ?", mais "Pourquoi as-tu utilisé cet outil à ce moment précis ?".
2. **Sécurité Déterministe** : Aucune commande n'est exécutée sans passer par le filtre du Paradox Engine (Brique 3). Si une action contredit un invariant de sécurité, le flux est physiquement coupé.
3. **Abstraction de Capacité** : Les agents manipulent des **Capacités** sémantiques (ex: search\_code) plutôt que des chemins d'accès bruts ou des commandes shell spécifiques. Cela garantit la portabilité de la Ruche entre Windows, Linux (Fedora/WSL2) et le Cloud (Infomaniak).

## 2. MODULE 9.1 : LE NOYAU MCP-NATIVE (HOST & CLIENT)

R2D2 redéfinit la plateforme MCP comme un écosystème bidirectionnel. Il ne se contente pas de consommer des outils ; il devient lui-même une ressource pour les autres.

### 2.1. R2D2 comme Serveur d'Introspection (Exposition)

Le Kernel expose ses mécanismes internes comme des outils MCP. Cela permet à des agents tiers ou à d'autres instances R2D2 dans l'essaim d'interroger la ruche de manière structurée sans accéder directement à la mémoire vive.

* **Outils d'Introspection critiques** :
  + r2d2\_inspect\_blackboard : Lecture filtrée de l'état actuel de la réflexion collective.
  + r2d2\_paradox\_audit : Soumission d'une proposition externe au moteur de cohérence locale.
  + r2d2\_request\_expert : Routage d'une requête vers un agent local spécialisé (ex: Expert Rust ou Expert Sécurité).

### 2.2. R2D2 comme Client Universel (Action)

La Gateway peut agréger et piloter n'importe quel serveur MCP conforme, qu'il soit local (scripts Python/Node) ou distant.

* **Multi-Transport Bridge** : Support natif de stdio pour les utilitaires CLI rapides et de SSE (Server-Sent Events) pour les services conteneurisés ou les micro-services distants via HTTP.
* **Contextual Discovery & Registry** : Au démarrage, la Gateway effectue un "Handshake" avec tous les serveurs déclarés. Elle construit un registre dynamique des capacités, permettant aux agents de savoir quels "muscles" ils peuvent solliciter sans avoir besoin de connaître la configuration logicielle sous-jacente.

## 3. MODULE 9.2 : REGISTRE D'OUTILS ET ATTRIBUTION DES RÔLES

L'accès aux outils est strictement segmenté selon le profil de l'agent. Cette segmentation est la première ligne de défense contre l'escalade de privilèges.

| **Profil Agent** | **Capacités & Outils MCP Autorisés** | **Implications de Sécurité** |
| --- | --- | --- |
| **Architecte** | read\_file, list\_directory, inspect\_db\_schema | Lecture seule. Accès à la structure sans droit de modification physique. |
| **Codeur** | edit\_file, create\_file, run\_build\_command | Modification locale restreinte. Limité strictement au répertoire du projet. |
| **Auditeur Sec** | static\_analysis, scan\_vulnerabilities, read\_logs | Accès profond aux métadonnées. Capacité de bloquer les autres agents. |
| **Médiateur** | user\_notification, request\_human\_input | Seul agent autorisé à franchir l'isolation pour interagir avec l'humain. |
| **DevOps** | docker\_control, deploy\_stack, network\_ping | Accès aux couches d'infrastructure. Nécessite une validation HITL systématique. |

## 4. MODULE 9.3 : PROXY DE PERMISSION ET FILTRAGE SÉMANTIQUE

L'exécution d'un outil est l'étape la plus risquée. R2D2 impose une vérification en trois couches pour empêcher toute dérive malveillante.

### 4.1. Analyse d'Intention **(Sanitisation Sémantique)**

Avant d'autoriser un call\_tool, l'intention est traduite en fragment JSONAI et auditée par le Paradox Engine (Brique 3).

* **Vérification de Cohérence** : Si un agent demande à supprimer un fichier que l'Architecte a marqué comme "Immutable" ou "Critique" dans le Blackboard, la Gateway rejette l'appel avant même qu'il ne soit transmis au système d'exploitation.
* **Détection d'Injections** : Le proxy analyse les arguments des fonctions MCP pour détecter des patterns de type "Prompt Injection" ou des tentatives de contournement des chemins (Path Traversal).

### 4.2. Human-in-the-loop (HITL) & **Escalade Progressive**

Les actions à haut risque déclenchent une suspension du flux QUIC (Brique 6).

* **Validation Visuelle** : L'utilisateur reçoit une notification avec un "Diff" clair de l'action prévue (ex: "L'agent Codeur veut supprimer 12 fichiers").
* **Signature de Commande** : L'exécution ne reprend qu'après une validation manuelle. R2D2 apprend de ces validations pour affiner son score de confiance (Brique 8).

### 4.3. Sandbox Hands (Exécution OCI-Isolée)

Toute commande système potentiellement destructrice est exécutée dans un environnement éphémère (Micro-VM ou conteneur OCI type gVisor ou Firecracker).

* **Isolation du FileSystem** : L'outil ne voit qu'un montage en lecture-écriture très restreint.
* **Network Throttling** : L'accès réseau de la sandbox est coupé par défaut, empêchant toute fuite de données (Exfiltration) non autorisée.

## 5. MODULE 9.4 : ORCHESTRATION ET COMPOSITION D'OUTILS

L'orchestration transforme R2D2 d'un exécuteur de commandes en un gestionnaire de workflow intelligent.

1. **Identification du Besoin** : L'Agent Codeur émet un fragment REQUIERT(Test\_Unitaire).
2. **Planification (DAG de Tâches)** : La Gateway ne se contente pas de lancer le test. Elle vérifie si les pré-conditions sont remplies (ex: Check\_Dependencies). Elle construit un graphe acyclique dirigé (DAG) des outils à invoquer.
3. **Exécution Parallèle & Isolation** : Si les tâches sont indépendantes, le Kernel utilise les 64 vCPUs (Brique 2) pour lancer plusieurs sandboxes simultanément.
4. **Feedback Sémantique & "Undo"** : Le résultat (Standard Out/Error) est réinjecté sur le Blackboard. Si un échec critique survient, la Gateway peut initier un protocole de "Rollback" (si l'outil le supporte) pour restaurer l'état précédent du système.

## 6. INTÉGRATION SENSORIELLE ET APPRENTISSAGE (BRIQUE 8)

L'utilisation des outils alimente le cycle circadien de R2D2, créant une boucle de rétroaction sur la qualité des ressources.

* **Score d'Harmonie (Fiabilité)** : Un serveur MCP qui répond avec précision et rapidité augmente son score de "Plaisir Sémantique". L'IA aura naturellement tendance à le privilégier pour les tâches critiques.
* **Score de Dissonance (Instabilité)** : Des erreurs répétées ou des comportements erratiques (Timeouts, sorties mal formées) génèrent un inconfort sémantique. Le Kernel peut décider de mettre un outil spécifique en "Quarantaine" ou de suggérer à l'utilisateur de remplacer le serveur MCP concerné.

## 7. GESTION DES RESSOURCES ET QUOTAS **(HARDWARE GUARD)**

La Gateway surveille la consommation induite par l'action pour protéger l'intégrité physique de la machine (Brique 12).

* **Timeouts Adaptatifs** : Si la température du Ryzen 7 ou de la RTX 3060 dépasse un seuil de sécurité, les délais d'attente des outils sont augmentés pour réduire la cadence d'exécution et permettre au matériel de refroidir.
* **I/O Budgeting** : Empêche un agent de saturer la bande passante disque ou réseau. Si un agent tente d'écrire des gigaoctets de logs inutiles via un outil MCP, le Kernel applique un "Throttling" (bridage) immédiat.

## 8. NOTE POUR LES DÉVELOPPEURS SENIORS

Le protocole MCP est la membrane sémiotique qui sépare le rêve de l'IA (le Blackboard) de la dure réalité du monde physique (le disque dur, le réseau). Sans cette gateway, R2D2 est un cerveau brillant enfermé dans un bocal. Avec elle, il devient un ingénieur de terrain capable de transformer le code en réalité.

**Règle d'Or de la Forge** : Ne développez jamais d'accès directs aux APIs système à l'intérieur de vos agents. Tout mouvement, toute lecture, toute écriture **doit** transiter par le standard MCP. L'opacité est l'ennemie de la souveraineté ; la transparence de la Gateway est la garantie de votre contrôle sur l'IA.

[Image d'un flux d'exécution MCP : Intention -> Audit -> Sandbox -> Résultat]