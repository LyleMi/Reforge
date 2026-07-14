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
      "--manifest-path",
      resolve(repositoryRoot, "Cargo.toml"),
      "--",
      "scan",
      resolve(reportAppRoot, "src"),
      "--max-file-lines",
      "1",
      "--max-function-lines",
      "1",
      "--max-imports",
      "1",
      "--churn",
      "off",
      "--output",
      "html",
      "--output-file",
      reportPath,
      "--progress",
      "never",
      "--color",
      "never",
    ],
    { cwd: repositoryRoot, stdio: "inherit" },
  );
}
