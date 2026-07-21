import type { ScanReport } from "./reportTypes";

export function normalizeReportPath(path: string): string {
  return path
    .replace(/\\/g, "/")
    .replace(/^\/\/\?\/UNC\//i, "//")
    .replace(/^\/\/\?\//, "");
}

function isAbsolute(path: string): boolean {
  return path.startsWith("/") || /^[a-z]:\//i.test(path);
}

function parentPath(path: string): string {
  const index = path.lastIndexOf("/");
  return index <= 0 ? path.slice(0, Math.max(0, index)) : path.slice(0, index);
}

function isWithin(path: string, root: string): boolean {
  const caseInsensitive = /^[a-z]:/i.test(path);
  const candidate = caseInsensitive ? path.toLowerCase() : path;
  const ancestor = caseInsensitive ? root.toLowerCase() : root;
  return candidate === ancestor || candidate.startsWith(`${ancestor}/`);
}

function commonRoot(paths: string[]): string {
  const absolutePaths = paths.map(normalizeReportPath).filter(isAbsolute);
  if (!absolutePaths.length) return "";

  let root = parentPath(absolutePaths[0]);
  for (const path of absolutePaths.slice(1)) {
    while (root && !isWithin(path, root)) root = parentPath(root);
  }
  if (["src", "app", "lib", "test", "tests"].includes(root.split("/").pop()?.toLowerCase() ?? "")) {
    root = parentPath(root);
  }
  return root;
}

function reportPaths(report: ScanReport): string[] {
  return [
    ...report.raw_metrics.files.map((metric) => metric.path),
    ...report.findings.flatMap((finding) => [
      finding.path,
      ...finding.related_locations.map((location) => location.path),
    ]),
    ...report.issues.map((issue) => issue.path),
    ...report.dependency_graph.edges.flatMap((edge) => [edge.from, edge.to]),
  ];
}

export function toDisplayReport(report: ScanReport): ScanReport {
  const root = commonRoot(reportPaths(report));
  const display = (path: string) => {
    const value = normalizeReportPath(path);
    return root && isAbsolute(value) && isWithin(value, root)
      ? value.slice(root.length).replace(/^\/+/, "")
      : value;
  };

  return {
    ...report,
    raw_metrics: {
      ...report.raw_metrics,
      files: report.raw_metrics.files.map((metric) => ({ ...metric, path: display(metric.path) })),
      functions: report.raw_metrics.functions.map((metric) => ({ ...metric, path: display(metric.path) })),
      types: report.raw_metrics.types.map((metric) => ({ ...metric, path: display(metric.path) })),
    },
    findings: report.findings.map((finding) => ({
      ...finding,
      path: display(finding.path),
      related_locations: finding.related_locations.map((location) => ({
        ...location,
        path: display(location.path),
      })),
    })),
    issues: report.issues.map((issue) => ({ ...issue, path: display(issue.path) })),
    dependency_graph: {
      nodes: report.dependency_graph.nodes.map((node) => ({ ...node, path: display(node.path) })),
      edges: report.dependency_graph.edges.map((edge) => ({
        from: display(edge.from),
        to: display(edge.to),
      })),
    },
    agent_evidence: {
      files: report.agent_evidence.files.map((evidence) => ({
        ...evidence,
        path: display(evidence.path),
        direct_test_files: evidence.direct_test_files.map(display),
        reachable_test_files: evidence.reachable_test_files.map(display),
        nearest_test_paths: evidence.nearest_test_paths.map(display),
      })),
      issues: report.agent_evidence.issues.map((evidence) => ({
        ...evidence,
        direct_test_files: evidence.direct_test_files.map(display),
        reachable_test_files: evidence.reachable_test_files.map(display),
        nearest_test_paths: evidence.nearest_test_paths.map(display),
        evidence_dispersion: {
          ...evidence.evidence_dispersion,
          evidence_files: evidence.evidence_dispersion.evidence_files.map(display),
        },
      })),
    },
  };
}
