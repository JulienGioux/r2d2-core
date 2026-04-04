# Résolution des Bugs et Gestion Dynamique des Modèles

Tu as tout à fait raison : pas de "mocks" cache-misère, on affronte et on règle le problème à la racine. Voici le plan d'action révisé et robuste :

## 1. Latence de 25s du Store (Le Vrai Fix)
Au lieu de supprimer le scan ou de recharger le FileSystem à chaque rafraîchissement (ce qui fige WSL), on va mettre en place une véritable architecture de cache mémoire pour les modèles HF.
- **[MODIFY]** `r2d2-ui/src/main.rs` :
  - Ajout d'une propriété `hf_models_cache: Arc<RwLock<Vec<String>>>` dans `AppState`.
  - Le chargement de `/ui/store` lira ce cache de façon instantanée (`< 1ms`).
  - Au démarrage de l'application, `tokio::spawn` lancera le scan (`list_local_hf_models()`) en arrière-plan et mettra à jour le cache automatiquement, sans jamais ralentir l'utilisateur.
  - Le bouton "**Indexer le FileSystem**" de la page Store appellera une nouvelle route asynchrone `/api/store/scan` qui lancera le scan en tâche de fond et remplacera le cache à la fin.

## 2. Configuration & Attributs des Modèles (UX Store)
Actuellement, les modèles sont juste "listés". Pour les rendre exploitables :
- **[MODIFY]** `r2d2-ui/templates/store.html` (Tableau du Cache Local) :
  - Chaque modèle possèdera désormais deux éléments interactifs : un **Bouton d'Activation (On/Off)** et un **Sélecteur de Rôle** (Sélecteur HTML HTMX) pointant vers une nouvelle route API.
  - Les rôles exploitables en dur dans R2D2 sont : *Main Reasoning* (Pilote principal), *Code Assistant* (Bitnet), *Vision/Embedding* (Multimodal).
- **[MODIFY]** `r2d2-ui/src/main.rs` :
  - Ajout d'une vraie structure de configuration (même minimale en mémoire via un `HashMap<String, String>` dans `AppState` pour faire le pont de mapping Modèle <-> Rôle) pour cette étape. Quand l'utilisateur changera le rôle via l'UI, le backend affichera le succès de l'assignation.

## 3. Options Masquées & Bitnet Manquant (Workspace Chat)
- **[MODIFY]** `r2d2-ui/static/style.css` : Le texte "blanc sur blanc" au survol du menu déroulant provient d'une hérésie native du navigateur non écrasée par le thème. J'ajouterai :
  ```css
  .custom-select select option { background: var(--bg-void); color: var(--text-primary); }
  ```
- **[MODIFY]** `r2d2-ui/templates/chat.html` : 
  - J'ajouterai `<option value="bitnet">R2D2 (Bitnet Local)</option>` dans le composant `<select>` de la zone de chat.

## Open Questions (Pour Toi)
La gestion des rôles (Point 2) sera pour le moment persistée dans l'interface mais en mémoire volatile dans `AppState` sur cette itération. Confirme-moi si une architecture en RAM locale sans base de données te convient de manière temporaire le temps pour moi d'insérer l'architecture définitive plus tard, ou si je dois l'intégrer avec ton `PostgresBlackboard` (ou un fichier `models.json`) tout de suite pour sauvegarder l'assignation des modèles même lors du redémarrage ?

## Verification Plan
1. Lancement : La page Store s'ouvre **instantanément**.
2. Chat : Les polices du `<select>` sont visibles et R2D2(Bitnet) y est présent.
3. Assignation : Changer le rôle d'un modèle déclenche une réponse asynchrone visuellement propre.
