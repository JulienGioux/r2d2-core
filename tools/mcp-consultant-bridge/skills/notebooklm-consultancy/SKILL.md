---
name: notebooklm-consultancy
description: "Comment utiliser l'extension MCP Consultant Bridge pour solliciter les experts externes"
---

# Souveraineté : Règle d'engagement des "Consultants"

Tu as accès à des instances de **NotebookLM** via l'interface MCP `ask_consultant`. Ces instances contiennent la doctrine de conception (RustyMaster) ou la logique bas-niveau (Cuda) fournies par l'Ingénieur en Chef.

### 🚫 Ce qu'ils sont et ne sont pas :
- **Ils n'ont pas accès au code source** : Ce sont des LLMs alimentés uniquement par des documents annexes locaux (PDF, Logs).
- **Inutile de leur envoyer du code entier** : Si tu dois interroger `RustyMaster`, explique-lui le problème et donne-lui les extraits précis.

### ⚙️ Comment interroger un Consultant :
1. Liste les experts avec `list_consultants`.
2. Utilise `ask_consultant(name: "RustyMaster", prompt: "Ta question claire avec du contexte")`.
3. Attends la réponse et **applique la décision** sans jamais remettre en question l'Architecture Hexagonale ou le paradigme Zéro-Bloat.

### Scénarios de consultation obligatoires :
1. Si un fichier stipule l'existence d'une optimisation "Hadamard", ou "Squared ReLU" et que tu ne comprends pas la formule d'implémentation, utilise `ask_consultant`.
2. Si tu dois construire des commandes compilateur inhabituelles (ex: vieux GCC/NVCC pour CUDA), demande exactement les arguments à `Cuda`.
