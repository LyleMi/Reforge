import { expect, test } from "@playwright/test";

const playgroundUrl = "http://127.0.0.1:4174/playground/";
const rules = {
  "rust-similarity": "reforge.codebase.similar_functions",
  "typescript-cycle": "reforge.codebase.dependency_cycle",
  "python-long-function": "reforge.codebase.long_function",
};

test("parses the unique real issue for every scenario and links to each report", async ({ page }) => {
  await page.goto(playgroundUrl);
  await expect(page.locator(".scenario-card")).toHaveCount(3);
  for (const [scenario, rule] of Object.entries(rules)) {
    await page.locator(`[data-scenario="${scenario}"]`).click();
    await expect(page.locator("#scenario-rule")).toHaveText(rule);
    await expect(page.locator("#issue-title")).not.toBeEmpty();
    await expect(page.locator("#evidence-message")).not.toBeEmpty();
    await expect(page.locator("#source-files code")).not.toBeEmpty();
    await expect(page.locator(".evidence-line, .file-evidence").first()).toBeVisible();
    await expect(page.locator("#report-link")).toHaveAttribute("href", `reports/${scenario}/?lang=en`);
  }
});

test("switches source, config, and command tabs without a stray plus", async ({ page }) => {
  await page.goto(`${playgroundUrl}?scenario=typescript-cycle&lang=en`);
  await expect(page.locator("#scenario-rule")).toHaveText(rules["typescript-cycle"]);
  await page.getByRole("tab", { name: "Config" }).click();
  await expect(page.locator("#scenario-config")).toContainText("version = 2");
  await page.getByRole("tab", { name: "Command" }).click();
  await expect(page.locator("#scenario-command")).toContainText("--reproducible");
  await expect(page.locator("#scenario-command")).not.toContainText("+");
  await page.getByRole("tab", { name: "Source" }).click();
  await expect(page.locator("#file-tabs [role=tab]")).toHaveCount(3);
  await page.locator("#file-tabs [role=tab]").nth(1).click();
  await expect(page.locator("#file-tabs [aria-selected=true]")).toHaveCount(1);
});

test("switches languages and persists scenario and locale in the URL", async ({ page }) => {
  await page.goto(`${playgroundUrl}?scenario=typescript-cycle`);
  await page.getByLabel("Language").selectOption("zh-CN");
  await expect(page.getByRole("heading", { name: "选择分析场景" })).toBeVisible();
  await expect(page.locator("#report-link")).toHaveAttribute("href", "reports/typescript-cycle/?lang=zh-CN");
  await page.reload();
  await expect(page.getByRole("heading", { name: "选择分析场景" })).toBeVisible();
  await page.goto(`${playgroundUrl}?scenario=rust-similarity&lang=en`);
  await expect(page.getByRole("heading", { name: "Choose a scenario" })).toBeVisible();
});

test("keeps source usable when the generated report request fails", async ({ page }) => {
  await page.route("**/playground/reports/**", route => route.abort());
  await page.goto(`${playgroundUrl}?scenario=python-long-function&lang=en`);
  await expect(page.locator("#result-error")).toContainText("could not be loaded");
  await expect(page.locator("#source-files code")).not.toBeEmpty();
  await expect(page.locator("#report-link")).toHaveAttribute("href", "reports/python-long-function/?lang=en");
});

test("has no horizontal overflow and puts results before source on mobile", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(`${playgroundUrl}?scenario=typescript-cycle&lang=zh-CN`);
  await expect(page.locator("#scenario-rule")).toHaveText(rules["typescript-cycle"]);
  const order = await page.evaluate(() => {
    const result = document.querySelector(".result-pane");
    const code = document.querySelector(".code-pane");
    return [getComputedStyle(result!).order, getComputedStyle(code!).order];
  });
  expect(order).toEqual(["1", "2"]);
  const widths = await page.evaluate(() => [document.documentElement.scrollWidth, document.documentElement.clientWidth]);
  expect(widths[0]).toBeLessThanOrEqual(widths[1]);
});
