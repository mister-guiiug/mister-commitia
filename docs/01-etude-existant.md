# 1. Étude de l'existant

Recherche menée le **2026-07-24** (sources officielles citées ; détail des capacités vérifié outil par outil). Convention : **[Vérifié]** = constaté sur la documentation/le dépôt officiel à cette date ; « rien trouvé » signifie qu'une recherche active n'a rien donné — cela ne prouve pas l'inexistence.

Légende capacités : **RI** rebase interactif en GUI · **RM** réécriture de messages en masse · **CC** Conventional Commits · **IA** génération IA de messages · **CI** nettoyage de runs CI/CD.

## 1.1 Moteurs de réécriture d'historique (CLI)

| Outil | Licence | Capacités | Pertinence pour le besoin |
|---|---|---|---|
| **git rebase -i** (natif) | GPLv2 | squash/reword/reorder/drop sur un segment | Le moteur de référence ; pilotable de façon scriptée via `GIT_SEQUENCE_EDITOR` [Vérifié — git-scm.com/docs/git-var] |
| **git-filter-repo** | GPL-2.0 (mentions MIT dans le dépôt) | **RM : oui** — `--message-callback` réécrit les messages de commits et de tags (10 callbacks disponibles) [Vérifié — github.com/newren/git-filter-repo] | Référence pour la réécriture massive de tout un historique ; aucun UI, aucune IA |
| **BFG Repo-Cleaner** | GPLv3+ | Blobs/fichiers uniquement (gros fichiers, secrets) ; **ne réécrit pas les messages** [Vérifié] | Hors périmètre messages |

## 1.2 Clients Git desktop (GUI/TUI)

| Outil | Modèle | Plateformes | RI | IA | Éléments notables |
|---|---|---|---|---|---|
| **GitKraken Desktop** | Propriétaire, freemium (IA sur plans payants) | Win/mac/Linux | ✔ drag&drop | ✔ messages + **Commit Composer** | Voir §1.6 — l'outil le plus proche du besoin de réécriture assistée |
| **GitButler** | **FSL-1.1-MIT** (usage/modif permis, « Competing Use » interdit, bascule MIT 2 ans après chaque release) | mac/Win/Linux | ✔ drag&drop (uncommit, reword, amend, move, split, squash) | ✔ messages/branches/PR | Stack **Tauri + Rust + Svelte** — valide la faisabilité de la stack pressentie [Vérifié — github.com/gitbutlerapp/gitbutler] |
| Fork | Propriétaire (59,99 $) | mac/Win | ✔ | — | |
| Tower | Propriétaire (abonnement) | mac/Win | ✔ drag&drop | — | |
| SmartGit | Propriétaire (gratuit non commercial) | Win/mac/Linux | ✔ | — | |
| Sourcetree | Propriétaire gratuit (Atlassian) | Win/mac | ✔ | — | |
| Sublime Merge | Propriétaire | Win/mac/Linux | ✔ (« Edit Commit ») | — | |
| lazygit | MIT (TUI) | Win/mac/Linux | ✔ (TUI) | — | |
| Git Extensions | GPLv3 | Windows | non vérifié | — | |

## 1.3 Convention et qualité des messages

| Outil | Licence | Capacité vérifiée |
|---|---|---|
| **commitlint** | MIT | Lint Conventional Commits (`@commitlint/config-conventional`) en hook/CI |
| **gitlint** | MIT | Lint avec **règles custom en Python** ; règle contrib `CT1` pour Conventional Commits |
| **commitizen (cz-cli)** | MIT | Assistant interactif de rédaction conforme |
| **commitizen-tools** (Python) | MIT | `cz check` (validation), `cz bump`, changelog |
| **cocogitto** | MIT (Rust) | `cog check` (conformité), bump, changelog, hooks |
| **git-cliff** | Apache-2.0 OU MIT (Rust) | Changelog depuis les conventional commits |

Tous agissent **au moment du commit ou en validation** — aucun ne réécrit l'historique existant.

## 1.4 Génération IA de messages (au moment du commit)

| Outil | Licence | Capacité vérifiée |
|---|---|---|
| **OpenCommit** | MIT | Message généré depuis le diff staged ; providers OpenAI/Anthropic/Azure/Ollama/…; format conventional |
| **aicommits** | MIT | Idem, multi-providers dont Ollama et endpoints compatibles OpenAI |
| **GitHub Copilot** (VS Code) | Service propriétaire | Génération de messages de commit et descriptions de PR dans la vue Source Control [Vérifié — code.visualstudio.com] |

Même limite : ces outils créent des messages **pour les nouveaux commits** ; ils ne retraitent pas l'historique.

## 1.5 Nettoyage CI/CD

| Outil | Statut | Capacité |
|---|---|---|
| **gh CLI `gh run delete`** | [Vérifié — cli.github.com] | Suppression **unitaire** d'un run GitHub Actions (pas de masse native) |
| **Action `Mattraks/delete-workflow-runs`** | [Vérifié — 249 ⭐] | Nettoyage par rétention (`retain_days`, `keep_minimum_runs`, filtres, dry-run) — s'exécute **dans** GitHub Actions, côté serveur |
| **`github-workflow-runs-cleaner`** (simbo) | [Vérifié — 3 ⭐] | CLI npm dédiée au bulk delete (support GitHub Enterprise) — adoption quasi nulle |
| **az CLI (extension azure-devops)** | [Vérifié] | **Aucune commande de suppression de build/run n'existe** (`az pipelines runs` : list/show/artifact/tag ; `az pipelines build` : cancel/list/queue/show/tag). Feature request ouverte depuis 2021 (Azure/azure-cli#17968) |
| Rétention native des plateformes | [Vérifié] | GitHub : rétention logs/artifacts 90 j par défaut (1-400 j) ; Azure DevOps : politiques de rétention projet + **retention leases** ; suppression de runs AzDO **uniquement via l'API REST Builds–Delete** |
| Extensions Marketplace AzDO (« Build Cleanup », « Post Build Cleanup ») | [Vérifié] | Nettoient les répertoires de build **sur les agents**, pas les runs — hors sujet |

## 1.6 Recherches ciblées sur le besoin exact

**A. Application desktop de réécriture d'historique assistée par IA avec validation humaine — 1 produit trouvé.**
**GitKraken Commit Composer** (GitKraken Desktop 11.3, 06/08/2025) : l'IA analyse des changements ou une série de commits existants et propose une restructuration (squash, réorganisation, messages régénérés) avec prévisualisation et validation avant application — l'historique reste intact jusqu'à validation. Propriétaire ; fonctions IA réservées aux plans payants. [Vérifié — gitkraken.com/blog]
Équivalents **CLI uniquement** : `f/git-rewrite-commits` (réécriture batch des messages via Ollama/GPT), `can1357/llm-git` (réécriture via `git commit-tree` en préservant arbres/auteurs/dates). Nichés, sans UI, sans gouvernance.

**B. Outil dédié au nettoyage en masse des runs GitHub Actions** (hors scripts/actions) : un seul trouvé, marginal (`github-workflow-runs-cleaner`, 3 ⭐).

**C. Outil de nettoyage des runs/pipelines Azure DevOps : rien trouvé.** L'écosystème = rétention native + API REST + scripts maison.

**D. Outil détectant/supprimant les signatures d'assistants IA dans les messages : rien trouvé.** Ce qui existe : la **prévention** à la source (configuration d'attribution de l'outil assistant, hooks `commit-msg`/`prepare-commit-msg`) et la réécriture générique a posteriori (`git filter-repo --message-callback`). Aucun produit packagé dédié à la détection/normalisation gouvernée.

## 1.7 Synthèse factuelle

1. Chaque **brique** du besoin existe isolément et est mature : moteurs de réécriture (rebase/filter-repo), GUIs de rebase interactif, linters de convention, générateurs IA de messages, endpoints de suppression de runs.
2. **Un seul produit** combine IA + restructuration d'historique + validation humaine en desktop (GitKraken Commit Composer) — propriétaire, sans nettoyage CI/CD Azure DevOps, sans skills gouvernées, sans plan reproductible exportable.
3. **Personne** ne couvre : la combinaison Git + CI/CD multi-plateformes (GitHub Enterprise **et** Azure DevOps), les politiques de rétention simulées côté client, la gouvernance de skills versionnables, la détection normée des signatures d'outils IA.
4. Azure DevOps est le parent pauvre de l'outillage de nettoyage (pas même une commande CLI officielle de suppression).
