import { expect, test } from "@playwright/test";

const playgroundUrl = "http://127.0.0.1:4174/playground/";
const rules = {
  "typescript-service-bypass": "reforge.dataflow.adapter_flow_bypass",
  "python-duplicated-validation": "reforge.codebase.shadowed_abstraction",
  "typescript-helper-cycle": "reforge.codebase.dependency_cycle",
};

test("establishes the review context before asking users to choose a change", async ({ page }) => {
  await page.goto(`${playgroundUrl}?lang=en`);
  await expect(page.getByRole("heading", { name: "Simulate one pull-request review" })).toBeVisible();
  await expect(page.locator(".context-steps li")).toHaveCount(3);
  await expect(page.getByRole("heading", { name: "Which change do you want to review?" })).toBeVisible();
  await expect(page.locator(".fixture-note.light")).toContainText("built-in fixtures");
  await expect(page.locator("#scenario-detail")).toBeHidden();
  await expect(page.locator(".selected-label")).toHaveCount(0);
  await page.locator('[data-scenario="typescript-service-bypass"]').click();
  await expect(page.locator(".selected-label")).toHaveText("Reviewing");
  await expect(page.locator("#review-title")).toHaveText("Bypasses the service layer");
  await expect(page.locator("#review-setup")).toContainText("user requests through a service layer");
  await expect(page.locator("#reading-path")).toHaveAttribute("aria-label", "How to read this example");
  await expect(page.locator("#reading-path li")).toHaveCount(3);
});

test("loads each real report and its dedicated evidence view", async ({ page }) => {
  await page.goto(playgroundUrl);
  await expect(page.locator(".scenario-card")).toHaveCount(3);
  for (const [scenario, rule] of Object.entries(rules)) {
    await page.locator(`[data-scenario="${scenario}"]`).click();
    await expect(page.locator("#scenario-rule")).toHaveText(rule);
    await expect(page.locator("#plain-explanation")).not.toBeEmpty();
    await expect(page.locator("#patch-files code")).not.toBeEmpty();
    await expect(page.locator("#coverage-status")).toContainText("Observed");
    await expect(page.locator("#report-link")).toHaveAttribute("href", `reports/${scenario}/?lang=en`);
  }
  await page.locator('[data-scenario="typescript-service-bypass"]').click();
  await expect(page.locator(".witness")).toContainText("exact");
  await page.locator('[data-scenario="python-duplicated-validation"]').click();
  await expect(page.locator(".implementation-group button")).toHaveCount(3);
  await page.locator('[data-scenario="typescript-helper-cycle"]').click();
  await expect(page.locator(".cycle-ring span")).toHaveCount(3);
});

test("shows patch, repository context, full source, config, and command", async ({ page }) => {
  await page.goto(`${playgroundUrl}?scenario=typescript-helper-cycle&lang=en`);
  await expect(page.locator("#patch-files .diff-addition").filter({ hasText: "requestLocale" }).first()).toBeVisible();
  await page.getByRole("tab", { name: "Repository context" }).click();
  await expect(page.locator("#context-note")).toContainText("before the patch");
  await expect(page.locator("#context-files code")).not.toBeEmpty();
  await page.getByRole("tab", { name: "Full source" }).click();
  await expect(page.locator("#source-files code")).not.toBeEmpty();
  await page.getByRole("tab", { name: "Config" }).click();
  await expect(page.locator("#scenario-config")).toContainText("version = 2");
  await page.getByRole("tab", { name: "Command" }).click();
  await expect(page.locator("#scenario-command")).toContainText("/after");
  await expect(page.locator("#scenario-command")).toContainText("--reproducible");
  await expect(page.locator("#scenario-command")).not.toContainText("+");
});

test("evidence locations select the matching patch or source line", async ({ page }) => {
  await page.goto(`${playgroundUrl}?scenario=typescript-service-bypass&lang=en`);
  await page.locator('#evidence-locations [data-evidence-path="src/user_search_route.ts"]').first().click();
  await expect(page.getByRole("tab", { name: "Patch" })).toHaveAttribute("aria-selected", "true");
  await expect(page.locator('#patch-files [data-source-path="src/user_search_route.ts"]')).toBeVisible();
  await page.locator('#evidence-locations [data-evidence-path="src/database.ts"]').first().click();
  await expect(page.getByRole("tab", { name: "Full source" })).toHaveAttribute("aria-selected", "true");
  await expect(page.locator('#source-files [data-new-line="1"]')).toBeVisible();
});

test("switches languages and persists scenario and locale in the URL", async ({ page }) => {
  await page.goto(`${playgroundUrl}?scenario=typescript-helper-cycle`);
  await page.getByLabel("Language").selectOption("zh-CN");
  await expect(page.getByRole("heading", { name: "你想审查哪一份变更？" })).toBeVisible();
  await expect(page.locator("#report-link")).toHaveAttribute("href", "reports/typescript-helper-cycle/?lang=zh-CN");
  await page.reload();
  await expect(page.getByRole("heading", { name: "你想审查哪一份变更？" })).toBeVisible();
  await page.goto(`${playgroundUrl}?scenario=typescript-service-bypass&lang=en`);
  await expect(page.getByRole("heading", { name: "Which change do you want to review?" })).toBeVisible();
});

test("keeps all repository review material usable when report loading fails", async ({ page }) => {
  await page.route("**/playground/reports/**", route => route.abort());
  await page.goto(`${playgroundUrl}?scenario=python-duplicated-validation&lang=en`);
  await expect(page.locator("#result-error")).toContainText("could not be loaded");
  await expect(page.locator("#patch-files code")).not.toBeEmpty();
  await page.getByRole("tab", { name: "Repository context" }).click();
  await expect(page.locator("#context-files code")).not.toBeEmpty();
  await page.getByRole("tab", { name: "Config" }).click();
  await expect(page.locator("#scenario-config")).toContainText("shadowed_abstraction");
  await page.getByRole("tab", { name: "Command" }).click();
  await expect(page.locator("#scenario-command")).toContainText("/after");
});

test("has no horizontal overflow and follows the mobile review order", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(`${playgroundUrl}?scenario=typescript-helper-cycle&lang=zh-CN`);
  await expect(page.locator("#scenario-rule")).toHaveText(rules["typescript-helper-cycle"]);
  const order = await page.evaluate(() => [".review-intro", ".review-brief", ".code-pane", ".result-pane"].map(selector => getComputedStyle(document.querySelector(selector)!).order));
  expect(order).toEqual(["0", "1", "2", "3"]);
  const widths = await page.evaluate(() => [document.documentElement.scrollWidth, document.documentElement.clientWidth]);
  expect(widths[0]).toBeLessThanOrEqual(widths[1]);
});
