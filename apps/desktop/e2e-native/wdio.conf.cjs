// Configuration WebdriverIO pour les E2E DESKTOP natifs via tauri-driver (T7).
// Pilote l'application Tauri buildée (fenêtre WebView réelle), contrairement
// aux E2E Playwright qui pilotent le build web en mode démonstration.
//
// Prérequis (voir README) : `cargo install tauri-driver`, msedgedriver sur le
// PATH (Windows), et l'exe produit par `tauri build`.

const { spawn } = require("child_process");
const { resolve } = require("path");
const os = require("os");
const fs = require("fs");

// Binaire produit par `tauri build` (nom de produit = mister-commitia).
const candidates = [
  resolve(__dirname, "..", "..", "..", "target", "release", "mister-commitia.exe"),
  resolve(__dirname, "..", "..", "..", "target", "release", "mc-desktop.exe"),
];
const application = candidates.find((p) => fs.existsSync(p)) || candidates[0];

const home = process.env.USERPROFILE || process.env.HOME || os.homedir();
const tauriDriverBin = resolve(home, ".cargo", "bin", "tauri-driver");

let tauriDriver;

exports.config = {
  specs: ["./specs/**/*.cjs"],
  maxInstances: 1,
  capabilities: [{ "tauri:options": { application } }],
  reporters: ["spec"],
  framework: "mocha",
  mochaOpts: { ui: "bdd", timeout: 120000 },
  hostname: "127.0.0.1",
  port: 4444,

  // tauri-driver fait le pont entre WebDriver et la WebView de l'application.
  beforeSession: () => {
    tauriDriver = spawn(tauriDriverBin, [], {
      stdio: [null, process.stdout, process.stderr],
    });
  },
  afterSession: () => {
    if (tauriDriver) tauriDriver.kill();
  },
};
