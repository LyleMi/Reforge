import { describe, expect, it } from "vitest";

import {
  deriveFileOverviews,
  formatRiskScore,
  toDisplayReport,
  validateReport,
} from "../src/reportModel";
import type { ScanReport } from "../src/reportTypes";

function report(overrides: Partial<ScanReport> = {}): ScanReport {
  return { schema_version: 18, ...overrides };
}

describe("validateReport", () => {
  it("accepts schema 18", () => {
    expect(validateReport({ schema_version: 18 })).toEqual({ schema_version: 18 });
  });

  it.each([{}, { schema_version: 15 }, { schema_version: 16 }, { schema_version: 17 }])(
    "rejects reports outside schema 18",
    (candidate) => {
      expect(() => validateReport(candidate)).toThrow(/requires schema 18/);
    },
  );
});

describe("formatRiskScore", () => {
  it("rounds an existing 0-100 score without rescaling it", () => {
    expect(formatRiskScore(68.22)).toBe(68);
  });
});

describe("toDisplayReport", () => {
  it("removes a shared absolute report root from every displayed path", () => {
    const source = report({
      raw_metrics: {
        files: [
          { path: "/home/runner/work/Reforge/Reforge/src/main.rs" },
          { path: "/home/runner/work/Reforge/Reforge/web/report-app/src/main.tsx" },
        ],
        functions: [{ path: "/home/runner/work/Reforge/Reforge/src/main.rs", name: "main" }],
        types: [{ path: "/home/runner/work/Reforge/Reforge/src/model/mod.rs", name: "ScanReport" }],
      },
      dependency_graph: {
        nodes: [
          { path: "/home/runner/work/Reforge/Reforge/src/main.rs" },
          { path: "/home/runner/work/Reforge/Reforge/src/cli.rs" },
        ],
        edges: [{
          from: "/home/runner/work/Reforge/Reforge/src/main.rs",
          to: "/home/runner/work/Reforge/Reforge/src/cli.rs",
        }],
      },
      hotspots: [{
        level: "file",
        path: "/home/runner/work/Reforge/Reforge/src/main.rs",
      }],
      findings: [{
        kind: "similar_functions",
        severity: "warning",
        path: "/home/runner/work/Reforge/Reforge/src/main.rs",
        related_locations: [{
          path: "/home/runner/work/Reforge/Reforge/web/report-app/src/main.tsx",
          line: 10,
        }],
      }],
    });

    const displayed = toDisplayReport(source);

    expect(displayed.raw_metrics?.files?.map((file) => file.path)).toEqual([
      "src/main.rs",
      "web/report-app/src/main.tsx",
    ]);
    expect(displayed.raw_metrics?.functions?.[0].path).toBe("src/main.rs");
    expect(displayed.raw_metrics?.types?.[0].path).toBe("src/model/mod.rs");
    expect(displayed.dependency_graph?.nodes?.map((node) => node.path)).toEqual([
      "src/main.rs",
      "src/cli.rs",
    ]);
    expect(displayed.dependency_graph?.edges?.[0]).toEqual({
      from: "src/main.rs",
      to: "src/cli.rs",
    });
    expect(displayed.hotspots?.[0].path).toBe("src/main.rs");
    expect(displayed.findings?.[0].path).toBe("src/main.rs");
    expect(displayed.findings?.[0].related_locations?.[0].path).toBe(
      "web/report-app/src/main.tsx",
    );
  });

  it("preserves a conventional source directory when it is the common directory", () => {
    const displayed = toDisplayReport(report({
      raw_metrics: {
        files: [
          { path: "/work/project/src/main.rs" },
          { path: "/work/project/src/lib.rs" },
        ],
      },
    }));

    expect(displayed.raw_metrics?.files?.map((file) => file.path)).toEqual([
      "src/main.rs",
      "src/lib.rs",
    ]);
  });

  it("uses root documentation findings when sources share a nested package root", () => {
    const displayed = toDisplayReport(report({
      raw_metrics: {
        files: [
          { path: "/home/runner/work/project/project/packages/core/src/main.ts" },
          { path: "/home/runner/work/project/project/packages/core/src/model.ts" },
        ],
      },
      findings: [{
        kind: "stale_cli_documentation",
        severity: "warning",
        path: "/home/runner/work/project/project/README.md",
      }],
    }));

    expect(displayed.raw_metrics?.files?.map((file) => file.path)).toEqual([
      "packages/core/src/main.ts",
      "packages/core/src/model.ts",
    ]);
    expect(displayed.findings?.[0].path).toBe("README.md");
  });

  it("normalizes Windows extended paths and compares drive paths case-insensitively", () => {
    const displayed = toDisplayReport(report({
      raw_metrics: {
        files: [
          { path: "//?/D:/Work/Project/src/main.rs" },
          { path: "d:\\work\\project\\tests\\main_test.rs" },
        ],
      },
    }));

    expect(displayed.raw_metrics?.files?.map((file) => file.path)).toEqual([
      "src/main.rs",
      "tests/main_test.rs",
    ]);
  });

  it("normalizes Windows extended UNC paths", () => {
    const displayed = toDisplayReport(report({
      raw_metrics: {
        files: [
          { path: "\\\\?\\UNC\\server\\share\\project\\src\\main.rs" },
          { path: "\\\\server\\share\\project\\tests\\main_test.rs" },
        ],
      },
    }));

    expect(displayed.raw_metrics?.files?.map((file) => file.path)).toEqual([
      "src/main.rs",
      "tests/main_test.rs",
    ]);
  });
});

describe("deriveFileOverviews", () => {
  it("uses only hotspot priority and recent weighted churn as ranking signals", () => {
    const overviews = deriveFileOverviews(report({
      raw_metrics: {
        files: [
          {
            path: "src/high-priority.rs",
            loc: 20,
            churn: { lines_added: 900, lines_deleted: 700, recent_weighted_churn: 7 },
          },
          {
            path: "src/high-churn.rs",
            loc: 400,
            churn: { lines_added: 1, lines_deleted: 1, recent_weighted_churn: 80 },
          },
        ],
      },
      hotspots: [
        { level: "file", path: "src/high-priority.rs", priority: 68 },
        { level: "function", path: "src/high-priority.rs", priority: 97 },
        { level: "type", path: "src/high-churn.rs", priority: 99 },
      ],
      findings: [{
        kind: "large_file",
        severity: "critical",
        path: "src/high-churn.rs",
        priority: 99,
      }],
    }));

    expect(overviews.map((file) => file.path)).toEqual([
      "src/high-priority.rs",
      "src/high-churn.rs",
    ]);
    expect(overviews[0].hotspotPriority).toBe(68);
    expect(overviews[0].recentWeightedChurn).toBe(7);
    expect(overviews[0]).not.toHaveProperty("risk");
    expect(overviews[1].hotspotPriority).toBeNull();
  });
});
