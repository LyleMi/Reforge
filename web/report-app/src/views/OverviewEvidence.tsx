import React from "react";
import type { EvidenceReport } from "./EvidenceTypes";
import type { ViewState } from "../viewState";
import { evidenceFamilies } from "../reportModel";
import { Card, FindingRow, label, location } from "./Common";

export function OverviewView({ report }: { report: EvidenceReport }) {
  const gaps = report.coverage_manifest.filter(cell => !["observed", "no_entities", "not_applicable", "intentionally_out_of_scope"].includes(cell.status));
  const families = evidenceFamilies(report);
  return <section>
    <div className="cards"><Card label="Issues" value={report.issues.length} meta="decision units"/><Card label="Atomic findings" value={report.findings.length} meta="detector evidence"/><Card label="Coverage gaps" value={gaps.length} meta="partial or unsupported"/><Card label="Suppressions" value={report.suppression_summary.suppressed_count} meta="removed evidence"/><Card label="Evidence families" value={families.length} meta="observed issue families"/></div>
    <div className="columns">
      <section className="panel"><h2>Evidence families</h2>{families.length ? families.map(([family, count]) => <div className="bar" key={family}><span>{label(family)}</span><i style={{ width: `${Math.max(4, count / Math.max(1, families[0][1]) * 100)}%` }}/><b>{count}</b></div>) : <p className="empty">No unsuppressed issue families were observed.</p>}</section>
      <section className="panel"><h2>Observation limits</h2>{gaps.slice(0, 12).map(cell => <article className="compact" key={`${cell.mechanism}-${cell.entity_scope}`}><b>{label(cell.mechanism)} · {label(cell.entity_scope)}</b><span>{label(cell.status)}</span><p>{cell.reason}</p></article>)}{!gaps.length && <p className="empty">No coverage gaps were reported.</p>}</section>
    </div>
  </section>;
}

type EvidenceProps = { report: EvidenceReport; state: ViewState; go: (patch: Partial<ViewState>) => void; openFile: (path: string) => void };
export function EvidenceView({ report, state, go, openFile }: EvidenceProps) {
  const query = state.query.toLowerCase();
  const kinds = [...new Set(report.findings.map(item => item.kind))].sort();
  const issues = report.issues.filter(issue => `${issue.summary} ${issue.path} ${issue.family}`.toLowerCase().includes(query));
  const findings = report.findings.filter(item => (!state.kind || item.kind === state.kind) && `${item.kind} ${item.path} ${item.message}`.toLowerCase().includes(query));
  return <section className="panel">
    <div className="toolbar"><input aria-label="Search evidence" placeholder="Search evidence" value={state.query} onChange={event => go({ query: event.target.value })}/><select aria-label="Finding kind" value={state.kind} onChange={event => go({ kind: event.target.value })}><option value="">All finding kinds</option>{kinds.map(kind => <option key={kind}>{kind}</option>)}</select><select aria-label="Sort files" value={state.sort} onChange={event => go({ sort: event.target.value as ViewState["sort"] })}><option value="counts">Issue and finding count</option><option value="path">Path</option><option value="churn">Churn</option></select></div>
    <h2>Issues and atomic evidence</h2>
    {issues.map(issue => <article className="issue-row" key={issue.id}><div><b>{issue.summary}</b><button onClick={() => openFile(issue.path)}>{location(issue.path, issue.line)}</button></div><span>{label(issue.action)}</span><small>{issue.finding_ids.length} atomic finding{issue.finding_ids.length === 1 ? "" : "s"} · {issue.id}</small></article>)}
    <div className="finding-list">{findings.map(item => <FindingRow key={item.id} finding={item} onFile={openFile}/>)}</div>
  </section>;
}
