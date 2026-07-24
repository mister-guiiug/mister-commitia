# 15. Stratégie de tests

Statut : **[Recommandé]**.

## 15.1 Vue d'ensemble

```
        E2E desktop (parcours P1/P2/P3)          ← peu, lents, critiques
      Tests de contrat API (GitHub / AzDO mockés)
    Intégration Git (dépôts de fixtures réels)
  Tests de propriétés & invariants (plan-engine)
Unitaires (parsers, heuristiques, policy engine, redaction)
```

Trois familles transverses s'ajoutent à la pyramide : **sécurité**, **performance**, **tests de skills**.

## 15.2 Dépôts Git de test (fixtures)

Un générateur programmatique (`fixtures/`) construit des dépôts synthétiques reproductibles (graine fixe) :

| Fixture | Contenu | Sert à tester |
|---|---|---|
| `linear-simple` | 20 commits linéaires, messages mixtes (conformes, « wip », doublons) | Analyse, reword, squash |
| `with-merges` | Branches + merges (dont merge à conserver) | Délimitation de segment, refus des cas non supportés |
| `shared-branch` | Remote simulé (`git clone --bare` local) avec branche poussée | Détection « partagée », confirmation renforcée, `--force-with-lease` |
| `signed-commits` | Commits signés (clé de test) | Avertissement perte de signature |
| `with-trailers` | `Signed-off-by`, `Co-authored-by`, mentions générées par outils | Trailers protégés, skill de nettoyage, gouvernance |
| `tagged-history` | Tags sur commits du segment réécrit | Rapport d'impact références |
| `submodule-lfs` | Submodule + pointeurs LFS | Refus explicite MVP |
| `big-history` | 100 000 commits générés | Benchmarks (voir 15.7) |
| `hostile-messages` | Messages avec injections (contenu ressemblant à des instructions pour un LLM), encodages exotiques, très longs | Robustesse parsing + sécurité prompt (voir 15.6) |

Les fixtures sont construites par script au démarrage de la suite (pas de binaires versionnés) et mises en cache par hash de script.

## 15.3 Tests unitaires

- Heuristiques de détection (messages faibles, motifs générés, conformité Conventional Commits) : cas nominaux + limites + faux positifs connus.
- Compilateur de plan (opérations → todo list de séquenceur / flux de réécriture) : golden tests.
- Moteur de politiques de rétention CI : jeux de runs synthétiques → ensembles candidats/protégés attendus.
- Redaction : aucun motif de secret (tokens de test) ne traverse le formateur de logs.
- Parsing/sérialisation `plan.json` (rétrocompatibilité de version de schéma).

## 15.4 Propriétés et invariants (plan-engine)

Tests de propriétés (génération aléatoire de plans valides sur les fixtures) :

1. **Innocuité du reword** : plan 100 % `reword` ⇒ arbres identiques pour chaque paire du mapping (CA-4).
2. **Aller-retour** : `apply(plan)` puis `rollback` ⇒ réfs et arbres strictement identiques à l'état initial (CA-8).
3. **Idempotence du dry-run** : deux dry-runs du même plan sur le même état ⇒ mêmes SHA de preview (CA-5).
4. **Conservation** : nombre de commits après = avant − (squashés) − (droppés) ; aucun fichier du sommet ne change hors opérations d'édition.
5. **Empreinte** : toute mutation du dépôt entre dry-run et apply ⇒ refus (CA-5).

## 15.5 Tests de contrat des connecteurs (GitHub / Azure DevOps)

- Serveur HTTP mocké (cassettes enregistrées + scénarios synthétiques) rejouant les réponses officielles : pagination, `429` + `Retry-After`, `403` permission manquante, `404`, run sous **lease** (AzDO), en-têtes `x-ratelimit-*`.
- Assertions côté client : backoff exponentiel respecté, reprise sur checkpoint, **aucun appel DELETE en mode simulation** (CA-11), lease ⇒ jamais de tentative (CA-12).
- Optionnel (nightly, non bloquant) : organisation sandbox réelle GitHub + projet AzDO de test pour valider les contrats contre les vraies APIs — jamais dans la CI standard.

## 15.6 Tests de sécurité

- **Zéro secret** : scénarios E2E puis scan de la base SQLite, des logs, des exports et des fichiers temporaires à la recherche des tokens de test (CA-9).
- **Coffre OS** : aller-retour keyring (écriture, lecture, suppression, rotation) sur les 3 OS de la matrice CI.
- **Injection via contenu de dépôt** : les messages de commits de `hostile-messages` (texte imitant des instructions) ne doivent produire aucune action non demandée — les propositions restent des propositions, les garde-fous applicatifs revalident la sortie du modèle (post-conditions).
- Revue de dépendances (audit + SBOM) en CI.

## 15.7 Performance

Benchmarks suivis en CI (tendance, seuils indicatifs à calibrer au spike) :
- analyse initiale de `big-history` (100k commits) ;
- dry-run d'un plan de 50 opérations sur un segment de 200 commits ;
- inventaire CI paginé de 10 000 runs (mock).

## 15.8 Tests de skills

Le runner de skills exécute les `tests/cases.yaml` de chaque skill (assertions `contains`, `matches`, `not_contains`, `risk_at_most`, `must_refuse`) :
- en mode **déterministe** (modèle simulé renvoyant des sorties types) pour la CI standard — on teste les garde-fous applicatifs et le câblage, pas le modèle ;
- en mode **réel** (nightly, optionnel) contre un modèle local Ollama pour surveiller la dérive des prompts.

## 15.9 E2E desktop

- Pilotage de l'UI (WebDriver via `tauri-driver` si stack Tauri ; Playwright si Electron — décision liée à l'[architecture](05-architecture-cible.md)).
- Scénarios : parcours P1 (nettoyage de branche, dry-run, apply, rollback), P2 (inventaire CI, simulation, suppression unitaire mockée), P3 (enregistrement de token — coffre OS réel en local, simulé en CI).
- Matrice CI : Windows / macOS / Linux (build + unitaires + intégration partout ; E2E complet au minimum sur Windows et Linux).

## 15.10 Portes de qualité

| Porte | Seuil |
|---|---|
| Couverture `plan-engine` + `policy-engine` | ≥ 80 % lignes, 100 % des invariants 15.4 verts |
| Scan « zéro secret » | 0 occurrence |
| E2E P1/P2 | verts sur la matrice cible |
| Lint + typage strict | 0 erreur |
| Audit dépendances | 0 vulnérabilité critique non traitée |

Aucune release sans ces portes ; les benchmarks en dégradation > 20 % bloquent en revue.
