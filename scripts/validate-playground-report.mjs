#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const scenarios = {
  "rust-similarity": { rule: "reforge.codebase.similar_functions", files: ["src/providers.rs"] },
  "typescript-cycle": { rule: "reforge.codebase.dependency_cycle", files: ["src/checkout.ts", "src/pricing.ts", "src/promotions.ts"] },
  "python-long-function": { rule: "reforge.codebase.long_function", files: ["orders.py"] },
};

const [reportPath, scenarioId] = process.argv.slice(2);
const scenario = scenarios[scenarioId];
if (!reportPath || !scenario) {
  console.error("usage: node scripts/validate-playground-report.mjs <report.html> <scenario-id>");
  process.exit(2);
}

function extractReport(html) {
  const open = '<script id="reforge-report-data" type="application/json">';
  const start = html.indexOf(open);
  const end = html.indexOf("</script>", start + open.length);
  if (start < 0 || end < 0) throw new Error("missing embedded report JSON");
  return JSON.parse(html.slice(start + open.length, end));
}

function reportPaths(report) {
  const paths = [];
  for (const issue of report.issues ?? []) {
    if (issue.subject?.entity?.path) paths.push(issue.subject.entity.path);
    for (const member of issue.subject?.members ?? []) if (member.path) paths.push(member.path);
    for (const evidence of issue.evidence ?? []) {
      for (const location of evidence.locations ?? []) if (location.path) paths.push(location.path);
    }
  }
  return paths.map(path => path.replaceAll("\\", "/").replace(/^\.\//, ""));
}

try {
  const report = extractReport(await readFile(reportPath, "utf8"));
  const errors = [];
  if (report.schema_version !== 27) errors.push(`expected schema 27, found ${report.schema_version}`);
  if (report.coverage?.codebase?.status !== "observed") errors.push(`expected observed Codebase coverage, found ${report.coverage?.codebase?.status}`);
  if (report.issues?.length !== 1) errors.push(`expected exactly one Issue, found ${report.issues?.length ?? 0}`);
  const evidenceRules = new Set((report.issues ?? []).flatMap(issue => (issue.evidence ?? []).map(evidence => evidence.rule)));
  if (evidenceRules.size !== 1 || !evidenceRules.has(scenario.rule)) errors.push(`expected only ${scenario.rule}, found ${[...evidenceRules].join(", ")}`);
  const unexpectedPaths = reportPaths(report).filter(path => !scenario.files.includes(path));
  if (unexpectedPaths.length) errors.push(`unexpected fixture paths: ${[...new Set(unexpectedPaths)].join(", ")}`);
  const html = await readFile(reportPath, "utf8");
  for (const otherId of Object.keys(scenarios).filter(id => id !== scenarioId)) {
    if (html.includes(`playground/fixtures/${otherId}`)) errors.push(`report references other fixture ${otherId}`);
  }
  if (errors.length) throw new Error(errors.join("; "));
  console.log(`${scenarioId}: schema 27, observed coverage, one ${scenario.rule} Issue.`);
} catch (error) {
  console.error(`Playground report validation failed: ${error.message}`);
  process.exit(1);
}
