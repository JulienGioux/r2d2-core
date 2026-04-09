# TODO List R2D2 - Sovereign Shield (r2d2-inference-cpu)

**Date :** 2026-04-09
**Cible :** `r2d2-inference-cpu/src/lib.rs`

## Tâches d'Architecture Sécuritaire :
- `[ ]` **Tâche 1 : Zéro-Branching & `SimdArchitecture`**
  - Supprimer la propriété instanciée `is_x86_feature_detected!` dans la méthode `generate_thought` ou de sa boucle.
  - Coder un trait `SimdArchitecture` abstrait implémenté par 2 struct distincts : `Avx512Engine` et `Avx2Engine`. La détection a lieu uniquement lors de l'instanciation (`try_new()`) par l'Orchestrateur au démarrage de R2D2.
- `[ ]` **Tâche 2 : Alignement Mathématique (Mémoire 64/32 bytes)**
  - Coder un type `#[repr(C, align(64))] struct AlignedBlock([u8; 64])` pour héberger les tenseurs AVX-512 (ou 32 pour AVX2). Ceci évite toute erreur matérielle SIGSEGV de chargement SIMD.
- `[ ]` **Tâche 3 : `slice::as_chunks` pour les Matrices Asymétriques**
  - Remplacer les accès de pointeurs nus par `let (simd_chunks, tail) = tenseur.as_chunks::<64>()`.
  - La partie traitée manuellement ou scalairement (le Reste/Tail loop) doit gérer strictement les dimensions qui ne divisent pas les blocs SIMD avec perfection.
- `[ ]` **Tâche 4 : "NewType" Transparent**
  - Encapsuler la Crate `core::arch::x86_64` (Type natif en C) dans des types sémantiques R2D2 (ex: `#[repr(transparent)] pub struct PackedTernaryWeights`), pour masquer les appels barbares et implémenter leurs primitives manuellement.
