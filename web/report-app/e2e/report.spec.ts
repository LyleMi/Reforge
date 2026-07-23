import { expect, test, type Page } from "@playwright/test";
import { pathToFileURL } from "node:url";
import { reportPath } from "./globalSetup";

const reportUrl = pathToFileURL(reportPath).href;

async function openReport(page: Page) {
  const errors: string[] = [];
  page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(reportUrl);
  await expect(page.getByRole("heading", { name: "Refactoring issues" })).toBeVisible();
  expect(errors).toEqual([]);
}

test("renders schema 26 issues, nested evidence, and coverage", async ({ page }) => {
  await openReport(page);
  await expect(page.getByText("Schema 26 analysis report")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Coverage", exact: true })).toBeVisible();
  await expect(page.locator(".issue").first()).toBeVisible();
  await page.locator(".evidence summary").first().click();
  await expect(page.locator(".evidence[open]").first()).toBeVisible();
  await expect(page.getByText(/priority|severity|hotspot|watchlist/i)).toHaveCount(0);
});

test("filters issues without a server", async ({ page }) => {
  await openReport(page);
  const filter = page.getByLabel("Filter issues");
  await filter.fill("no-such-issue-token");
  await expect(page.getByText("No issues reported.")).toBeVisible();
});

test("has no horizontal overflow on mobile", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openReport(page);
  const widths = await page.evaluate(() => [document.documentElement.scrollWidth, document.documentElement.clientWidth]);
  expect(widths[0]).toBeLessThanOrEqual(widths[1]);
});
