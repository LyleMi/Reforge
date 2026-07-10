import React, { useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";

import type {
  DependencyGraphEdge,
  DependencyGraphNode,
  Finding,
  FindingMetric,
  Hotspot,
  RelatedLocation,
  ScanReport,
  Severity,
} from "./reportTypes";
import {
  deriveFileOverviews,
  formatRiskScore,
  toDisplayReport,
  validateReport,
  type FileOverview,
} from "./reportModel";

type PositionedNode = DependencyGraphNode & {
  x: number;
  y: number;
  priority: number;
  isCycle: boolean;
  isHub: boolean;
};

const INITIAL_FILE_LIMIT = 18;
const INITIAL_HOTSPOT_LIMIT = 12;
const INITIAL_SIMILAR_GROUP_LIMIT = 8;
const DEPENDENCY_MAP_NODE_LIMIT = 28;
const DEPENDENCY_MAP_EDGE_LIMIT = 70;
const severityOrder: Record<string, number> = { critical: 3, warning: 2, info: 1 };
const riskLabels = ["calm", "watch", "elevated", "high"];

function parseReport(): { report?: ScanReport; error?: string } {
  const element = document.getElementById("reforge-report-data");
  if (!element?.textContent?.trim()) {
    return { error: "Missing JSON report data in #reforge-report-data." };
  }

  try {
    return { report: validateReport(JSON.parse(element.textContent)) };
  } catch (error) {
    return {
      error: error instanceof Error ? error.message : "Report JSON could not be parsed."
    };
  }
}

function number(value: unknown, fallback = 0): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function text(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function formatKind(kind: string): string {
  return kind.replace(/_/g, " ");
}

function formatNumber(value: number): string {
  return new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 }).format(value);
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${formatNumber(ms)} ms`;
  return `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)} s`;
}

function severityRank(severity: Severity | undefined): number {
  return severityOrder[String(severity ?? "info").toLowerCase()] ?? 0;
}

function severityClass(severity: Severity | undefined): string {
  return `severity-${String(severity ?? "info").toLowerCase()}`;
}

function scoreBand(score: number): string {
  if (score >= 75) return riskLabels[3];
  if (score >= 50) return riskLabels[2];
  if (score >= 25) return riskLabels[1];
  return riskLabels[0];
}

function location(path: string, line?: number | null, name?: string | null): string {
  const linePart = line ? `:${line}` : "";
  const namePart = name ? ` ${name}` : "";
  return `${path}${linePart}${namePart}`;
}

function groupSimilarFindings(findings: Finding[]): Finding[] {
  return findings
    .filter((finding) => finding.kind === "similar_functions")
    .sort(
      (left, right) =>
        number(right.priority) - number(left.priority) ||
        (right.related_locations?.length ?? 0) - (left.related_locations?.length ?? 0) ||
        left.path.localeCompare(right.path)
    );
}

function dependencyContext(findings: Finding[]) {
  const priority = new Map<string, number>();
  const cycles = new Set<string>();
  const hubs = new Set<string>();
  let count = 0;

  const recordPriority = (path: string, value: number) => {
    priority.set(path, Math.max(priority.get(path) ?? 0, value));
  };

  for (const finding of findings) {
    if (finding.kind === "dependency_cycle") {
      count += 1;
      cycles.add(finding.path);
      recordPriority(finding.path, number(finding.priority));
      for (const related of finding.related_locations ?? []) {
        cycles.add(related.path);
        recordPriority(related.path, number(finding.priority));
      }
    }
    if (finding.kind === "dependency_hub") {
      count += 1;
      hubs.add(finding.path);
      recordPriority(finding.path, number(finding.priority));
    }
  }

  return { priority, cycles, hubs, count };
}

function dependencyLayout(
  report: ScanReport,
): { nodes: PositionedNode[]; edges: DependencyGraphEdge[]; totalNodes: number; selectedEdges: number } {
  const nodes = report.dependency_graph?.nodes ?? [];
  const edges = report.dependency_graph?.edges ?? [];
  const context = dependencyContext(report.findings ?? []);
  const selected = [...nodes]
    .sort((left, right) => {
      const leftPriority = context.priority.get(left.path) ?? 0;
      const rightPriority = context.priority.get(right.path) ?? 0;
      return (
        rightPriority - leftPriority ||
        Number(context.cycles.has(right.path)) - Number(context.cycles.has(left.path)) ||
        Number(context.hubs.has(right.path)) - Number(context.hubs.has(left.path)) ||
        number(right.fan_in) + number(right.fan_out) - (number(left.fan_in) + number(left.fan_out)) ||
        left.path.localeCompare(right.path)
      );
    })
    .slice(0, DEPENDENCY_MAP_NODE_LIMIT);

  const selectedPaths = new Set(selected.map((node) => node.path));
  const radius = 36;
  const center = 50;
  const positioned = selected.map((node, index) => {
    const angle = (Math.PI * 2 * index) / Math.max(1, selected.length) - Math.PI / 2;
    const priority = context.priority.get(node.path) ?? 0;
    return {
      ...node,
      x: center + Math.cos(angle) * radius,
      y: center + Math.sin(angle) * radius,
      priority,
      isCycle: context.cycles.has(node.path),
      isHub: context.hubs.has(node.path)
    };
  });

  const selectedEdges = edges.filter((edge) => selectedPaths.has(edge.from) && selectedPaths.has(edge.to));
  const visibleEdges = selectedEdges.slice(0, DEPENDENCY_MAP_EDGE_LIMIT);

  return {
    nodes: positioned,
    edges: visibleEdges,
    totalNodes: nodes.length,
    selectedEdges: selectedEdges.length
  };
}

function SummaryCard({ label, value, meta }: { label: string; value: string; meta?: string }) {
  return (
    <div className="summary-card">
      <span>{label}</span>
      <strong>{value}</strong>
      {meta ? <small>{meta}</small> : null}
    </div>
  );
}

function Section({ title, meta, children }: { title: string; meta?: string; children: React.ReactNode }) {
  return (
    <section className="panel">
      <div className="section-head">
        <h2>{title}</h2>
        {meta ? <span>{meta}</span> : null}
      </div>
      {children}
    </section>
  );
}

function Badge({ children, className = "" }: { children: React.ReactNode; className?: string }) {
  return <span className={`badge ${className}`}>{children}</span>;
}

function ShowMoreButton({
  expanded,
  initialLimit,
  onToggle,
  total,
}: {
  expanded: boolean;
  initialLimit: number;
  onToggle: () => void;
  total: number;
}) {
  if (total <= initialLimit) {
    return null;
  }

  return (
    <button className="text-button" type="button" onClick={onToggle}>
      {expanded ? `Show first ${formatNumber(initialLimit)}` : `Show all ${formatNumber(total)}`}
    </button>
  );
}

function RiskDistribution({ findings }: { findings: Finding[] }) {
  const counts = ["critical", "warning", "info"].map((severity) => ({
    severity,
    count: findings.filter((finding) => String(finding.severity).toLowerCase() === severity).length
  }));
  const total = Math.max(1, findings.length);

  return (
    <div className="risk-distribution">
      {counts.map((item) => (
        <div key={item.severity} className="risk-row">
          <span>{item.severity}</span>
          <div className="risk-track">
            <i className={item.severity} style={{ width: `${(item.count / total) * 100}%` }} />
          </div>
          <strong>{item.count}</strong>
        </div>
      ))}
    </div>
  );
}

function MetricsSummary({ report }: { report: ScanReport }) {
  const fileMetrics = Object.entries(report.metrics_summary?.files ?? {}).slice(0, 4);
  const functionMetrics = Object.entries(report.metrics_summary?.functions ?? {}).slice(0, 4);
  const rows = [...fileMetrics.map(([name, value]) => ["file", name, value] as const), ...functionMetrics.map(([name, value]) => ["fn", name, value] as const)];

  if (rows.length === 0) {
    return <p className="empty">No percentile metrics were included in this report.</p>;
  }

  return (
    <div className="metric-grid">
      {rows.map(([scope, name, value]) => (
        <div className="metric-tile" key={`${scope}-${name}`}>
          <span>{scope}</span>
          <strong>{name.replace(/_/g, " ")}</strong>
          <small>
            p90 {formatNumber(number(value.p90))} · max {formatNumber(number(value.max))}
          </small>
        </div>
      ))}
    </div>
  );
}

function FileOverviewList({ files }: { files: FileOverview[] }) {
  const [expanded, setExpanded] = useState(false);

  if (files.length === 0) return <p className="empty">No raw file metrics were included in this report.</p>;
  const visibleFiles = expanded ? files : files.slice(0, INITIAL_FILE_LIMIT);

  return (
    <div className="file-list">
      {visibleFiles.map((file) => (
        <div className="file-row" key={file.path}>
          <div>
            <strong>{file.path}</strong>
            <span>
              {formatNumber(file.loc)} loc · {formatNumber(file.imports)} imports · {formatNumber(file.recentWeightedChurn)} recent weighted churn · {formatNumber(file.findings)} findings
              {file.isTest ? " · test" : ""}
            </span>
          </div>
          <div
            className="priority-meter"
            aria-label={file.hotspotPriority === null ? "not in hotspot watchlist" : `hotspot priority ${file.hotspotPriority}`}
          >
            <i style={{ width: `${Math.min(100, file.hotspotPriority ?? 0)}%` }} />
          </div>
          <Badge className={file.hotspotPriority === null ? "" : `band-${scoreBand(file.hotspotPriority)}`}>
            {file.hotspotPriority === null ? "unranked" : `p${file.hotspotPriority}`}
          </Badge>
        </div>
      ))}
      <ShowMoreButton expanded={expanded} initialLimit={INITIAL_FILE_LIMIT} onToggle={() => setExpanded(!expanded)} total={files.length} />
    </div>
  );
}

function Hotspots({ hotspots }: { hotspots: Hotspot[] }) {
  const [query, setQuery] = useState("");
  const [level, setLevel] = useState("");
  const [expanded, setExpanded] = useState(false);
  const filtered = hotspots
    .filter((hotspot) => !level || hotspot.level === level)
    .filter((hotspot) => {
      const haystack = `${hotspot.path} ${hotspot.name ?? ""} ${hotspot.reason ?? ""}`.toLowerCase();
      return haystack.includes(query.toLowerCase());
    })
    .sort((left, right) => number(right.priority) - number(left.priority));
  const visibleHotspots = expanded ? filtered : filtered.slice(0, INITIAL_HOTSPOT_LIMIT);

  return (
    <>
      <div className="controls">
        <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search hotspots" type="search" />
        <select value={level} onChange={(event) => setLevel(event.target.value)} aria-label="Hotspot level">
          <option value="">All levels</option>
          <option value="file">File</option>
          <option value="function">Function</option>
          <option value="type">Type</option>
        </select>
      </div>
      {filtered.length === 0 ? (
        <p className="empty">No matching hotspots.</p>
      ) : (
        <div className="row-stack">
          {visibleHotspots.map((hotspot) => (
            <article className="row-card" key={`${hotspot.path}-${hotspot.line ?? 0}-${hotspot.name ?? ""}`}>
              <div>
                <div className="row-title">{location(hotspot.path, hotspot.line, hotspot.name)}</div>
                <p>{hotspot.reason || "No reason supplied."}</p>
                <div className="chips">
                  <Badge>{hotspot.level}</Badge>
                  <Badge className={severityClass(hotspot.severity)}>{hotspot.severity ?? "info"}</Badge>
                  <Badge>static {formatRiskScore(hotspot.static_risk)}</Badge>
                  <Badge>churn {formatRiskScore(hotspot.churn_risk)}</Badge>
                </div>
              </div>
              <strong className="priority">{number(hotspot.priority)}</strong>
            </article>
          ))}
          <ShowMoreButton expanded={expanded} initialLimit={INITIAL_HOTSPOT_LIMIT} onToggle={() => setExpanded(!expanded)} total={filtered.length} />
        </div>
      )}
    </>
  );
}

function SimilarGroups({ groups }: { groups: Finding[] }) {
  const [expanded, setExpanded] = useState(false);

  if (groups.length === 0) return <p className="empty">No similar function groups crossed the configured threshold.</p>;
  const visibleGroups = expanded ? groups : groups.slice(0, INITIAL_SIMILAR_GROUP_LIMIT);

  return (
    <div className="row-stack">
      {visibleGroups.map((finding) => (
        <article className="row-card" key={finding.id ?? `${finding.path}-${finding.line ?? 0}`}>
          <div>
            <div className="row-title">{location(finding.path, finding.line)}</div>
            <p>{finding.message}</p>
            <div className="related-grid">
              {(finding.related_locations ?? []).map((related) => (
                <span key={`${related.path}-${related.line}-${related.name ?? ""}`}>{location(related.path, related.line, related.name)}</span>
              ))}
            </div>
          </div>
          <strong className="priority">{number(finding.priority)}</strong>
        </article>
      ))}
      <ShowMoreButton expanded={expanded} initialLimit={INITIAL_SIMILAR_GROUP_LIMIT} onToggle={() => setExpanded(!expanded)} total={groups.length} />
    </div>
  );
}

function DependencyMap({ report }: { report: ScanReport }) {
  const graph = useMemo(() => dependencyLayout(report), [report]);
  const nodeByPath = new Map(graph.nodes.map((node) => [node.path, node]));

  if (graph.nodes.length === 0) return <p className="empty">No dependency graph data was included in this report.</p>;

  return (
    <div className="dependency-wrap">
      <svg viewBox="0 0 100 100" role="img" aria-label="Dependency graph focus map">
        {graph.edges.map((edge) => {
          const from = nodeByPath.get(edge.from);
          const to = nodeByPath.get(edge.to);
          if (!from || !to) return null;
          const hot = from.isCycle || to.isCycle || from.isHub || to.isHub;
          return <line key={`${edge.from}->${edge.to}`} x1={from.x} y1={from.y} x2={to.x} y2={to.y} className={hot ? "edge edge-hot" : "edge"} />;
        })}
        {graph.nodes.map((node) => (
          <g key={node.path}>
            <circle cx={node.x} cy={node.y} r={node.isCycle ? 2.7 : node.isHub ? 2.3 : 1.9} className={node.isCycle ? "node cycle" : node.isHub ? "node hub" : "node"} />
            <title>{`${node.path} · in ${number(node.fan_in)} · out ${number(node.fan_out)}`}</title>
          </g>
        ))}
      </svg>
      <div className="dependency-list">
        {graph.nodes.map((node) => (
          <div key={node.path}>
            <strong>{node.path}</strong>
            <span>
              fan-in {number(node.fan_in)} · fan-out {number(node.fan_out)}
              {node.isCycle ? " · cycle" : node.isHub ? " · hub" : ""}
            </span>
          </div>
        ))}
        {(graph.nodes.length < graph.totalNodes || graph.edges.length < graph.selectedEdges) && (
          <p className="dependency-note">
            Showing {formatNumber(graph.nodes.length)} of {formatNumber(graph.totalNodes)} nodes and {formatNumber(graph.edges.length)} of {formatNumber(graph.selectedEdges)} selected edges.
          </p>
        )}
      </div>
    </div>
  );
}

function Findings({ findings }: { findings: Finding[] }) {
  const kinds = [...new Set(findings.map((finding) => finding.kind))].sort();
  const [query, setQuery] = useState("");
  const [severity, setSeverity] = useState("");
  const [kind, setKind] = useState("");
  const [sort, setSort] = useState("priority");
  const filtered = filterFindings(findings, query, severity, kind, sort);

  return (
    <>
      <FindingControls
        kind={kind}
        kinds={kinds}
        query={query}
        setKind={setKind}
        setQuery={setQuery}
        setSeverity={setSeverity}
        setSort={setSort}
        severity={severity}
        sort={sort}
      />
      {filtered.length === 0 ? (
        <p className="empty">No matching findings.</p>
      ) : (
        <div className="finding-list">
          {filtered.map((finding) => (
            <FindingCard finding={finding} key={finding.id ?? `${finding.kind}-${finding.path}-${finding.line ?? 0}`} />
          ))}
        </div>
      )}
    </>
  );
}

function FindingControls({
  kind,
  kinds,
  query,
  setKind,
  setQuery,
  setSeverity,
  setSort,
  severity,
  sort,
}: {
  kind: string;
  kinds: string[];
  query: string;
  setKind: React.Dispatch<React.SetStateAction<string>>;
  setQuery: React.Dispatch<React.SetStateAction<string>>;
  setSeverity: React.Dispatch<React.SetStateAction<string>>;
  setSort: React.Dispatch<React.SetStateAction<string>>;
  severity: string;
  sort: string;
}) {
  return (
    <div className="controls controls-wide">
      <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search findings" type="search" />
      <select value={severity} onChange={(event) => setSeverity(event.target.value)} aria-label="Finding severity">
        <option value="">All severities</option>
        <option value="critical">Critical</option>
        <option value="warning">Warning</option>
        <option value="info">Info</option>
      </select>
      <select value={kind} onChange={(event) => setKind(event.target.value)} aria-label="Finding kind">
        <option value="">All kinds</option>
        {kinds.map((candidate) => (
          <option key={candidate} value={candidate}>
            {formatKind(candidate)}
          </option>
        ))}
      </select>
      <select value={sort} onChange={(event) => setSort(event.target.value)} aria-label="Sort findings">
        <option value="priority">Priority</option>
        <option value="severity">Severity</option>
        <option value="path">Path</option>
        <option value="kind">Kind</option>
      </select>
    </div>
  );
}

function FindingCard({ finding }: { finding: Finding }) {
  return (
    <article className="finding-card">
      <div className="finding-main">
        <div className="finding-head">
          <Badge className={severityClass(finding.severity)}>{finding.severity}</Badge>
          <Badge>{formatKind(finding.kind)}</Badge>
          <strong>{location(finding.path, finding.line)}</strong>
        </div>
        <p>{finding.message}</p>
        {finding.rank_explanation ? <small className="rank">{finding.rank_explanation}</small> : null}
        <FindingMetrics metrics={finding.metrics ?? []} />
        {finding.recommendation ? <p className="recommendation">{finding.recommendation}</p> : null}
        <FindingRelatedLocations locations={finding.related_locations ?? []} />
      </div>
      <div className="finding-score">
        <span>priority</span>
        <strong>{number(finding.priority)}</strong>
        <small>{Math.round(number(finding.confidence) * 100)}% conf</small>
      </div>
    </article>
  );
}

function FindingMetrics({ metrics }: { metrics: FindingMetric[] }) {
  return (
    <div className="metric-strip">
      {metrics.slice(0, 5).map((metric) => (
        <span key={metric.name}>
          {metric.name.replace(/_/g, " ")} <strong>{formatNumber(number(metric.value))}</strong>
          {metric.threshold ? ` / ${formatNumber(metric.threshold)}` : ""} {text(metric.unit)}
        </span>
      ))}
    </div>
  );
}

function FindingRelatedLocations({ locations }: { locations: RelatedLocation[] }) {
  if (locations.length === 0) {
    return null;
  }

  return (
    <details>
      <summary>Related locations ({locations.length})</summary>
      <div className="related-grid">
        {locations.map((related) => (
          <span key={`${related.path}-${related.line}-${related.name ?? ""}`}>{location(related.path, related.line, related.name)}</span>
        ))}
      </div>
    </details>
  );
}

function filterFindings(
  findings: Finding[],
  query: string,
  severity: string,
  kind: string,
  sort: string,
): Finding[] {
  return findings
    .filter((finding) => (!severity || finding.severity === severity) && (!kind || finding.kind === kind))
    .filter((finding) => findingMatchesQuery(finding, query))
    .sort((left, right) => compareFindings(left, right, sort));
}

function findingMatchesQuery(finding: Finding, query: string): boolean {
  const metrics = (finding.metrics ?? []).map((metric) => `${metric.name} ${metric.value}`).join(" ");
  const haystack = `${finding.path} ${finding.kind} ${finding.message ?? ""} ${metrics}`.toLowerCase();
  return haystack.includes(query.toLowerCase());
}

function compareFindings(left: Finding, right: Finding, sort: string): number {
  if (sort === "path") return left.path.localeCompare(right.path);
  if (sort === "kind") return left.kind.localeCompare(right.kind);
  if (sort === "severity") return severityRank(right.severity) - severityRank(left.severity);
  return number(right.priority) - number(left.priority) || severityRank(right.severity) - severityRank(left.severity);
}

function ReportApp({ report }: { report: ScanReport }) {
  const displayReport = useMemo(() => toDisplayReport(report), [report]);
  const findings = displayReport.findings ?? [];
  const hotspots = displayReport.hotspots ?? [];
  const files = useMemo(() => deriveFileOverviews(displayReport), [displayReport]);
  const similarGroups = useMemo(() => groupSimilarFindings(findings), [findings]);
  const summary = displayReport.summary ?? {};
  const stats = displayReport.stats ?? {};
  const suppression = displayReport.suppression_summary ?? {};
  const suppressedCount = number(suppression.suppressed_count);
  const highestSuppressedPriority = suppression.highest_suppressed_priority;
  const suppressionMeta =
    typeof highestSuppressedPriority === "number" && Number.isFinite(highestSuppressedPriority)
      ? `highest p${highestSuppressedPriority}`
      : "accepted";

  return (
    <main className="report-shell">
      <header className="report-header">
        <div>
          <span className="eyebrow">Reforge schema {displayReport.schema_version}</span>
          <h1>Reforge scan report</h1>
          <p>
            {formatNumber(number(summary.scanned_files, number(stats.source_files_scanned)))} files scanned in {formatDuration(number(summary.duration_ms))}; churn is {summary.churn?.enabled ? "enabled" : "disabled"}.
          </p>
        </div>
        <div className="header-aside">
          <strong>{formatNumber(findings.length)}</strong>
          <span>threshold findings</span>
        </div>
      </header>

      <section className="summary-grid" aria-label="Report summary">
        <SummaryCard label="Hotspots" value={formatNumber(hotspots.length)} meta={text(summary.hotspot_model, "model")} />
        <SummaryCard
          label="Suppressed"
          value={formatNumber(suppressedCount)}
          meta={suppressionMeta}
        />
        <SummaryCard label="Similar groups" value={formatNumber(similarGroups.length)} meta="duplication" />
        <SummaryCard label="Functions" value={formatNumber(number(stats.function_candidates))} meta="candidates" />
        <SummaryCard label="Directories" value={formatNumber(number(stats.directories_scanned))} meta="scanned" />
      </section>

      <div className="dashboard-grid">
        <Section title="Risk Distribution" meta={`${findings.length} findings`}>
          <RiskDistribution findings={findings} />
        </Section>
        <Section title="Metric Percentiles">
          <MetricsSummary report={report} />
        </Section>
      </div>

      <Section title="File Overview" meta={`${files.length} files · priority then churn`}>
        <FileOverviewList files={files} />
      </Section>

      <div className="dashboard-grid wide-left">
        <Section title="Dependency Map" meta={`${displayReport.dependency_graph?.nodes?.length ?? 0} nodes`}>
          <DependencyMap report={displayReport} />
        </Section>
        <Section title="Similar Function Groups" meta={`${similarGroups.length} groups`}>
          <SimilarGroups groups={similarGroups} />
        </Section>
      </div>

      <Section title="Watchlist" meta={`${hotspots.length} hotspots`}>
        <Hotspots hotspots={hotspots} />
      </Section>

      <Section title="Findings" meta={`${findings.length} total`}>
        <Findings findings={findings} />
      </Section>
    </main>
  );
}

function Boot() {
  const parsed = parseReport();
  if (parsed.error || !parsed.report) {
    return (
      <main className="report-shell">
        <section className="panel error-panel">
          <h1>Report data could not be loaded</h1>
          <p>{parsed.error}</p>
        </section>
      </main>
    );
  }
  return <ReportApp report={parsed.report} />;
}

const root = document.getElementById("reforge-report-root");

if (root) {
  createRoot(root).render(<Boot />);
}
