import type { Finding, FileRawMetric, ScanReport, Severity } from "./reportTypes";

export const REPORT_SCHEMA_VERSION = 20;

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

export type MapLayer = "priority" | "severity" | "churn" | "findings";

export type RepositoryFileView = {
  path: string;
  loc: number;
  weight: number;
  priority: number;
  severity: Severity;
  findings: number;
  issues: number;
  churn: number;
  fanIn: number;
  fanOut: number;
  isCycle: boolean;
  isHub: boolean;
  similarityGroups: Finding[];
};

export type RepositoryTreeNode = {
  name: string;
  path: string;
  weight: number;
  children: RepositoryTreeNode[];
  file?: RepositoryFileView;
};

export type TreemapRect = {
  path: string;
  x: number;
  y: number;
  width: number;
  height: number;
  depth: number;
  file?: RepositoryFileView;
};

export type FileInspectorData = RepositoryFileView & {
  fileFindings: Finding[];
  incoming: string[];
  outgoing: string[];
  riskReasons: string[];
};

export function keepSelectedWithinLimit<T extends { path: string }>(
  items: T[],
  selectedPath: string | null | undefined,
  limit: number,
): T[] {
  if (!selectedPath) return items.slice(0, limit);
  const selected = items.find((item) => item.path === selectedPath);
  if (!selected) return items.slice(0, limit);
  return [selected, ...items.filter((item) => item !== selected)].slice(0, limit);
}

function number(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

const severityRank: Record<string, number> = { info: 1, warning: 2, critical: 3 };

function maxSeverity(left: Severity, right: Severity): Severity {
  return (severityRank[String(right).toLowerCase()] ?? 0) >
    (severityRank[String(left).toLowerCase()] ?? 0)
    ? right
    : left;
}

export function deriveRepositoryFiles(report: ScanReport): RepositoryFileView[] {
  const paths = new Set(reportFilePaths(report).filter(Boolean).map(normalizeReportPath));
  const raw = new Map((report.raw_metrics?.files ?? []).map((file) => [normalizeReportPath(file.path), file]));
  const nodes = new Map((report.dependency_graph?.nodes ?? []).map((node) => [normalizeReportPath(node.path), node]));
  const cycles = new Set<string>();
  const hubs = new Set<string>();
  for (const finding of report.findings ?? []) {
    if (finding.kind === "dependency_cycle") {
      cycles.add(normalizeReportPath(finding.path));
      for (const related of finding.related_locations ?? []) cycles.add(normalizeReportPath(related.path));
    }
    if (finding.kind === "dependency_hub") hubs.add(normalizeReportPath(finding.path));
  }

  return [...paths].sort().map((path) => {
    const metric = raw.get(path);
    const fileFindings = (report.findings ?? []).filter((finding) => normalizeReportPath(finding.path) === path);
    const fileIssues = (report.issues ?? []).filter((issue) => normalizeReportPath(issue.path) === path);
    const hotspot = (report.hotspots ?? []).filter((item) => normalizeReportPath(item.path) === path);
    const priority = Math.max(0, ...fileFindings.map((item) => number(item.priority)), ...fileIssues.map((item) => number(item.priority)), ...hotspot.map((item) => number(item.priority)));
    const severity = [...fileFindings.map((item) => item.severity), ...fileIssues.map((item) => item.severity), ...hotspot.map((item) => item.severity ?? "info")].reduce<Severity>(maxSeverity, "info");
    const dependency = nodes.get(path);
    const similarityGroups = (report.findings ?? []).filter((finding) => finding.kind === "similar_functions" && [finding.path, ...(finding.related_locations ?? []).map((item) => item.path)].map(normalizeReportPath).includes(path));
    const loc = number(metric?.loc);
    return { path, loc, weight: loc || 1, priority, severity, findings: fileFindings.length, issues: fileIssues.length, churn: number(metric?.churn?.recent_weighted_churn), fanIn: number(dependency?.fan_in), fanOut: number(dependency?.fan_out), isCycle: cycles.has(path), isHub: hubs.has(path), similarityGroups };
  });
}

export function buildRepositoryTree(files: RepositoryFileView[]): RepositoryTreeNode {
  const root: RepositoryTreeNode = { name: "repository", path: "", weight: 0, children: [] };
  for (const file of [...files].sort((a, b) => a.path.localeCompare(b.path))) {
    const parts = file.path.split("/").filter(Boolean);
    let node = root;
    parts.forEach((part, index) => {
      const path = parts.slice(0, index + 1).join("/");
      let child = node.children.find((candidate) => candidate.name === part);
      if (!child) {
        child = { name: part, path, weight: 0, children: [] };
        node.children.push(child);
      }
      node = child;
    });
    node.file = file;
  }
  const total = (node: RepositoryTreeNode): number => {
    node.children.sort((a, b) => a.path.localeCompare(b.path));
    node.weight = node.file?.weight ?? node.children.reduce((sum, child) => sum + total(child), 0);
    return node.weight;
  };
  total(root);
  return root;
}

export function layoutRepositoryTreemap(tree: RepositoryTreeNode, width = 1000, height = 520): TreemapRect[] {
  const rectangles: TreemapRect[] = [];
  const place = (node: RepositoryTreeNode, bounds: { x: number; y: number; w: number; h: number; depth: number }) => {
    const { x, y, w, h, depth } = bounds;
    rectangles.push({ path: node.path, x, y, width: w, height: h, depth, file: node.file });
    if (!node.children.length || node.weight <= 0) return;
    let cursor = 0;
    const horizontal = w >= h;
    for (const child of node.children) {
      const share = child.weight / node.weight;
      const childW = horizontal ? w * share : w;
      const childH = horizontal ? h : h * share;
      place(child, { x: x + (horizontal ? cursor : 0), y: y + (horizontal ? 0 : cursor), w: childW, h: childH, depth: depth + 1 });
      cursor += horizontal ? childW : childH;
    }
  };
  place(tree, { x: 0, y: 0, w: width, h: height, depth: 0 });
  return rectangles;
}

export function deriveFileInspector(report: ScanReport, file: RepositoryFileView): FileInspectorData {
  const edges = report.dependency_graph?.edges ?? [];
  const fileFindings = (report.findings ?? []).filter((finding) => normalizeReportPath(finding.path) === file.path);
  const incoming = edges.filter((edge) => normalizeReportPath(edge.to) === file.path).map((edge) => normalizeReportPath(edge.from)).sort();
  const outgoing = edges.filter((edge) => normalizeReportPath(edge.from) === file.path).map((edge) => normalizeReportPath(edge.to)).sort();
  const riskReasons = [...new Set([...(report.hotspots ?? []).filter((item) => normalizeReportPath(item.path) === file.path).map((item) => item.reason).filter((value): value is string => Boolean(value)), ...fileFindings.map((item) => item.rank_explanation).filter((value): value is string => Boolean(value))])];
  return { ...file, fileFindings, incoming, outgoing, riskReasons };
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
    ...(report.issues ?? []).map((issue) => issue.path),
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
    issues: report.issues?.map((issue) => ({
      ...issue,
      path: displayPath(issue.path),
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
