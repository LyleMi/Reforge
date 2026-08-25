import React, { useEffect, useMemo, useState } from "react";
import "./coverage.css";
import { initialLocale, persistLocale, t, translatedLabel, type Locale } from "./locale";
import { parseEmbeddedReport, subjectLabel } from "./reportModel";
import type { CoverageLimitation, Evidence, Issue, Report } from "./reportTypes";

const location = (path: string, line?: number) => line ? `${path}:${line}` : path;

function LanguagePicker({ locale, onChange }: { locale: Locale; onChange: (locale: Locale) => void }) {
  return <label className="language-picker"><span>{t(locale, "language")}</span><select aria-label={t(locale, "language")} value={locale} onChange={event => onChange(event.target.value as Locale)}><option value="en">EN</option><option value="zh-CN">中文</option></select></label>;
}

function Limitations({ items, locale }: { items?: CoverageLimitation[]; locale: Locale }) {
  if (!items?.length) return null;
  return <div className="limitations-list"><strong>{t(locale, "limitations")}</strong>{items.map(item => <p key={item.code}>{item.code} ({item.count}): {item.message}</p>)}</div>;
}

function Witness({ evidence, locale }: { evidence: Evidence; locale: Locale }) {
  const witness = evidence.witness;
  if (!witness) return null;
  return <div className="flow-witness">
    <b>{witness.source.symbol} → {witness.sink.symbol}</b>
    <small>{witness.function_hops} {t(locale, "functionHops")} · {witness.module_hops} {t(locale, "moduleHops")} · {translatedLabel(locale, witness.resolution)}</small>
    <ol>{witness.ordered_steps.map((step, index) =>
      <li key={`${step.path}-${step.symbol}-${index}`}>{translatedLabel(locale, step.operation)} · {location(step.path, step.line)} · {step.symbol}</li>)}
    </ol>
  </div>;
}

function IssueView({ issue, maturity, baseline, locale }: { issue: Issue; maturity: string; baseline: string; locale: Locale }) {
  return <article className="issue">
    <div><span className="eyebrow">{issue.family}</span><div className="issue-meta"><span className={`badge ${issue.kind}`}>{translatedLabel(locale, issue.kind)}</span><span className="badge">{translatedLabel(locale, maturity)}</span>{baseline && <span className={`badge ${baseline}`}>{translatedLabel(locale, baseline)}</span>}</div><h3>{issue.title}</h3><p>{issue.guidance}</p><small>{subjectLabel(issue.subject, locale)} · {issue.id}</small></div>
    {issue.evidence.map(evidence => <details className="evidence" key={evidence.id}>
      <summary>{evidence.rule}: {evidence.message}</summary>
      <div className="locations">{evidence.locations?.map(item => <code key={`${item.path}-${item.line}-${item.symbol}`}>{location(item.path, item.line)}{item.symbol ? ` · ${item.symbol}` : ""}</code>)}</div>
      {evidence.measurements?.length ? <dl>{evidence.measurements.map(item => <React.Fragment key={item.name}><dt>{translatedLabel(locale, item.name)}</dt><dd>{String(item.value)} {translatedLabel(locale, item.unit)}{item.threshold === undefined ? "" : ` (${t(locale, "threshold")} ${String(item.threshold)})`}</dd></React.Fragment>)}</dl> : null}
      <Witness evidence={evidence} locale={locale} />
    </details>)}
  </article>;
}

function ReportView({ report, locale, onLocaleChange }: { report: Report; locale: Locale; onLocaleChange: (locale: Locale) => void }) {
  const [query, setQuery] = useState("");
  const [analysis, setAnalysis] = useState("");
  const [kind, setKind] = useState("");
  const [maturity, setMaturity] = useState("");
  const [baseline, setBaseline] = useState("");
  const analyses = Object.keys(report.coverage).sort();
  const issueMaturity = (issue: Issue) => issue.evidence.map(evidence => report.coverage[issue.analysis]?.rules?.[evidence.rule]?.maturity).find(Boolean) ?? "unknown";
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
    || subjectLabel(left.subject, locale).localeCompare(subjectLabel(right.subject, locale))
    || left.id.localeCompare(right.id)
  ), [report, query, analysis, kind, maturity, baseline, locale]);
  const baselineCounts = Object.values(report.baseline_comparison?.issues ?? {}).reduce<Record<string, number>>((counts, entry) => {
    counts[entry.state] = (counts[entry.state] ?? 0) + 1;
    return counts;
  }, {});
  return <main>
    <div className="report-toolbar"><LanguagePicker locale={locale} onChange={onLocaleChange} /></div>
    <header><div><span className="eyebrow">{t(locale, "reportEyebrow")}</span><h1>{t(locale, "title")}</h1><p>{report.producer.name} {report.producer.version}</p></div><div className="scan-meta"><b>{report.summary.issue_count} {t(locale, report.summary.issue_count === 1 ? "issue" : "issues")}</b><span>{report.summary.evidence_count} {t(locale, "evidence")} {t(locale, report.summary.evidence_count === 1 ? "item" : "items")}</span><span>{report.summary.scanned_files} {t(locale, "files")}</span></div></header>
    <section className="cards"><article className="card"><span>{t(locale, "issuesCard")}</span><strong>{report.summary.issue_count}</strong></article><article className="card"><span>{t(locale, "evidenceCard")}</span><strong>{report.summary.evidence_count}</strong></article><article className="card"><span>{t(locale, "suppressed")}</span><strong>{report.suppression.evidence_count}</strong></article></section>
    {report.baseline_comparison && <section className="panel"><h2>{t(locale, "baseline")}</h2><p>{Object.entries(baselineCounts).sort().map(([state, count]) => `${count} ${translatedLabel(locale, state)}`).join(" · ")}</p></section>}
    <section><div className="list-heading"><h2>{t(locale, "list")}</h2><div className="filters"><input aria-label={t(locale, "filter")} placeholder={t(locale, "search")} value={query} onChange={event => setQuery(event.target.value)} /><select aria-label={t(locale, "analysis")} value={analysis} onChange={event => setAnalysis(event.target.value)}><option value="">{t(locale, "allAnalyses")}</option>{analyses.map(value => <option value={value} key={value}>{translatedLabel(locale, value)}</option>)}</select><select aria-label={t(locale, "kind")} value={kind} onChange={event => setKind(event.target.value)}><option value="">{t(locale, "allKinds")}</option><option value="policy">{t(locale, "policy")}</option><option value="advisory">{t(locale, "advisory")}</option></select><select aria-label={t(locale, "maturity")} value={maturity} onChange={event => setMaturity(event.target.value)}><option value="">{t(locale, "allMaturities")}</option>{["stable", "preview", "experimental"].map(value => <option value={value} key={value}>{translatedLabel(locale, value)}</option>)}</select><select aria-label={t(locale, "baselineState")} value={baseline} onChange={event => setBaseline(event.target.value)}><option value="">{t(locale, "allBaseline")}</option>{["new", "updated", "unknown", "unchanged", "absent"].map(value => <option value={value} key={value}>{translatedLabel(locale, value)}</option>)}</select></div></div>{issues.length ? issues.map(issue => <IssueView issue={issue} maturity={issueMaturity(issue)} baseline={issueBaseline(issue)} locale={locale} key={issue.id} />) : <p className="empty">{t(locale, "noIssues")}</p>}</section>
    <details className="panel coverage-panel" open={report.summary.issue_count === 0}><summary><h2>{t(locale, "coverage")}</h2><span>{analyses.map(name => `${translatedLabel(locale, name)}: ${translatedLabel(locale, report.coverage[name].status)}`).join(" · ")}</span></summary>{analyses.map(name => {
      const coverage = report.coverage[name];
      return <article className="coverage" key={name}><h3>{translatedLabel(locale, name)}</h3><span className={`status ${coverage.status}`}>{translatedLabel(locale, coverage.status)}</span><p>{coverage.scanned_files} {t(locale, "scannedFiles")}</p>
        {Object.entries(coverage.languages ?? {}).map(([language, counts]) => <div key={language}>
          <small>{translatedLabel(locale, language)}: {translatedLabel(locale, counts.status)} · {counts.files} {t(locale, "files")} · {counts.functions} {translatedLabel(locale, "functions")}</small>
          <Limitations items={counts.limitations} locale={locale} />
          {Object.entries(counts.capabilities ?? {}).map(([capability, receipt]) => <div key={`${language}-${capability}`}>
            <small>{translatedLabel(locale, capability)}: {translatedLabel(locale, receipt.status)}</small>
            <Limitations items={receipt.limitations} locale={locale} />
          </div>)}
        </div>)}
        <Limitations items={coverage.limitations} locale={locale} />
        {Object.entries(coverage.rules ?? {}).map(([ruleName, rule]) =>
          <div key={ruleName}><p>{ruleName}: {translatedLabel(locale, rule.maturity)} · {translatedLabel(locale, rule.enabled_source)} · {translatedLabel(locale, rule.status)}</p>
            {rule.observations?.map(item => <small key={item.name}>{translatedLabel(locale, item.name)}: {item.count} {translatedLabel(locale, item.unit)}</small>)}
            <Limitations items={rule.limitations} locale={locale} />
          </div>)}
      </article>;
    })}</details>
    <footer>{report.target.workspace_identity} · {t(locale, "absence")}</footer>
  </main>;
}

export function App() {
  const parsed = useMemo(parseEmbeddedReport, []);
  const [locale, setLocale] = useState<Locale>(initialLocale);
  useEffect(() => {
    document.documentElement.lang = locale;
    document.title = locale === "zh-CN" ? "Reforge 分析报告" : "Reforge analysis report";
  }, [locale]);
  const changeLocale = (next: Locale) => { persistLocale(next); setLocale(next); };
  if (parsed.error) return <main className="error"><LanguagePicker locale={locale} onChange={changeLocale} /><h1>{t(locale, "reportUnavailable")}</h1><p>{t(locale, "errorLead")}</p><details><summary>{t(locale, "details")}</summary><p>{parsed.error}</p></details></main>;
  return <ReportView report={parsed.report!} locale={locale} onLocaleChange={changeLocale} />;
}
