import { expect, test, type Page } from "@playwright/test";
import { pathToFileURL } from "node:url";
import { reportPath } from "./globalSetup";

const reportUrl = pathToFileURL(reportPath).href;

async function openReport(page: Page, suffix = "") {
  const errors: string[] = [];
  page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(`${reportUrl}${suffix}`);
  await expect(page.getByRole("heading", { name: "Reforge Evidence Workbench" })).toBeVisible();
  expect(errors).toEqual([]);
}

test("renders a selected schema 27 issue with readable evidence and folded coverage", async ({ page }) => {
  await openReport(page);
  await expect(page.getByRole("listbox", { name: "Issue list" })).toBeVisible();
  await expect(page.locator(".issue-option[aria-selected=true]")).toHaveCount(1);
  await expect(page.locator(".issue-detail")).toBeVisible();
  await expect(page.locator(".evidence").first()).toBeVisible();
  await expect(page.locator(".locations code").first()).toBeVisible();
  await expect(page.locator(".measurements").first()).toBeVisible();
  await expect(page.locator(".coverage-panel")).not.toHaveAttribute("open", "");
  await page.locator(".coverage-panel summary").click();
  await expect(page.getByText("Direct Calls: Partial").first()).toBeVisible();
  await expect(page.getByText(/unresolved_direct_call/).first()).toBeVisible();
  await expect(page.getByText(/priority|severity|hotspot|watchlist/i)).toHaveCount(0);
});

test("selects issues, persists hash deep links, and falls back from an invalid hash", async ({ page }) => {
  await openReport(page);
  const options = page.locator(".issue-option");
  expect(await options.count()).toBeGreaterThan(1);
  const second = options.nth(1);
  const title = (await second.locator("strong").textContent())!;
  await second.click();
  await expect(page.locator("#issue-detail-title")).toHaveText(title);
  await expect.poll(() => decodeURIComponent(new URL(page.url()).hash)).toContain("#issue=ri7-");
  const deepLink = page.url();
  await page.reload();
  expect(page.url()).toBe(deepLink);
  await expect(page.locator("#issue-detail-title")).toHaveText(title);
  await page.goto(`${reportUrl}#issue=not-a-real-issue`);
  await expect(page.locator(".issue-option[aria-selected=true]")).toHaveCount(1);
  expect(page.url()).not.toContain("not-a-real-issue");
});

test("combines filters, reselects the first result, and exposes an empty state", async ({ page }) => {
  await openReport(page);
  await page.getByLabel("Kind").selectOption("advisory");
  await expect(page.locator(".issue-option[aria-selected=true]")).toHaveCount(1);
  await page.getByLabel("Filter issues").fill("no-such-issue-token");
  await expect(page.getByText("No issues match these filters.")).toBeVisible();
  await expect(page.locator(".issue-detail")).toHaveCount(0);
  await page.getByRole("button", { name: "Clear filters" }).click();
  await expect(page.locator(".issue-option[aria-selected=true]")).toHaveCount(1);
});

test("supports arrow-key issue navigation", async ({ page }) => {
  await openReport(page);
  const options = page.locator(".issue-option");
  await options.first().focus();
  await page.keyboard.press("ArrowDown");
  await expect(options.nth(1)).toBeFocused();
  await expect(options.nth(1)).toHaveAttribute("aria-selected", "true");
});

test("has no horizontal overflow on mobile and moves focus to selected detail", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openReport(page);
  await page.locator(".issue-option").nth(1).click();
  await expect(page.locator("#issue-detail-title")).toBeFocused();
  const widths = await page.evaluate(() => [document.documentElement.scrollWidth, document.documentElement.clientWidth]);
  expect(widths[0]).toBeLessThanOrEqual(widths[1]);
});

test("switches locale, persists it, and lets query override storage", async ({ page }) => {
  await openReport(page);
  const originalTitle = await page.locator("#issue-detail-title").textContent();
  await page.getByLabel("Report language").selectOption("zh-CN");
  await expect(page.getByRole("listbox", { name: "问题列表" })).toBeVisible();
  await expect(page.locator("#issue-detail-title")).toHaveText(originalTitle!);
  await page.reload();
  await expect(page.getByRole("listbox", { name: "问题列表" })).toBeVisible();
  await page.goto(`${reportUrl}?lang=en`);
  await expect(page.getByRole("listbox", { name: "Issue list" })).toBeVisible();
});
