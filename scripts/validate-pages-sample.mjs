#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const MIN_ISSUES = 8;
const MAX_ISSUES = 40;
const MIN_EVIDENCE_RULES = 3;
const REPORT_DATA_OPEN =
  '<script id="reforge-report-data" type="application/json">';
const REPORT_DATA_CLOSE = "</script>";
const GENERATED_REPORT_ASSETS = [
  "assets/report-app.js",
  "crates/reforge-output/assets/report-app.js",
];

const reportPath = process.argv[2];
if (!reportPath) {
  console.error("usage: node scripts/validate-pages-sample.mjs <report.html>");
  process.exit(2);
}

function normalizePath(path) {
  return path.replaceAll("\\", "/").replace(/^\.\//, "");
}

function isGeneratedReportAsset(path) {
  const normalized = normalizePath(path);
  return GENERATED_REPORT_ASSETS.some(
    (asset) => normalized === asset || normalized.endsWith(`/${asset}`),
  );
}

function subjectPaths(subject) {
  if (typeof subject?.entity?.path === "string") {
    return [subject.entity.path];
  }
  if (Array.isArray(subject?.members)) {
    return subject.members
      .map((member) => member?.path)
      .filter((path) => typeof path === "string");
  }
  return [];
}

function extractReport(html) {
  const payloadStart = html.indexOf(REPORT_DATA_OPEN);
  if (payloadStart === -1) {
    throw new Error("missing reforge-report-data JSON script");
  }
  const jsonStart = payloadStart + REPORT_DATA_OPEN.length;
  const jsonEnd = html.indexOf(REPORT_DATA_CLOSE, jsonStart);
  if (jsonEnd === -1) {
    throw new Error("unterminated reforge-report-data JSON script");
  }
  return JSON.parse(html.slice(jsonStart, jsonEnd));
}

let report;
try {
  report = extractReport(await readFile(reportPath, "utf8"));
} catch (error) {
  console.error(`Pages sample validation failed: ${error.message}`);
  process.exit(1);
}

const errors = [];
const issues = Array.isArray(report.issues) ? report.issues : [];
if (issues.length < MIN_ISSUES || issues.length > MAX_ISSUES) {
  errors.push(
    `expected ${MIN_ISSUES}-${MAX_ISSUES} issues, found ${issues.length}`,
  );
}

const evidenceRules = new Set();
const generatedAssetReferences = new Set();
for (const issue of issues) {
  for (const path of subjectPaths(issue.subject)) {
    if (isGeneratedReportAsset(path)) {
      generatedAssetReferences.add(path);
    }
  }
  for (const evidence of Array.isArray(issue.evidence) ? issue.evidence : []) {
    if (typeof evidence.rule === "string") {
      evidenceRules.add(evidence.rule);
    }
    for (const location of Array.isArray(evidence.locations)
      ? evidence.locations
      : []) {
      if (
        typeof location?.path === "string" &&
        isGeneratedReportAsset(location.path)
      ) {
        generatedAssetReferences.add(location.path);
      }
    }
  }
}

if (evidenceRules.size < MIN_EVIDENCE_RULES) {
  errors.push(
    `expected at least ${MIN_EVIDENCE_RULES} evidence rules, found ${evidenceRules.size}`,
  );
}

const scannedFiles = report.coverage?.codebase?.scanned_files;
if (!Number.isInteger(scannedFiles) || scannedFiles <= 0) {
  errors.push(`expected nonzero Codebase scanned files, found ${scannedFiles ?? 0}`);
}

if (generatedAssetReferences.size > 0) {
  errors.push(
    `generated report assets appear in findings: ${[...generatedAssetReferences].join(
      ", ",
    )}`,
  );
}

if (errors.length > 0) {
  console.error("Pages sample validation failed:");
  for (const error of errors) {
    console.error(`- ${error}`);
  }
  process.exit(1);
}

console.log(
  `Pages sample validated: ${issues.length} issues, ${evidenceRules.size} evidence rules, ${scannedFiles} scanned files.`,
);
