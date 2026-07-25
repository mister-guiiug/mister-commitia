# E2E desktop natifs (tauri-driver)

Ces tests pilotent l'**application Tauri buildée** (fenêtre WebView réelle) via
[`tauri-driver`](https://tauri.app/develop/tests/webdriver/) et WebdriverIO —
contrairement aux E2E [Playwright](../e2e/) qui pilotent le build **web** en mode
démonstration (mock IPC), fiables et lancés à chaque push.

> **Statut : expérimental.** Le job CI `E2E desktop` ne s'exécute qu'à la demande
> (`workflow_dispatch`), pas à chaque push : l'app native ne se compile pas sur
> tous les postes (EDR) et la session WebDriver dépend de l'environnement du
> runner (msedgedriver / WebView2). Les E2E Playwright couvrent l'UI à chaque push.

## Prérequis

- **Windows** avec WebView2 (Edge) et `msedgedriver` sur le `PATH`.
- `cargo install tauri-driver`
- L'application buildée : depuis `apps/desktop`, `npx tauri build` (ou
  `npx tauri build --no-bundle` pour l'exe seul). Le binaire attendu est
  `target/release/mister-commitia.exe`.

## Lancer

```bash
cd apps/desktop/e2e-native
npm ci
npm test
```

`wdio.conf.cjs` démarre `tauri-driver`, ouvre l'application et exécute les
specs de `specs/`.
