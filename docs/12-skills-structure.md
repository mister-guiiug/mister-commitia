# 12. Structure des skills

Statut : **[Recommandé]**. Les exemples concrets et exécutables sont dans le dossier [`skills/`](../skills/) du dépôt.

## 12.1 Principes

1. **Une skill = des fichiers versionnables** (YAML + Markdown), éditables dans l'application ou dans un éditeur, stockables dans un dépôt Git d'entreprise pour distribution.
2. **Déclaratif, pas de code exécutable** au MVP : une skill décrit des entrées, des règles, un prompt, des garde-fous et des tests. Aucune skill ne peut exécuter du code arbitraire (surface d'attaque maîtrisée). Une API de plugins sandboxée est un sujet V2/V3 distinct.
3. **Gouvernance intégrée** : nom, description, version (semver), owner, statut de cycle de vie, scope d'application, règles, exemples, tests.
4. **La skill propose, l'humain dispose** : la sortie d'une skill est toujours une *proposition* avec explication et niveau de risque, jamais une action.
5. **Les garde-fous du cœur priment** : une skill ne peut pas outrepasser les règles non négociables (trailers protégés, branches protégées, dry-run…).

## 12.2 Arborescence type

```
skills/
├── conventional-commits/          # exemple complet (fichiers séparés)
│   ├── skill.yaml                 # manifeste + règles + garde-fous
│   ├── prompt.md                  # template de prompt (variables {{...}})
│   └── tests/
│       └── cases.yaml             # cas de test exécutables par le runner
├── commit-synthesis/skill.yaml    # variantes compactes (prompt embarqué)
├── ai-signature-cleaner/skill.yaml
├── squash-advisor/skill.yaml
├── ci-cleanup-policy/skill.yaml
└── risk-reviewer/skill.yaml
```

## 12.3 Schéma du manifeste (`skill.yaml`)

Champs obligatoires en gras. Validation par JSON Schema à l'import et à la sauvegarde.

| Champ | Type | Description |
|---|---|---|
| **`apiVersion`** | string | `mister-commitia/skill.v1` |
| **`name`** | string (kebab) | Identifiant unique |
| **`version`** | semver | Version de la skill |
| **`owner`** | string | Responsable (email/équipe) |
| **`status`** | enum | `draft` \| `review` \| `published` \| `deprecated` |
| **`description`** | string | Ce que fait la skill, pour l'UI |
| `scope` | objet | Où elle s'applique : motifs de dépôts, branches, opérations produites (`reword`, `squash`, `report`…) |
| `risk_default` | enum | Niveau de risque par défaut des propositions (`low`/`medium`/`high`) |
| `inputs` | liste | Données fournies au prompt (`commit.subject`, `commit.body`, `diff.stat`, `repo.convention`, `ci.runs`…) — liste fermée contrôlée par l'application |
| `output` | objet | Type de proposition attendue (`message-proposal`, `group-proposal`, `report`) + contraintes (`must_explain: true`) |
| `rules` | liste | Règles métier lisibles (affichées à l'utilisateur, injectées au prompt) |
| **`guardrails`** | liste | Interdictions vérifiées **par l'application** après génération (post-conditions), pas seulement demandées au modèle |
| `prompt` / `prompt_file` | string | Template inline ou fichier séparé |
| `examples` | liste | Paires avant/après pédagogiques (UI + few-shot) |
| `tests` | liste / fichier | Cas de test pour le runner de skills |

Point important : les `guardrails` sont doublés côté application (vérification programmatique de la sortie — ex. présence conservée d'un ticket, trailer protégé intact). Un prompt ne constitue jamais à lui seul un mécanisme de sécurité.

## 12.4 Cycle de vie et gouvernance

```
draft ──(revue par owner+pair)──▶ review ──(tests verts + validation)──▶ published ──▶ deprecated
```

- **Création/édition** : dans l'UI (formulaire + éditeur) ou à la main ; `content_hash` recalculé, statut repasse en `draft` à toute modification d'une skill `published`.
- **Import/export** : archive `.zip` contenant les fichiers + `manifest.json` (nom, version, hash de chaque fichier). En V2 : signature de l'archive pour distribution d'entreprise.
- **Tests de skills** : le runner exécute `tests/cases.yaml` — chaque cas fournit des entrées simulées et des assertions sur la proposition (`contains`, `matches`, `not_contains`, `risk_at_most`, `must_refuse`). Les tests tournent à l'import, à la publication et en CI.
- **Traçabilité** : chaque proposition enregistre `skill_name@version` ([modèle de données](06-modele-donnees.md), table `proposal`).

## 12.5 Les six skills attendues

| Skill | Sortie | Spécificité |
|---|---|---|
| `conventional-commits` | `message-proposal` | Choix du type (`feat`, `fix`, `refactor`, `chore`, `docs`, `test`, `ci`, `perf`…) expliqué ; conservation des références (tickets) vérifiée par garde-fou applicatif |
| `commit-synthesis` | `message-proposal` (pour un groupe) | Synthétise N messages en une intention fonctionnelle ; liste ce qui est conservé/perdu |
| `ai-signature-cleaner` | `message-proposal` | Détecte les mentions générées (motifs configurables) ; **consulte `governance.ai_attribution_policy` et refuse si la politique impose la conservation** ; le contenu retiré est journalisé |
| `squash-advisor` | `group-proposal` | Identifie les groupes fusionnables (même intention/auteur/fichiers, fenêtre temporelle) ; explique bénéfice et risque ; produit un pré-plan |
| `ci-cleanup-policy` | `report` | Analyse l'inventaire des runs et propose une politique de rétention ; distingue à conserver / supprimable avec motifs ; jamais d'action |
| `risk-reviewer` | `report` | Évalue un plan avant application (branches partagées, signatures, trailers, volumétrie) ; peut **exiger** une confirmation renforcée via son garde-fou |

Le détail exécutable de chacune est dans [`skills/`](../skills/) — `conventional-commits` sert de référence complète (manifeste + prompt + tests séparés).
