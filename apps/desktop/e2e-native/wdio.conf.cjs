// Configuration WebdriverIO pour les E2E DESKTOP natifs (T7) : pilote
// l'application Tauri buildée (fenêtre WebView2 réelle), contrairement aux E2E
// Playwright qui pilotent le build web en mode démonstration.
//
// MODE « ATTACH » (Windows). Le test lance lui-même l'app, construite avec la
// surcharge de `tauri-e2e-config.cjs` qui ouvre son port de débogage, puis
// msedgedriver s'y ATTACHE (`ms:edgeOptions.debuggerAddress`). Jusqu'au
// 25/09/2026, tauri-driver laissait msedgedriver LANCER l'app : sur une
// WebView2 récente, le port demandé par variable d'environnement ne s'ouvre
// plus, et la session échouait sur « DevToolsActivePort file doesn't exist ».
// L'explication complète est dans l'en-tête de `tauri-e2e-config.cjs`.
//
// Prérequis (voir README) : le build de test de l'app, et msedgedriver à la
// version EXACTE de la WebView2, sur le PATH ou dans MSEDGEDRIVER.

const { spawn } = require("child_process");
const { resolve } = require("path");
const fs = require("fs");
const { DEBUG_PORT } = require("./tauri-e2e-config.cjs");

// Binaire produit par `tauri build` (nom de produit = mister-commitia).
const candidates = [
  resolve(__dirname, "..", "..", "..", "target", "release", "mister-commitia.exe"),
  resolve(__dirname, "..", "..", "..", "target", "release", "mc-desktop.exe"),
];
const application = candidates.find((p) => fs.existsSync(p)) || candidates[0];

const DRIVER_PORT = 4444;
const driverBin = process.env.MSEDGEDRIVER || "msedgedriver";

let app;
let driver;

/** Attend qu'une URL réponde 200, ou lève avec un message qui dit quoi. */
async function attendre(url, delaiMs, quoi) {
  const fin = Date.now() + delaiMs;
  while (Date.now() < fin) {
    try {
      const reponse = await fetch(url, { signal: AbortSignal.timeout(2000) });
      if (reponse.ok) return;
    } catch {
      // pas encore prêt
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(`${quoi} ne répond pas après ${delaiMs / 1000} s (${url})`);
}

exports.config = {
  specs: ["./specs/**/*.cjs"],
  maxInstances: 1,
  capabilities: [
    {
      browserName: "webview2",
      "ms:edgeOptions": { debuggerAddress: `127.0.0.1:${DEBUG_PORT}` },
      // WebDriver classique : l'attache à une WebView2 existante ne passe pas
      // par BiDi, que WebdriverIO 9 demanderait sinon d'office.
      "wdio:enforceWebDriverClassic": true,
    },
  ],
  reporters: ["spec"],
  framework: "mocha",
  mochaOpts: { ui: "bdd", timeout: 120000 },
  hostname: "127.0.0.1",
  port: DRIVER_PORT,

  onPrepare: async () => {
    if (!fs.existsSync(application)) {
      throw new Error(`application introuvable : ${application} (build de test, voir README)`);
    }
    app = spawn(application, [], { stdio: "ignore" });
    // Le port ne s'ouvre que si le binaire est le BUILD DE TEST : un build de
    // release attendrait ici jusqu'au délai, et le message le dit.
    await attendre(
      `http://127.0.0.1:${DEBUG_PORT}/json/version`,
      60000,
      `Le port de débogage de l'app (build de test ?)`,
    );
    driver = spawn(driverBin, [`--port=${DRIVER_PORT}`], { stdio: "inherit" });
    await attendre(`http://127.0.0.1:${DRIVER_PORT}/status`, 30000, "msedgedriver");
  },
  onComplete: () => {
    if (driver) driver.kill();
    if (app) app.kill();
  },
};
