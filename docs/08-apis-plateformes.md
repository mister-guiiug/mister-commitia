# 8. APIs Azure DevOps et GitHub Enterprise nécessaires

Tout ce chapitre est **[Vérifié]** le 2026-07-24 contre les documentations officielles (docs.github.com, learn.microsoft.com), sauf mention contraire. Les scopes/permissions détaillés côté sécurité sont dans [07-securite-tokens.md](07-securite-tokens.md).

## 8.1 GitHub / GitHub Enterprise (REST, version d'API `2022-11-28`)

### Endpoints requis

| Usage | Endpoint | Droit requis (fine-grained PAT) |
|---|---|---|
| Lister les workflows | `GET /repos/{owner}/{repo}/actions/workflows` | Actions : read |
| Lister les runs (filtres `actor`, `branch`, `event`, `status`, `created`, `head_sha`, `per_page` ≤ 100) | `GET /repos/{owner}/{repo}/actions/runs` | Actions : read |
| Détail d'un run | `GET /repos/{owner}/{repo}/actions/runs/{run_id}` | Actions : read |
| **Supprimer un run** | `DELETE /repos/{owner}/{repo}/actions/runs/{run_id}` | Actions : write |
| Supprimer les logs d'un run | `DELETE /repos/{owner}/{repo}/actions/runs/{run_id}/logs` | Actions : write |
| Lister les artifacts (dépôt / run) | `GET /repos/{o}/{r}/actions/artifacts` · `GET .../runs/{run_id}/artifacts` | Actions : read |
| Supprimer un artifact | `DELETE /repos/{o}/{r}/actions/artifacts/{artifact_id}` | Actions : write |
| Protection de branche (détection branches protégées) | `GET /repos/{o}/{r}/branches/{branch}/protection` | Administration : read |
| Règles actives sur une branche (rulesets) | `GET /repos/{o}/{r}/rules/branches/{branch}` | lecture dépôt |
| Contrôle de limite | `GET /rate_limit` | — |

Notes utiles au produit :
- Le filtre `created` accepte la syntaxe de recherche de dates GitHub (`>=YYYY-MM-DD`, plages `A..B`, jokers) — idéal pour les politiques d'âge.
- Le paramètre `status` mélange statuts et conclusions (`queued`, `in_progress`, `success`, `failure`, …) : un seul paramètre pour les deux notions.
- Les **rulesets** peuvent imposer des motifs de message de commit (`commit_message_pattern`, catégorie « metadata restrictions ») — réservé au plan **GitHub Enterprise**. L'application lit ces règles pour aligner sa configuration de convention.
- Rétention native logs+artifacts : **90 jours par défaut**, configurable 1-90 j (dépôts publics) et 1-400 j (privés) — la politique de nettoyage de l'application complète ce mécanisme, elle ne le remplace pas.

### Limites de débit

| Contexte | Limite |
|---|---|
| PAT / utilisateur authentifié | **5 000 req/h** (15 000 pour apps appartenant à une org GitHub Enterprise Cloud) |
| Non authentifié | 60 req/h |
| En-têtes | `x-ratelimit-limit` / `-remaining` / `-used` / `-reset` (epoch UTC) / `-resource` |
| Dépassement secondaire | 403 ou 429 ; si `retry-after` présent → attendre ce délai ; sinon si `remaining=0` → attendre `reset` ; sinon ≥ 1 min + backoff exponentiel (persister peut mener au bannissement) |

### GitHub Enterprise Server (on-prem)

- Base URL : **`http(s)://HOSTNAME/api/v3`** (vérifié GHES 3.17).
- **Rate limits désactivées par défaut** sur GHES ; activables/configurables par l'admin (Management Console) — le client doit donc traiter les deux cas (présence ou absence des en-têtes).

## 8.2 Azure DevOps (REST, `api-version=7.1` — version GA courante)

### Endpoints requis

| Usage | Endpoint | Scope PAT |
|---|---|---|
| Lister les builds (filtres `minTime`/`maxTime`, `definitions`, `statusFilter`, `resultFilter`, `branchName`, `$top`, `continuationToken`, `queryOrder`) | `GET https://dev.azure.com/{org}/{project}/_apis/build/builds?api-version=7.1` | `vso.build` |
| **Supprimer un build/run** | `DELETE .../_apis/build/builds/{buildId}?api-version=7.1` | `vso.build_execute` |
| Leases de rétention d'un run | `GET .../_apis/build/builds/{buildId}/leases?api-version=7.1` (groupe *Builds*) | `vso.build` |
| Gérer les leases (lister par owner/user, ajouter, **supprimer** `?ids={ids}`) | `.../_apis/build/retention/leases` | `vso.build_execute` (delete) |
| Réglages de rétention du projet | `GET` / `PATCH .../_apis/build/retention?api-version=7.1` (`purgeRuns`, `purgeArtifacts`, `retainRunsPerProtectedBranch`…) | `vso.build` / `vso.build_execute` |
| Lister pipelines / runs (lecture moderne) | `GET .../_apis/pipelines` · `GET .../_apis/pipelines/{id}/runs` | `vso.build` |
| Dépôts Git (métadonnées, association) | `GET .../_apis/git/repositories` | `vso.code` (lecture) |
| Politiques de branche (détection branches protégées) | `GET .../_apis/policy/configurations` | `vso.code` (lecture) — **[Supposé]** scope exact à confirmer à l'implémentation |

Points structurants vérifiés :
- **L'API Pipelines (groupe Runs) n'a pas d'opération DELETE** (seulement Get / List / Run Pipeline) : toute suppression passe par **Builds – Delete**. Le CLI `az pipelines` n'a pas non plus de commande de suppression (feature request ouverte depuis 2021, Azure/azure-cli#17968).
- **Retention lease** : « A valid retention lease prevents automated systems from deleting a pipeline run » ; la doc rétention impose de retirer les leases avant suppression. Le code d'erreur HTTP exact d'un DELETE sur un build sous lease n'est **pas documenté** — Je ne sais pas ; à caractériser par test de contrat (voir §8.4).
- `minTime`/`maxTime` filtrent selon le champ défini par `queryOrder` (finish/start/queue) : toujours fixer `queryOrder` explicitement (ex. `finishTimeDescending`) pour des politiques d'âge déterministes.
- Permission objet **« Delete builds »** requise en plus du scope (voir 7.3) ; les builds supprimés passent par un onglet *Deleted* avant destruction (« Destroy builds » pour purge définitive) — la suppression n'est donc pas immédiatement irrémédiable côté Azure DevOps, ce que le rapport utilisateur doit refléter.

### Limites de débit (Azure DevOps Services)

- Limite globale : **200 TSTUs par fenêtre glissante de 5 min** ; blocage → **429** (`TF400733`), ou ralentissement (réponses 200 retardées jusqu'à 30 s).
- En-têtes : `Retry-After` (secondes), `X-RateLimit-Resource` / `-Delay` / `-Limit` / `-Remaining` / `-Reset` (epoch), `X-RateLimit-Cost` — envoyés **seulement à l'approche du seuil** : le client ne doit pas supposer leur présence.
- Les rate limits documentées concernent **Azure DevOps Services** (cloud) uniquement.

### Azure DevOps Server (on-prem)

- Format d'URL : `https://{server[:port]}/{collection}/{project}/_apis/...` (port non-SSL par défaut 8080, collection par défaut `DefaultCollection`).
- Parité de versions : **Server 2022 supporte l'api-version jusqu'à 7.0** (vues doc 7.0/7.1 pour 2022.x), Server 2020 → 6.0, Server 2019 → 5.0. Le connecteur doit négocier `api-version` selon la cible.

## 8.3 Authentification

- **PAT** sur les deux plateformes au MVP (en-tête `Authorization` ; AzDO : Basic base64, GitHub : Bearer).
- Azure DevOps OAuth « vssps » est **déprécié (fin 2026)** au profit de **Microsoft Entra ID** ; les scopes `vso.*` restent valables pour les PAT. L'option Entra ID est au backlog V2 (V2-P-8).

## 8.4 Matrice d'erreurs à gérer (contrats de test)

| Code | Situation | Comportement de l'application |
|---|---|---|
| 401 | Token invalide/expiré | Invite à la rotation (7.4) ; pas de retry |
| 403 | Scope ou permission objet manquant (ex. « Delete builds ») ; ou limite secondaire GitHub | Message explicite nommant le droit manquant ; si en-têtes de limite présents → traiter comme 429 |
| 404 | Run déjà supprimé / ressource inconnue | Marquer « déjà absent », continuer le job |
| 409 / erreur lease | Rétention bloquante | Run classé « protégé (lease) », jamais retenté ; code exact à caractériser par test de contrat (non documenté) |
| 429 | Limite de débit | Pause selon `Retry-After`, reprise sur checkpoint, incident mentionné au rapport |
| 5xx | Indisponibilité | Backoff exponentiel plafonné, job resumable |

## 8.5 Sources

GitHub : `docs.github.com/en/rest/actions/*` (workflow-runs, workflows, artifacts), `.../rest/using-the-rest-api/rate-limits-for-the-rest-api`, `.../rest/branches/branch-protection`, `.../rest/repos/rules`, `.../enterprise-server@3.17/rest/quickstart`, page « permissions required for fine-grained PAT », page rétention Actions. Azure DevOps : `learn.microsoft.com/en-us/rest/api/azure/devops/build/*` (builds list/delete, leases, retention), `.../rest/api/azure/devops/pipelines/runs`, `.../azure/devops/integrate/concepts/rate-limits`, `.../integrate/concepts/rest-api-versioning`, `.../organizations/security/permissions`, `.../azure/devops/pipelines/policies/retention`. Consultées le 2026-07-24.
