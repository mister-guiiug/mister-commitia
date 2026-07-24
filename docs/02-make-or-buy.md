# 2. Tableau comparatif make or buy

S'appuie sur l'[étude de l'existant](01-etude-existant.md) ([Vérifié] 2026-07-24). Notation de couverture : ✔ couvert · ◐ partiel · ✖ absent.

## 2.1 Couverture du besoin par les familles d'outils existantes

| Besoin cible | GUI Git (GitKraken / GitButler / Fork…) | Moteurs CLI (rebase -i, filter-repo) | Linters CC (commitlint, gitlint, cocogitto) | IA commit (OpenCommit, Copilot…) | Nettoyage CI existant (gh, action Mattraks, rétention native) |
|---|---|---|---|---|---|
| Multi-dépôts déclarés et analysés (stats, conventions, auteurs) | ◐ (multi-repos ouverts, pas d'analyse de conformité) | ✖ | ◐ (validation unitaire, pas d'analyse d'historique) | ✖ | ✖ |
| Détection messages faibles / non conformes / générés | ✖ | ✖ | ◐ (règles sur un message donné) | ✖ | ✖ |
| Réécriture de messages selon convention configurable | ◐ (reword manuel) | ✔ (filter-repo `--message-callback`, sans assistance) | ✖ | ✖ | ✖ |
| Synthèse de plusieurs commits en une intention (IA) | ◐ (**GitKraken Commit Composer** ; GitButler messages IA) | ✖ | ✖ | ◐ (nouveaux commits seulement) | ✖ |
| Squash/reword/reorder/drop avec validation explicite | ✔ (rebase interactif GUI) | ✔ (manuel) | ✖ | ✖ | ✖ |
| Aperçu d'impact avant application | ◐ (préview Commit Composer ; todo list rebase) | ✖ | ✖ | ✖ | ✖ |
| **Plan de réécriture reproductible, exportable** | ✖ | ◐ (scripts rejouables à écrire soi-même) | ✖ | ✖ | ✖ |
| Backup/tag automatique + dry-run systématique + rollback | ✖ (reflog implicite seulement) | ✖ (à scripter) | ✖ | ✖ | ✖ |
| Blocage branches protégées/partagées avec confirmation renforcée | ◐ (avertissements force-push variables) | ✖ | ✖ | ✖ | ✖ |
| Nettoyage mentions d'outils IA **gouverné par politique de dépôt** | ✖ | ◐ (callback générique à écrire) | ✖ | ✖ | ✖ |
| Inventaire runs GitHub Actions **et** Azure DevOps | ✖ | ✖ | ✖ | ✖ | ◐ (GitHub oui ; AzDO : API seulement) |
| Politique de rétention simulée puis validée, journalisée | ✖ | ✖ | ✖ | ✖ | ◐ (dry-run action Mattraks, côté GitHub uniquement, sans journal local ni validation UI) |
| Gestion des retention leases AzDO (rétention bloquante) | ✖ | ✖ | ✖ | ✖ | ◐ (API REST disponible, aucun outil) |
| Tokens au coffre OS, scopes affichés, multi-org | ◐ (les GUI gèrent leurs propres credentials, sans modèle multi-tokens exposé) | ✖ | ✖ | ✖ | ✖ |
| Agent IA à skills **versionnables, testables, gouvernées** | ✖ | ✖ | ✖ | ✖ | ✖ |
| Journal d'audit local des actions destructives | ✖ | ✖ | ✖ | ✖ | ✖ |

Lecture : aucune famille ne dépasse la couverture partielle sur plus d'un tiers du besoin ; les lignes différenciantes (plan reproductible, garde-fous systématiques, CI/CD bi-plateforme, skills gouvernées, audit) sont vides partout.

## 2.2 Options make-or-buy évaluées

Critères notés de 1 (mauvais) à 4 (bon) ; pondération entre parenthèses. **[Recommandé]** — grille d'aide à la décision, pas une mesure objective.

| Critère (poids) | O1. Utiliser l'existant (GitKraken + scripts + action Mattraks + API AzDO) | O2. Contribuer à un outil open source (GitButler) | O3. Plugin / surcouche d'un outil existant | O4. Créer une application (en réutilisant des briques) |
|---|---|---|---|---|
| Couverture fonctionnelle atteignable (×3) | 2 | 2 | 1 | 4 |
| Effort / délai (×2) | 4 | 2 | 3 | 1 |
| Gouvernance & conformité d'entreprise (×3) | 1 | 2 | 1 | 4 |
| Sécurité (tokens, audit, offline) (×2) | 2 | 3 | 2 | 4 |
| Pérennité / dépendance éditeur (×2) | 2 | 2 | 1 | 3 |
| Coût récurrent (licences) (×1) | 2 | 4 | 3 | 4 |
| **Total pondéré /52** | **26** | **29** | **21** | **44** |

### Analyse par option

- **O1 — Utiliser l'existant.** Viable pour un besoin réduit (voir conditions de renoncement en [04-recommandation.md](04-recommandation.md) §4.4). Mais : Commit Composer est propriétaire (IA en plan payant), sans plan exportable ni garde-fous d'entreprise ; le nettoyage AzDO reste à scripter intégralement ; aucune gouvernance de skills ; multiplicité d'outils = friction et pas d'audit unifié.
- **O2 — Contribuer à GitButler.** La stack (Tauri/Rust) est alignée et le produit est ouvert, mais : licence **FSL-1.1-MIT** (interdiction d'« usage concurrent » — une distribution interne dérivée est à faire valider juridiquement) ; la roadmap du produit (virtual branches, workflow de commit) ne recouvre ni le nettoyage CI/CD, ni les skills gouvernées, ni Azure DevOps ; porter ces modules dans leur base = dépendance forte à l'acceptation des mainteneurs. Contribution ponctuelle possible (ex. améliorations rebase), mais pas comme véhicule du besoin.
- **O3 — Plugin/surcouche.** Aucun hôte adapté : GitKraken n'expose pas de système de plugins couvrant ce besoin ; VS Code imposerait l'IDE et limiterait l'UX desktop autonome ; une surcouche de `lazygit`/CLI ne fournit ni l'UI riche ni le modèle de sécurité. Option la plus faible.
- **O4 — Créer, en assemblant des briques éprouvées.** Seule option couvrant les lignes différenciantes ; coût initial le plus élevé, réduit par la réutilisation : moteur Git natif (sequencer piloté par `GIT_SEQUENCE_EDITOR` [Vérifié]), lecture via `git2-rs`/LibGit2Sharp [Vérifié maintenus], règles Conventional Commits reprises de la spec (et non réinventées), APIs REST officielles ([08-apis-plateformes.md](08-apis-plateformes.md)), coffre OS via `keyring` [Vérifié].

## 2.3 Conclusion du comparatif

L'écart entre le besoin et l'existant est **structurel** (gouvernance, reproductibilité, bi-plateforme CI/CD, audit) et non incrémental : aucune option « buy » ou « contribute » ne l'absorbe. → **Créer une application nouvelle en réutilisant des briques**, avec les conditions de renoncement explicites documentées dans la [recommandation](04-recommandation.md).
