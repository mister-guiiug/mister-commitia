# 10. Backlog Version 2

Statut : **[Recommandé]**. Items candidats après stabilisation du MVP, regroupés par thème et priorisés (P1 = premier incrément V2).

## 10.1 CI/CD avancé

| ID | Item | Priorité | Notes |
|---|---|---|---|
| V2-CI-1 | Suppression **en masse** par politique de rétention (batch, reprise sur erreur, throttling adaptatif aux limites d'API) | P1 | Exige la télémétrie d'erreurs du MVP (E6-S5) |
| V2-CI-2 | Planification des nettoyages (fenêtres récurrentes) avec rapport différé et validation asynchrone | P2 | Jamais de suppression sans validation explicite préalable du rapport |
| V2-CI-3 | Gestion explicite des **retention leases** Azure DevOps : visualisation des leases, libération volontaire (permission dédiée, confirmation renforcée), jamais de libération implicite | P1 | Cf. risque R8 |
| V2-CI-4 | Nettoyage des artefacts et logs (endpoints dédiés) en complément des runs | P2 | |
| V2-CI-5 | Support certifié GitHub Enterprise **Server** et Azure DevOps **Server** (on-prem) : matrices de versions, proxys d'entreprise, certificats internes | P1 | Le MVP vise cloud + GHES « best effort » |

## 10.2 Réécriture Git avancée

| ID | Item | Priorité | Notes |
|---|---|---|---|
| V2-GIT-1 | **Re-signature** des commits réécrits (GPG/SSH signing) avec la clé de l'utilisateur, et rapport des signatures perdues | P1 | La réécriture invalide les signatures existantes (risque R2) |
| V2-GIT-2 | Réécriture d'historique **complet** (toutes branches/tags) pour les cas de type migration de convention, avec moteur de filtrage massif | P2 | Périmètre MVP : segment de branche |
| V2-GIT-3 | Prise en charge fine des cas limites : merges préservés (`--rebase-merges`), submodules, LFS, notes Git | P2 | |
| V2-GIT-4 | Détection des références externes aux SHA réécrits (PR ouvertes, tags, CHANGELOG) et rapport d'impact | P1 | |
| V2-GIT-5 | Push assisté post-réécriture : `--force-with-lease` uniquement, checklist de coordination d'équipe générée | P1 | |

## 10.3 IA & skills

| ID | Item | Priorité | Notes |
|---|---|---|---|
| V2-IA-1 | Gouvernance complète des skills : workflow `draft → review → published → deprecated`, owner obligatoire, tests de skill exécutés à l'import | P1 | |
| V2-IA-2 | Import/export signé des skills (manifest + hash + signature) pour distribution interne d'entreprise | P2 | |
| V2-IA-3 | Skills additionnelles : `squash-advisor` et `ci-cleanup-policy` et `risk-reviewer` complètes (le MVP embarque les 3 premières) | P1 | Cf. [12-skills-structure.md](12-skills-structure.md) |
| V2-IA-4 | Évaluation continue : jeu de tests de non-régression des prompts (golden outputs tolérants) exécuté en CI | P2 | |
| V2-IA-5 | Redaction configurable du contexte envoyé aux LLM distants (masquage chemins, emails, secrets détectés) | P1 | |

## 10.4 Produit & plateforme

| ID | Item | Priorité | Notes |
|---|---|---|---|
| V2-P-1 | **CLI headless** (mêmes moteurs, sortie JSON) pour usage en CI et scripting | P1 | L'architecture cœur/UI du MVP le permet sans refonte |
| V2-P-2 | Mode équipe : export/revue de plans à deux (fichier de plan signé, statut « revu par ») | P2 | |
| V2-P-3 | API de plugins sandboxée (au-delà des skills déclaratives) | P3 | Risque sécurité — à cadrer |
| V2-P-4 | i18n (FR/EN), thème sombre complet, accessibilité (navigation clavier, lecteurs d'écran) | P2 | |
| V2-P-5 | Auto-update signé (canaux stable/beta) | P2 | |
| V2-P-6 | Connecteur **GitLab** (extension de périmètre) | P3 | Hors besoin initial |
| V2-P-7 | Mode « auditeur » en lecture seule (analyse et rapports sans aucune capacité d'écriture) | P2 | Utile conformité |
| V2-P-8 | OAuth / Microsoft Entra ID pour Azure DevOps (alternative aux PAT) | P2 | Réduit la gestion manuelle de tokens |

## 10.5 Critère d'entrée en V2

Le passage en V2 suppose : MVP déployé auprès d'un groupe pilote (≥ 5 utilisateurs), zéro incident de perte de données sur 1 mois d'usage, retours d'expérience intégrés au backlog ci-dessus (repriorisation attendue).
