import { spawn } from "node:child_process";
import { cpSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const reportAppRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = resolve(reportAppRoot, "../..");
const siteRoot = resolve(repositoryRoot, "target/playwright/site");
mkdirSync(siteRoot, { recursive: true });
cpSync(resolve(repositoryRoot, "playground"), resolve(siteRoot, "playground"), { recursive: true });

const server = spawn("python3", ["-m", "http.server", "4174", "--bind", "127.0.0.1", "--directory", siteRoot], { stdio: "inherit" });
for (const signal of ["SIGINT", "SIGTERM"]) process.on(signal, () => server.kill(signal));
server.on("exit", code => process.exit(code ?? 0));
