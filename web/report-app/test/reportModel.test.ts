import { describe, expect, it } from "vitest";
import { subjectLabel, validateReport } from "../src/reportModel";
import type { Report } from "../src/reportTypes";

const report = (overrides: Partial<Report> = {}): Report => ({
  schema_version: 26,
  producer: { name: "reforge.analyze", version: "test" },
  target: { root: "/work", workspace_identity: "rw5-test" },
  summary: { issue_count: 0, evidence_count: 0, scanned_files: 2 },
  suppression: { evidence_count: 0, by_rule: {} },
  coverage: { codebase: { status: "observed", scanned_files: 2 } },
  issues: [],
  ...overrides,
});

describe("schema 26 report model", () => {
  it("accepts compact schema 26 and rejects old or transitional reports", () => {
    expect(validateReport(report()).schema_version).toBe(26);
    expect(() => validateReport({ schema_version: 18 })).toThrow(/requires schema 26/);
    expect(() => validateReport({ ...report(), extensions: {} })).toThrow(/must not contain extensions/);
  });
  it("renders canonical subjects", () => {
    expect(subjectLabel({ kind: "symbol", path: "src/lib.rs", symbol: "run" })).toBe("run in src/lib.rs");
    expect(subjectLabel({ kind: "group", members: ["a", "b"] })).toBe("2 related items");
  });
  it("rejects non-numeric measurements", () => {
    const invalid = {
      ...report(),
      issues: [{
        id: "ri6-test",
        analysis: "codebase",
        family: "reforge.codebase.large_file",
        subject: { kind: "file", path: "src/lib.rs" },
        title: "Large file",
        guidance: "Split it",
        evidence: [{
          id: "re6-test",
          rule: "reforge.codebase.large_file",
          message: "large",
          measurements: [{ name: "file.loc", value: "700", unit: "lines" }],
        }],
      }],
    };
    expect(() => validateReport(invalid)).toThrow(/JSON numbers/);
  });
});
