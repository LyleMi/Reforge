import { execFileSync } from "node:child_process";
import { cpSync, mkdirSync } from "node:fs";
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
      "reforge-cli",
      "--manifest-path",
      resolve(repositoryRoot, "Cargo.toml"),
      "--",
      "analyze",
      resolve(reportAppRoot, "src"),
      "--analysis",
      "codebase",
      "--analysis",
      "dataflow",
      "--set",
      "codebase.max-file-lines=1",
      "--set",
      "codebase.max-function-lines=1",
      "--set",
      "codebase.max-imports=1",
      "--set",
      "rules.enable=[\"reforge.codebase.large_file\",\"reforge.codebase.long_function\",\"reforge.codebase.import_heavy_file\"]",
      "--output",
      "html",
      "--output-file",
      reportPath,
    ],
    { cwd: repositoryRoot, stdio: "inherit" },
  );

  const siteRoot = resolve(repositoryRoot, "target/playwright/site");
  cpSync(resolve(repositoryRoot, "playground"), resolve(siteRoot, "playground"), { recursive: true });
  mkdirSync(resolve(siteRoot, "assets"), { recursive: true });
  cpSync(resolve(repositoryRoot, "assets/reforge-logo.png"), resolve(siteRoot, "assets/reforge-logo.png"));
  for (const scenario of ["typescript-boundary-bypass", "python-shadowed-abstraction", "typescript-cycle"]) {
    const fixture = resolve(repositoryRoot, "playground/fixtures", scenario);
    const output = resolve(siteRoot, "playground/reports", scenario, "index.html");
    mkdirSync(dirname(output), { recursive: true });
    execFileSync(
      "cargo",
      [
        "run", "--locked", "--quiet", "-p", "reforge-cli", "--manifest-path", resolve(repositoryRoot, "Cargo.toml"), "--",
        "analyze", resolve(fixture, "after"), "--config", resolve(fixture, "reforge.toml"), "--output", "html", "--output-file", output, "--reproducible",
      ],
      { cwd: repositoryRoot, stdio: "inherit" },
    );
  }
}
