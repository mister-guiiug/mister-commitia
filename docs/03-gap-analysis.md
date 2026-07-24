# 3. Gap analysis

Écarts entre le besoin cible et le meilleur existant identifié ([01-etude-existant.md](01-etude-existant.md), [Vérifié] 2026-07-24). Sévérité de l'écart : 🔴 bloquant (le besoin ne peut pas être rendu) · 🟠 contournable avec effort · 🟢 mineur.

## 3.1 Écarts fonctionnels

| Besoin | Meilleur existant | Écart | Sévérité |
|---|---|---|---|
| Réécriture assistée par IA avec validation, en desktop | GitKraken Commit Composer | Propriétaire ; IA en plan payant ; pas de convention configurable par dépôt ; pas de skill « synthèse » paramétrable ; pas d'explication de risque | 🟠 |
| Plan de réécriture **reproductible, exportable, rejouable** | Scripts `git rebase`/`filter-repo` maison | Aucun outil ne matérialise le plan comme artefact (empreinte d'état, mapping SHA, statuts) | 🔴 |
| Dry-run systématique avec résultat réel prévisualisé | Todo list de rebase (aperçu d'intention, pas de résultat) ; préview Commit Composer (non exportable) | Pas de construction préalable de l'historique cible hors branche | 🔴 |
| Backup/tag automatique + rollback guidé | Reflog implicite de Git | Aucune création automatique de réfs de sauvegarde nommées ni procédure de rollback | 🟠 |
| Fusion/synthèse de messages multi-commits avec conservation garantie (tickets, BREAKING CHANGE) | IA généralistes | Aucune garantie post-conditions (la conservation dépend du prompt, pas d'une vérification) | 🔴 |
| Nettoyage des mentions d'outils IA **gouverné** | Hooks de prévention ; `filter-repo --message-callback` générique | Aucune notion de politique de dépôt (`keep-required` vs `normalization-allowed`), aucun refus motivé, aucun journal du contenu retiré | 🔴 |
| Inventaire + rétention CI/CD **GitHub Enterprise et Azure DevOps** | gh CLI + action Mattraks (GitHub) ; API REST seule (AzDO) | Aucun outil bi-plateforme ; côté AzDO aucun outil du tout (ni CLI officielle) ; pas de simulation locale avec rapport validable | 🔴 |
| Respect des retention leases (rétention bloquante) | API REST AzDO | Aucune UX existante ; le code d'erreur d'une suppression sous lease n'est même pas documenté | 🟠 |
| Multi-dépôts avec tableau de bord de qualité d'historique | GUIs multi-repos | Pas d'analyse de conformité/qualité agrégée | 🟠 |

## 3.2 Écarts techniques

| Sujet | Constat | Sévérité |
|---|---|---|
| Reproductibilité | Les GUIs appliquent immédiatement ; aucun format d'échange de plan | 🔴 |
| Mode offline complet (Git + IA locale) | GUIs : Git offline oui ; IA locale via Ollama rarement intégrée (OpenCommit/aicommits le font, mais en CLI au commit) | 🟠 |
| Extensibilité contrôlée (skills déclaratives testables) | Inexistant partout | 🔴 |
| Intégration des règles serveur (branch protection, rulesets `commit_message_pattern`) dans un outil client | Inexistant ; les APIs nécessaires existent ([08-apis-plateformes.md](08-apis-plateformes.md)) | 🟠 |

## 3.3 Écarts sécurité

| Sujet | Constat | Sévérité |
|---|---|---|
| Tokens multi-org au coffre OS avec scopes affichés et rotation | Les GUIs stockent leurs propres credentials sans modèle multi-tokens exposé ni pédagogie de scopes ; les CLI reposent sur variables d'environnement ou fichiers | 🟠 |
| Journal d'audit local des actions destructives (Git et CI) | Inexistant partout | 🔴 |
| Séparation stricte secrets / contexte IA | Non documenté dans les outils IA existants | 🟠 |
| Consentement explicite avant envoi de diffs à un LLM distant | Variable ; rarement un aperçu de ce qui part | 🟠 |

## 3.4 Écarts gouvernance

| Sujet | Constat | Sévérité |
|---|---|---|
| Politique par dépôt (trailers protégés, politique d'attribution IA, convention cible, périmètre réécrivable) | Inexistant : les outils appliquent ce que l'utilisateur demande, sans notion de règles du dépôt | 🔴 |
| Skills versionnées avec owner, cycle de vie, tests | Inexistant | 🔴 |
| Distinction conservation/suppression motivée pour la conformité (audit, PI, sécurité) | Inexistant | 🔴 |
| Traçabilité des normalisations appliquées (qui, quoi, quand, contenu retiré) | Inexistant | 🔴 |

## 3.5 Synthèse

Les écarts 🔴 se concentrent exactement sur ce qui fait la valeur du besoin exprimé : **gouvernance, reproductibilité, garanties post-conditions, bi-plateforme CI/CD, audit**. Ce sont des écarts d'architecture (modèle de plan, moteur de politiques, registre de skills) qu'aucune configuration ou combinaison d'outils existants ne comble ; à l'inverse, les briques d'exécution (rebase, callbacks de réécriture, endpoints REST, coffres OS) existent et sont réutilisables. C'est la matrice de la [recommandation](04-recommandation.md).
