import { expect, test, type Page } from "@playwright/test";
import { pathToFileURL } from "node:url";
import { reportPath } from "./globalSetup";

const reportUrl = pathToFileURL(reportPath).href;
async function openReport(page: Page, hash = "") {
  const errors: string[] = [];
  page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(`${reportUrl}${hash}`);
  await expect(page.getByRole("heading", { name: "Refactoring review" })).toBeVisible();
  expect(errors).toEqual([]);
}

test("opens overview and navigates all four views", async ({ page }) => {
  await openReport(page);
  await expect(page.getByRole("tab", { name: "Overview" })).toHaveAttribute("aria-selected", "true");
  for (const name of ["Issues", "Code map", "Metrics"]) {
    await page.getByRole("tab", { name: new RegExp(`^${name}`) }).click();
    await expect(page.getByRole("tab", { name: new RegExp(`^${name}`) })).toHaveAttribute("aria-selected", "true");
  }
});

test("restores issue filters from a deep link", async ({ page }) => {
  await openReport(page, "#issues?query=main&severity=critical&kind=large_file&sort=path");
  await expect(page.getByPlaceholder("Search findings")).toHaveValue("main");
  await expect(page.getByLabel("Finding severity")).toHaveValue("critical");
  await expect(page.getByLabel("Finding kind")).toHaveValue("large_file");
  await expect(page.getByLabel("Sort findings")).toHaveValue("path");
  await page.reload();
  await expect(page.getByPlaceholder("Search findings")).toHaveValue("main");
});

test("changes map layers and opens a file inspector", async ({ page }) => {
  await openReport(page, "#map");
  await page.getByRole("button", { name: "Severity", exact: true }).click();
  await expect(page).toHaveURL(/#map\?layer=severity$/);
  const treemap = page.getByRole("group", { name: /repository treemap/i });
  await treemap.getByRole("button").first().click();
  await expect(page.getByRole("complementary", { name: "File inspector" })).toBeVisible();
});

test("has no horizontal overflow on mobile", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openReport(page);
  const widths = await page.evaluate(() => [document.documentElement.scrollWidth, document.documentElement.clientWidth]);
  expect(widths[0]).toBeLessThanOrEqual(widths[1]);
});
