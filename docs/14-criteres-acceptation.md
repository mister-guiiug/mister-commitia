# 14. Critères d'acceptation

Statut : **[Recommandé]**. Format Gherkin (français). Ces scénarios sont la base de la suite E2E ([15-strategie-tests.md](15-strategie-tests.md)) ; les backlogs y font référence par identifiant.

## CA-1 — Backup systématique

```gherkin
Scénario: Toute application de plan crée une sauvegarde préalable
  Étant donné un plan validé en dry-run sur la branche "feature/x"
  Quand j'applique le plan
  Alors une réf "refs/mc/backup/feature/x/<horodatage>" existe
  Et un tag "mc-backup/<plan_id>" existe
  Et tous deux pointent sur l'ancien sommet de la branche

Scénario: L'échec de création du backup annule l'application
  Étant donné un dépôt où la création de réf de backup échoue (simulée)
  Quand j'applique le plan
  Alors la branche cible est inchangée
  Et l'erreur est journalisée avec le motif
```

## CA-2 — Analyse sans effet de bord

```gherkin
Scénario: L'analyse ne modifie jamais le dépôt
  Étant donné un dépôt avec des messages faibles et des mentions générées par outil
  Quand je lance l'analyse
  Alors les indicateurs (score, motifs) sont produits
  Et "git status" est vierge et aucune réf n'a changé
```

## CA-3 — Dry-run obligatoire et sans écriture

```gherkin
Scénario: Le dry-run ne touche ni branche ni index
  Étant donné un plan en état "draft"
  Quand je lance le dry-run
  Alors le résultat est construit sous "refs/mc/preview/<plan_id>"
  Et aucune réf de "refs/heads" ni "refs/tags" n'a changé

Scénario: Impossible d'appliquer sans dry-run du même plan
  Étant donné un plan modifié après son dernier dry-run réussi
  Quand je tente d'appliquer
  Alors l'application est refusée avec le motif "dry-run requis"
```

## CA-4 — Innocuité du reword

```gherkin
Scénario: Un plan composé uniquement de rewords ne change pas le contenu
  Étant donné un plan ne contenant que des opérations "reword"
  Quand je l'applique
  Alors pour chaque paire (ancien SHA, nouveau SHA) du mapping, les arbres Git sont identiques
  Et le diff de contenu entre l'ancienne et la nouvelle tête est vide
```

## CA-5 — Plan reproductible et protégé contre la dérive

```gherkin
Scénario: Un plan exporté est rejouable à l'identique
  Étant donné un plan exporté en JSON puis le dépôt restauré à l'état initial
  Quand j'importe et j'applique le plan sur ce même état
  Alors l'historique résultant est identique (mêmes SHA) à la première application

Scénario: Un plan est refusé si le dépôt a bougé
  Étant donné un plan dont l'empreinte référence le sommet "A"
  Et un nouveau commit "B" ajouté depuis
  Quand je tente d'appliquer
  Alors l'application est refusée avec le motif "empreinte invalide"
```

## CA-6 — Branches protégées et partagées

```gherkin
Scénario: Branche protégée bloquée
  Étant donné une branche listée comme protégée (localement ou via la plateforme)
  Quand je tente de composer un plan dessus
  Alors la composition est refusée et le motif affiché

Scénario: Branche partagée exige une confirmation renforcée
  Étant donné une branche présente sur le remote
  Quand j'applique un plan dessus
  Alors une saisie exacte du nom de la branche est exigée avant exécution
```

## CA-7 — L'IA ne décide jamais

```gherkin
Scénario: Aucune proposition n'est appliquée sans décision humaine
  Étant donné des propositions générées par une skill
  Quand je n'interagis pas avec elles
  Alors aucun plan n'est créé ni modifié

Scénario: La skill de nettoyage respecte la gouvernance du dépôt
  Étant donné un dépôt dont la politique "ai_attribution_policy" vaut "keep-required"
  Quand la skill "ai-signature-cleaner" analyse un message contenant une mention d'outil IA
  Alors elle produit un refus motivé au lieu d'une proposition de suppression

Scénario: Les trailers protégés sont intouchables
  Étant donné "Signed-off-by" listé dans les trailers protégés
  Quand une proposition acceptée aboutirait à sa suppression
  Alors la conversion en opération de plan est refusée par le garde-fou applicatif
```

## CA-8 — Rollback

```gherkin
Scénario: Rollback direct possible tant que la branche n'a pas avancé
  Étant donné un plan appliqué sur "feature/x" et aucun commit ajouté depuis
  Quand je déclenche le rollback
  Alors la branche pointe à nouveau sur le sommet sauvegardé
  Et l'événement est journalisé

Scénario: Rollback guidé si la branche a avancé
  Étant donné un commit ajouté après l'application
  Quand je déclenche le rollback
  Alors l'application n'exécute rien automatiquement et affiche la procédure guidée
```

## CA-9 — Secrets et confidentialité

```gherkin
Scénario: Aucun secret en clair
  Étant donné un token enregistré
  Quand je scanne la base SQLite, les fichiers de configuration, les journaux et les exports
  Alors la valeur du token n'apparaît nulle part

Scénario: Scopes affichés avant enregistrement
  Étant donné l'écran d'ajout d'un accès plateforme
  Alors le tableau fonctionnalité → scope requis est visible avant la saisie du token

Scénario: Consentement avant envoi à un LLM distant
  Étant donné un fournisseur IA distant configuré
  Quand je demande des propositions
  Alors l'aperçu des données à transmettre est affiché et mon accord explicite est requis
```

## CA-10 — Mode offline

```gherkin
Scénario: Toutes les opérations Git locales fonctionnent sans réseau
  Étant donné la carte réseau désactivée
  Quand je déroule le parcours P1 avec un fournisseur IA local
  Alors analyse, propositions, plan, dry-run, application et rollback aboutissent
```

## CA-11 — Nettoyage CI : simulation puis validation

```gherkin
Scénario: La simulation ne supprime rien
  Étant donné une politique de rétention et un inventaire de runs
  Quand je lance la simulation
  Alors le rapport liste candidats et protégés avec motifs
  Et aucun appel de suppression n'a été émis (vérifié par le mock d'API)

Scénario: Une exécution exige une simulation préalable et une double confirmation
  Étant donné aucune simulation réussie pour ce périmètre
  Quand je tente une exécution
  Alors elle est refusée
  Étant donné une simulation réussie
  Quand je confirme deux fois (dont saisie du nom du périmètre)
  Alors les suppressions sont exécutées et chacune est journalisée avant l'appel
```

## CA-12 — Rétention bloquante respectée

```gherkin
Scénario: Un run sous retention lease n'est jamais supprimé
  Étant donné un run Azure DevOps marqué "retenu par lease"
  Quand une politique le classerait candidat
  Alors il apparaît dans "protégés" avec le motif "lease"
  Et aucune tentative de suppression n'est émise
```

## CA-13 — Erreurs d'API expliquées

```gherkin
Scénario: Permission insuffisante
  Étant donné un token sans le droit de suppression
  Quand je tente une suppression
  Alors l'erreur affichée nomme le scope/la permission manquant(e) et l'action corrective

Scénario: Limite de débit
  Étant donné une réponse 429 avec en-tête Retry-After
  Quand le job de nettoyage la rencontre
  Alors il se met en pause, reprend après le délai, et le rapport mentionne l'incident
```

## CA-14 — Journal d'audit

```gherkin
Scénario: Complétude du journal
  Étant donné le déroulé des parcours P1 et P2
  Alors chaque action sensible (application de plan, suppression, accès secret, modification de skill)
    correspond à exactement un événement d'audit horodaté
  Et l'export JSONL est rejouable chronologiquement sans trous de séquence
```
