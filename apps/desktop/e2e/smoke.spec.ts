import { expect, test } from "@playwright/test";

// Parcours de fumée de l'UI en mode démonstration (mock IPC) : shell, thème,
// i18n, analyse, vue graphe. Couvre les fonctionnalités des lots 2 à 5.

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  // Fermer l'onboarding premier lancement s'il apparaît.
  const later = page.getByRole("button", { name: /Plus tard|Later/ });
  if (await later.isVisible().catch(() => false)) await later.click();
});

test("shell : navigation FR présente", async ({ page }) => {
  await expect(page.getByRole("button", { name: "Dépôts" })).toBeVisible();
  await expect(page.getByRole("button", { name: "CI/CD" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Journal" })).toBeVisible();
});

test("U2 : bascule de thème clair/sombre persistée", async ({ page }) => {
  const html = page.locator("html");
  await expect(html).toHaveAttribute("data-theme", "dark");
  await page.getByRole("button", { name: "Thème" }).click();
  await expect(html).toHaveAttribute("data-theme", "light");
  // Persistance après rechargement.
  await page.reload();
  await expect(html).toHaveAttribute("data-theme", "light");
  // Remettre sombre (light → system → dark).
  await page.getByRole("button", { name: "Thème" }).click();
  await page.getByRole("button", { name: "Thème" }).click();
  await expect(html).toHaveAttribute("data-theme", "dark");
});

test("F11 : bascule de langue FR ↔ EN sur la navigation", async ({ page }) => {
  await expect(page.getByRole("button", { name: "Dépôts" })).toBeVisible();
  await page.getByRole("button", { name: "Langue" }).click();
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page.getByRole("button", { name: "Repositories" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Settings" })).toBeVisible();
  await page.getByRole("button", { name: "Language" }).click();
  await expect(page.getByRole("button", { name: "Dépôts" })).toBeVisible();
});

test("F1 : analyse puis vue graphe SVG", async ({ page }) => {
  await page.getByRole("button", { name: /Analyse & plan/ }).click();
  // Attendre la FIN de l'analyse (mock : phases + progression) : la table des
  // commits est rendue et le layout s'est stabilisé (panneau de progression parti).
  await expect(page.locator("table tbody tr").first()).toBeVisible({ timeout: 25_000 });
  // Basculer en vue graphe (force : le toggle peut rester momentanément instable
  // le temps que le layout se stabilise après l'analyse).
  await page.getByRole("button", { name: "Graphe" }).click({ force: true });
  await expect(page.locator('svg[aria-label="Graphe des commits"]')).toBeVisible({ timeout: 10_000 });
  // Au moins un nœud (cercle) rendu.
  expect(await page.locator('svg[aria-label="Graphe des commits"] circle').count()).toBeGreaterThan(0);
});
