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

function IssueView({ issue, maturity, baseline }: { issue: Issue; maturity: string; baseline: string }) {
  return <article className="issue">
    <div><span className="eyebrow">{issue.family}</span><div className="issue-meta"><span className={`badge ${issue.kind}`}>{label(issue.kind)}</span><span className="badge">{label(maturity)}</span>{baseline && <span className={`badge ${baseline}`}>{label(baseline)}</span>}</div><h3>{issue.title}</h3><p>{issue.guidance}</p><small>{subjectLabel(issue.subject)} · {issue.id}</small></div>
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
  const [kind, setKind] = useState("");
  const [maturity, setMaturity] = useState("");
  const [baseline, setBaseline] = useState("");
  const analyses = Object.keys(report.coverage).sort();
  const issueMaturity = (issue: Issue) => issue.evidence
    .map(evidence => report.coverage[issue.analysis]?.rules?.[evidence.rule]?.maturity)
    .find(Boolean) ?? "unknown";
  const issueBaseline = (issue: Issue) => report.baseline_comparison?.issues[issue.id]?.state ?? "";
  const baselineRank: Record<string, number> = { new: 0, updated: 1, unknown: 2, unchanged: 3, absent: 4, "": 5 };
  const issues = useMemo(() => report.issues.filter(issue =>
    JSON.stringify(issue).toLowerCase().includes(query.toLowerCase())
    && (!analysis || issue.analysis === analysis)
    && (!kind || issue.kind === kind)
    && (!maturity || issueMaturity(issue) === maturity)
    && (!baseline || issueBaseline(issue) === baseline)
  ).sort((left, right) =>
    Number(left.kind !== "policy") - Number(right.kind !== "policy")
    || baselineRank[issueBaseline(left)] - baselineRank[issueBaseline(right)]
    || left.analysis.localeCompare(right.analysis)
    || left.family.localeCompare(right.family)
    || subjectLabel(left.subject).localeCompare(subjectLabel(right.subject))
    || left.id.localeCompare(right.id)
  ), [report, query, analysis, kind, maturity, baseline]);
  const baselineCounts = Object.values(report.baseline_comparison?.issues ?? {}).reduce<Record<string, number>>((counts, entry) => {
    counts[entry.state] = (counts[entry.state] ?? 0) + 1;
    return counts;
  }, {});
  return <main>
    <header><div><span className="eyebrow">Schema 27 analysis report</span><h1>Refactoring evidence</h1><p>{report.producer.name} {report.producer.version}</p></div><div className="scan-meta"><b>{report.summary.issue_count} issues</b><span>{report.summary.evidence_count} evidence</span><span>{report.summary.scanned_files} files</span></div></header>
    <section className="cards"><article className="card"><span>Issues</span><strong>{report.summary.issue_count}</strong></article><article className="card"><span>Evidence</span><strong>{report.summary.evidence_count}</strong></article><article className="card"><span>Suppressed</span><strong>{report.suppression.evidence_count}</strong></article></section>
    {report.baseline_comparison && <section className="panel"><h2>Baseline</h2><p>{Object.entries(baselineCounts).sort().map(([state, count]) => `${count} ${state}`).join(" · ")}</p></section>}
    <section className="panel"><h2>Coverage</h2>{analyses.map(name => {
      const coverage = report.coverage[name];
      return <article className="coverage" key={name}><h3>{label(name)}</h3><span className={`status ${coverage.status}`}>{label(coverage.status)}</span><p>{coverage.scanned_files} scanned files</p>
        {Object.entries(coverage.languages ?? {}).map(([language, counts]) => <div key={language}>
          <small>{label(language)}: {label(counts.status)} · {counts.files} files · {counts.functions} functions</small>
          {counts.limitations?.map(item => <p key={`${language}-${item.code}`}>{item.code} ({item.count}): {item.message}</p>)}
          {Object.entries(counts.capabilities ?? {}).map(([capability, receipt]) => <div key={`${language}-${capability}`}>
            <small>{label(capability)}: {label(receipt.status)}</small>
            {receipt.limitations?.map(item => <p key={`${language}-${capability}-${item.code}`}>{item.code} ({item.count}): {item.message}</p>)}
          </div>)}
        </div>)}
        {coverage.limitations?.map(item => <p key={item.code}>{item.code} ({item.count}): {item.message}</p>)}
        {Object.entries(coverage.rules ?? {}).map(([ruleName, rule]) =>
          <div key={ruleName}><p>{ruleName}: {label(rule.maturity)} · {label(rule.enabled_source)} · {label(rule.status)}</p>
            {rule.observations?.map(item => <small key={item.name}>{label(item.name)}: {item.count} {item.unit}</small>)}
            {rule.limitations?.map(item => <p key={`${ruleName}-${item.code}`}>{item.code} ({item.count}): {item.message}</p>)}
          </div>)}
      </article>;
    })}</section>
    <section><div className="list-heading"><h2>Issues and evidence</h2><div className="filters"><input aria-label="Filter issues" placeholder="Search issues and evidence" value={query} onChange={event => setQuery(event.target.value)} /><select aria-label="Analysis" value={analysis} onChange={event => setAnalysis(event.target.value)}><option value="">All analyses</option>{analyses.map(value => <option value={value} key={value}>{label(value)}</option>)}</select><select aria-label="Kind" value={kind} onChange={event => setKind(event.target.value)}><option value="">Advisory and policy</option><option value="policy">Policy</option><option value="advisory">Advisory</option></select><select aria-label="Maturity" value={maturity} onChange={event => setMaturity(event.target.value)}><option value="">All maturities</option><option value="stable">Stable</option><option value="preview">Preview</option><option value="experimental">Experimental</option></select><select aria-label="Baseline state" value={baseline} onChange={event => setBaseline(event.target.value)}><option value="">All baseline states</option>{["new", "updated", "unknown", "unchanged", "absent"].map(value => <option value={value} key={value}>{label(value)}</option>)}</select></div></div>{issues.length ? issues.map(issue => <IssueView issue={issue} maturity={issueMaturity(issue)} baseline={issueBaseline(issue)} key={issue.id} />) : <p className="empty">No issues reported.</p>}</section>
    <footer>{report.target.workspace_identity} · absence is meaningful only for observed analyses.</footer>
  </main>;
}

export function App() {
  const parsed = useMemo(parseEmbeddedReport, []);
  if (parsed.error) return <main className="error"><h1>Report unavailable</h1><p>{parsed.error}</p></main>;
  return <ReportView report={parsed.report!} />;
}
