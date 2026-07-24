# 13. Exemples de parcours utilisateur

Statut : **[Recommandé]**. Trois parcours de référence ; P1 et P2 sont les parcours E2E automatisés du MVP.

## P1 — Nettoyer une branche de travail avant ouverture de PR

Persona : développeuse sur `feature/express-payment` (12 commits locaux, non poussés pour la plupart), convention Conventional Commits exigée par le dépôt.

| # | Écran / panneau | Action utilisateur | Comportement & garde-fous |
|---|---|---|---|
| 1 | Accueil multi-repos | Sélectionne le dépôt `webapp-checkout` | L'analyse incrémentale se lance (locale, sans réseau) |
| 2 | Vue graphe + liste | Choisit la branche `feature/express-payment` | Le segment réécrivable est délimité automatiquement (depuis `merge-base` avec `main`) ; les commits déjà poussés sont marqués « partagés » |
| 3 | Panneau analyse | Constate les indicateurs : 7 messages non conformes, 3 « wip », 2 mentions générées par outil | Détection = signalement uniquement, rien n'est modifié |
| 4 | Panneau propositions IA | Clique « Proposer un nettoyage » avec les skills `conventional-commits` + `commit-synthesis` | Aperçu de ce qui sera envoyé au fournisseur IA + consentement (ou traitement 100 % local via Ollama) |
| 5 | Panneau propositions IA | Passe en revue chaque proposition : accepte 8, modifie 2 (éditeur inline), rejette 1 | Chaque proposition porte explication + niveau de risque ; rien n'est appliqué |
| 6 | Composeur de plan | Voit le plan généré : 6 `reword`, 2 `squash` (12 → 7 commits) | Le panneau risques (`risk-reviewer`) signale : 1 commit signé GPG dans le périmètre → avertissement |
| 7 | Composeur de plan | Réordonne une opération par glisser-déposer, retire le commit signé du périmètre | Le plan est revalidé, hash recalculé |
| 8 | Dry-run | Clique « Dry-run » | Le résultat réel est construit dans `refs/mc/preview/…` ; la branche n'est pas touchée |
| 9 | Aperçu avant/après | Compare : messages, graphe, et vérification « arbres identiques » pour les rewords | Invariant affiché : « contenu de code inchangé ✔ » |
| 10 | Application | Clique « Appliquer » | Backup automatique `refs/mc/backup/feature/express-payment/2026-07-24T09-30Z` + tag ; puis bascule de la réf de branche ; mapping ancien→nouveau SHA exporté |
| 11 | Rapport | Télécharge `plan.json` + mapping | Journal d'audit alimenté |
| 12 | (Optionnel) Push | La branche existait sur le remote → l'app exige la saisie du nom de la branche et propose `--force-with-lease` uniquement | Refus si la branche est protégée côté plateforme |

**Rollback** : tant qu'aucun nouveau commit n'est ajouté, un clic restaure la branche depuis le backup.

## P2 — Assainir l'historique CI/CD d'un projet

Persona : responsable d'équipe, 1 organisation GitHub Enterprise + 1 projet Azure DevOps.

| # | Écran / panneau | Action | Comportement & garde-fous |
|---|---|---|---|
| 1 | Paramètres → Accès plateformes | Ajoute un PAT GitHub Enterprise | Les scopes requis par fonctionnalité sont affichés **avant** l'enregistrement ; le token part au coffre OS ; un appel de validation confirme les droits |
| 2 | Vue CI/CD | Lance l'inventaire (workflows GitHub + pipelines AzDO) | Pagination + respect des limites d'API ; les runs Azure DevOps sous **retention lease** sont marqués « retenus » |
| 3 | Politiques de rétention | Crée une politique : > 180 jours, conserver 10 derniers runs par pipeline, protéger releases/tags et runs sous lease | La skill `ci-cleanup-policy` peut proposer une politique initiale à partir de l'inventaire (rapport, pas d'action) |
| 4 | Simulation | Lance la **simulation** | Rapport : 234 runs candidats, 18 protégés (motifs affichés : lease, release, N derniers), 0 suppression effectuée |
| 5 | Rapport de simulation | Exporte le rapport, le fait valider par l'équipe | Une exécution réelle est impossible sans simulation préalable sur le même périmètre |
| 6 | Exécution | Confirme l'exécution (MVP : suppression unitaire ; V2 : batch) — double confirmation | Chaque suppression est journalisée avant l'appel API ; erreurs 403/429 expliquées (permission « Delete builds » manquante, limite de débit → reprise) |
| 7 | Journal | Consulte l'historique des suppressions | Append-only, exportable |

## P3 — Premier lancement : enregistrer un accès en sécurité

| # | Étape | Garde-fou |
|---|---|---|
| 1 | Choisir la plateforme (GitHub / GHES / Azure DevOps / AzDO Server) et l'URL de base | Validation du format d'URL, détection cloud/on-prem |
| 2 | L'application affiche le tableau « fonctionnalité → scope requis » (lecture seule vs suppression) | L'utilisateur peut créer un token **minimal** (lecture seule) pour commencer |
| 3 | Saisie du token | Champ masqué ; jamais écrit sur disque en clair ; envoi direct au coffre OS |
| 4 | Validation en ligne | Appel de contrôle ; les scopes effectifs détectés sont comparés aux scopes annoncés |
| 5 | Métadonnées | Date d'expiration déclarée → rappels de rotation |
| 6 | Confirmation | Récapitulatif : où est stocké le secret (nom d'entrée du coffre), ce que l'app pourra faire |

## Choix UX structurants

- **Un seul « chemin dangereux »** : toute action destructive passe par la même séquence Proposition → Plan → Dry-run → Backup → Application, quel que soit le point d'entrée.
- **Le dry-run est un vrai résultat**, pas une estimation : l'aperçu avant/après montre l'historique réellement construit.
- **Confirmation renforcée** = saisie au clavier du nom de la cible (branche, pipeline), jamais une simple case à cocher.
- **Tout refus est expliqué** (branche protégée, lease, politique de gouvernance, scope manquant) avec l'action corrective possible.
