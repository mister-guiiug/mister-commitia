import { defineConfig, devices } from "@playwright/test";

// E2E de l'interface (T7) : pilote le build de production servi en mode
// démonstration (mock IPC) via un navigateur headless — famille WebDriver,
// fiable en CI. Complète le harnais desktop natif (e2e-native/, tauri-driver).
export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "line" : "list",
  use: {
    baseURL: "http://localhost:4173",
    trace: "on-first-retry",
    // Déterminisme : neutralise les animations/transitions (l'app honore déjà
    // `prefers-reduced-motion`) — évite les échecs « element not stable » en CI.
    reducedMotion: "reduce",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "npm run build && npm run preview -- --port 4173 --strictPort",
    url: "http://localhost:4173",
    timeout: 180_000,
    reuseExistingServer: !process.env.CI,
  },
});
