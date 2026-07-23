import React, { useMemo, useState } from "react";
import { parseEmbeddedReport, subjectLabel } from "./reportModel";
import type { Evidence, Issue, Report } from "./reportTypes";

const label = (value: string) => value.replace(/[._]/g, " ").replace(/\b\w/g, character => character.toUpperCase());
const location = (path: string, line?: number) => line ? `${path}:${line}` : path;

function Witness({ evidence }: { evidence: Evidence }) {
  const witness = evidence.witness;
  if (!witness) return null;
  return <div className="flow-witness">
    <b>{witness.source.symbol} → {witness.sink.symbol}</b>
    <small>{witness.function_hops} function hops · {witness.module_hops} module hops · {label(witness.resolution)}</small>
    <ol>{witness.ordered_steps.map((step, index) =>
      <li key={`${step.path}-${step.symbol}-${index}`}>{label(step.operation)} · {location(step.path, step.line)} · {step.symbol}</li>)}
    </ol>
  </div>;
}

function IssueView({ issue }: { issue: Issue }) {
  return <article className="issue">
    <div><span className="eyebrow">{issue.family}</span><h3>{issue.title}</h3><p>{issue.guidance}</p><small>{subjectLabel(issue.subject)} · {issue.id}</small></div>
    {issue.evidence.map(evidence => <details className="evidence" key={evidence.id}>
      <summary>{evidence.rule}: {evidence.message}</summary>
      <div className="locations">{evidence.locations?.map(item => <code key={`${item.path}-${item.line}-${item.symbol}`}>{location(item.path, item.line)}{item.symbol ? ` · ${item.symbol}` : ""}</code>)}</div>
      {evidence.measurements?.length ? <dl>{evidence.measurements.map(item => <React.Fragment key={item.name}><dt>{label(item.name)}</dt><dd>{String(item.value)} {item.unit}{item.threshold === undefined ? "" : ` (threshold ${String(item.threshold)})`}</dd></React.Fragment>)}</dl> : null}
      <Witness evidence={evidence} />
    </details>)}
  </article>;
}

function ReportView({ report }: { report: Report }) {
  const [query, setQuery] = useState("");
  const [analysis, setAnalysis] = useState("");
  const analyses = Object.keys(report.coverage).sort();
  const issues = useMemo(() => report.issues.filter(issue =>
    JSON.stringify(issue).toLowerCase().includes(query.toLowerCase())
    && (!analysis || issue.analysis === analysis)
  ), [report.issues, query, analysis]);
  return <main>
    <header><div><span className="eyebrow">Schema 26 analysis report</span><h1>Refactoring issues</h1><p>{report.producer.name} {report.producer.version}</p></div><div className="scan-meta"><b>{report.summary.issue_count} issues</b><span>{report.summary.evidence_count} evidence</span><span>{report.summary.scanned_files} files</span></div></header>
    <section className="cards"><article className="card"><span>Issues</span><strong>{report.summary.issue_count}</strong></article><article className="card"><span>Evidence</span><strong>{report.summary.evidence_count}</strong></article><article className="card"><span>Suppressed</span><strong>{report.suppression.evidence_count}</strong></article></section>
    {report.baseline_comparison && <section className="panel"><h2>Baseline</h2><p>{report.baseline_comparison.new_issue_ids.length} new · {report.baseline_comparison.resolved_issue_ids.length} resolved · {report.baseline_comparison.unchanged_issue_count} unchanged</p></section>}
    <section className="panel"><h2>Coverage</h2>{analyses.map(name => {
      const coverage = report.coverage[name];
      return <article className="coverage" key={name}><h3>{label(name)}</h3><span className={`status ${coverage.status}`}>{label(coverage.status)}</span><p>{coverage.scanned_files} scanned files</p>
        {Object.entries(coverage.languages ?? {}).map(([language, counts]) => <div key={language}>
          <small>{label(language)}: {label(counts.status)} · {counts.files} files · {counts.functions} functions</small>
          {counts.limitations?.map(item => <p key={`${language}-${item.code}`}>{item.code} ({item.count}): {item.message}</p>)}
        </div>)}
        {coverage.limitations?.map(item => <p key={item.code}>{item.code} ({item.count}): {item.message}</p>)}
        {Object.entries(coverage.rules ?? {}).map(([ruleName, rule]) =>
          <div key={ruleName}><p>{ruleName}: {label(rule.status)}</p>
            {rule.observations?.map(item => <small key={item.name}>{label(item.name)}: {item.count} {item.unit}</small>)}
            {rule.limitations?.map(item => <p key={`${ruleName}-${item.code}`}>{item.code} ({item.count}): {item.message}</p>)}
          </div>)}
      </article>;
    })}</section>
    <section><div className="list-heading"><h2>Issues and evidence</h2><div className="filters"><input aria-label="Filter issues" placeholder="Search issues and evidence" value={query} onChange={event => setQuery(event.target.value)} /><select aria-label="Analysis" value={analysis} onChange={event => setAnalysis(event.target.value)}><option value="">All analyses</option>{analyses.map(value => <option value={value} key={value}>{label(value)}</option>)}</select></div></div>{issues.length ? issues.map(issue => <IssueView issue={issue} key={issue.id} />) : <p className="empty">No issues reported.</p>}</section>
    <footer>{report.target.workspace_identity} · absence is meaningful only for observed analyses.</footer>
  </main>;
}

export function App() {
  const parsed = useMemo(parseEmbeddedReport, []);
  if (parsed.error) return <main className="error"><h1>Report unavailable</h1><p>{parsed.error}</p></main>;
  return <ReportView report={parsed.report!} />;
}
