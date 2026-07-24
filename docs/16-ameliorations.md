# 16. Améliorations proposées (post-MVP)

Statut : **[Recommandé]** — propositions issues du développement du MVP (2026-07-24), classées par nature puis priorisées. Effort : S (< 1 j) · M (1-3 j) · L (> 3 j).

## 16.1 Fonctionnelles

| # | Proposition | Effort | Notes |
|---|---|---|---|
| F1 | **Vue graphe Git réelle** (lanes SVG, merges visibles) à côté de la vue liste | M | Promise de l'étude (« vue graphe ») ; la liste ordonnée actuelle suffit pour des branches linéaires mais pas pour visualiser le contexte |
| F2 | **Réordonnancement par glisser-déposer** dans le composeur de plan | M | L'opération `reorder` existe et est testée côté cœur mais **n'est pas exposée dans l'UI** ; idem `fixup` |
| F3 | **Diff de contenu par commit** (viewer unifié, tronqué au-delà d'un seuil) | M | Aujourd'hui : stats seulement ; nécessite une commande IPC `commit_diff(sha)` |
| F4 | **Push assisté post-application** : `--force-with-lease` guidé, checklist de coordination, détection des PR ouvertes via l'API | M | Backlog V2-GIT-5 ; complète le parcours P1 jusqu'au bout |
| F5 | **Import de plan dans l'UI** | S | `plan_import` existe (cœur + commande IPC) sans bouton ; le pendant de l'export |
| F6 | **Choix de la base du segment** (autre branche/сommit que le merge-base auto) | S | Utile pour branches empilées |
| F7 | **Nettoyage CI en masse** : exécution par lot du rapport de simulation avec throttling adaptatif, reprise sur checkpoint, puis logs/artifacts GitHub | L | Backlog V2-CI-1/4 ; le cœur gère déjà 429/Retry-After à l'unité |
| F8 | **Éditeur de skills intégré** : édition YAML avec validation JSON-Schema live, exécution des self-tests, diff avant/après publication | M | La page Skills est en lecture seule ; le runner existe |
| F9 | **Rapport HTML exportable** d'un plan (avant/après, risques, mapping) pour revue d'équipe hors outil | S | Simple templating depuis `plan.json` |
| F10 | **Onboarding premier lancement** : déclarer un dépôt, choisir IA locale/distante, expliquer les garde-fous | S | Réduit la marche d'entrée |
| F11 | i18n FR/EN | M | Chaînes à externaliser (UI actuellement 100 % FR en dur) |
| F12 | Négociation `api-version` Azure DevOps Server (7.1 → 7.0) effective + test on-prem | S | Le connecteur envoie 7.1 fixe |

## 16.2 Techniques

| # | Proposition | Effort | Notes |
|---|---|---|---|
| T1 | **Erreurs IPC typées** `{code, message}` au lieu de chaînes | S | Point faible actuel : l'UI détecte `message.includes("consentement")` pour ouvrir le dialogue — fragile et lié à la langue ; codes attendus : `consent_required`, `confirm_required`, `rate_limited{retry}`, `refused`… |
| T2 | **Progression + annulation** des opérations longues (dry-run, inventaire) via events/channels Tauri + `spawn_blocking` pour ne pas occuper les workers | M | Aujourd'hui : spinner booléen, pas d'annulation |
| T3 | **Migrations SQLite versionnées** (`schema_version`) | S | Le schéma v1 en `CREATE IF NOT EXISTS` ne permettra pas d'évoluer proprement |
| T4 | **Journalisation structurée** (`tracing`) avec redaction branchée sur le subscriber + niveau debug activable dans l'UI | S | La redaction existe, les logs techniques non |
| T5 | **CI qualité** : clippy + rustfmt (`--check`), `cargo-deny` (licences/advisories), couverture `cargo-llvm-cov` avec gate ≥ 80 % sur plan-engine (DoD §9.2) | S | rustfmt/clippy absents du poste local (profil minimal) mais triviaux en CI |
| T6 | **Tests de propriétés** (proptest) sur `compile()` du plan : séquences d'opérations aléatoires → invariants (§15.4) | M | Complète les 30 tests exemple-par-exemple |
| T7 | **E2E desktop** via tauri-driver/WebDriver sur le runner Windows | L | Dernier trou de la stratégie de tests MVP |
| T8 | **Signature des binaires** (Azure Trusted Signing ou certificat OV) + `tauri-plugin-updater` signé | M | Bundles actuellement non signés → SmartScreen ; prérequis à une distribution large |
| T9 | Durcir la **CSP** (supprimer `unsafe-inline` styles) et ajouter un audit `cargo auditable`/SBOM | S | Aligné avec la pratique mister-doc (CSP hash) |
| T10 | **Merges dans le segment** : support `--rebase-merges` + rapport de conflits par fichier | L | MVP refuse les merges (garde-fou explicite) ; V2-GIT-3 |
| T11 | **Streaming des réponses IA** + retry/backoff + budget de tokens par lot | M | UX d'attente sur groupes nombreux |
| T12 | Script `scripts/dev-env.ps1` qui configure la toolchain windows-gnu locale (PATH/CC/AR, recette documentée) | S | Pour contributeurs sans MSVC ; la recette existe en mémoire de session, pas dans le dépôt |
| T13 | Cache d'analyse incrémental par SHA + virtualisation des longues listes | M | Confort au-delà de ~500 commits (cap actuel) |

## 16.3 Harmonisation UX/UI

Constats sur l'UI MVP (volontairement spartiate) et remèdes :

| # | Proposition | Effort | Constat actuel |
|---|---|---|---|
| U1 | **Design tokens centralisés** : palette sémantique unique (teal=action, rose=destructif, amber=attention, sky=information, violet=IA), échelles d'espacement et de tailles d'icônes (14/16/20) | S | Classes Tailwind ad hoc répétées ; tailles d'icônes 13→16 au hasard |
| U2 | **Mode clair + bascule persistée** | M | `color-scheme: dark` forcé ; thème sombre uniquement |
| U3 | **Modal unique accessible** (focus-trap, Échap, aria) réutilisé partout | S | Deux modals maison divergents (consentement IA, suppression CI) ; pattern `ConfirmProvider/useConfirm` déjà éprouvé sur mister-doc |
| U4 | **Composant « confirmation par saisie du nom »** unifié | S | Deux implémentations différentes (input inline pour la branche partagée, modal pour le run CI) pour le même concept de sécurité |
| U5 | **Toasts non bloquants** (succès/erreur) + états de chargement homogènes (boutons `loading`, skeletons) | S | Les succès sont silencieux ; `busy` booléen unique par page |
| U6 | **En-tête global contextuel** : repo/branche sélectionnés visibles partout, sélection de dépôt depuis la barre latérale | M | Le contexte est répété ou absent selon la page ; l'état des pages se perd au changement d'onglet (montage/démontage) → conserver l'état par page |
| U7 | **Tables harmonisées** : hover, tri (date/taille/statut), SHA en `font-mono` partout, troncature avec infobulle | S | Tables hétérogènes entre Analyse, CI et Journal |
| U8 | **Échelle de verdicts unique** ok/attention/bloquant réutilisée par le panneau risques ET le rapport CI (protégés = même sémantique ambre) avec légende commune | S | Deux vocabulaifes visuels pour la même idée |
| U9 | **Accessibilité** : aria-labels sur les cases de sélection de commits, focus visible, contraste des textes `slate-500` relevé, navigation clavier des listes, `prefers-reduced-motion` | M | Aucun aria sur les checkboxes ; contrastes limites |
| U10 | **États vides actionnables** : chaque Empty propose l'action suivante (ex. page CI vide → « Ajouter un accès ») ; erreurs réseau avec « Réessayer » | S | Empty informatifs sans action |
| U11 | **Lexique FR harmonisé** : run/exécution, dépôt/repo, capitalisation des boutons, espaces insécables avant « : » et « ? » | S | Mélanges ponctuels |
| U12 | **Raccourcis clavier** (actualiser, dry-run, naviguer entre onglets) + affichage `?` | M | Aucun raccourci |

## 16.4 Priorisation proposée

1. **Quick wins immédiats (≈ 1 semaine)** : T1 (erreurs typées — débloque U3/U4 proprement), U1, U3, U4, U5, U7, U8, U10, U11, F5, T3, T5, T12.
2. **Structurants (≈ 1 sprint)** : F2 (reorder UI), F3 (diff), T2 (progression/annulation), U6, U9, F8, F9, F10, T4, T11.
3. **Ambitieux / V2** : F1 (graphe), F4 (push assisté), F7 (masse CI), T6, T7 (E2E), T8 (signature), T10 (merges), U2, F11, T13.

Le fil conducteur : d'abord fiabiliser le contrat UI↔cœur (T1) et unifier les primitives (U1/U3/U4/U5), ensuite enrichir les parcours (reorder, diff, push), enfin ouvrir les chantiers V2 déjà cadrés au [backlog](10-backlog-v2.md).
