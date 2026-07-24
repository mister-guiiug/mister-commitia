# 9. Backlog MVP

Statut : **[Recommandé]**. Priorisation MoSCoW ; taille en T-shirt (S/M/L/XL). Les identifiants `CA-x` renvoient aux [critères d'acceptation](14-criteres-acceptation.md).

## 9.0 Périmètre et non-objectifs du MVP

**Objectif MVP** : sur un poste développeur, analyser des dépôts locaux, produire et appliquer en sécurité des plans de réécriture (reword/squash/reorder/drop) assistés par IA sur des **branches locales non protégées**, et inventorier/simuler le nettoyage CI/CD des deux plateformes, avec suppression **unitaire** validée.

**Non-objectifs MVP** (reportés en V2 — voir [10-backlog-v2.md](10-backlog-v2.md)) :
- suppression CI/CD **en masse** par politique planifiée ;
- re-signature (GPG/SSH) des commits réécrits ;
- gestion des retention leases Azure DevOps au-delà de la détection/protection ;
- mode équipe / partage de plans ;
- CLI headless, plugin API, i18n, auto-update.

## 9.1 Épics et user stories

### E1 — Socle applicatif & workspace (Must)
| ID | Story | Taille | AC |
|---|---|---|---|
| E1-S1 | En tant qu'utilisateur, je déclare un dépôt Git local (chemin) et je vois ses métadonnées (branches, remote, branche par défaut) | M | — |
| E1-S2 | Je déclare plusieurs dépôts et je navigue entre eux (vue multi-repos) | M | — |
| E1-S3 | Je peux associer un dépôt à un compte plateforme (GitHub/GHES/AzDO) ou le laisser **offline** | S | CA-9 |
| E1-S4 | Les opérations Git locales fonctionnent sans réseau (mode offline complet) | M | CA-10 |

### E2 — Analyse d'historique (Must)
| ID | Story | Taille | AC |
|---|---|---|---|
| E2-S1 | Je visualise le graphe des commits (branches, merges) et une vue liste filtrable (auteur, date, taille de diff) | L | — |
| E2-S2 | L'analyse détecte les messages faibles (heuristiques : longueur, vocabulaire vide type « wip/fix/update », doublons consécutifs, gros diff sans corps) avec un score | M | CA-2 |
| E2-S3 | L'analyse vérifie la conformité Conventional Commits (type, portée, format) selon la convention configurée du dépôt | M | CA-2 |
| E2-S4 | L'analyse détecte les mentions non fonctionnelles générées par des outils (motifs configurables : « Generated with Claude Code », liens de session, trailers d'assistants) et les classe **sans les modifier** | M | CA-2, CA-7 |
| E2-S5 | Un tableau de bord par dépôt affiche les statistiques (taux de conformité, top motifs, auteurs) | S | — |

### E3 — Moteur de plan de réécriture (Must)
| ID | Story | Taille | AC |
|---|---|---|---|
| E3-S1 | Je compose un plan d'opérations `reword` / `squash` / `fixup` / `reorder` / `drop` / `edit_trailers` sur une plage de commits | L | CA-4 |
| E3-S2 | Le **dry-run** construit le résultat réel dans `refs/mc/preview/<plan>` sans toucher la branche, et m'affiche l'avant/après (messages + graphe + diff d'arbres) | L | CA-3 |
| E3-S3 | L'application d'un plan exige un dry-run réussi **du même plan** (hash) et crée automatiquement backup branch + tag avant toute écriture | M | CA-1, CA-3 |
| E3-S4 | Le plan est exporté/importé en JSON reproductible avec empreinte d'état du dépôt ; l'application est refusée si le dépôt a bougé | M | CA-5 |
| E3-S5 | Le **rollback** restaure la branche depuis la réf de backup tant qu'aucun nouveau commit n'a été ajouté ; sinon il me guide | M | CA-8 |
| E3-S6 | Toute branche **protégée** (déclarée ou détectée via la plateforme) est bloquée en réécriture ; une branche **partagée** (présente sur le remote) exige une confirmation renforcée saisie au clavier | M | CA-6 |
| E3-S7 | Un `reword` seul ne modifie jamais les arbres (contenu) — invariant vérifié automatiquement après application | S | CA-4 |

### E4 — Agent IA & skills v1 (Must)
| ID | Story | Taille | AC |
|---|---|---|---|
| E4-S1 | Je configure un fournisseur IA : **local** (Ollama/endpoint compatible) ou **distant** (endpoint d'entreprise, Anthropic/OpenAI-compatible), avec clé stockée au coffre | M | CA-9 |
| E4-S2 | L'agent produit des **propositions** (jamais d'action directe) : message reformulé, synthèse de groupe, nettoyage de mentions — chacune avec explication et niveau de risque | L | CA-7 |
| E4-S3 | Je peux accepter / modifier / rejeter chaque proposition ; seules les acceptées deviennent des opérations de plan | M | CA-7 |
| E4-S4 | Les 3 skills embarquées fonctionnent : `conventional-commits`, `commit-synthesis`, `ai-signature-cleaner` (cette dernière **refuse** si la gouvernance du dépôt l'exige) | L | CA-7 |
| E4-S5 | Avant tout envoi à un fournisseur distant, l'application m'affiche ce qui sera transmis (messages, diffs) et me demande consentement ; un mode « local uniquement » est disponible | M | CA-9 |

### E5 — Sécurité & secrets (Must)
| ID | Story | Taille | AC |
|---|---|---|---|
| E5-S1 | Les tokens sont stockés dans le coffre OS (Credential Manager / Keychain / Secret Service) ; jamais en clair sur disque ni en base | M | CA-9 |
| E5-S2 | À l'enregistrement d'un token, l'application affiche les scopes requis par fonctionnalité et valide le token par un appel de contrôle | S | CA-9 |
| E5-S3 | Je peux enregistrer des tokens distincts par organisation / projet / dépôt (résolution hiérarchique) | M | — |
| E5-S4 | La rotation est guidée : alerte à l'approche de l'expiration déclarée, remplacement, invalidation de l'ancienne entrée | S | — |
| E5-S5 | Les journaux et messages d'erreur sont systématiquement expurgés des secrets (middleware de redaction testé) | S | CA-9 |

### E6 — CI/CD : inventaire, simulation, suppression unitaire (Must/Should)
| ID | Story | Taille | AC |
|---|---|---|---|
| E6-S1 | (Must) J'inventorie workflows et runs GitHub Actions (GitHub.com et GHES) avec filtres date/statut/workflow | M | — |
| E6-S2 | (Must) J'inventorie pipelines et builds Azure DevOps (cloud) avec les mêmes filtres, **y compris l'état de rétention (lease)** | M | CA-12 |
| E6-S3 | (Must) Je définis une politique de rétention (âge, nombre à conserver, protections) et je lance une **simulation** qui produit un rapport détaillé sans rien supprimer | M | CA-11 |
| E6-S4 | (Should) Je supprime un run **unitairement** après double confirmation ; l'action est journalisée ; les runs protégés (lease, en cours) sont refusés | M | CA-11, CA-12 |
| E6-S5 | (Must) Les erreurs d'API (401/403/404/429) sont expliquées en clair (permission manquante, rétention bloquante, limite de débit) avec la conduite à tenir | S | CA-13 |

### E7 — Journal & audit (Must)
| ID | Story | Taille | AC |
|---|---|---|---|
| E7-S1 | Toute action sensible (réécriture, suppression CI, secret, skill) produit un événement d'audit local append-only | S | CA-14 |
| E7-S2 | Je consulte l'historique des actions dans l'UI et je l'exporte en JSONL | S | CA-14 |

### E8 — Packaging & distribution (Should)
| ID | Story | Taille | AC |
|---|---|---|---|
| E8-S1 | Installeurs Windows (MSI) et macOS (dmg), build Linux (AppImage/deb) produits par la CI | M | — |
| E8-S2 | Documentation utilisateur de première prise en main (déclarer un repo, premier plan, premier nettoyage CI simulé) | S | — |

## 9.2 Definition of Done (MVP)

- Dry-run obligatoire et backup automatique **non désactivables** ;
- couverture de tests du module `plan-engine` ≥ 80 % + suite d'intégration sur dépôts de fixtures ([15-strategie-tests.md](15-strategie-tests.md)) ;
- scan « zéro secret » vert sur les artefacts (logs, base, exports) ;
- parcours P1 et P2 ([13-parcours-utilisateur.md](13-parcours-utilisateur.md)) déroulés en E2E automatisé ;
- journal d'audit alimenté pour 100 % des actions sensibles.

## 9.3 Estimation macro

**[Supposé — estimation]** : MVP réalisable par 2 développeurs seniors en ≈ 4 à 6 mois calendaire, dont ~40 % sur E3 (moteur de plan) et E4 (IA/skills), en réutilisant les briques identifiées dans la [recommandation](04-recommandation.md). À affiner après spike technique (2 semaines) sur le moteur de réécriture.
