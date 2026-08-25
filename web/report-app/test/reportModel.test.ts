import { describe, expect, it } from "vitest";
import { normalizeLocale, resolveLocale } from "../src/locale";
import { translatedLabel } from "../src/reportApp";
import { subjectLabel, validateReport } from "../src/reportModel";
import type { Report } from "../src/reportTypes";

const report = (overrides: Partial<Report> = {}): Report => ({
  schema_version: 27,
  producer: { name: "reforge.analyze", version: "test" },
  target: { root: "/work", workspace_identity: "rw5-test" },
  provenance: { identity_scheme: "reforge-identity-v7", scope_digest: "scope", analyses: {}, rules: {} },
  summary: { issue_count: 0, evidence_count: 0, scanned_files: 2 },
  suppression: { evidence_count: 0, by_rule: {} },
  coverage: { codebase: { status: "observed", scanned_files: 2 } },
  issues: [],
  ...overrides,
});

describe("report locale", () => {
  it("normalizes supported Chinese locale forms", () => {
    expect(normalizeLocale("zh")).toBe("zh-CN");
    expect(normalizeLocale("zh-CN")).toBe("zh-CN");
    expect(normalizeLocale("fr")).toBeUndefined();
  });

  it("uses query, storage, browser, then English priority", () => {
    expect(resolveLocale({ search: "?lang=en", stored: "zh-CN", browserLanguages: ["zh"] })).toBe("en");
    expect(resolveLocale({ search: "", stored: "zh", browserLanguages: ["en-US"] })).toBe("zh-CN");
    expect(resolveLocale({ search: "", stored: null, browserLanguages: ["fr-FR", "zh-CN"] })).toBe("zh-CN");
    expect(resolveLocale({ search: "", stored: null, browserLanguages: ["fr-FR"] })).toBe("en");
  });

  it("falls back to the existing English formatter for unknown labels", () => {
    expect(translatedLabel("zh-CN", "future_status")).toBe("Future Status");
  });
});

describe("schema 27 report model", () => {
  it("accepts compact schema 27 and rejects old or transitional reports", () => {
    expect(validateReport(report()).schema_version).toBe(27);
    expect(() => validateReport({ schema_version: 26 })).toThrow(/requires schema 27/);
    expect(() => validateReport({ ...report(), extensions: {} })).toThrow(/must not contain extensions/);
  });
  it("renders canonical subjects", () => {
    expect(subjectLabel({ kind: "symbol", entity: { key: "rust:function:run", path: "src/lib.rs", symbol: "run" } })).toBe("run in src/lib.rs");
    expect(subjectLabel({ kind: "group", members: [{ key: "a", path: "a" }, { key: "b", path: "b" }] })).toBe("2 related items");
  });
  it("rejects non-numeric measurements", () => {
    const invalid = {
      ...report(),
      issues: [{
        id: "ri7-test",
        kind: "advisory",
        content_fingerprint: "rc7-test",
        analysis: "codebase",
        family: "reforge.codebase.large_file",
        subject: { kind: "file", entity: { key: "file", path: "src/lib.rs" } },
        title: "Large file",
        guidance: "Split it",
        evidence: [{
          id: "re7-test",
          rule: "reforge.codebase.large_file",
          message: "large",
          measurements: [{ name: "file.loc", value: "700", unit: "lines" }],
        }],
      }],
    };
    expect(() => validateReport(invalid)).toThrow(/JSON numbers/);
  });
});
