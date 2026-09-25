// Surcharge de configuration Tauri pour le build de TEST des E2E natifs : la
// WebView2 y ouvre son port de débogage, auquel msedgedriver s'attache.
//
// POURQUOI LA CONFIG DE L'APP, ET PLUS msedgedriver. msedgedriver lançait
// l'app et passait `--remote-debugging-port` par la variable
// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS. Or wry pose TOUJOURS ses propres
// arguments (`--disable-features=msWebOOUI,…`) par l'API de WebView2, et les
// runtimes récents les font primer sur la variable : sur windows-latest
// (WebView2 152 et 153), le port ne s'ouvrait plus. La session échouait sur
// « DevToolsActivePort file doesn't exist ». Mesuré le 25/09/2026 : la même
// option posée ici ouvre le port sur la 152, là où la variable échoue.
//
// Rien de tout cela n'entre dans le build de RELEASE : cette surcharge ne sert
// qu'au job E2E, qui n'expose aucun artefact.
//
// Usage : node tauri-e2e-config.cjs <fichier de sortie>
//   puis, depuis apps/desktop : npx tauri build --no-bundle --config <fichier>

const fs = require("fs");
const { resolve } = require("path");

/** Port de débogage ouvert par le build de test, lu aussi par wdio.conf.cjs. */
const DEBUG_PORT = 9222;

// Les arguments que wry pose d'office quand l'app n'en donne aucun : les
// REMPLACER effacerait ces correctifs (menu contextuel « mini », SmartScreen).
const WRY_DEFAULT_ARGS = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

function e2eConfig() {
  const base = JSON.parse(
    fs.readFileSync(resolve(__dirname, "..", "src-tauri", "tauri.conf.json"), "utf8"),
  );
  // Un JSON merge patch REMPLACE les tableaux : chaque fenêtre est donc
  // recopiée entière, avec ses arguments complétés.
  const windows = base.app.windows.map((w) => ({
    ...w,
    additionalBrowserArgs: `${w.additionalBrowserArgs ?? WRY_DEFAULT_ARGS} --remote-debugging-port=${DEBUG_PORT}`,
  }));
  return { app: { windows } };
}

module.exports = { DEBUG_PORT, e2eConfig };

if (require.main === module) {
  const sortie = process.argv[2];
  if (!sortie) {
    console.error("usage : node tauri-e2e-config.cjs <fichier de sortie>");
    process.exit(2);
  }
  fs.writeFileSync(sortie, JSON.stringify(e2eConfig(), null, 2) + "\n");
  console.log(`surcharge E2E écrite : ${sortie} (port de débogage ${DEBUG_PORT})`);
}
