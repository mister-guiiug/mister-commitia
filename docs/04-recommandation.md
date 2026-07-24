# 4. Recommandation argumentée

## 4.1 Recommandation

**Créer une application nouvelle (option O4), conçue comme un cœur de moteurs réutilisables surmonté d'une UI desktop, en déléguant l'exécution aux briques éprouvées de l'écosystème.** **[Recommandé]**

Concrètement :
- **Ne pas réinventer** : l'exécution des réécritures s'appuie sur le sequencer natif de Git (`git rebase -i` piloté par `GIT_SEQUENCE_EDITOR`, mécanisme documenté officiellement [Vérifié]) et, pour les réécritures massives V2, sur le modèle éprouvé de `git filter-repo` ; la lecture du graphe passe par libgit2 (`git2-rs` 0.21.0 / LibGit2Sharp 0.32.0, tous deux activement maintenus [Vérifié]) ; la convention est celle de Conventional Commits 1.0.0 (spécification publique) ; les suppressions CI/CD utilisent les endpoints REST officiels ([08-apis-plateformes.md](08-apis-plateformes.md)) ; les secrets vont au coffre OS via `keyring` [Vérifié].
- **Réserver l'effort de développement** aux écarts 🔴 de la [gap analysis](03-gap-analysis.md) : moteur de plan reproductible avec dry-run réel et garde-fous, moteur de politiques de rétention bi-plateforme, registre de skills gouvernées, journal d'audit, modèle de sécurité des tokens.

## 4.2 Pourquoi pas les autres options

| Option | Motif du rejet (résumé — détail en [02-make-or-buy.md](02-make-or-buy.md)) |
|---|---|
| **Utiliser l'existant** | Couvre ~la moitié du besoin en juxtaposant 4+ outils hétérogènes ; laisse vides les exigences de gouvernance, de reproductibilité et d'audit ; Azure DevOps reste entièrement à scripter ; dépendance à un produit propriétaire (GitKraken) pour la partie IA |
| **Contribuer (GitButler)** | Licence FSL-1.1-MIT à faire arbitrer juridiquement pour un dérivé interne ; périmètre produit (workflow de commit) disjoint du nôtre (gouvernance + CI/CD) ; vitesse dépendante des mainteneurs. Des contributions ponctuelles restent souhaitables, sans en faire le véhicule du besoin |
| **Plugin/surcouche** | Aucun hôte n'expose les points d'extension nécessaires (UI riche, hooks de sécurité, stockage) ; un plugin IDE imposerait l'environnement et dégraderait l'UX cible |

## 4.3 Trajectoire recommandée (réduction du risque)

1. **Spike (2 semaines)** : valider le moteur de plan sur les fixtures ([15-strategie-tests.md](15-strategie-tests.md) §15.2) — dry-run dans `refs/mc/preview`, application par bascule de réf, rollback ; c'est le cœur de la valeur et du risque.
2. **MVP** ([09-backlog-mvp.md](09-backlog-mvp.md)) : cœur + UI Tauri ; CI/CD en inventaire + simulation + suppression unitaire.
3. **V2** ([10-backlog-v2.md](10-backlog-v2.md)) : masse, leases, re-signature, CLI headless, gouvernance de skills complète.

Le cœur étant une bibliothèque distincte de l'UI ([05-architecture-cible.md](05-architecture-cible.md)), un CLI headless V2 s'obtient sans refonte — c'est aussi l'assurance anti-impasse si la couche desktop devait changer.

## 4.4 Conditions de renoncement (honnêteté du make-or-buy)

Reconsidérer la décision **avant** le développement si l'une de ces conditions se vérifie :
- Le besoin réel se réduit à « nettoyer mes branches locales avant PR » → un abonnement GitKraken (Commit Composer) + discipline d'équipe suffit ; inutile de construire.
- Le besoin CI/CD se réduit à GitHub seul → l'action `Mattraks/delete-workflow-runs` + `gh run delete` couvrent l'essentiel sans UI.
- L'organisation interdit tout envoi de contexte à un LLM **et** refuse l'IA locale → la moitié de la valeur (assistance) tombe ; un simple linter (commitlint/gitlint) + rebase GUI existants peuvent suffire.
- Un acteur du marché sort d'ici au lancement un produit couvrant gouvernance + bi-plateforme (veille à refaire au jalon de lancement — la présente étude date du **2026-07-24**).

## 4.5 Ce qui est vérifié / supposé / recommandé dans cette conclusion

- **[Vérifié]** : l'inexistence d'un outil couvrant le besoin complet à la date de l'étude (recherches actives documentées, §1.6) ; la disponibilité et la maintenance des briques citées ; les APIs nécessaires.
- **[Supposé]** : l'estimation d'effort MVP (§9.3) ; l'acceptation juridique interne d'une base FSL comme *inspiration* d'architecture (aucun code GitButler ne serait repris — la stack Tauri/Rust est un choix indépendant).
- **[Recommandé]** : l'option O4, la trajectoire en 3 temps, les conditions de renoncement.
