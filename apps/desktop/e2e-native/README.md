# E2E desktop natifs (WebView2)

Ces tests pilotent l'**application Tauri buildée** (fenêtre WebView2 réelle)
par WebdriverIO et
[Microsoft Edge WebDriver](https://learn.microsoft.com/microsoft-edge/webview2/how-to/webdriver),
contrairement aux E2E [Playwright](../e2e/), qui pilotent le build **web** en
mode démonstration (mock IPC) : fiables, ceux-là sont lancés à chaque push.

> **Statut : expérimental.** Le job CI `E2E desktop` ne s'exécute qu'à la demande
> (`workflow_dispatch`), pas à chaque push : l'app native ne se compile pas sur
> tous les postes (EDR) et la session WebDriver dépend de l'environnement du
> runner (msedgedriver / WebView2). Les E2E Playwright couvrent l'UI à chaque push.

## Mode « attach » : pourquoi

Jusqu'au 25/09/2026, `tauri-driver` laissait msedgedriver **lancer** l'app.
msedgedriver passe alors `--remote-debugging-port` à la WebView2 par la
variable `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`. Or wry pose toujours ses
propres arguments par l'API de WebView2, et les runtimes récents (152 et plus)
les font primer sur la variable. Le port ne s'ouvrait plus, et la session
échouait sur « DevToolsActivePort file doesn't exist ».

Désormais, c'est la **config de l'app** qui ouvre le port, dans un build de
test seulement (`tauri-e2e-config.cjs`). Le test lance l'app, et msedgedriver
s'y **attache** (`ms:edgeOptions.debuggerAddress`). Le build de release, lui,
n'ouvre aucun port.

## Prérequis

- **Windows** avec la WebView2.
- **msedgedriver à la version EXACTE de la WebView2**, et non à celle du
  navigateur Edge : les deux divergent (sur l'image `windows-2022` de GitHub,
  131 contre 152). Il doit être sur le `PATH`, ou désigné par la variable
  `MSEDGEDRIVER`. Version du runtime :
  `reg query "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" /v pv`,
  puis `https://msedgedriver.microsoft.com/<version>/edgedriver_win64.zip`.
- **Le build de test** de l'application, depuis `apps/desktop` :

  ```bash
  node e2e-native/tauri-e2e-config.cjs tauri.e2e.conf.json
  npx tauri build --no-bundle --config tauri.e2e.conf.json
  ```

  Le binaire attendu est `target/release/mister-commitia.exe` (ou
  `mc-desktop.exe`). Un build de release ne convient pas : il n'ouvre pas le
  port, et le test le dit au bout d'une minute.

## Lancer

```bash
cd apps/desktop/e2e-native
npm install
npm test
```

`wdio.conf.cjs` lance l'application, attend son port de débogage (9222),
démarre msedgedriver (4444), puis exécute les specs de `specs/`.
