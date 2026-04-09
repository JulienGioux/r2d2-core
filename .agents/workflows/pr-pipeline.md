---
description: PrePushGit
---

**Quand utiliser ce workflow :** 
Dès qu'une fonctionnalité (Feature), une correction de bug (Fix) ou une phase d'architecture est déclarée "terminée" et prête à être envoyée vers le dépôt distant pour validation finale par l'Architecte.

**Protocole d'engagement inviolable (Zero-Direct-Push) :**

1. **Vérification Stratégique Locale (CI)**
   - Lancer IMPÉRATIVEMENT la suite d'intégration locale via la commande : `./scripts/local_ci.sh`.
   - Si la commande produit autre chose que `Exit code 0` (Erreurs de Linter, Clippy, Formatage, ou Tests), la PR est **interdite**. Toutes les erreurs doivent être résolues au préalable.

2. **Isolation Git (Branching)**
   - Ne jamais travailler ou commit directement sur `main`.
   - Créer et basculer sur une branche dédiée avec une sémantique stricte : 
     - Feature : `git checkout -b feature/nom-de-la-feature`
     - Fix : `git checkout -b fix/nom-du-bug`

3. **Validation & Commit (Conventional Commits)**
   - Exécuter `git add .`
   - Formater une description atomique. Ex : `git commit -m "feat(module): description claire"`

4. **Authentification GitHub Silencieuse (Vault)**
   - Extraire le token "GITHUB_PERSONAL_ACCESS_TOKEN" depuis le coffre-fort des secrets : `/home/jgx/source/R2D2/data/.secrets.json`.
   - Exporter ce token silencieusement dans le terminal via la variable `GH_TOKEN`.

5. **Génération de la Pull Request**
   - Propulser la branche : `git push -u origin nom-de-la-branche`
   - Forger et publier la PR avec l'outil GitHub CLI : `GH_TOKEN=<token> gh pr create --title "..." --body "..."`
   - Le "Body" de la PR doit être formaté de façon professionnelle, listant les `Modifications Architectures` et certifiant le succès du `local_ci.sh`.

6. **Passation de pouvoir**
   - Rendre le contrôle à l'Architecte en lui fournissant l'URL générée de la Pull Request pour qu'il procède à une analyse humaine (Review) et un `Merge` manuel vers `main`.
