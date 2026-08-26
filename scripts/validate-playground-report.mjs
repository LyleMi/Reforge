#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const scenarios = {
  "typescript-boundary-bypass": {
    analysis: "dataflow",
    rule: "reforge.dataflow.adapter_flow_bypass",
    files: ["src/application_checkout.ts", "src/application_refunds.ts", "src/payment_gateway.ts", "src/transport.ts"],
  },
  "python-shadowed-abstraction": {
    analysis: "codebase",
    rule: "reforge.codebase.shadowed_abstraction",
    files: ["providers/legacy_webhook_helper.py", "providers/orbit_webhook_helper.py", "shared/event_normalizer.py"],
  },
  "typescript-cycle": {
    analysis: "codebase",
    rule: "reforge.codebase.dependency_cycle",
    files: ["src/checkout.ts", "src/pricing.ts", "src/promotions.ts"],
  },
};

const [beforePath, reportPath, scenarioId] = process.argv.slice(2);
const scenario = scenarios[scenarioId];
if (!beforePath || !reportPath || !scenario) {
  console.error("usage: node scripts/validate-playground-report.mjs <before.json> <after.html> <scenario-id>");
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
  const before = JSON.parse(await readFile(beforePath, "utf8"));
  const html = await readFile(reportPath, "utf8");
  const report = extractReport(html);
  const errors = [];
  if (before.schema_version !== 27) errors.push(`before: expected schema 27, found ${before.schema_version}`);
  if (before.issues?.length !== 0) errors.push(`before: expected zero Issues, found ${before.issues?.length ?? 0}`);
  if (report.schema_version !== 27) errors.push(`expected schema 27, found ${report.schema_version}`);
  if (before.coverage?.[scenario.analysis]?.status !== "observed") errors.push(`before: expected observed ${scenario.analysis} coverage, found ${before.coverage?.[scenario.analysis]?.status}`);
  if (report.coverage?.[scenario.analysis]?.status !== "observed") errors.push(`expected observed ${scenario.analysis} coverage, found ${report.coverage?.[scenario.analysis]?.status}`);
  if (report.issues?.length !== 1) errors.push(`expected exactly one Issue, found ${report.issues?.length ?? 0}`);
  const evidenceRules = new Set((report.issues ?? []).flatMap(issue => (issue.evidence ?? []).map(evidence => evidence.rule)));
  if (evidenceRules.size !== 1 || !evidenceRules.has(scenario.rule)) errors.push(`expected only ${scenario.rule}, found ${[...evidenceRules].join(", ")}`);
  const unexpectedPaths = reportPaths(report).filter(path => !scenario.files.includes(path));
  if (unexpectedPaths.length) errors.push(`unexpected fixture paths: ${[...new Set(unexpectedPaths)].join(", ")}`);
  const evidence = report.issues?.[0]?.evidence?.find(item => item.rule === scenario.rule);
  if (scenarioId === "typescript-boundary-bypass") {
    const witness = evidence?.witness;
    if (witness?.resolution !== "exact") errors.push(`expected exact witness, found ${witness?.resolution ?? "none"}`);
    if (!witness?.source || !witness?.sink || !witness?.ordered_steps?.length) errors.push("expected complete source, sink, and ordered witness steps");
    if (witness?.ordered_steps?.some(step => step.resolution !== "exact")) errors.push("expected every witness step to be exact");
    if (witness?.source?.path !== "src/application_refunds.ts") errors.push(`unexpected witness source ${witness?.source?.path ?? "none"}`);
    if (witness?.sink?.path !== "src/transport.ts") errors.push(`unexpected witness sink ${witness?.sink?.path ?? "none"}`);
  }
  if (scenarioId === "python-shadowed-abstraction") {
    const members = new Set(report.issues?.[0]?.subject?.members?.map(member => member.path));
    for (const path of scenario.files) if (!members.has(path)) errors.push(`missing shadowed abstraction member ${path}`);
  }
  if (scenarioId === "typescript-cycle") {
    const members = new Set(report.issues?.[0]?.subject?.members?.map(member => member.path));
    for (const path of scenario.files) if (!members.has(path)) errors.push(`missing dependency cycle member ${path}`);
    const edgeCount = evidence?.measurements?.find(item => item.name === "dependency.cycle_edges")?.value;
    if (edgeCount !== 3) errors.push(`expected 3 internal cycle edges, found ${edgeCount ?? "none"}`);
  }
  for (const otherId of Object.keys(scenarios).filter(id => id !== scenarioId)) {
    if (html.includes(`playground/fixtures/${otherId}`)) errors.push(`report references other fixture ${otherId}`);
  }
  if (errors.length) throw new Error(errors.join("; "));
  console.log(`${scenarioId}: before=0, after=1 ${scenario.rule}, observed coverage.`);
} catch (error) {
  console.error(`Playground report validation failed: ${error.message}`);
  process.exit(1);
}
