// Test de fumée desktop natif : l'application se lance et affiche sa
// navigation. `$` / `expect` sont injectés par WebdriverIO (framework mocha).

describe("mister-commitia desktop", function () {
  it("affiche la navigation principale", async function () {
    const repos = await $("button=Dépôts");
    await repos.waitForExist({ timeout: 20000 });
    await expect(repos).toBeExisting();

    const ci = await $("button=CI/CD");
    await expect(ci).toBeExisting();
  });
});
