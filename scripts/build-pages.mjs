#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { cp, mkdir } from "node:fs/promises";
import { delimiter, resolve } from "node:path";

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

for (const scenario of ["rust-similarity", "typescript-cycle", "python-long-function"]) {
  const fixture = resolve(root, "playground/fixtures", scenario);
  const report = resolve(site, "playground/reports", scenario, "index.html");
  await mkdir(resolve(report, ".."), { recursive: true });
  run(binary, ["analyze", fixture, "--config", resolve(fixture, "reforge.toml"), "--output", "html", "--output-file", report, "--reproducible"]);
  run("node", ["scripts/validate-playground-report.mjs", report, scenario]);
}

console.log(`Pages site built at ${site}`);
