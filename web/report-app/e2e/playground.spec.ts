import { expect, test } from "@playwright/test";

const playgroundUrl = "http://127.0.0.1:4174/playground/";
const rules = {
  "typescript-boundary-bypass": "reforge.dataflow.adapter_flow_bypass",
  "python-shadowed-abstraction": "reforge.codebase.shadowed_abstraction",
  "typescript-cycle": "reforge.codebase.dependency_cycle",
};

test("introduces the prepared example before presenting its patch", async ({ page }) => {
  await page.goto(`${playgroundUrl}?scenario=typescript-boundary-bypass&lang=en`);
  await expect(page.locator(".fixture-note")).toContainText("built-in fixtures");
  await expect(page.locator(".selected-label")).toHaveText("Selected");
  await expect(page.locator("#review-title")).toHaveText("Bypasses an existing boundary");
  await expect(page.locator("#review-setup")).toContainText("needs refund support");
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
  await page.locator('[data-scenario="typescript-boundary-bypass"]').click();
  await expect(page.locator(".witness")).toContainText("exact");
  await page.locator('[data-scenario="python-shadowed-abstraction"]').click();
  await expect(page.locator(".implementation-group button")).toHaveCount(3);
  await page.locator('[data-scenario="typescript-cycle"]').click();
  await expect(page.locator(".cycle-ring span")).toHaveCount(3);
});

test("shows patch, repository context, full source, config, and command", async ({ page }) => {
  await page.goto(`${playgroundUrl}?scenario=typescript-cycle&lang=en`);
  await expect(page.locator("#patch-files .diff-addition").filter({ hasText: "hasCompletedOrder" }).first()).toBeVisible();
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
  await page.goto(`${playgroundUrl}?scenario=typescript-boundary-bypass&lang=en`);
  await page.locator('#evidence-locations [data-evidence-path="src/application_refunds.ts"]').first().click();
  await expect(page.getByRole("tab", { name: "Patch" })).toHaveAttribute("aria-selected", "true");
  await expect(page.locator('#patch-files [data-source-path="src/application_refunds.ts"]')).toBeVisible();
  await page.locator('#evidence-locations [data-evidence-path="src/transport.ts"]').first().click();
  await expect(page.getByRole("tab", { name: "Full source" })).toHaveAttribute("aria-selected", "true");
  await expect(page.locator('#source-files [data-new-line="1"]')).toBeVisible();
});

test("switches languages and persists scenario and locale in the URL", async ({ page }) => {
  await page.goto(`${playgroundUrl}?scenario=typescript-cycle`);
  await page.getByLabel("Language").selectOption("zh-CN");
  await expect(page.getByRole("heading", { name: "选择一个审查示例" })).toBeVisible();
  await expect(page.locator("#report-link")).toHaveAttribute("href", "reports/typescript-cycle/?lang=zh-CN");
  await page.reload();
  await expect(page.getByRole("heading", { name: "选择一个审查示例" })).toBeVisible();
  await page.goto(`${playgroundUrl}?scenario=typescript-boundary-bypass&lang=en`);
  await expect(page.getByRole("heading", { name: "Choose a review to explore" })).toBeVisible();
});

test("keeps all repository review material usable when report loading fails", async ({ page }) => {
  await page.route("**/playground/reports/**", route => route.abort());
  await page.goto(`${playgroundUrl}?scenario=python-shadowed-abstraction&lang=en`);
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
  await page.goto(`${playgroundUrl}?scenario=typescript-cycle&lang=zh-CN`);
  await expect(page.locator("#scenario-rule")).toHaveText(rules["typescript-cycle"]);
  const order = await page.evaluate(() => [".review-intro", ".review-brief", ".code-pane", ".result-pane"].map(selector => getComputedStyle(document.querySelector(selector)!).order));
  expect(order).toEqual(["0", "1", "2", "3"]);
  const widths = await page.evaluate(() => [document.documentElement.scrollWidth, document.documentElement.clientWidth]);
  expect(widths[0]).toBeLessThanOrEqual(widths[1]);
});
