import React, { useEffect, useMemo, useState } from "react";
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
  buildRepositoryTree,
  deriveFileInspector,
  deriveRepositoryFiles,
  formatRiskScore,
  keepSelectedWithinLimit,
  layoutRepositoryTreemap,
  toDisplayReport,
  validateReport,
  type FileOverview,
  type MapLayer,
  type RepositoryFileView,
} from "./reportModel";
import {
  parseViewState,
  serializeViewState,
  type MapLayer as ViewMapLayer,
  type ViewState,
} from "./viewState";

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
const BUTTON_TYPE = "button";
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
  selectedPath?: string | null,
): { nodes: PositionedNode[]; edges: DependencyGraphEdge[]; totalNodes: number; selectedEdges: number } {
  const nodes = report.dependency_graph?.nodes ?? [];
  const edges = report.dependency_graph?.edges ?? [];
  const context = dependencyContext(report.findings ?? []);
  const candidates = selectedPath ? nodes.filter((node) => node.path === selectedPath || edges.some((edge) => (edge.from === selectedPath && edge.to === node.path) || (edge.to === selectedPath && edge.from === node.path))) : nodes;
  const ranked = [...candidates]
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
    });
  const selected = keepSelectedWithinLimit(ranked, selectedPath, DEPENDENCY_MAP_NODE_LIMIT);

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
    <button className="text-button" onClick={onToggle}>
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

const layerLabels: Record<MapLayer, string> = { priority: "Priority", severity: "Severity", churn: "Churn", findings: "Findings" };

function mapColor(file: RepositoryFileView, layer: MapLayer, maxima: { churn: number; findings: number }) {
  if (layer === "severity") return file.severity === "critical" ? "#b63846" : file.severity === "warning" ? "#c66b3d" : file.findings ? "#68a3aa" : "#d8e2e0";
  const value = layer === "priority" ? file.priority / 100 : layer === "churn" ? file.churn / maxima.churn : file.findings / maxima.findings;
  const light = 92 - Math.min(1, value || 0) * 52;
  return layer === "churn" ? `hsl(186 47% ${light}%)` : layer === "findings" ? `hsl(20 54% ${light}%)` : `hsl(193 43% ${light}%)`;
}

function RepositoryMap({ files, selectedPath, onSelectPath, layer, onLayerChange }: { files: RepositoryFileView[]; selectedPath: string | null; onSelectPath: (path: string) => void; layer: MapLayer; onLayerChange: (layer: MapLayer) => void }) {
  const rectangles = useMemo(() => layoutRepositoryTreemap(buildRepositoryTree(files)), [files]);
  const maxima = { churn: Math.max(1, ...files.map((file) => file.churn)), findings: Math.max(1, ...files.map((file) => file.findings)) };
  if (!files.length) return <p className="empty">No file metrics, findings, hotspots, or dependency nodes were included.</p>;
  return <section className="repository-workbench panel" id="repository-map">
    <div className="map-toolbar"><div><span className="eyebrow">Repository topography</span><h2>Find the load-bearing code</h2><p>Footprint is lines of code. Tone is {layerLabels[layer].toLowerCase()}. Select a file to trace its signals.</p></div><div className="layer-switch" aria-label="Map data layer">{(Object.keys(layerLabels) as MapLayer[]).map((item) => <button type={BUTTON_TYPE} className={layer === item ? "active" : ""} aria-pressed={layer === item} onClick={() => onLayerChange(item)} key={item}>{layerLabels[item]}</button>)}</div></div>
    <div className="map-legend"><span><i className="legend-low" />low</span><span><i className="legend-high" />high</span><strong>{files.length} files visible</strong></div>
    <div className="treemap" role="group" aria-label={`${layerLabels[layer]} repository treemap`}>
      {rectangles.filter((rect) => rect.file).map((rect) => { const file = rect.file!; return <button type={BUTTON_TYPE} key={file.path} className={`treemap-file ${selectedPath === file.path ? "selected" : ""}`} style={{ left: `${rect.x / 10}%`, top: `${rect.y / 5.2}%`, width: `${rect.width / 10}%`, height: `${rect.height / 5.2}%`, background: mapColor(file, layer, maxima) }} onClick={() => onSelectPath(file.path)} title={`${file.path}\n${file.loc} LOC · p${file.priority} · ${file.findings} findings · churn ${file.churn}`}><span>{file.path.split("/").pop()}</span></button>; })}
    </div>
  </section>;
}

function ReviewStrip({ files, onSelectPath }: { files: RepositoryFileView[]; onSelectPath: (path: string) => void }) {
  const critical = files.filter((file) => file.severity === "critical").sort((a,b) => b.priority-a.priority)[0];
  const cycle = files.filter((file) => file.isCycle).sort((a,b) => b.priority-a.priority)[0];
  const churn = [...files].sort((a,b) => b.churn-a.churn)[0];
  const directoryTotals = new Map<string, number>(); files.forEach((file) => { const dir = file.path.includes("/") ? file.path.slice(0,file.path.lastIndexOf("/")) : "."; directoryTotals.set(dir,(directoryTotals.get(dir)??0)+file.priority); });
  const directory = [...directoryTotals].sort((a,b)=>b[1]-a[1])[0];
  const items = [critical && { label: "Critical issues", value: critical.path, path: critical.path }, cycle && { label: "Dependency cycle", value: cycle.path, path: cycle.path }, churn?.churn ? { label: "Highest churn", value: `${churn.path} · ${formatNumber(churn.churn)}`, path: churn.path } : null, directory?.[1] ? { label: "Risk concentration", value: directory[0], path: files.filter(f=>f.path.startsWith(directory[0])) .sort((a,b)=>b.priority-a.priority)[0]?.path } : null].filter(Boolean) as {label:string;value:string;path:string}[];
  if (!items.length) return <div className="review-strip calm"><strong>Review strip</strong><span>No priority, dependency, or churn signals were included.</span></div>;
  return <nav className="review-strip" aria-label="Review priorities">{items.map((item)=><button type={BUTTON_TYPE} key={item.label} onClick={()=>onSelectPath(item.path)}><span>{item.label}</span><strong>{item.value}</strong></button>)}</nav>;
}

function FileOverviewList({ files, onSelectPath }: { files: FileOverview[]; onSelectPath: (path:string)=>void }) {
  const [expanded, setExpanded] = useState(false);

  if (files.length === 0) return <p className="empty">No raw file metrics were included in this report.</p>;
  const visibleFiles = expanded ? files : files.slice(0, INITIAL_FILE_LIMIT);

  return (
    <div className="file-list">
      {visibleFiles.map((file) => (
        <button type={BUTTON_TYPE} className="file-row selectable-row" key={file.path} onClick={()=>onSelectPath(file.path)}>
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
        </button>
      ))}
      <ShowMoreButton expanded={expanded} initialLimit={INITIAL_FILE_LIMIT} onToggle={() => setExpanded(!expanded)} total={files.length} />
    </div>
  );
}

function Hotspots({ hotspots, onSelectPath }: { hotspots: Hotspot[]; onSelectPath: (path:string)=>void }) {
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
            <article className="row-card selectable-row" tabIndex={0} role="button" onClick={()=>onSelectPath(hotspot.path)} onKeyDown={(event)=>event.key==="Enter"&&onSelectPath(hotspot.path)} key={`${hotspot.path}-${hotspot.line ?? 0}-${hotspot.name ?? ""}`}>
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

function SimilarGroups({ groups, onSelectPath }: { groups: Finding[]; onSelectPath: (path:string)=>void }) {
  const [expanded, setExpanded] = useState(false);

  if (groups.length === 0) return <p className="empty">No similar function groups crossed the configured threshold.</p>;
  const visibleGroups = expanded ? groups : groups.slice(0, INITIAL_SIMILAR_GROUP_LIMIT);

  return (
    <div className="row-stack">
      {visibleGroups.map((finding) => (
        <article className="row-card selectable-row" role="button" tabIndex={0} onClick={()=>onSelectPath(finding.path)} onKeyDown={(event)=>event.key==="Enter"&&onSelectPath(finding.path)} key={finding.id ?? `${finding.path}-${finding.line ?? 0}`}>
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

function DependencyMap({ report, selectedPath, onSelectPath }: { report: ScanReport; selectedPath: string|null; onSelectPath:(path:string)=>void }) {
  const graph = useMemo(() => dependencyLayout(report, selectedPath), [report, selectedPath]);
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
          <g key={node.path} className={selectedPath === node.path ? "selected-node" : ""} role="button" tabIndex={0} onClick={()=>onSelectPath(node.path)} onKeyDown={(event)=>event.key==="Enter"&&onSelectPath(node.path)}>
            {selectedPath === node.path ? <circle cx={node.x} cy={node.y} r={4} className="node-ring" /> : null}
            <circle cx={node.x} cy={node.y} r={node.isCycle ? 2.7 : node.isHub ? 2.3 : 1.9} className={node.isCycle ? "node cycle" : node.isHub ? "node hub" : "node"} />
            <title>{`${node.path} · in ${number(node.fan_in)} · out ${number(node.fan_out)}`}</title>
          </g>
        ))}
      </svg>
      <div className="dependency-list">
        {graph.nodes.map((node) => (
          <button type={BUTTON_TYPE} key={node.path} onClick={()=>onSelectPath(node.path)}>
            <strong>{node.path}</strong>
            <span>
              fan-in {number(node.fan_in)} · fan-out {number(node.fan_out)}
              {node.isCycle ? " · cycle" : node.isHub ? " · hub" : ""}
            </span>
          </button>
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

function Findings({ findings, onSelectPath, view, onViewChange }: { findings: Finding[]; onSelectPath: (path:string)=>void; view: ViewState; onViewChange: (patch: Partial<ViewState>) => void }) {
  const kinds = [...new Set(findings.map((finding) => finding.kind))].sort();
  const filtered = filterFindings(findings, view.query, view.severity, view.kind, view.sort);

  return (
    <>
      <FindingControls
        kind={view.kind}
        kinds={kinds}
        query={view.query}
        setKind={(kind) => onViewChange({ kind })}
        setQuery={(query) => onViewChange({ query })}
        setSeverity={(severity) => onViewChange({ severity: severity as ViewState["severity"] })}
        setSort={(sort) => onViewChange({ sort: sort as ViewState["sort"] })}
        severity={view.severity}
        sort={view.sort}
      />
      {filtered.length === 0 ? (
        <p className="empty">No matching findings.</p>
      ) : (
        <div className="finding-list">
          {filtered.map((finding) => (
            <FindingCard finding={finding} onSelectPath={onSelectPath} key={finding.id ?? `${finding.kind}-${finding.path}-${finding.line ?? 0}`} />
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
  setKind: (value: string) => void;
  setQuery: (value: string) => void;
  setSeverity: (value: string) => void;
  setSort: (value: string) => void;
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

function FindingCard({ finding, onSelectPath }: { finding: Finding; onSelectPath: (path:string)=>void }) {
  return (
    <article className="finding-card selectable-row" tabIndex={0} role="button" onClick={()=>onSelectPath(finding.path)} onKeyDown={(event)=>event.key==="Enter"&&onSelectPath(finding.path)}>
      <div className="finding-main">
        <div className="finding-head">
          <Badge className={severityClass(finding.severity)}>{finding.severity}</Badge>
          <Badge>{formatKind(finding.kind)}</Badge>
          {finding.construct ? <Badge>{formatKind(finding.construct)}</Badge> : null}
          {finding.mechanism ? <Badge>{formatKind(finding.mechanism)}</Badge> : null}
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
        <small>{Math.round(number(finding.detection_reliability) * number(finding.interpretation_reliability) * 100)}% action probability</small>
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

function reportIssues(report: ScanReport): Finding[] {
  const primaryFindingIds = new Set(
    (report.issues ?? []).map((issue) => issue.primary_finding_id),
  );
  return (report.findings ?? []).filter(
    (finding) => !finding.issue_id || primaryFindingIds.has(finding.id ?? ""),
  );
}

function Inspector({ report, file, onClose, onSelectPath }: { report: ScanReport; file: RepositoryFileView; onClose:()=>void; onSelectPath:(path:string)=>void }) {
  const data = useMemo(()=>deriveFileInspector(report,file),[report,file]);
  useEffect(()=>{ const close=(event:KeyboardEvent)=>{if(event.key==="Escape") onClose();}; window.addEventListener("keydown",close); return()=>window.removeEventListener("keydown",close); },[onClose]);
  return <aside className="inspector" aria-label="File inspector"><div className="inspector-head"><div><span className="eyebrow">File inspector</span><h2>{data.path}</h2></div><button type={BUTTON_TYPE} onClick={onClose} aria-label="Close inspector">×</button></div>
    <div className="inspector-metrics"><span><strong>{formatNumber(data.loc)}</strong> LOC</span><span><strong>{data.priority}</strong> priority</span><span><strong>{data.findings}</strong> findings</span><span><strong>{formatNumber(data.churn)}</strong> churn</span></div>
    {data.riskReasons.length ? <section><h3>Why this file is here</h3>{data.riskReasons.slice(0,4).map(reason=><p key={reason}>{reason}</p>)}</section>:null}
    <section><h3>Signals</h3>{data.fileFindings.length ? data.fileFindings.map(finding=><button className="inspector-item" type={BUTTON_TYPE} key={finding.id??`${finding.kind}-${finding.line}`}><strong>{formatKind(finding.kind)}</strong><span>{finding.message}</span></button>):<p className="empty">No direct findings for this file.</p>}</section>
    {data.similarityGroups.length ? <section><h3>Similar groups</h3><p>{data.similarityGroups.length} group{data.similarityGroups.length===1?"":"s"} include this file.</p></section>:null}
    <section><h3>Dependencies</h3><p>fan-in {data.fanIn} · fan-out {data.fanOut}{data.isCycle?" · cycle":""}{data.isHub?" · hub":""}</p><div className="inspector-links">{[...data.incoming,...data.outgoing].slice(0,12).map(path=><button type={BUTTON_TYPE} onClick={()=>onSelectPath(path)} key={path}>{path}</button>)}</div></section>
  </aside>;
}

function ReportLead({ report, issues }: { report: ScanReport; issues: Finding[] }) {
  const summary = report.summary ?? {};
  const stats = report.stats ?? {};
  return <>
    <header className="report-header"><div><span className="eyebrow">Reforge report · schema {report.schema_version}</span><h1>Refactoring review</h1><p>{formatNumber(number(summary.scanned_files, number(stats.source_files_scanned)))} files scanned in {formatDuration(number(summary.duration_ms))} · churn {summary.churn?.enabled ? "included" : "not available"}</p></div><div className="header-aside"><small>Issues to review</small><strong>{formatNumber(issues.length)}</strong></div></header>
  </>;
}

const tabs = [{id:"overview",label:"Overview"},{id:"issues",label:"Issues"},{id:"map",label:"Code map"},{id:"metrics",label:"Metrics"},{id:"coverage",label:"Coverage"}] as const;

const coverageMechanisms = ["cognitive_load","dependency_propagation","responsibility_dispersion","duplication_divergence","change_pressure","verification_difficulty","knowledge_drift"];
const coverageScopes = ["repository","directory","file","function","type","finding_group"];

function CoveragePage({report}:{report:ScanReport}) {
  const cells=report.coverage_manifest??[];
  const [selected,setSelected]=useState(cells.find(cell=>cell.expectation==="required")??cells[0]);
  const [filter,setFilter]=useState("all");
  const receipts=(selected?.detectors??[]).map(kind=>report.detector_execution?.find(receipt=>receipt.kind===kind)).filter(Boolean);
  const selectedMetricIds=new Set((selected?.detectors??[]).flatMap(kind=>report.detector_manifest?.find(detector=>detector.kind===kind)?.input_metrics??[]));
  const metrics=(report.raw_metric_coverage??[]).filter(metric=>selectedMetricIds.has(metric.metric));
  return <div className="page-stack"><div className="page-heading"><div><span className="eyebrow">Measurement audit</span><h2>Coverage</h2><p>Inspect what Reforge was expected to observe, what ran, and where evidence was unavailable.</p></div><label className="coverage-filter">Status<select value={filter} onChange={event=>setFilter(event.target.value)}><option value="all">All statuses</option>{[...new Set(cells.map(cell=>cell.status))].map(status=><option key={status} value={status}>{formatKind(status)}</option>)}</select></label></div>
    <Section title="Mechanism × entity scope" meta="42 declared cells"><div className="coverage-scroll"><div className="coverage-matrix" role="grid" aria-label="Coverage audit matrix"><div className="coverage-corner"/><>{coverageScopes.map(scope=><div role="columnheader" className="coverage-scope" key={scope}>{formatKind(scope)}</div>)}</>{coverageMechanisms.flatMap(mechanism=>[<div role="rowheader" className="coverage-mechanism" key={`${mechanism}-label`}>{formatKind(mechanism)}</div>,...coverageScopes.map(scope=>{const cell=cells.find(item=>item.mechanism===mechanism&&item.entity_scope===scope);const hidden=filter!=="all"&&cell?.status!==filter;return <button type={BUTTON_TYPE} role="gridcell" aria-label={`${formatKind(mechanism)}, ${formatKind(scope)}: ${formatKind(cell?.status??"missing")}`} aria-selected={selected===cell} className={`coverage-cell status-${cell?.status??"missing"} ${hidden?"filtered":""}`} key={`${mechanism}-${scope}`} onClick={()=>cell&&setSelected(cell)}><span>{formatKind(cell?.status??"missing")}</span><small>{cell?.entity_count??0}</small></button>})])}</div></div></Section>
    {selected?<Section title={`${formatKind(selected.mechanism)} · ${formatKind(selected.entity_scope)}`} meta={formatKind(selected.status)}><div className="coverage-detail"><dl><div><dt>Expectation</dt><dd>{formatKind(selected.expectation)}</dd></div><div><dt>Entities</dt><dd>{formatNumber(selected.entity_count??0)}</dd></div><div><dt>Reason</dt><dd>{selected.reason}</dd></div></dl><div><h3>Detector receipts</h3>{receipts.length?receipts.map(receipt=><p key={receipt!.kind}><strong>{formatKind(receipt!.kind)}</strong> · {formatKind(receipt!.status)} · {formatNumber(receipt!.analyzed_entities??0)} entities{receipt!.candidate_groups?` · ${receipt!.candidate_groups} candidate groups`:""}</p>):<p className="empty">No detector is assigned to this cell.</p>}<h3>Unobservable evidence</h3>{selected.unobservable_reasons?.length?selected.unobservable_reasons.map(reason=><p key={reason}>{reason}</p>):<p className="empty">No unobservable entities were reported.</p>}{metrics.length?<><h3>Raw metrics</h3>{metrics.map(metric=><p key={metric.metric}><strong>{formatKind(metric.metric)}</strong> · {formatKind(metric.status)}</p>)}</>:null}</div></div></Section>:null}
  </div>;
}

function MetricsPage({ report, view, update }: { report: ScanReport; view: ViewState; update: (patch: Partial<ViewState>) => void }) {
  const values = report.metrics_summary?.[view.scope] ?? {};
  return <div className="page-stack"><div className="page-heading"><div><span className="eyebrow">Distribution</span><h2>Metrics</h2><p>Compare percentile pressure across the scanned population.</p></div><div className="scope-tabs" aria-label="Metric scope">{(["files","functions","types","churn"] as const).map(scope=><button type={BUTTON_TYPE} className={view.scope===scope?"active":""} aria-pressed={view.scope===scope} onClick={()=>update({scope})} key={scope}>{scope}</button>)}</div></div><Section title={`${view.scope} percentiles`}>{Object.keys(values).length?<div className="percentile-table">{Object.entries(values).map(([name,value])=><div className="percentile-row" key={name}><strong>{formatKind(name)}</strong>{(["p50","p75","p90","p95","max"] as const).map(key=><span key={key}><small>{key}</small>{formatNumber(number(value[key]))}</span>)}</div>)}</div>:<p className="empty">No {view.scope} percentile data was included. Enable the matching metrics and generate the report again.</p>}</Section></div>;
}

function ReportApp({ report }: { report: ScanReport }) {
  const displayReport = useMemo(() => toDisplayReport(report), [report]);
  const findings = displayReport.findings ?? [];
  const issues = reportIssues(displayReport);
  const hotspots = displayReport.hotspots ?? [];
  const files = useMemo(() => deriveFileOverviews(displayReport), [displayReport]);
  const repositoryFiles = useMemo(() => deriveRepositoryFiles(displayReport), [displayReport]);
  const similarGroups = useMemo(() => groupSimilarFindings(findings), [findings]);
  const [view, setView] = useState<ViewState>(()=>parseViewState(window.location.hash,displayReport));
  const update = (patch:Partial<ViewState>)=>setView(current=>({...current,...patch}));
  useEffect(()=>{const sync=()=>setView(parseViewState(window.location.hash,displayReport));window.addEventListener("hashchange",sync);return()=>window.removeEventListener("hashchange",sync);},[displayReport]);
  useEffect(()=>{const hash=serializeViewState(view);if(window.location.hash!==hash) history.replaceState(null,"",hash);},[view]);
  const selectedFile = repositoryFiles.find(file=>file.path===view.file);
  const selectFile=(file:string)=>update({file});
  const counts={overview:null,issues:issues.length,map:repositoryFiles.length,metrics:Object.values(displayReport.metrics_summary??{}).reduce((n,group)=>n+Object.keys(group??{}).length,0),coverage:displayReport.coverage_manifest?.length??0};

  return (
    <main className="report-shell">
      <ReportLead report={displayReport} issues={issues} />
      <nav className="app-tabs" aria-label="Report sections" role="tablist">{tabs.map(tab=><a role="tab" aria-selected={view.tab===tab.id} className={view.tab===tab.id?"active":""} href={`#${tab.id}`} key={tab.id}>{tab.label}{counts[tab.id]!==null?<span>{counts[tab.id]}</span>:null}</a>)}</nav>
      {view.tab==="overview"&&<div className="page-stack"><section className="summary-grid" aria-label="Report summary"><SummaryCard label="Issues" value={formatNumber(issues.length)} meta={`${findings.length} raw signals`}/><SummaryCard label="Hotspots" value={formatNumber(hotspots.length)} meta="watchlist"/><SummaryCard label="Similar groups" value={formatNumber(similarGroups.length)} meta="duplication"/><SummaryCard label="Suppressed" value={formatNumber(number(displayReport.suppression_summary?.suppressed_count))} meta="accepted"/></section><ReviewStrip files={repositoryFiles} onSelectPath={file=>update({tab:"map",file})}/><div className="dashboard-grid"><Section title="Risk distribution" meta={`${issues.length} issues`}><RiskDistribution findings={issues}/></Section><Section title="Highest-risk files"><FileOverviewList files={files.slice(0,6)} onSelectPath={file=>update({tab:"map",file})}/></Section></div></div>}
      {view.tab==="issues"&&<div className="page-stack"><div className="page-heading"><div><span className="eyebrow">Review queue</span><h2>Issues</h2><p>Filter actionable signals without losing your place.</p></div></div><Section title="Issues" meta={`${issues.length} issues · ${findings.length} raw signals`}><Findings findings={issues} onSelectPath={selectFile} view={view} onViewChange={update}/></Section><div className="dashboard-grid"><Section title="Watchlist" meta={`${hotspots.length} hotspots`}><Hotspots hotspots={hotspots} onSelectPath={selectFile}/></Section><Section title="Similar groups" meta={`${similarGroups.length} groups`}><SimilarGroups groups={similarGroups} onSelectPath={selectFile}/></Section></div></div>}
      {view.tab==="map"&&<div className="page-stack"><RepositoryMap files={repositoryFiles} selectedPath={view.file} onSelectPath={selectFile} layer={view.layer as MapLayer} onLayerChange={layer=>update({layer:layer as ViewMapLayer})}/><div className="dashboard-grid wide-left"><Section title="Dependency graph" meta={`${displayReport.dependency_graph?.nodes?.length??0} nodes`}><DependencyMap report={displayReport} selectedPath={view.file} onSelectPath={selectFile}/></Section><Section title="File overview" meta={`${files.length} files`}><FileOverviewList files={files} onSelectPath={selectFile}/></Section></div></div>}
      {view.tab==="metrics"&&<MetricsPage report={displayReport} view={view} update={update}/>} 
      {view.tab==="coverage"&&<CoveragePage report={displayReport}/>}
      {selectedFile?<Inspector report={displayReport} file={selectedFile} onClose={()=>update({file:null})} onSelectPath={selectFile}/>:null}
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
