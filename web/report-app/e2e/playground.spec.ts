import { expect, test } from "@playwright/test";

const playgroundUrl = "http://127.0.0.1:4174/playground/";

test("shows all scenarios, full source and configuration, and opens each report", async ({ page }) => {
  await page.goto(playgroundUrl);
  const cards = page.locator(".scenario-card");
  await expect(cards).toHaveCount(3);
  for (const scenario of ["rust-similarity", "typescript-cycle", "python-long-function"]) {
    await page.locator(`[data-scenario="${scenario}"]`).click();
    await expect(page.locator("#source-files code").first()).not.toBeEmpty();
    await expect(page.locator("#scenario-config")).toContainText("version = 2");
    const href = await page.locator("#report-link").getAttribute("href");
    expect(href).toBe(`reports/${scenario}/?lang=en`);
    await page.locator("#report-link").click();
    await expect(page.getByRole("heading", { name: "Refactoring evidence" })).toBeVisible();
    await page.goBack();
  }
});

test("switches languages and passes the locale to reports", async ({ page }) => {
  await page.goto(`${playgroundUrl}?scenario=typescript-cycle`);
  await page.getByLabel("Language").selectOption("zh-CN");
  await expect(page.getByRole("heading", { name: "选择一个场景" })).toBeVisible();
  await expect(page.locator("#report-link")).toHaveAttribute("href", "reports/typescript-cycle/?lang=zh-CN");
  await page.reload();
  await expect(page.getByRole("heading", { name: "选择一个场景" })).toBeVisible();
  await page.goto(`${playgroundUrl}?scenario=rust-similarity&lang=en`);
  await expect(page.getByRole("heading", { name: "Choose a scenario" })).toBeVisible();
});

test("has no horizontal overflow on mobile", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(`${playgroundUrl}?scenario=typescript-cycle&lang=zh-CN`);
  await expect(page.locator("#source-files code").first()).not.toBeEmpty();
  const widths = await page.evaluate(() => [document.documentElement.scrollWidth, document.documentElement.clientWidth]);
  expect(widths[0]).toBeLessThanOrEqual(widths[1]);
});
