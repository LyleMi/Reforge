import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const reportPath = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../../../target/playwright/reforge-report.html",
);

export default function generateReport() {
  const reportAppRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const repositoryRoot = resolve(reportAppRoot, "../..");
  mkdirSync(dirname(reportPath), { recursive: true });

  execFileSync(
    "cargo",
    [
      "run",
      "--locked",
      "--quiet",
      "-p",
      "reforge",
      "--manifest-path",
      resolve(repositoryRoot, "Cargo.toml"),
      "--",
      "analyze",
      resolve(reportAppRoot, "src"),
      "--analysis",
      "codebase",
      "--set",
      "codebase.max-file-lines=1",
      "--set",
      "codebase.max-function-lines=1",
      "--set",
      "codebase.max-imports=1",
      "--output",
      "html",
      "--output-file",
      reportPath,
    ],
    { cwd: repositoryRoot, stdio: "inherit" },
  );
}
