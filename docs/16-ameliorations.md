# 16. Améliorations proposées (post-MVP)

Statut : **[Recommandé]** — propositions issues du développement du MVP (2026-07-24), classées par nature puis priorisées. Effort : S (< 1 j) · M (1-3 j) · L (> 3 j).

## 16.1 Fonctionnelles

| # | Proposition | Effort | Notes |
|---|---|---|---|
| F1 | ✅ **Vue graphe Git réelle** (lanes SVG, merges visibles) à côté de la vue liste | M | **Livré (lot 4)** : lanes calculées par le cœur (`graph::build_graph`, pure & testée), bascule Liste/Graphe, SVG (nœuds, arêtes courbes, merges, bornes hors-segment) |
| F2 | ✅ **Réordonnancement par glisser-déposer** dans le composeur de plan | M | **Livré (lot 2)** : DnD + boutons clavier → opération `reorder` du plan |
| F3 | ✅ **Diff de contenu par commit** (viewer unifié, tronqué au-delà d'un seuil) | M | **Livré (lot 2)** : `commit_diff(sha)` + visionneuse colorée (tronquée à 200 Ko) |
| F4 | ✅ **Push assisté post-application** : `--force-with-lease` guidé, checklist de coordination, détection des PR ouvertes via l'API | M | **Livré (lot 4)** : `push_preview`/`push_execute` (bail explicite, garde-fous protégée+confirmation typée, audit avant/après), PR ouvertes via GitHub, panneau UI avec checklist |
| F5 | ✅ **Import de plan dans l'UI** | S | **Livré (lot 1)** : bouton d'import câblé sur `plan_import` |
| F6 | ✅ **Choix de la base du segment** (autre branche/commit que le merge-base auto) | S | **Livré (lot 8)** : `repo_scan_base` (base branche/tag/SHA résolue via revparse, validée ancêtre strict du sommet) + champ « base » dans la page Analyse (« appliquer »/« auto ») |
| F7 | ✅ **Nettoyage CI en masse** : exécution par lot du rapport de simulation avec throttling, reprise sur checkpoint | L | **Livré (lot 7)** : `ci_delete_batch` (mêmes garde-fous qu'à l'unité, 429/Retry-After → attente annulable, checkpoint des `run_id` pour reprise, progression, audit par run) + UI (bouton « Tout supprimer (N) »/« Reprendre », confirmation par le nombre). **Purge logs/artefacts livrée (lot 8)** : `ci_purge_assets` (conserve les runs, GitHub) + action UI « Purger logs + artefacts » |
| F8 | ✅ **Éditeur de skills intégré** : édition YAML, validation à l'enregistrement, self-tests | M | **Livré (lot 2)** : édition du manifeste (`name` immuable, anti-traversée), éditions journalisées |
| F9 | ✅ **Rapport HTML exportable** d'un plan (avant/après, risques, mapping) pour revue d'équipe hors outil | S | **Livré (lot 2)** : export HTML autonome depuis le plan |
| F10 | ✅ **Onboarding premier lancement** : déclarer un dépôt, choisir IA locale/distante, expliquer les garde-fous | S | **Livré (lot 2)** : accueil 3 étapes (localStorage) |
| F11 | ✅ i18n FR/EN | M | **Livré (lots 5 & 8)** : scaffold `t()` + dictionnaire (~180 clés), langue persistée, bascule réactive (`useLang`) ; **corps de page externalisé** — Réglages/Dépôts/Journal/Skills intégralement, CI/Analyse pour tout le chrome (titres, libellés, options, boutons, en-têtes, états vides, titres de modales). Résidu : quelques toasts transitoires interpolés et longues descriptions de modales CI/Analyse restent en FR |
| F12 | ✅ Négociation `api-version` Azure DevOps Server (7.1 → 7.0) effective + test on-prem | S | **Livré (lot 8)** : helper `send` centralisé qui tente 7.1 puis se rabat sur 7.0 sur un 400 « version hors plage » (mémorisé) ; reprise sûre même en DELETE (rejet = rien exécuté) ; test mockhttp |

## 16.2 Techniques

| # | Proposition | Effort | Notes |
|---|---|---|---|
| T1 | ✅ **Erreurs IPC typées** `{code, message, expected}` au lieu de chaînes | S | **Livré (lot 1)** : l'UI se branche sur `consent_required`/`confirm_required`/`rate_limited`/`refused`, jamais sur le texte |
| T2 | ✅ **Progression + annulation** des opérations longues via events/channels Tauri + `spawn_blocking` | M | **Livré (lots 2 & 3)** : canal `mc://task`, annulation coopérative (`cancelled`), points de non-retour préservés |
| T3 | ✅ **Migrations SQLite versionnées** (`schema_version`) | S | **Livré (lot 1)** : runner transactionnel `MIGRATIONS` |
| T4 | ✅ **Journalisation structurée** (`tracing`) avec redaction branchée sur le subscriber, niveau via `MC_LOG` | S | **Livré (lot 2)** : writer fichier redacté avant écriture |
| T5 | ✅ **CI qualité** : clippy + rustfmt (`--check`), `cargo-deny` (advisories), couverture `cargo-llvm-cov` avec gate ≥ 80 % sur plan-engine (DoD §9.2) | S | **Livré** (lots 1 & 6) : fmt/clippy/advisories + **job `couverture`** gate plan.rs ≥ 80 % (mesuré : **87,7 %** ; global mc-core 80,5 %) |
| T6 | ✅ **Tests de propriétés** (proptest) sur `compile()` du plan : séquences d'opérations aléatoires → invariants (§15.4) | M | **Livré (lot 5)** : 400 cas — jamais de panic, invariants leaders/commits/reword-only |
| T7 | ✅ **E2E desktop** via tauri-driver/WebDriver sur le runner Windows | L | **Livré (lot 5)** : E2E Playwright (web/mock, job CI à chaque push) + harnais natif tauri-driver (`e2e-native/`, job `workflow_dispatch` expérimental) |
| T8 | ✅ **Signature des binaires** (Azure Trusted Signing ou certificat OV) | M | **Livré partiel (lot 7)** : étape de signature Authenticode dans `release.yml` conditionnée à un secret certificat (signe MSI/NSIS/exe + horodatage), sinon avertissement « non signé ». Non vérifiable sans certificat ; `tauri-plugin-updater` signé : reporté (keypair) |
| T9 | ✅ Durcir la **CSP** + SBOM | S | **Livré (lot 7)** : CSP durcie (`object-src 'none'`, `base-uri 'self'`, `frame-src/frame-ancestors 'none'`, `script-src 'self'`) — `style-src 'unsafe-inline'` conservé (styles React inline) ; **SBOM CycloneDX** généré en CI (artefact) |
| T10 | ✅ **Merges dans le segment** (partiel) : rapport de conflits par fichier | L | **Livré partiel (lot 5)** : `reword_dag` réécrit les MESSAGES à travers un merge (topologie/arbres préservés) ; le sequencer liste les fichiers en conflit. Changements de structure à travers un merge : toujours refusés (sûreté). `--rebase-merges` complet : reporté |
| T11 | ✅ **Streaming des réponses IA** + retry/backoff + budget de tokens par lot | M | **Livré (lot 3)** : SSE/NDJSON relayés en direct, backoff plafonné `Retry-After`, budget 256..1024/groupe |
| T12 | ✅ Script `scripts/dev-env.ps1` qui configure la toolchain windows-gnu locale (PATH/CC/AR, recette documentée) | S | **Livré (lot 1)** : recette versionnée (BOM UTF-8 + PATH `dlltool-only`) |
| T13 | ✅ Cache d'analyse incrémental par SHA + virtualisation des longues listes | M | **Livré (lot 5)** : cache `CommitInfo` par SHA (`on_remote` recalculé hors cache) ; virtualisation de la vue graphe au-delà de 150 commits |

## 16.3 Harmonisation UX/UI

Constats sur l'UI MVP (volontairement spartiate) et remèdes :

| # | Proposition | Effort | Constat actuel |
|---|---|---|---|
| U1 | ✅ **Design tokens centralisés** : palette sémantique (teal=action, rose=destructif, amber=attention, sky=information, violet=IA), échelles d'icônes | S | **Livré (lot 1)** : tokens dans `ui.tsx` (ICON_SM/MD, badgeTones, classes partagées) |
| U2 | ✅ **Mode clair + bascule persistée** | M | **Livré (lot 5)** : thème sombre/clair/système persisté ; le clair inverse l'échelle `slate` via les variables CSS de Tailwind 4 (tout le chrome bascule) |
| U3 | ✅ **Modal unique accessible** (focus-trap, Échap, aria) réutilisé partout | S | **Livré (lot 1)** : `Modal` (role dialog, aria-modal, piège de focus) |
| U4 | ✅ **Composant « confirmation par saisie du nom »** unifié | S | **Livré (lot 1)** : `ConfirmTyped` piloté par le cœur (`confirm_required`) |
| U5 | ✅ **Toasts non bloquants** (succès/erreur) + états de chargement homogènes (boutons `loading`) | S | **Livré (lot 1)** : `ToastProvider` aria-live + boutons `loading` |
| U6 | ✅ **En-tête global contextuel** : repo/branche visibles partout, état des pages conservé | M | **Livré (lot 2)** : en-tête global + pages montées en permanence (état conservé entre onglets) |
| U7 | ✅ **Tables harmonisées** : hover, tri, SHA en `font-mono` partout, troncature avec infobulle | S | **Livré** (lots 1 & 6) : classes partagées, hover, SHA mono ; **tri clic-colonne** sur candidats CI **et Journal** |
| U8 | ✅ **Échelle de verdicts unique** ok/attention/bloquant (panneau risques ET rapport CI) avec légende commune | S | **Livré (lot 1)** : `VerdictBadge` + `VerdictLegend` partagés |
| U9 | ✅ **Accessibilité** : aria-labels sur les cases de commits, focus visible, navigation clavier, `prefers-reduced-motion` | M | **Livré (lot 2)** : focus-visible, aria-labels/aria-current, mouvement réduit respecté |
| U10 | ✅ **États vides actionnables** : chaque `Empty` propose l'action suivante | S | **Livré (lot 1)** : `Empty` avec `actionLabel`/`onAction` |
| U11 | ✅ **Lexique FR harmonisé** : capitalisation des boutons, espaces insécables avant « : » et « ? » | S | **Livré** (lots 1 & 6) : libellés principaux + passe sur les écrans secondaires ; plus aucune violation d'espace insécable dans le texte visible |
| U12 | ✅ **Raccourcis clavier** (naviguer entre onglets) + affichage `?` | M | **Livré (lot 7)** : `1`–`6` changent d'onglet, `?` ouvre l'aide des raccourcis, `Échap` ferme (ignorés dans les champs de saisie) |

## 16.4 Priorisation proposée

1. **Quick wins immédiats (≈ 1 semaine)** — ✅ **tous livrés** : ~~T1~~, ~~U1~~, ~~U3~~, ~~U4~~, ~~U5~~, ~~U7~~, ~~U8~~, ~~U10~~, ~~U11~~, ~~F5~~, ~~T3~~, ~~T5~~ (couverture incluse), ~~T12~~.
2. **Structurants (≈ 1 sprint)** — ✅ **tous livrés** : ~~F2 (reorder UI)~~, ~~F3 (diff)~~, ~~T2 (progression/annulation)~~, ~~U6~~, ~~U9~~, ~~F8~~, ~~F9~~, ~~F10~~, ~~T4~~, ~~T11~~.
3. **Ambitieux / V2** — ✅ **livrés** : ~~F1 (graphe)~~, ~~F4 (push assisté)~~, ~~T6~~, ~~T7 (E2E)~~, ~~T10 (merges, partiel)~~, ~~T13~~, ~~U2~~, ~~F11 (i18n)~~, ~~F7 (masse CI + purge)~~, ~~T8 (signature, scaffold)~~, ~~T9 (CSP/SBOM)~~, ~~U12 (raccourcis)~~, ~~F6 (base du segment)~~, ~~F12 (api-version AzDO)~~ · **restant (2 items)** : **T10 complet** (`--rebase-merges` : réécrire la *structure* à travers un merge, pas seulement les messages) et **`tauri-plugin-updater` signé** (auto-update ; nécessite un keypair minisign).

Le fil conducteur : d'abord fiabiliser le contrat UI↔cœur (T1) et unifier les primitives (U1/U3/U4/U5), ensuite enrichir les parcours (reorder, diff, push), enfin ouvrir les chantiers V2 déjà cadrés au [backlog](10-backlog-v2.md).

> **Lot 1 livré le 2026-07-24** : T1 (erreurs `{code, message, expected}`, l'UI se branche sur
> `consent_required`/`confirm_required`), T3 (migrations `schema_version`), T5 (fmt + clippy
> `-D warnings` + advisories RustSec en CI — couverture : reportée), T12 (`scripts/dev-env.ps1`),
> U1 (tokens centralisés dans `ui.tsx`), U3 (Modal accessible unique : focus-trap, Échap,
> aria-modal), U4 (`ConfirmTyped` unifié, piloté par le cœur), U5 (toasts aria-live + boutons
> `loading`), U7 (classes de tables partagées, SHA mono, hover — tri livré sur les candidats CI),
> U8 (`VerdictBadge` + légende, réutilisés par le rapport CI), U10 (états vides actionnables),
> U11 (partiel : espaces insécables et libellés principaux), F5 (import de plan dans l'UI).
> Vérifié : 31 tests cœur verts, parcours navigateur (confirmation renforcée, consentement,
> toasts) rejoués.

> **Lot 2 livré le 2026-07-24** : F2 (réordonnancement drag & drop **et** boutons clavier
> accessibles → op `reorder` du plan), F3 (`commit_diff` + visionneuse colorée), F8 (éditeur
> YAML des skills — validation à l'enregistrement, `name` immuable, éditions journalisées),
> F9 (rapport HTML autonome du plan), F10 (onboarding premier lancement), U6 (en-tête
> contextuel global + pages montées en permanence = état conservé entre onglets), U9
> (focus-visible, `prefers-reduced-motion`, aria-labels/aria-current), T4 (tracing avec
> writer fichier **redacté**, niveau via `MC_LOG`), T2 **partiel** (scan/dry-run/apply/rollback
> en `spawn_blocking` : UI réactive — progression + annulation reportées).
> **Reportés au lot suivant** : T11 (streaming IA, retry/backoff) + progression/annulation
> des opérations longues (un même chantier événementiel).
> Vérifié : 35 tests cœur verts (dont reorder et éditeur), parcours navigateur rejoués
> (onboarding, diff, réordonnancement, édition de skill persistée + auditée).

> **Lot 3 livré le 2026-07-25** (le chantier événementiel) : T2 **complet** — progression
> des opérations longues émise sur un canal unique `mc://task` (scan par commit lu, dry-run
> par étape — empreinte, sequencer, réécriture des messages, invariants —, application,
> inventaire et simulation CI par page d'API) et **annulation coopérative** par jeton
> (`task_cancel`, code d'erreur stable `cancelled`, arrêt aux points sûrs uniquement ;
> les points de non-retour — backup puis bascule — ne sont jamais interrompus et le
> sequencer git est tué proprement avec nettoyage du worktree). T11 — **streaming des
> réponses IA** (SSE OpenAI-compatible et Anthropic, NDJSON Ollama, fragments relayés en
> direct dans l'UI), **réessais automatiques** avec backoff exponentiel plafonné par
> `Retry-After` (429/5xx/réseau, 3 essais), **budget de tokens par lot** réparti entre
> les groupes (borné [256, 1024]).
> Vérifié : 47 tests cœur verts (annulation sans effet de bord — préview jamais créée,
> pagination interrompue sans requête suivante —, 3 protocoles de flux, annulation en
> cours de flux, retry 429→200, épuisement 5xx, bout-en-bout SSE→événements de tâche) ;
> parcours navigateur rejoués (barres de progression annulables, flux IA affiché en
> direct, annulation en cours de génération conservant les propositions déjà produites).
> Au passage, `scripts/dev-env.ps1` corrigé : BOM UTF-8 (requis par PowerShell 5.1) et
> PATH minimal `dlltool-only` — exposer tout le `bin` de llvm-mingw masquait le linker
> self-contained de rustup (échec sur `-lgcc`/`-lgcc_eh`).

> **Lot 4 livré le 2026-07-25** (les deux chantiers V2 « graphe + push ») : F1 — **vue graphe
> Git** : `graph::build_graph` calcule les lanes (fonction pure, du plus récent au plus ancien),
> ajoutée à `ScanResult` ; bascule Liste/Graphe dans l'UI, rendu SVG (nœuds, arêtes courbes
> par lane, merges en anneau, bornes pointillées pour les parents hors segment — la base).
> F4 — **push assisté** : `push_preview` (rafraîchit le remote-tracking, calcule avance/retard
> et le besoin de force, détecte les PR ouvertes via GitHub, avertissements de coordination)
> et `push_execute` (**`--force-with-lease` explicite** sur le SHA distant vu — protège le
> travail non revu ; **refus net sur branche protégée**, **confirmation typée** du nom de
> branche, **journal avant/après**). Panneau UI : divergence, PR ouvertes, checklist, saisie
> de confirmation pour le push forcé.
> Vérifié : 53 tests cœur verts (graphe linéaire = 1 lane + borne, merge interne = lanes
> multiples ; push forcé réussi réécrivant bien le bare, **bail --force-with-lease qui abort
> sans rien écraser quand le remote a bougé**, refus sur branche protégée, détection de PR
> mockée) ; parcours navigateur rejoués (graphe SVG rendu, preview de push avec PR + checklist,
> force-with-lease confirmé par saisie → succès).
> **Reste au backlog V2** : F7 (masse CI), T7 (E2E tauri-driver), T8 (signature), T10 (merges
> réécrivables), U2 (thème clair), F11 (i18n), T6/T13.

> **Lot 5 livré le 2026-07-25** : T6 — **proptest** sur `compile()` (400 cas : jamais de panic ;
> invariants leaders distincts, chaque commit au plus une fois, reword-only ⇒ structure et
> ordre préservés). T13 — **cache d'analyse par SHA** (parties SHA-invariantes de `CommitInfo` ;
> `on_remote`, contextuel, toujours recalculé hors cache) + **virtualisation** de la vue graphe
> au-delà de 150 commits. T10 **partiel** — `reword_dag` généralise la réécriture de messages à
> un DAG : un segment contenant un **merge** est réécrivable (topologie et arbres préservés,
> aucun conflit possible) ; les changements de structure à travers un merge restent refusés
> (sûreté) ; le sequencer **liste les fichiers en conflit**. U2 — **thème clair/sombre/système**
> persisté ; le clair **inverse l'échelle `slate`** via les variables `--color-slate-*` de
> Tailwind 4 (tout le chrome bascule sans réécrire une classe). F11 **partiel** — scaffold i18n
> FR/EN (`t()`, dictionnaire, langue persistée, bascule) câblé sur le shell (en-tête, nav,
> onboarding) ; corps de page à externaliser. T7 — **E2E** : Playwright sur le build web/mock
> (job CI à chaque push : shell, thème, i18n, graphe) + harnais **desktop natif** tauri-driver
> (`e2e-native/`, job `workflow_dispatch` expérimental).
> Vérifié : **57 tests cœur verts** (proptest, reword-merge/refus, cache/on_remote) ; parcours
> navigateur (thème clair inversé + retour sombre, bascule FR↔EN, persistance) ; E2E Playwright
> validés par la CI ubuntu (Chromium bloqué localement par l'EDR, comme le binaire Tauri).
> **Reste au backlog V2** : F7 (masse CI), T8 (signature), T9 (CSP/SBOM), T10 complet
> (`--rebase-merges`), U12 (raccourcis), F11 (corps de page).

> **Lot 6 livré le 2026-07-25** (finalisation des tiers « quick wins » & « structurants ») :
> T5 — **couverture** en CI (`cargo-llvm-cov`), job dédié qui **gate `plan.rs` à ≥ 80 %**
> (mesuré : **87,7 %** ; couverture globale mc-core 80,5 %). U7 — **tri clic-colonne du Journal
> d'audit** (# / catégorie / action / résultat) avec indicateur de sens, harmonisé avec le tri
> des candidats CI. U11 — passe typographique FR (espaces insécables) sur les écrans secondaires
> (Réglages, Dépôts) : plus aucune violation dans le texte visible.
> **Les tiers 1 (quick wins) et 2 (structurants) de la priorisation §16.4 sont désormais
> intégralement livrés.** Vérifié : CI entièrement verte (dont le job couverture) ; tri du
> Journal rejoué en navigateur.

> **Lot 7 livré le 2026-07-25** (tier 3 « ambitieux » restant) : F7 — **nettoyage CI en masse**
> `ci_delete_batch` : supprime le lot des candidats d'une simulation, résiste au throttling
> (429/Retry-After → attente **annulable**), garde un **point de reprise** (`run_id` déjà
> supprimés) pour relancer sans doublon, émet la progression et journalise chaque run ; UI
> (bouton « Tout supprimer (N) » / « Reprendre », confirmation par le **nombre** de runs).
> U12 — **raccourcis clavier** (`1`–`6` = onglets, `?` = aide, `Échap` = fermer ; ignorés dans
> les champs). T9 — **CSP durcie** (object/frame/base-uri) + **SBOM CycloneDX** en CI ;
> `style-src 'unsafe-inline'` conservé (styles React inline). T8 — **signature Authenticode**
> scaffoldée dans `release.yml`, conditionnée à un secret certificat (sinon non signé).
> Vérifié : **58 tests cœur verts** (dont F7 : throttling 429→reprise, confirmation par le
> nombre, checkpoint sans re-suppression) ; parcours navigateur (raccourcis 1–6 + `?` + Échap ;
> suppression en masse : bouton, confirmation, exécution retirant les candidats).
> **Reste au backlog V2** : F6, F12, T10 complet (`--rebase-merges`), updater signé,
> F11 (corps de page), suppression des logs/artifacts CI.

> **Lot 8 livré le 2026-07-25** (les partiels restants + 2 nouveaux) : F6 — **choix
> explicite de la base du segment** (`repo_scan_base` : base branche/tag/SHA résolue et
> validée ancêtre strict du sommet ; champ « base » dans Analyse). F12 — **négociation
> de l'api-version Azure DevOps** (tente 7.1, se rabat sur 7.0 pour un Server on-prem plus
> ancien, choix mémorisé ; reprise sûre même en DELETE). F7 — **purge des logs/artefacts**
> (`ci_purge_assets` : reclaim de stockage qui CONSERVE les runs, GitHub, confirmation par
> le nombre, annulable ; UI dédiée). F11 — **externalisation i18n du corps des pages**
> (~180 clés, `useLang` réactif ; Réglages/Dépôts/Journal/Skills complets, CI/Analyse pour
> tout le chrome). Vérifié : **61 tests cœur verts** (F6 base override, F12 négociation,
> F7 purge) ; bascule FR→EN rejouée au navigateur sur les six pages ; purge et base
> vérifiées au navigateur.
> **Reste au backlog V2 (2 items)** : T10 complet (`--rebase-merges`, réécriture de
> structure à travers un merge) ; `tauri-plugin-updater` signé (auto-update, keypair
> minisign requis — le poste durci EDR ne permet ni de générer le keypair ni de tester
> le bundle localement, comme la signature Authenticode gated du lot 7). Ces deux items
> touchent des zones non vérifiables localement (moteur de réécriture safety-critical sans
> toolchain git locale ; outillage de signature indisponible) : ils sont reportés plutôt
> que livrés à l'aveugle.
