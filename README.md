# mister-commitia

Étude d'opportunité et d'architecture pour une **application desktop d'assainissement gouverné d'historiques Git et d'exécutions CI/CD** (GitHub Enterprise / Azure DevOps) : analyse de dépôts, réécriture **contrôlée** de l'historique des commits (reword, squash, reorder, drop) assistée par un agent IA à skills configurables, et politiques de rétention CI/CD simulées puis validées.

> **Statut : MVP en développement** (étude livrée le 2026-07-24, développement lancé le même jour selon le [backlog MVP](docs/09-backlog-mvp.md)). Voir l'[avancement](#avancement-du-mvp) ci-dessous. Recherche documentaire effectuée le **2026-07-24** (sources officielles citées dans chaque document).

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
| 16 | [Améliorations proposées](docs/16-ameliorations.md) | Post-MVP : fonctionnel, technique, harmonisation UX/UI, priorisation |

Les six skills attendues sont fournies en exemples exécutables dans [`skills/`](skills/) (`conventional-commits` sert de référence complète : manifeste + prompt + tests).

## Avancement du MVP

Architecture réalisée conformément à l'[architecture cible](docs/05-architecture-cible.md) : cœur **Rust** réutilisable [`crates/mc-core`](crates/mc-core) + application **Tauri v2 / React** [`apps/desktop`](apps/desktop). Les six skills de [`skills/`](skills/) sont chargées telles quelles par le moteur (YAML + prompts + tests).

| Epic | État | Détail |
|---|---|---|
| E1 Socle & workspace | ✅ | Déclaration multi-dépôts, SQLite local, mode offline complet |
| E2 Analyse d'historique | ✅ | Segment merge-base..tip (ou **base au choix** — branche/tag/SHA validé ancêtre, F6), heuristiques (faible/conforme/mentions générées/doublons), fichiers & trailers & signatures & partage par commit ; **vue graphe SVG** (lanes calculées par le cœur, merges visibles, bornes hors-segment) à côté de la liste |
| E3 Moteur de plan | ✅ | Reword pur git2 (arbres garantis intacts, **y compris à travers un merge** via `reword_dag`) + squash/fixup/drop/reorder via le **sequencer natif de Git** (rapport de conflits par fichier), **changements de structure à travers un merge préservé** (`--rebase-merges`) ; dry-run **réel** dans `refs/mc/preview/*` ; backup réf+tag ; apply par bascule ; rollback ; export/import JSON ; empreinte anti-dérive ; **push assisté** (`--force-with-lease` guidé, checklist, PR ouvertes, branche protégée refusée) |
| E4 Agent IA & skills | ✅ | Chargement des skills YAML, **garde-fous vérifiés par l'application** (post-conditions), providers Ollama / endpoint compatible OpenAI / Anthropic + **assistant local déterministe** (repli hors-ligne), consentement avec aperçu avant tout envoi distant, runner de self-tests ; réponses **streamées** (SSE/NDJSON) avec réessais automatiques (backoff, `Retry-After`) et budget de tokens par lot |
| E5 Sécurité & secrets | ✅ | Coffre OS via crate `keyring` (backend mémoire pour tests/CI), scopes affichés avant enregistrement, redaction systématique des sorties |
| E6 CI/CD | ✅ | Clients GitHub Actions & Azure DevOps Builds (pagination, continuation token, 429/Retry-After, **api-version AzDO négociée 7.1→7.0** pour Server on-prem), inventaire avec leases, politique + **simulation obligatoire**, suppression unitaire à double confirmation, **suppression en masse** (throttling, reprise sur checkpoint, annulable), **purge des logs/artefacts** (reclaim de stockage en conservant les runs, GitHub), revérification des leases avant tout DELETE ; PR ouvertes détectées (push assisté) |
| E7 Journal & audit | ✅ | Append-only SQLite, journalisation **avant** chaque suppression, export JSONL |
| E8 Packaging | ✅ | CI GitHub Actions : tests cœur Linux/Windows, compilation `mc-desktop` (MSVC) à chaque push, et sur tag/dispatch : **MSI + installeur NSIS + version portable** (zip exe+skills, sans installation ni droits admin — données déportables via `MC_DATA_DIR`, tokens toujours au coffre Windows). Signature des binaires : V2 |

Les opérations longues (analyse, dry-run, application, inventaire/simulation CI, génération IA) émettent leur **progression** sur un canal d'événements unique et s'**annulent** proprement : arrêt coopératif aux points sûrs uniquement, points de non-retour (backup puis bascule) jamais interrompus.

**Vérification** : 62 tests cœur (dont **proptest** sur le compilateur de plan) sur dépôts Git synthétiques couvrant les critères CA-1 → CA-14 ([détail](docs/14-criteres-acceptation.md)) + **couverture** `cargo-llvm-cov` (gate plan-engine ≥ 80 %) et **SBOM** CycloneDX en CI ; **E2E de l'interface** (Playwright, mode démonstration) à chaque push, plus un harnais **desktop natif** tauri-driver ([`e2e-native/`](apps/desktop/e2e-native), expérimental) ; UI vérifiée en mode navigateur (mock IPC intégré : `npm run dev` dans `apps/desktop` sans Tauri). Thème clair/sombre, **i18n FR/EN du corps des pages** (bascule réactive), raccourcis clavier (`?`).

**Reste pour clore le MVP** (voir [backlog](docs/09-backlog-mvp.md)) : packaging signé (SmartScreen), stabilisation du job E2E desktop natif sur le runner, documentation utilisateur de prise en main.

### Développer

```bash
# Cœur (tests) — nécessite git dans le PATH
cargo test -p mc-core

# UI seule dans un navigateur (mock, aucune écriture réelle)
cd apps/desktop && npm install && npm run dev

# Application desktop complète
cd apps/desktop && npm run tauri dev
```

### Publier une release

```bash
# la version de apps/desktop/src-tauri/tauri.conf.json doit correspondre au tag
git tag -a v0.1.0 -m "mister-commitia 0.1.0"
git push origin v0.1.0
```

Le workflow [Release](.github/workflows/release.yml) exécute alors : tests du cœur (gate), contrôle de cohérence tag ↔ version, bundles Windows (**MSI, installeur NSIS, zip portable**), `SHA256SUMS.txt`, puis crée la **GitHub Release** avec notes générées automatiquement.

**Toolchain Windows** : les bundles officiels sont construits par la CI (windows-latest, MSVC). En local, MSVC Build Tools est la voie recommandée pour `mc-desktop`. Sans MSVC, l'hôte `x86_64-pc-windows-gnu` + llvm-mingw suffit pour **développer et tester `mc-core`** (validé : 58 tests — recette outillée : `. .\scripts\dev-env.ps1`) ; la compilation du binaire Tauri en gnu peut échouer selon l'environnement (build scripts volumineux — constaté sur poste durci EDR).

## Conventions de lecture

- **[Vérifié]** : constaté le 2026-07-24 sur une source officielle citée (documentation éditeur, dépôt officiel).
- **[Supposé]** : connaissance ou estimation non re-vérifiée, signalée comme telle.
- **[Recommandé]** : choix de conception proposé, à valider.
- « **Je ne sais pas** » : information recherchée mais non vérifiable (ex. code HTTP exact d'une suppression de build sous retention lease).

## Note de gouvernance

La fonctionnalité de normalisation des messages — y compris la détection des mentions ajoutées automatiquement par des outils d'assistance (« Generated with Claude Code », liens de session, signatures d'assistants) — est conçue comme une **normalisation configurable soumise aux règles de gouvernance du dépôt** : si la politique du dépôt impose la traçabilité des contributions assistées par IA, l'application refuse la suppression et l'explique ; les trailers d'audit et de conformité (`Signed-off-by`, etc.) ne sont jamais supprimés ; chaque normalisation est journalisée localement avec le contenu retiré. Elle n'est ni conçue ni présentée comme un moyen de contourner une règle de transparence.

## Licence

Distribué sous licence [MIT](LICENSE).
