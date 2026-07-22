import { describe, expect, it } from "vitest";
import { deriveFiles, deriveInspector, evidenceFamilies, layerValue, toDisplayReport, validateReport } from "../src/reportModel";
import type { ScanReport } from "../src/reportTypes";

function report(overrides: Partial<ScanReport> = {}): ScanReport {
  return {
    schema_version: 24,
    provenance: { engine: { version: "test", build_revision: null }, source: { git_revision: null, dirty: null }, configuration: { effective: {}, hash: "sha256-test" }, detector_policy_hash: "sha256-test" },
    baseline_comparison: null,
    summary: { scanned_files: 0, finding_count: 0, issue_count: 0, similar_function_group_count: 0, duration_ms: 0, churn: { mode: "off", enabled: false, status: "disabled", window_days: 180, max_commit_lines: 2000 } },
    stats: { source_files_discovered: 0, source_files_analyzed: 0, directories_scanned: 0, function_candidates: 0 },
    metrics_summary: {}, raw_metrics: { directories: [], files: [], functions: [], types: [] }, raw_metric_manifest: [],
    dependency_graph: { nodes: [], edges: [] }, agent_evidence: { files: [], issues: [] }, unity_project: {},
    suppression_summary: { suppressed_count: 0, suppressed_by_kind: {} },
    flow_analysis: { status: "disabled", functions_analyzed: 0, exact_edges: 0, unresolved_edges: 0, truncated_paths: 0, capabilities: [] },
    coverage_manifest: [], coverage_summary: {}, detector_execution: [], raw_metric_coverage: [], issues: [], detector_manifest: [], findings: [],
    ...overrides,
  };
}

const churn = (recent_weighted_churn: number) => ({ commits_touched: 0, lines_added: 0, lines_deleted: 0, authors_count: 0, recent_weighted_churn });

describe("schema 24 evidence model", () => {
  it("rejects every non-24 report", () => {
    expect(() => validateReport({ schema_version: 23 })).toThrow(/requires schema 24/);
    expect(validateReport(report()).schema_version).toBe(24);
  });

  it("sorts files by issue count then finding count without ranking fields", () => {
    const value = report({
      raw_metrics: { directories: [], functions: [], types: [], files: [
        { path: "a.rs", loc: 10, imports: 1, public_items: 1, is_test: false, churn: churn(2) },
        { path: "b.rs", loc: 20, imports: 2, public_items: 2, is_test: false, churn: churn(8) },
      ] },
      issues: [{ id: "ri4-a", family: "size", summary: "size", construct: "modifiability", mechanism: "responsibility_dispersion", action: "decompose", path: "b.rs", primary_finding_id: "rf4-b", finding_ids: ["rf4-b"], kinds: ["large_file"], subject: {} }],
    });
    expect(deriveFiles(value).map(x => x.path)).toEqual(["b.rs", "a.rs"]);
    expect(deriveFiles(value)[0]).not.toHaveProperty("priority");
  });

  it("builds metrics, locations, dependency and test reachability for the inspector", () => {
    const value = report({
      raw_metrics: { directories: [], functions: [], types: [], files: [{ path: "a.rs", loc: 10, imports: 1, public_items: 2, is_test: false, churn: churn(4) }] },
      dependency_graph: { nodes: [{ path: "a.rs", fan_in: 1, fan_out: 1 }, { path: "b.rs", fan_in: 1, fan_out: 0 }], edges: [{ from: "a.rs", to: "b.rs" }] },
      agent_evidence: { issues: [], files: [{ path: "a.rs", coverage_status: "observed", context_closure_files: 2, context_closure_loc: 30, unresolved_local_dependencies: 0, direct_test_files: ["a_test.rs"], reachable_test_files: ["a_test.rs"], reachable_test_file_count: 1, nearest_test_distance: 1, nearest_test_paths: ["a_test.rs"], paths_truncated: false }] },
    });
    expect(deriveInspector(value, deriveFiles(value)[0])).toMatchObject({ outgoing: ["b.rs"], agent: { reachable_test_file_count: 1 } });
  });

  it("supports findings, issues, churn and coverage map layers", () => {
    const file = { path: "a", loc: 1, imports: 0, publicItems: 0, churn: 4, findings: 3, issues: 2, coverageStatus: "partial", isTest: false };
    expect((["findings", "issues", "churn", "coverage"] as const).map(layer => layerValue(file, layer))).toEqual([3, 2, 4, 0]);
  });

  it("normalizes a shared absolute root across evidence paths", () => {
    const value = report({ raw_metrics: { directories: [], functions: [], types: [], files: [
      { path: "/work/p/src/a.rs", loc: 1, imports: 0, public_items: 0, is_test: false, churn: churn(0) },
      { path: "/work/p/tests/a.rs", loc: 1, imports: 0, public_items: 0, is_test: true, churn: churn(0) },
    ] } });
    expect(toDisplayReport(value).raw_metrics.files.map(x => x.path)).toEqual(["src/a.rs", "tests/a.rs"]);
  });

  it("counts issue evidence families", () => {
    const issue = { id: "ri4-a", family: "dependency", summary: "cycle", construct: "modularity", mechanism: "dependency_propagation", action: "break_cycle", path: "a", primary_finding_id: "rf4-a", finding_ids: ["rf4-a"], kinds: ["dependency_cycle"], subject: {} };
    expect(evidenceFamilies(report({ issues: [issue, { ...issue, id: "ri4-b" }] }))).toEqual([["dependency", 2]]);
  });
});
