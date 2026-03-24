# 🌐 BRIQUE 6 : SWARM NETWORK (P2P FABRIC)

## Système Nerveux Distribué, Découverte d'Experts et Transport Résilient

**Version :** 1.3.0-PRO-SPEC

**Statut :** Spécification Maîtresse (Architecture Réseau Souveraine)

**Stack Technique :** Rust libp2p, QUIC (UDP), Noise Protocol, Kademlia DHT, Gossipsub v1.1

## 1. OBJECTIF FONCTIONNEL : L'ESSAIM COGNITIF

La Brique 6 établit la connectivité fondamentale entre les nœuds de l'essaim R2D2. Son rôle dépasse le simple transfert d'octets : elle crée une **continuité logique** entre des ressources géographiquement distantes. Elle permet à un agent "Architecte" tournant sur ton serveur haute-performance Infomaniak (64 vCPUs) de collaborer avec un agent "Sécurité" sur ton PC local (RTX 3060) comme s'ils partageaient le même bus système.

Cette couche réseau garantit la **Souveraineté Cognitive** en s'affranchissant des serveurs centraux. Elle assure :

1. **L'Invisibilité de la Distance** : Latence optimisée par le choix intelligent des routes et le multiplexage de flux.
2. **La Résilience Totale** : Une architecture "Self-Healing" où la disparition d'un nœud ne fragilise pas la ruche.
3. **L'Autonomie d'Accès** : Utilisation de protocoles de traversée de NAT pour une connectivité "Plug-and-Play" sans configuration de routeur.

## 2. MODULE 6.1 : TRANSPORT ET SÉCURITÉ (NOISE & QUIC)

La sécurité dans R2D2 est "By Design". Chaque bit transitant sur le réseau est protégé par des primitives cryptographiques de pointe.

### 2.1. Transport QUIC (UDP) et Multiplexage de Flux

Le protocole **QUIC** est l'épine dorsale de la réactivité de l'essaim.

* **Multiplexage sans blocage** : Contrairement au TCP, QUIC permet d'ouvrir des centaines de flux unidirectionnels ou bidirectionnels sur une seule connexion. Cela permet d'envoyer simultanément des fragments JSONAI, des flux de télémétrie matérielle (Brique 12) et des signaux de contrôle sans qu'une perte de paquet sur un flux ne ralentisse les autres (**Head-of-Line Blocking Prevention**).
* **Hole Punching & NAT Traversal** : En utilisant UDP, R2D2 implémente des techniques de "poinçonnage de ports". Le protocole **Autonat** de libp2p détecte si le nœud est derrière un pare-feu et tente d'établir des connexions directes via **STUN/TURN**, minimisant le recours à des relais lents.
* **0-RTT Handshake** : Pour les nœuds qui se connaissent déjà (ex: ton PC et ton serveur), la connexion s'établit sans l'attente habituelle des allers-retours de négociation, permettant une reprise d'activité instantanée.

### 2.2. Couche de Chiffrement Noise et Identité

* **Perfect Forward Secrecy (PFS)** : R2D2 utilise le protocole Noise pour dériver des clés de session éphémères. Même si une clé est interceptée dans le futur, elle ne permettra pas de déchiffrer les échanges passés.
* **PeerId Ed25519** : L'adresse réseau d'un nœud n'est pas une IP, mais le hash de sa clé publique. Cette identité est liée au matériel (Brique 0). Si un nœud change d'IP, son identité cognitive reste la même, permettant une persistance des relations de confiance au sein de la ruche.

## 3. MODULE 6.2 : RÉSOLUTION D'EXPERTS (KADEMLIA DHT)

R2D2 transforme l'internet en un **Cerveau Global** via une Table de Hachage Distribuée (DHT) optimisée pour la recherche de compétences.

### 3.1. Capability-Based Routing (Routage par Aptitude)

Au lieu de chercher "qui est à cette adresse", l'essaim demande "qui possède cette capacité".

* **Espace de nom sémantique** : Les clés DHT sont dérivées des capacités annoncées :
  + capability/inference/bitnet/sm86 : Nœuds possédant un GPU Ampere pour le ternaire.
  + capability/audit/rust/unsafe : Nœuds avec un agent expert en audit de mémoire Rust.
  + resource/storage/persistent/high\_speed : Nœuds offrant du stockage NVMe pour le Blackboard.
* **Distance XOR** : Kademlia organise les nœuds dans un arbre logique basé sur la distance binaire de leurs IDs. Cela permet de localiser n'importe quel expert mondial en ![](data:image/png;base64,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) sauts maximum, garantissant une montée en charge infinie de l'essaim.

### 3.2. Monitoring de la Fraîcheur (Liveness)

Chaque annonce de capacité est assortie d'un **TTL (Time To Live)**. Un nœud doit périodiquement "re-publier" ses services. Si ton PC local s'éteint, ses services disparaissent de la DHT en quelques minutes, empêchant l'essaim de gaspiller des ressources à tenter de le contacter.

## 4. MODULE 6.3 : PROTOCOLES DE DIALOGUE (GOSSIPSUB)

Le réseau orchestre les échanges selon l'urgence et la nature de l'information.

### 4.1. Request-Response (Le Dialogue Privé)

Utilisé pour les tâches critiques et lourdes. Lorsqu'un agent Architecte confie une tâche à un agent Codeur, un flux dédié est ouvert. C'est un tunnel privé pour le transfert sécurisé de contextes JSONAI v3.1 volumineux.

### 4.2. Gossipsub v1.1 (Le Blackboard Global)

C'est le protocole de diffusion "épidémique" pour la synchronisation de la conscience collective.

* **Maillage Dynamique (Mesh)** : Gossipsub construit un graphe de pairs hautement connectés. Lorsqu'un agent publie une nouvelle vérité sur le Blackboard, elle se propage comme une onde de choc à travers l'essaim.
* **PRUNING & GRAFTING** : Le protocole évalue la qualité des voisins. Si un pair transmet les messages trop lentement, il est "élagué" (Pruned). S'il est rapide, il est "greffé" (Grafted) au cœur du maillage haute-performance.
* **Signature de Message** : Chaque message Gossipsub est signé. Un nœud malveillant ne peut pas injecter de fausses informations sans posséder la clé privée d'un agent membre de la ruche.

## 5. MODULE 6.4 : QUALITÉ DE SERVICE (QoS) ET CONGESTION COGNITIVE

Le réseau est "vivant" et s'adapte en temps réel aux contraintes physiques des machines.

### 5.1. Score de Réputation (Peer Scoring)

Le Kernel Rust maintient une matrice de performance pour chaque pair :

* **Latence (RTT)** : Temps de réponse réseau brut.
* **Success Rate** : Ratio de fragments validés par le Paradox Engine (Brique 3). Un nœud qui "hallucine" trop voit son score s'effondrer.
* **Cognitive Backpressure** : Si un nœud est surchargé, il renvoie un signal de "pression arrière". La Brique 6 déroute alors les flux vers des pairs plus disponibles.

### 5.2. Load Balancing Sémantique et Failover

Si ton PC local détecte une saturation de VRAM (Brique 12), la Brique 6 effectue un **basculement sémantique**. Elle redirige les calculs vers ton serveur Infomaniak. Pour l'utilisateur, l'interface reste fluide, le calcul a simplement "migré" là où la ressource est abondante.

### 5.3. Circuit Breaker (Isolation de Sécurité)

En cas de détection d'une anomalie majeure (tentative d'injection, données corrompues massives), la Brique 6 coupe physiquement le flux vers le pair incriminé. Ce dernier est banni pour une durée déterminée, protégeant le reste de l'essaim d'une potentielle contagion.

## 6. SÉCURITÉ CONTRE LES ATTAQUES DISTRIBUÉES

* **Sybil Attack Guard** : Pour éviter qu'un attaquant ne sature le réseau de milliers de faux nœuds, R2D2 peut exiger une **Preuve de Calcul** (PoW) légère ou une signature liée à un token d'invitation pour valider l'entrée dans l'essaim privé.
* **Eclipse Attack Prevention** : Le nœud se connecte à un mélange de "Bootstrap Nodes" fixes et de pairs découverts aléatoirement. Cela empêche un attaquant d'entourer ton nœud uniquement de pairs malveillants pour lui cacher la réalité de l'essaim.

## 7. INTÉGRATION AVEC LE KERNEL RUST (BRIQUE 2)

Le Kernel Logique utilise la Brique 6 via une abstraction de haut niveau. Il ne voit pas des IPs, mais des **Services** :

1. **Le Besoin** : L'Architecte a besoin d'une révision de sécurité.
2. **L'Appel** : swarm.find\_expert("audit/security").await
3. **La Résolution** : La Brique 6 interroge la DHT, sélectionne le nœud ayant le meilleur score de réputation et la plus basse latence.
4. **L'Exécution** : Un flux QUIC est ouvert, le JSONAI est transmis, et la réponse est réinjectée dans le Paradox Engine local.

## 8. ANALOGIE POUR LE CHEF DE FORGE

Voyez la Brique 6 comme la **télépathie de la ruche**. Imaginez que chaque machine (ton PC, ton serveur, le PC d'un ami) est un neurone. La Brique 6, ce sont les synapses. Elles ne font pas que transporter l'information ; elles décident quel chemin est le plus rapide, elles isolent les zones "malades" (nœuds lents ou corrompus) et elles permettent à un neurone situé dans ton bureau de "ressentir" et d'utiliser instantanément la puissance de calcul d'un neurone situé à l'autre bout du pays.

C'est un web sans serveurs, où la connaissance circule librement mais sous une garde cryptographique absolue.

*Note technique : L'implémentation Rust repose sur l'exécuteur asynchrone tokio pour gérer le cycle d'événements du Swarm de manière non-bloquante, garantissant que le réseau n'impacte jamais les performances d'inférence CPU/GPU.*