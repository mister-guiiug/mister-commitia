# mister-commitia

Étude d'opportunité et d'architecture pour une **application desktop d'assainissement gouverné d'historiques Git et d'exécutions CI/CD** (GitHub Enterprise / Azure DevOps) : analyse de dépôts, réécriture **contrôlée** de l'historique des commits (reword, squash, reorder, drop) assistée par un agent IA à skills configurables, et politiques de rétention CI/CD simulées puis validées.

> **Statut : étude (make-or-buy + conception).** Aucun code applicatif — ce dépôt contient l'analyse de l'existant, la recommandation argumentée, l'architecture cible et les backlogs. Recherche documentaire effectuée le **2026-07-24** (sources officielles citées dans chaque document).

## Conclusion en bref

1. **Aucun outil existant ne couvre le besoin complet** ([étude](docs/01-etude-existant.md)) : le plus proche est GitKraken Commit Composer (restructuration de commits assistée par IA, propriétaire) ; rien n'existe pour le nettoyage gouverné bi-plateforme des runs CI/CD (Azure DevOps n'a même pas de commande CLI de suppression), ni pour des skills IA gouvernées, ni pour un plan de réécriture reproductible avec garde-fous.
2. **Recommandation** ([détail](docs/04-recommandation.md)) : **créer l'application en assemblant des briques éprouvées** — sequencer natif de Git piloté par `GIT_SEQUENCE_EDITOR`, `git2-rs`/libgit2 en lecture, spécification Conventional Commits, APIs REST officielles, coffre OS via `keyring` — et réserver l'effort aux vrais différenciateurs : moteur de plan avec dry-run réel, gouvernance (trailers protégés, politique d'attribution IA), rétention CI/CD simulée, journal d'audit.
3. **Stack recommandée** ([architecture](docs/05-architecture-cible.md)) : **Tauri v2 + cœur Rust** (alternative documentée pour équipe .NET : Avalonia + LibGit2Sharp).

## Sommaire des livrables

| # | Document | Contenu |
|---|---|---|
| 1 | [Étude de l'existant](docs/01-etude-existant.md) | Outils vérifiés, capacités, licences ; recherches ciblées et « rien trouvé » explicites |
| 2 | [Comparatif make or buy](docs/02-make-or-buy.md) | Couverture du besoin par famille d'outils ; scoring des 4 options |
| 3 | [Gap analysis](docs/03-gap-analysis.md) | Écarts fonctionnels, techniques, sécurité, gouvernance |
| 4 | [Recommandation argumentée](docs/04-recommandation.md) | Option retenue, trajectoire, conditions de renoncement |
| 5 | [Architecture cible](docs/05-architecture-cible.md) | Modules, ADR, choix de stack (faits vérifiés) |
| 6 | [Modèle de données local](docs/06-modele-donnees.md) | SQLite, plans, réfs de preview/backup, invariants |
| 7 | [Sécurité des tokens](docs/07-securite-tokens.md) | Coffres OS, scopes affichés, rotation, menaces |
| 8 | [APIs GitHub / Azure DevOps](docs/08-apis-plateformes.md) | Endpoints, permissions, limites de débit, on-prem |
| 9 | [Backlog MVP](docs/09-backlog-mvp.md) | Épics, stories, Definition of Done |
| 10 | [Backlog V2](docs/10-backlog-v2.md) | Masse CI/CD, leases, re-signature, CLI headless… |
| 11 | [Risques et garde-fous](docs/11-risques-garde-fous.md) | Garde-fous non négociables + registre de 16 risques |
| 12 | [Structure des skills](docs/12-skills-structure.md) | Schéma, cycle de vie, gouvernance |
| 13 | [Parcours utilisateur](docs/13-parcours-utilisateur.md) | 3 parcours de référence pas à pas |
| 14 | [Critères d'acceptation](docs/14-criteres-acceptation.md) | 14 groupes de scénarios Gherkin |
| 15 | [Stratégie de tests](docs/15-strategie-tests.md) | Fixtures Git, invariants, contrats API, E2E, sécurité |

Les six skills attendues sont fournies en exemples exécutables dans [`skills/`](skills/) (`conventional-commits` sert de référence complète : manifeste + prompt + tests).

## Conventions de lecture

- **[Vérifié]** : constaté le 2026-07-24 sur une source officielle citée (documentation éditeur, dépôt officiel).
- **[Supposé]** : connaissance ou estimation non re-vérifiée, signalée comme telle.
- **[Recommandé]** : choix de conception proposé, à valider.
- « **Je ne sais pas** » : information recherchée mais non vérifiable (ex. code HTTP exact d'une suppression de build sous retention lease).

## Note de gouvernance

La fonctionnalité de normalisation des messages — y compris la détection des mentions ajoutées automatiquement par des outils d'assistance (« Generated with Claude Code », liens de session, signatures d'assistants) — est conçue comme une **normalisation configurable soumise aux règles de gouvernance du dépôt** : si la politique du dépôt impose la traçabilité des contributions assistées par IA, l'application refuse la suppression et l'explique ; les trailers d'audit et de conformité (`Signed-off-by`, etc.) ne sont jamais supprimés ; chaque normalisation est journalisée localement avec le contenu retiré. Elle n'est ni conçue ni présentée comme un moyen de contourner une règle de transparence.

## Licence

À définir par le propriétaire du dépôt (aucune licence appliquée à ce stade : tous droits réservés par défaut).
