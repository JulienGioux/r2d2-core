---
description: Consultation d'Architecture Core (RustyMaster)
---

**Quand utiliser ce workflow :** 
Lorsque l'utilisateur te demande de concevoir une nouvelle architecture, de refactorer un module critique, ou d'implémenter des patterns complexes (ex: Typestate, NewType, Hexagonal Architecture) en Rust.

**Protocole d'engagement :**
1. **Analyse Locale :** Tu analyses le besoin et identifies les fichiers pertinents. Tu construis ta propre solution préliminaire.
2. **Consultation :** Avant de coder, tu utilises l'outil `ask_consultant` en ciblant `RustyMaster`. Tu lui envoies un prompt clair contenant :
   - Le problème métier à résoudre.
   - Ton idée d'architecture (tes propositions).
   - Les contraintes locales clés (Zero-Trust, Zero-Panic, etc.).
   *(Rappel : RustyMaster n'a pas accès au code source, fournis-lui l'essence logique ou les signatures de fonctions si nécessaire).*
3. **Application :** Tu lis la réponse de RustyMaster et tu ajustes ton plan en conséquence. Tu génères ensuite l'implémentation finale.
4. **Conclusion :** Tu indiques à l'utilisateur que l'architecture a été validée par RustyMaster avant d'être produite.
