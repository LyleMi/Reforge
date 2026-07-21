import type { FileAgentEvidence, FileRawMetric, Finding, Issue, ScanReport } from "./reportTypes";
import type { EvidenceSort, MapLayer } from "./viewState";
export { normalizeReportPath, toDisplayReport } from "./pathModel";

export const REPORT_SCHEMA_VERSION = 23;
export type FileView = { path: string; loc: number; imports: number; publicItems: number; churn: number; findings: number; issues: number; coverageStatus: string; isTest: boolean };
export type FileInspector = FileView & { metrics?: FileRawMetric; fileFindings: Finding[]; fileIssues: Issue[]; relatedLocations: string[]; incoming: string[]; outgoing: string[]; agent?: FileAgentEvidence };

export function validateReport(value: unknown): ScanReport {
  if (!value || typeof value !== "object") throw new Error("Report data must be a JSON object.");
  const report = value as Partial<ScanReport>;
  if (report.schema_version !== REPORT_SCHEMA_VERSION) throw new Error(`Unsupported Reforge report schema ${String(report.schema_version ?? "missing")}; this report app requires schema 23.`);
  return value as ScanReport;
}

export function parseEmbeddedReport(): { report?: ScanReport; error?: string } {
  const node = document.getElementById("reforge-report-data");
  if (!node?.textContent?.trim()) return { error: "Missing JSON report data." };
  try {
    return { report: validateReport(JSON.parse(node.textContent)) };
  } catch (error) {
    return { error: error instanceof Error ? error.message : "Invalid report JSON." };
  }
}

export function deriveFiles(report: ScanReport, sort: EvidenceSort = "counts"): FileView[] {
  const metrics = new Map(report.raw_metrics.files.map(item=>[item.path,item]));
  const paths = new Set([...metrics.keys(),...report.findings.map(x=>x.path),...report.issues.map(x=>x.path),...report.dependency_graph.nodes.map(x=>x.path)]);
  const coverage = new Map(report.agent_evidence.files.map(x=>[x.path,x.coverage_status]));
  const files = [...paths].map(path=>{ const raw=metrics.get(path); return { path, loc:raw?.loc??0, imports:raw?.imports??0, publicItems:raw?.public_items??0, churn:raw?.churn?.recent_weighted_churn??0, findings:report.findings.filter(x=>x.path===path).length, issues:report.issues.filter(x=>x.path===path).length, coverageStatus:coverage.get(path)??"unsupported", isTest:raw?.is_test??false }; });
  return files.sort((a,b)=> sort==="path" ? a.path.localeCompare(b.path) : sort==="churn" ? b.churn-a.churn || a.path.localeCompare(b.path) : b.issues-a.issues || b.findings-a.findings || a.path.localeCompare(b.path) || b.churn-a.churn);
}

export function deriveInspector(report: ScanReport, file: FileView): FileInspector {
  const findings=report.findings.filter(x=>x.path===file.path || x.related_locations.some(y=>y.path===file.path));
  const issues=report.issues.filter(x=>x.path===file.path || findings.some(y=>y.issue_id===x.id));
  const related=[...new Set(findings.flatMap(x=>x.related_locations.map(y=>`${y.path}:${y.line}`)))].sort();
  const edges=report.dependency_graph.edges;
  return {...file,metrics:report.raw_metrics.files.find(x=>x.path===file.path),fileFindings:findings,fileIssues:issues,relatedLocations:related,incoming:edges.filter(x=>x.to===file.path).map(x=>x.from).sort(),outgoing:edges.filter(x=>x.from===file.path).map(x=>x.to).sort(),agent:report.agent_evidence.files.find(x=>x.path===file.path)};
}

export function layerValue(file: FileView, layer: MapLayer): number { return layer==="findings"?file.findings:layer==="issues"?file.issues:layer==="churn"?file.churn:["observed","not_applicable"].includes(file.coverageStatus)?1:0; }
export function evidenceFamilies(report: ScanReport) { const counts=new Map<string,number>(); for(const issue of report.issues) counts.set(issue.family,(counts.get(issue.family)??0)+1); return [...counts].sort((a,b)=>b[1]-a[1]||a[0].localeCompare(b[0])); }
