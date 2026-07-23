import type { Report, Subject } from "./reportTypes";

export const REPORT_SCHEMA_VERSION = 26;

export function validateReport(value: unknown): Report {
  if (!value || typeof value !== "object") {
    throw new Error("Report data must be a JSON object.");
  }
  const report = value as Partial<Report> & Record<string, unknown>;
  validateEnvelope(report);
  validateRemovedFields(report);
  validateIssueAnalyses(report as Report);
  validateMeasurements(report as Report);
  return report as Report;
}

function validateEnvelope(report: Partial<Report> & Record<string, unknown>): void {
  if (report.schema_version !== REPORT_SCHEMA_VERSION) {
    throw new Error(`Unsupported Reforge report schema ${String(report.schema_version ?? "missing")}; this report app requires schema 26.`);
  }
  if (!report.producer?.name || !report.target?.workspace_identity || !Array.isArray(report.issues) || !report.coverage || Array.isArray(report.coverage)) {
    throw new Error("The schema 26 report envelope is incomplete.");
  }
}

function validateRemovedFields(report: Record<string, unknown>): void {
  for (const removed of ["profile", "extensions", "findings"]) {
    if (removed in report) {
      throw new Error(`Schema 26 reports must not contain ${removed}.`);
    }
  }
}

function validateIssueAnalyses(report: Report): void {
  for (const issue of report.issues) {
    if (!issue.analysis || !(issue.analysis in report.coverage)) {
      throw new Error(`Issue ${issue.id} names an analysis absent from coverage.`);
    }
  }
}

function validateMeasurements(report: Report): void {
  for (const issue of report.issues) {
    for (const evidence of issue.evidence ?? []) {
      for (const measurement of evidence.measurements ?? []) {
        const invalidValue = typeof measurement.value !== "number";
        const invalidThreshold = measurement.threshold !== undefined
          && typeof measurement.threshold !== "number";
        if (invalidValue || invalidThreshold) {
          throw new Error("Schema 26 measurements must use JSON numbers.");
        }
      }
    }
  }
}

export function parseEmbeddedReport(): { report?: Report; error?: string } {
  const node = document.getElementById("reforge-report-data");
  if (!node?.textContent?.trim()) return { error: "Missing JSON report data." };
  try {
    return { report: validateReport(JSON.parse(node.textContent)) };
  } catch (error) {
    return { error: error instanceof Error ? error.message : "Invalid report JSON." };
  }
}

export function subjectLabel(subject: Subject): string {
  if (subject.kind === "repository") return "repository";
  if (subject.kind === "symbol") return `${subject.symbol} in ${subject.path}`;
  if (subject.kind === "group") return `${subject.members.length} related items`;
  return subject.path;
}
