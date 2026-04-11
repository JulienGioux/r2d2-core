# Livre 03 : L'Essaim et la Cryptographie (Swarm & Protocol)
**Classification: OUTBOUND / PEER-TO-PEER INTER-PROCESSUS**

L'architecture distribuée de R2D2 repose sur un postulat sans appel : ce qui s'échange en réseau doit être anonymisé et incassable, ce qui réside en RAM doit être effacé. Le réseau R2D2 transforme des milliers de machines isolées en une "Grille de Calcul" mondiale, sécurisée et anonyme.

## 1. COMMUNICATION WAN ET LAN (LIBP2P & ZENOH)

### 1.1 Kademlia et WAN (Réseau Étendu)
- **Découverte d'Experts par Capability Mapping :** Pour la recherche inter-nœuds distants, via la DHT Kademlia (`libp2p`), un nœud peut localiser en millisecondes un pair possédant l'expertise spécifique (ex: Expert en Cybersécurité Rust) nécessaire à une tâche complexe.
- **Chiffrement Noise & Transport QUIC :** Les échanges sont chiffrés de bout en bout avec des clés Ed25519 liées physiquement au matériel. Le protocole QUIC permet une traversée transparente des pare-feux domestiques, facilitant la collaboration P2P sans configuration complexe.

### 1.2 IPC et Essaim Local : Zenoh
Cependant, pour la communication inter-processus (IPC) et l'essaim local à ultra-haute performance, le document intègre **Zenoh**.
- Ce protocole Pub/Sub offre des performances de qualité militaire (jusqu'à 3,5 millions de messages par seconde avec 35 microsecondes de latence), idéal pour assurer le découplage entre l'émetteur cognitif et les processus d'audience.
- Les données de la mémoire partagée et les cascades d'activation Tensorielle transitent sur ce protocole sans briser le "Zero-Copy" des buffers IPC.

## 2. HYGIÈNE RAM : SECUREMEMGUARD ET ZEROIZATION

Pour prévenir les attaques par "Memory Scraping" ou les analyses de canaux auxiliaires, R2D2 impose une doctrine d'hygiène RAM absolue via `r2d2-secure-mem`.

### 2.1 Isolation Stricte
Chaque agent travaille dans une sandbox mémoire isolée, empêchant toute fuite d'information entre deux processus de réflexion.

### 2.2 Effacement Actif (Zeroizing)
Toute structure contenant de la donnée privilégiée (clés privées d'identité Ed25519, paramètres de poids de modèles de sécurité, jetons API temporaires GCP) **DOIT** :
- Implémenter le wrapper `Zeroizing<T>` (via la crate `zeroize`).
- Implémenter le trait `SecretVec` (via la crate `secrecy`).

Dès qu'un cycle d'inférence se termine, les registres SIMD (ZMM) et les segments de RAM utilisés sont physiquement écrasés par des zéros. Parallèlement, à l'instant microscopique où un block de scope est quitté ou le trait `Drop` est invoqué, la mémoire sous-jacente est broyée cryptographiquement. La pérennité d'un mot de passe dans le dump d'un processus crashé est une violation critique d'Architecture.

## 3. LE MODÈLE ÉCONOMIQUE : LA TAXE DE PROTOCOLE (1%)

Pour garantir l'indépendance totale de la Forge et refuser catégoriquement la revente de données personnelles, R2D2 instaure un modèle de Financement par le Flux.

### 3.1 Le Prélèvement au Flux (Micro-Toll)
Une taxe de 1% est appliquée sur la valeur de chaque unité de calcul (token ternaire) validée par le protocole (Proof of Semantic Work). Ce modèle remplace l'abonnement forfaitaire par une facturation à l'usage réel. 1% représente une fraction de centime pour une tâche standard, rendant le coût imperceptible pour l'utilisateur tout en assurant une puissance financière massive à l'échelle de l'essaim. L'autofinancement garantit l'invulnérabilité au rachat privé d'un tiers.

### 3.2 Répartition de la Valeur (Economie Circulaire)
Toute intégration d'un module Externe (adapter-*) devra respecter l'indexation algorithmique de signature garantissant la taxe des 1% (frais incompressibles destinés au maintien de la Forge Centrale R2D2).
- **60% (Fonds de Forge) :** Maintenance du code source, recherche sur les nouveaux backends (NPU, Apple Silicon) et serveurs de haute densité pour la R&D.
- **30% (Récompense des Nœuds) :** Rémunération directe des utilisateurs qui partagent leur puissance de calcul avec l'essaim. Votre PC devient un outil productif qui s'auto-finance.
- **10% (Réserve de Stabilité) :** Fonds de garantie pour assurer la résilience du réseau et stabiliser les coûts de calcul mondiaux.
