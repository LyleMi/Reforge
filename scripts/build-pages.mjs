#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { cp, mkdir, mkdtemp, rm } from "node:fs/promises";
import { delimiter, resolve } from "node:path";
import { tmpdir } from "node:os";
import { scenarios as playgroundScenarioDefinitions } from "../playground/scenarios.js";

const root = resolve(import.meta.dirname, "..");
const site = resolve(root, process.argv[2] ?? "_site");
const configuredBinary = process.env.REFORGE_BIN;
const binary = configuredBinary ? resolve(root, configuredBinary) : resolve(root, "target/release/reforge");

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit", env: { ...process.env, PATH: process.env.PATH?.split(delimiter).join(delimiter) } });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

if (!configuredBinary) run("cargo", ["build", "--locked", "--release", "-p", "reforge-cli"]);
run("mdbook", ["build", "--dest-dir", site]);
await cp(resolve(root, "playground"), resolve(site, "playground"), { recursive: true });
await mkdir(resolve(site, "sample"), { recursive: true });

run(binary, ["analyze", ".", "--config", ".github/pages/reforge.toml", "--output", "html", "--output-file", resolve(site, "sample/index.html"), "--reproducible"]);
run("node", ["scripts/validate-pages-sample.mjs", resolve(site, "sample/index.html")]);

const playgroundScenarios = Object.keys(playgroundScenarioDefinitions);
const validationDirectory = await mkdtemp(resolve(tmpdir(), "reforge-playground-"));

for (const scenario of playgroundScenarios) {
  const fixture = resolve(root, "playground/fixtures", scenario);
  const report = resolve(site, "playground/reports", scenario, "index.html");
  const beforeReport = resolve(validationDirectory, `${scenario}-before.json`);
  await mkdir(resolve(report, ".."), { recursive: true });
  run(binary, ["analyze", resolve(fixture, "before"), "--config", resolve(fixture, "reforge.toml"), "--output", "json", "--output-file", beforeReport, "--reproducible"]);
  run(binary, ["analyze", resolve(fixture, "after"), "--config", resolve(fixture, "reforge.toml"), "--output", "html", "--output-file", report, "--reproducible"]);
  run("node", ["scripts/validate-playground-report.mjs", beforeReport, report, scenario]);
}
await rm(validationDirectory, { recursive: true, force: true });

console.log(`Pages site built at ${site}`);
