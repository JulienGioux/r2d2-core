# Contribuer au Projet R2D2 🛡️

Bienvenue dans la R&D du projet R2D2. Nous construisons l'infrastructure souveraine de la cognition artificielle : un système sans vulnérabilités, orienté "Zero-Trust" et "Memory Safe".

Nous sommes extrêmement exigeants sur la qualité du code. Chaque PR sera analysée impitoyablement, non par élitisme, mais parce que ce que nous codons doit résister aux environnements les plus hostiles et aux erreurs sémantiques.

---

## 🏗️ La Doctrine d'Architecture "Rusty"

### 1. Tolérance Zéro pour les Panics
Dans un écosystème critique gérant simultanément la VRAM (BitNet), les flux réseau (QUIC/P2P) et un Paradox Engine (Kernel), planter l'application est **inacceptable**.
* **Interdiction stricte :** `unwrap()`, `expect()`, `panic!()`, `assert!()` (sauf dans les fichiers de test `#[cfg(test)]`).
* **Format exigé :** Retournez toujours un `Result<T, E>`. Utilisez les macros de la caisse `anyhow` pour remonter le contexte d'erreur, ou `thiserror` pour les librairies.

### 2. Typestate Pattern & Newtype
Le système de type de Rust vous donne le pouvoir de rendre vos erreurs impossibles à compiler.
* Ne passez pas un `String` non validé si vous attendez un "JSONAI v3.1". Utilisez un `NewType` struct, par exemple `struct JsonAiPayload(String);`.
* Si une entité a des états différents de confiance (Ex: Signal brut `->` Information validée vérifiée), chaque étape **DOIT** correspondre à un type différent.

```rust
// REFUSÉ : Un booléen ne protège pas contre l'erreur humaine.
struct Fragment {
    data: String,
    is_verified: bool,
}

// VALIDE (Industrial-Grade) :
struct UnverifiedFragment { data: String }
struct ValidatedFragment { data: String }

impl UnverifiedFragment {
    fn verify(self) -> Result<ValidatedFragment, Error> { ... }
}
```

### 3. Observabilité et Traçabilité Obligatoire
Aucun code complexe (appels API, calculs d'inférence, validations) ne doit être "muet".
* Chaque fonction clé doit comporter l'attribut `#[instrument]` de la caisse `tracing`.
* Les erreurs retournées doivent contenir un Trace ID.

## 🛠️ Comment Soumettre une PR

1. **Préparez l'espace de travail :**
   ```bash
   cargo fmt --all
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   ```
2. **Couverture de tests :** Toute modification logicielle doit être couverte par des tests unitaires prouvant son effet et par des tests gérant ses cas limites (Edge Cases).
3. **Convention de Commits :** Les commits doivent utiliser les Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`).
4. **Description :** Incluez toujours dans votre PR le "**Pourquoi**". Expliquez l'impact sémantique sur l'architecture hexagonale.

## 🤝 Bienveillance
Être exigeant techniquement n'empêche pas de rester respectueux et constructif. Consultez le [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md).

*Bienvenue dans l'Essaim.*
