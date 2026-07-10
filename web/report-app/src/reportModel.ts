import type { FileRawMetric, ScanReport } from "./reportTypes";

export const REPORT_SCHEMA_VERSION = 17;

export type FileOverview = {
  path: string;
  loc: number;
  imports: number;
  publicItems: number;
  recentWeightedChurn: number;
  findings: number;
  hotspotPriority: number | null;
  isTest: boolean;
};

function number(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

export function formatRiskScore(value: unknown): number {
  return Math.round(number(value));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function validateReport(value: unknown): ScanReport {
  if (!isRecord(value)) {
    throw new Error("Report data must be a JSON object.");
  }

  if (value.schema_version !== REPORT_SCHEMA_VERSION) {
    const received = value.schema_version === undefined ? "missing" : String(value.schema_version);
    throw new Error(
      `Unsupported Reforge report schema ${received}; this report app requires schema ${REPORT_SCHEMA_VERSION}.`,
    );
  }

  return value as unknown as ScanReport;
}

export function normalizeReportPath(path: string): string {
  return path
    .replace(/\\/g, "/")
    .replace(/^\/\/\?\/UNC\//i, "//")
    .replace(/^\/\/\?\//, "");
}

function isAbsolutePath(path: string): boolean {
  return path.startsWith("/") || /^[a-z]:\//i.test(path);
}

function parentDirectory(path: string): string {
  const slash = path.lastIndexOf("/");
  if (slash < 0) return "";
  if (slash === 0) return "/";
  return path.slice(0, slash);
}

function pathIsWithin(path: string, directory: string): boolean {
  const caseInsensitive = /^[a-z]:/i.test(path) || /^[a-z]:/i.test(directory);
  const candidate = caseInsensitive ? path.toLowerCase() : path;
  const root = caseInsensitive ? directory.toLowerCase() : directory;
  if (root === "/") return candidate.startsWith("/");
  return candidate === root || candidate.startsWith(`${root}/`);
}

function commonDirectory(paths: string[]): string {
  const absolutePaths = paths.map(normalizeReportPath).filter(isAbsolutePath);
  if (absolutePaths.length === 0) return "";

  let common = parentDirectory(absolutePaths[0]);
  for (const path of absolutePaths.slice(1)) {
    while (common && !pathIsWithin(path, common)) {
      const parent = parentDirectory(common);
      if (parent === common) return "";
      common = parent;
    }
  }

  const conventionalSourceRoots = new Set(["app", "lib", "spec", "specs", "src", "test", "tests"]);
  const finalSegment = common.slice(common.lastIndexOf("/") + 1).toLowerCase();
  if (conventionalSourceRoots.has(finalSegment)) {
    common = parentDirectory(common);
  }

  return common;
}

function reportFilePaths(report: ScanReport): string[] {
  return [
    ...(report.raw_metrics?.files ?? []).map((file) => file.path),
    ...(report.raw_metrics?.functions ?? []).map((metric) => metric.path),
    ...(report.raw_metrics?.types ?? []).map((metric) => metric.path),
    ...(report.findings ?? []).flatMap((finding) => [
      finding.path,
      ...(finding.related_locations ?? []).map((related) => related.path),
    ]),
    ...(report.hotspots ?? []).map((hotspot) => hotspot.path),
    ...(report.issue_clusters ?? []).map((cluster) => cluster.path),
    ...(report.dependency_graph?.nodes ?? []).map((node) => node.path),
    ...(report.dependency_graph?.edges ?? []).flatMap((edge) => [edge.from, edge.to]),
  ];
}

export function createPathFormatter(report: ScanReport): (path: string) => string {
  const root = commonDirectory(reportFilePaths(report));

  return (path: string) => {
    const normalized = normalizeReportPath(path);
    if (!root || !isAbsolutePath(normalized) || !pathIsWithin(normalized, root)) {
      return normalized;
    }

    const relative = normalized.slice(root.length).replace(/^\/+/, "");
    return relative || normalized.slice(normalized.lastIndexOf("/") + 1);
  };
}

export function toDisplayReport(report: ScanReport): ScanReport {
  const displayPath = createPathFormatter(report);
  const rawMetrics = report.raw_metrics;
  const dependencyGraph = report.dependency_graph;

  return {
    ...report,
    raw_metrics: rawMetrics
      ? {
          ...rawMetrics,
          files: rawMetrics.files?.map((file) => ({ ...file, path: displayPath(file.path) })),
          functions: rawMetrics.functions?.map((metric) => ({ ...metric, path: displayPath(metric.path) })),
          types: rawMetrics.types?.map((metric) => ({ ...metric, path: displayPath(metric.path) })),
        }
      : rawMetrics,
    dependency_graph: dependencyGraph
      ? {
          nodes: dependencyGraph.nodes?.map((node) => ({ ...node, path: displayPath(node.path) })),
          edges: dependencyGraph.edges?.map((edge) => ({
            ...edge,
            from: displayPath(edge.from),
            to: displayPath(edge.to),
          })),
        }
      : dependencyGraph,
    hotspots: report.hotspots?.map((hotspot) => ({ ...hotspot, path: displayPath(hotspot.path) })),
    issue_clusters: report.issue_clusters?.map((cluster) => ({
      ...cluster,
      path: displayPath(cluster.path),
    })),
    findings: report.findings?.map((finding) => ({
      ...finding,
      path: displayPath(finding.path),
      related_locations: finding.related_locations?.map((related) => ({
        ...related,
        path: displayPath(related.path),
      })),
    })),
  };
}

function recentWeightedChurn(file: FileRawMetric): number {
  return number(file.churn?.recent_weighted_churn);
}

export function deriveFileOverviews(report: ScanReport): FileOverview[] {
  const files = report.raw_metrics?.files ?? [];
  const findings = report.findings ?? [];
  const hotspots = report.hotspots ?? [];
  const findingCounts = new Map<string, number>();
  const hotspotPriorities = new Map<string, number>();

  for (const finding of findings) {
    findingCounts.set(finding.path, (findingCounts.get(finding.path) ?? 0) + 1);
  }

  for (const hotspot of hotspots) {
    if (hotspot.level !== "file") continue;
    hotspotPriorities.set(
      hotspot.path,
      Math.max(hotspotPriorities.get(hotspot.path) ?? 0, number(hotspot.priority)),
    );
  }

  return files
    .map((file) => ({
      path: file.path,
      loc: number(file.loc),
      imports: number(file.imports),
      publicItems: number(file.public_items),
      recentWeightedChurn: recentWeightedChurn(file),
      findings: findingCounts.get(file.path) ?? 0,
      hotspotPriority: hotspotPriorities.get(file.path) ?? null,
      isTest: Boolean(file.is_test),
    }))
    .sort(
      (left, right) =>
        (right.hotspotPriority ?? -1) - (left.hotspotPriority ?? -1) ||
        right.recentWeightedChurn - left.recentWeightedChurn ||
        left.path.localeCompare(right.path),
    );
}
