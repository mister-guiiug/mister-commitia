# 7. Modèle de sécurité pour les tokens

Faits externes **[Vérifié]** le 2026-07-24 sur les sources officielles citées ; choix de conception **[Recommandé]**.

## 7.1 Exigences (rappel du besoin)

Jamais de token en clair ; coffre OS quand disponible ; chiffrement au repos ; rotation ; affichage des scopes requis avant enregistrement ; tokens distincts par organisation/projet/dépôt ; permissions minimales ; journalisation sans exposition des secrets.

## 7.2 Où sont stockés les secrets

### Décision : coffre du système d'exploitation, via une bibliothèque unique

| OS | Coffre utilisé | Statut |
|---|---|---|
| Windows | **Windows Credential Manager** | [Vérifié] backend du crate Rust `keyring` (`windows-native-keyring-store`) |
| macOS | **Keychain** | [Vérifié] backend `apple-native-keyring-store` |
| Linux | **Secret Service** (D-Bus : GNOME Keyring, KWallet) | [Vérifié] backends `dbus-secret-service`/`zbus` — l'API freedesktop Secret Service est parlée directement en D-Bus (équivalent fonctionnel de libsecret, sans lier la bibliothèque C) |

- Crate **`keyring`** (Rust) : version 4.1.5 publiée le 14/07/2026, activement maintenu (repo `open-source-cooperative/keyring-rs`, licence MIT OR Apache-2.0). Depuis la v4, l'API vit dans `keyring-core` + un crate par backend. [Vérifié — docs.rs/keyring, crates.io]
- Équivalents .NET (si la stack retenue était .NET/Avalonia) : `System.Security.Cryptography.ProtectedData` (DPAPI, **Windows uniquement** — `PlatformNotSupportedException` ailleurs) pour le chiffrement, et `Meziantou.Framework.Win32.CredentialManager` (3.0.1, 08/07/2026, actif) pour le Credential Manager ; le package historique `CredentialManagement` est dormant depuis 2014. [Vérifié — learn.microsoft.com, nuget.org]
- Note Tauri : le plugin officiel **Store** est un stockage clé/valeur **non chiffré** — il ne doit jamais recevoir de secret ; le plugin officiel **Stronghold** (base chiffrée, moteur IOTA Stronghold) existe mais dépend d'un moteur upstream dont la fraîcheur n'a pas pu être établie. [Vérifié — tauri.app/plugin] → **[Recommandé]** : coffre OS via `keyring` en canal principal ; Stronghold non retenu au MVP.

### Politique de repli

Si aucun coffre n'est disponible (Linux headless sans Secret Service) : **par défaut, refus de stocker** — le token est demandé à chaque session (mémoire uniquement). Un repli « fichier chiffré » (AES-256-GCM, clé dérivée par Argon2id d'une phrase secrète, jamais stockée) est proposé **en option explicite** avec avertissement. **[Recommandé]**

### Nommage des entrées

```
service = "mister-commitia"
account = "<kind>:<base_url_hash>:<org>[:<project>][:<repo>]"
```

La base locale ne stocke que cet alias (`token_ref`), les scopes déclarés et l'expiration annoncée (métadonnées non secrètes) — voir [06-modele-donnees.md](06-modele-donnees.md).

## 7.3 Scopes affichés avant enregistrement

L'écran d'ajout affiche le tableau fonctionnalité → droit requis **avant** la saisie, pour inciter au token minimal :

### GitHub / GitHub Enterprise [Vérifié — docs.github.com]

| Fonctionnalité | PAT fine-grained (recommandé) | PAT classique |
|---|---|---|
| Inventaire workflows/runs/artifacts | Actions : **read** | `repo` (dépôt privé) |
| Suppression de runs / logs / artifacts | Actions : **write** | `repo` |
| Lecture de la protection de branche | Administration : **read** | scope non documenté explicitement — Je ne sais pas |
| Lecture des rulesets actifs d'une branche (`GET /repos/{o}/{r}/rules/branches/{branch}`) | lecture dépôt | lecture dépôt |

### Azure DevOps [Vérifié — learn.microsoft.com]

| Fonctionnalité | Scope PAT | Permission objet complémentaire |
|---|---|---|
| Lister builds, leases, réglages de rétention | `vso.build` (Build : read) | — |
| Supprimer un build, gérer leases/rétention | `vso.build_execute` (Build : read & execute — **classé « High privilege »**) | **« Delete builds »** au niveau pipeline (par défaut : accordée aux Build/Project Admins, pas aux Contributors sur Azure DevOps Services) ; « Destroy builds » pour la purge définitive de l'onglet Deleted |

Le scope PAT ne suffit donc pas côté Azure DevOps : la permission objet est vérifiée à l'usage et l'erreur 403 est expliquée en clair (voir CA-13).

## 7.4 Cycle de vie d'un token

1. **Enregistrement** : saisie masquée → écriture directe au coffre → appel de validation (endpoint léger) → comparaison scopes effectifs/annoncés → récapitulatif.
2. **Usage** : lecture au coffre à la demande, en mémoire le temps de l'appel ; buffers effacés après usage (zeroize) **[Recommandé]** ; jamais d'écriture disque, jamais dans les URLs.
3. **Rotation** : alerte à l'approche de `expires_at` ; assistant de remplacement (nouveau token validé **avant** invalidation de l'ancien) ; l'ancienne entrée du coffre est écrasée puis supprimée.
4. **Révocation** : suppression de l'entrée du coffre + métadonnées ; rappel à l'utilisateur que la révocation côté plateforme reste à faire (lien direct).
5. **Multi-tokens** : résolution hiérarchique dépôt → projet → organisation → défaut, affichée dans l'UI (« quel token sera utilisé pour cette action »).

## 7.5 Journalisation sans secrets

- Middleware de **redaction** appliqué à tous les canaux de sortie (logs, erreurs, rapports, exports, événements d'audit) : les valeurs issues du coffre sont enregistrées auprès du redacteur à leur lecture et masquées par motif (`***`) partout.
- Les en-têtes `Authorization` ne sont jamais loggés, même en mode debug.
- Test automatisé « zéro secret » ([15-strategie-tests.md](15-strategie-tests.md) §15.6).

## 7.6 Menaces et parades (résumé)

| Menace | Parade |
|---|---|
| Lecture du disque (base, config, logs) | Aucun secret hors coffre ; scan automatisé |
| Malware avec la session utilisateur | Limite de l'architecture : le coffre OS protège au repos et inter-comptes, pas contre un code exécuté dans la session — documenté honnêtement ; scopes minimaux + expiration courte réduisent l'impact |
| Exfiltration via LLM distant | Les tokens ne font jamais partie du contexte IA (séparation stricte des modules) ; consentement + aperçu pour le contenu Git |
| Token sur-privilégié | Tableau des scopes avant création ; validation des scopes effectifs ; mode lecture seule fonctionnel |
| Phishing d'URL de base (GHES/AzDO Server) | Validation d'URL, épinglage du host par compte, avertissement en cas de changement |
| Erreur d'aiguillage multi-comptes | Résolution hiérarchique affichée ; journal d'audit mentionne le compte utilisé (alias, jamais le secret) |
