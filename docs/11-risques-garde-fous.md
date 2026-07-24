# 11. Risques et garde-fous

Statut : **[Recommandé]** — registre initial, à faire vivre. Échelles : probabilité et impact 1-4.

## 11.1 Garde-fous non négociables (produit)

Ces règles sont câblées dans le cœur applicatif et **non désactivables par configuration** :

1. **Dry-run systématique** : aucune application de plan sans dry-run réussi du même plan (comparaison par hash du contenu du plan).
2. **Backup automatique** : branche de sauvegarde + tag créés avant toute opération destructive ; l'échec de création du backup annule l'opération.
3. **Jamais d'action IA automatique** : l'agent produit des propositions ; seule une décision humaine explicite (accepter/modifier) les convertit en opérations.
4. **Branches protégées bloquées** : les branches protégées (déclarées ou détectées via l'API de la plateforme) ne sont pas réécrivables ; les branches présentes sur un remote (« partagées ») exigent une confirmation renforcée saisie au clavier (nom de la branche).
5. **Trailers protégés** : les trailers listés par la gouvernance du dépôt (ex. `Signed-off-by`) ne sont jamais supprimés, quel que soit le skill.
6. **Suppression CI : simulation d'abord** : toute exécution réelle référence une simulation préalable ; chaque suppression est journalisée avant l'appel d'API.
7. **Secrets au coffre OS uniquement** ; journaux expurgés.
8. **Journal d'audit append-only** pour toute action sensible.

## 11.2 Registre des risques

| ID | Risque | P | I | Garde-fous produit | Garde-fous process |
|---|---|---|---|---|---|
| R1 | **Réécriture d'une branche partagée** → clones des collègues cassés, perte de travail | 3 | 4 | Blocage branches protégées ; détection « présente sur remote » → confirmation renforcée ; push assisté `--force-with-lease` uniquement (V2) | Convention d'équipe : réécrire uniquement avant revue/PR ; communication avant force-push |
| R2 | **Perte des signatures** GPG/SSH et invalidation des vérifications (« Verified ») | 3 | 3 | Avertissement bloquant si commits signés dans le périmètre ; rapport des signatures perdues ; re-signature en V2 | Politique interne : re-signer ou accepter la perte explicitement |
| R3 | **Références externes cassées** (PR, tickets, CHANGELOG, caches CI pointant d'anciens SHA) | 3 | 2 | Export systématique du mapping ancien→nouveau SHA ; rapport d'impact (V2) | Mise à jour manuelle des références critiques |
| R4 | **Suppression d'informations d'audit/conformité** dans les messages (trailers légaux, attributions requises) | 2 | 4 | `protected_trailers` par dépôt ; skill `ai-signature-cleaner` refuse si `ai_attribution_policy = keep-required` ; contenu retiré conservé au journal d'audit | La gouvernance du dépôt prime ; revue des règles par le responsable conformité |
| R5 | **Fuite de code/diffs vers un LLM distant** | 3 | 4 | Consentement explicite avec aperçu de ce qui part ; mode « local uniquement » (Ollama) ; endpoint d'entreprise contrôlé ; redaction (V2) | Politique interne IA ; choix du fournisseur validé par la sécurité |
| R6 | **Hallucination IA** : message proposé faux ou trompeur | 3 | 2 | Jamais d'auto-apply ; explication obligatoire ; diff avant/après ; niveau de risque affiché | Relecture humaine obligatoire (c'est le modèle du produit) |
| R7 | **Suppression CI irréversible** d'un run encore utile (audit de release, obligation légale) | 2 | 4 | Simulation obligatoire ; règles de protection (releases, N derniers succès, tags) ; double confirmation ; journal | Politique de rétention validée par l'équipe ; pas de purge des runs de release |
| R8 | **Contournement des retention leases** Azure DevOps (rétention bloquante volontaire) | 2 | 4 | Les runs sous lease sont **exclus** par défaut ; libération de lease = opération distincte, permission dédiée, confirmation renforcée (V2) | Leases documentées par les équipes qui les posent |
| R9 | **Limites d'API / bannissement temporaire** (suppressions en masse) | 3 | 2 | Throttling adaptatif, backoff exponentiel, reprise sur checkpoint, lecture des en-têtes de limite | Fenêtres de nettoyage hors pics |
| R10 | **Compromission d'un token** stocké | 2 | 4 | Coffre OS, jamais en clair ; scopes minimaux affichés et vérifiés ; rotation guidée ; validation périodique | PAT à granularité fine, expiration courte, révocation immédiate en cas de doute |
| R11 | **Interruption pendant l'application** (crash, coupure) → dépôt dans un état intermédiaire | 2 | 3 | Écriture atomique : le nouvel historique est construit dans `refs/mc/preview` **avant** de déplacer la réf de branche (une seule opération de bascule) ; backup toujours antérieur | Reprise guidée au redémarrage |
| R12 | **Cas Git limites** : merges complexes, submodules, LFS, hooks locaux, fins de ligne | 3 | 2 | Détection et refus explicite des cas non supportés au MVP (message clair) ; fixtures de tests dédiées | Documentation des limites |
| R13 | **Performance sur très gros dépôts** (>100k commits) | 2 | 2 | Analyse incrémentale, pagination, seuils configurables ; benchmarks en CI | Pilote sur dépôt représentatif avant déploiement |
| R14 | **Adoption/gouvernance interne** : l'outil est perçu comme « falsificateur d'historique » | 2 | 3 | Positionnement produit : normalisation **configurable et gouvernée** ; tout est tracé, réversible avant push, et soumis aux règles du dépôt ; mode auditeur (V2) | Communication : l'historique publié reste soumis aux protections serveur (branch protection, rulesets) qui ne sont pas contournées |
| R15 | **Dépendances embarquées** : licences et maintenance des briques réutilisées | 1 | 2 | Inventaire SBOM ; licences compatibles vérifiées avant intégration | Revue juridique si distribution externe |
| R16 | **Faux positifs de détection** (motif « signature IA » présent dans du texte légitime) | 2 | 1 | Détection = signalement, jamais d'action ; motifs configurables par dépôt ; aperçu du contexte | Ajustement des motifs par l'équipe |

## 11.3 Note de positionnement (conformité)

La fonctionnalité de normalisation des messages (dont le nettoyage de mentions générées par des outils) est **soumise aux règles de gouvernance du dépôt** : si la politique du dépôt ou de l'organisation impose la traçabilité des contributions assistées par IA, l'application **refuse** la suppression et le dit explicitement. L'objectif du produit est la qualité et la cohérence de l'historique, pas le contournement d'une règle de transparence ; le journal d'audit local conserve d'ailleurs la trace de chaque normalisation appliquée.
