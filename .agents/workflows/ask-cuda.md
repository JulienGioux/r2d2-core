---
description: Consultation d'Optimisation GPU/Kernel (Cuda)
---

**Quand utiliser ce workflow :** 
Lorsque l'utilisateur te demande de générer ou d'optimiser des kernels PTX, de manipuler les allocations mémoires GPU (buffers cudarc), ou de debugger des problèmes de performance liés au hardware.

**Protocole d'engagement :**
1. **Analyse Bas Niveau :** Étudie le code Rust (`r2d2-cuda-core`) et la traduction attendue en PTX/Cuda.
2. **Consultation :** Utilise `ask_consultant` en ciblant `Cuda`. Contexte à fournir :
   - Le kernel logique, ou l'équation mathématique à accélérer (ex: calcul Hadamard, Squared ReLU).
   - Les paramètres d'entrée/sortie (les tailles de block/grid attendues).
   - Les erreurs de compilation NVCC s'il y en a.
3. **Application :** Adapte ta génération de code ptx ou les appels Rust FFI (`cudarc`) en te basant rigoureusement sur le retour de l'Expert Cuda.
4. **Validation :** Présente le code avec ses modifications en expliquant les choix techniques validés par le consultant Cuda.
