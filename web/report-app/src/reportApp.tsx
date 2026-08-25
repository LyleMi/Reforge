import React, { useEffect, useMemo, useRef, useState } from "react";
import "./coverage.css";
import { initialLocale, persistLocale, t, translatedLabel, type Locale } from "./locale";
import { parseEmbeddedReport, subjectLabel } from "./reportModel";
import type { CoverageLimitation, Evidence, Issue, Report } from "./reportTypes";

const locationLabel = (path: string, line?: number) => line ? `${path}:${line}` : path;

function LanguagePicker({ locale, onChange }: { locale: Locale; onChange: (locale: Locale) => void }) {
  return <label className="language-picker"><span>{t(locale, "language")}</span><select aria-label={t(locale, "language")} value={locale} onChange={event => onChange(event.target.value as Locale)}><option value="en">EN</option><option value="zh-CN">中文</option></select></label>;
}

function Limitations({ items, locale }: { items?: CoverageLimitation[]; locale: Locale }) {
  if (!items?.length) return null;
  return <div className="limitations-list"><strong>{t(locale, "limitations")}</strong>{items.map(item => <p key={item.code}><code>{item.code}</code> ({item.count}): {item.message}</p>)}</div>;
}

function Witness({ evidence, locale }: { evidence: Evidence; locale: Locale }) {
  const witness = evidence.witness;
  if (!witness) return null;
  return <section className="flow-witness" aria-label={t(locale, "witness")}>
    <div className="section-label">{t(locale, "witness")}</div>
    <b>{witness.source.symbol} <span aria-hidden="true">→</span> {witness.sink.symbol}</b>
    <small>{witness.function_hops} {t(locale, "functionHops")} · {witness.module_hops} {t(locale, "moduleHops")} · {translatedLabel(locale, witness.resolution)}</small>
    <ol>{witness.ordered_steps.map((step, index) => <li key={`${step.path}-${step.symbol}-${index}`}><span>{index + 1}</span><div><b>{step.symbol}</b><small>{translatedLabel(locale, step.operation)} · {locationLabel(step.path, step.line)}</small></div></li>)}</ol>
  </section>;
}

function EvidenceView({ evidence, locale }: { evidence: Evidence; locale: Locale }) {
  return <article className="evidence" data-testid="evidence">
    <div className="evidence-heading"><code>{evidence.rule}</code><p>{evidence.message}</p></div>
    {evidence.locations?.length ? <section><div className="section-label">{t(locale, "locations")}</div><div className="locations">{evidence.locations.map(item => <code key={`${item.path}-${item.line}-${item.symbol}`}>{locationLabel(item.path, item.line)}{item.symbol ? <span> · {item.symbol}</span> : null}</code>)}</div></section> : null}
    {evidence.measurements?.length ? <section><div className="section-label">{t(locale, "measurements")}</div><dl className="measurements">{evidence.measurements.map(item => <React.Fragment key={item.name}><dt>{translatedLabel(locale, item.name)}</dt><dd><strong>{String(item.value)}</strong> {translatedLabel(locale, item.unit)}{item.threshold === undefined ? "" : <small>{t(locale, "threshold")}: {String(item.threshold)}</small>}</dd></React.Fragment>)}</dl></section> : null}
    <Witness evidence={evidence} locale={locale} />
    <details className="technical"><summary>{t(locale, "technical")}</summary><dl><dt>{t(locale, "evidenceId")}</dt><dd><code>{evidence.id}</code></dd></dl></details>
  </article>;
}

function IssueDetail({ issue, maturity, baseline, locale, headingRef }: { issue: Issue; maturity: string; baseline: string; locale: Locale; headingRef: React.RefObject<HTMLHeadingElement | null> }) {
  return <article className="issue-detail issue" aria-labelledby="issue-detail-title">
    <div className="detail-kicker"><span className={`kind-mark ${issue.kind}`}>{translatedLabel(locale, issue.kind)}</span>{baseline ? <span className={`baseline-mark ${baseline}`}>{translatedLabel(locale, baseline)}</span> : <span className="baseline-mark muted">{t(locale, "baselineUnavailable")}</span>}<span className="maturity">{translatedLabel(locale, maturity)}</span></div>
    <p className="family">{issue.analysis} / {issue.family}</p>
    <h2 id="issue-detail-title" tabIndex={-1} ref={headingRef}>{issue.title}</h2>
    <div className="detail-intro"><section><div className="section-label">{t(locale, "guidance")}</div><p>{issue.guidance}</p></section><section><div className="section-label">{t(locale, "subject")}</div><p className="subject">{subjectLabel(issue.subject, locale)}</p></section></div>
    <div className="evidence-stack">{issue.evidence.map(evidence => <EvidenceView evidence={evidence} locale={locale} key={evidence.id} />)}</div>
    <details className="technical issue-technical"><summary>{t(locale, "technical")}</summary><dl><dt>{t(locale, "issueId")}</dt><dd><code>{issue.id}</code></dd><dt>{t(locale, "fingerprint")}</dt><dd><code>{issue.content_fingerprint}</code></dd></dl></details>
  </article>;
}

function Coverage({ report, analyses, locale }: { report: Report; analyses: string[]; locale: Locale }) {
  return <details className="coverage-panel" open={report.summary.issue_count === 0}>
    <summary><div><span className="section-label">{t(locale, "summary")}</span><h2>{t(locale, "coverage")}</h2></div><span>{analyses.map(name => `${translatedLabel(locale, name)}: ${translatedLabel(locale, report.coverage[name].status)}`).join(" · ")}</span></summary>
    <div className="coverage-grid">{analyses.map(name => {
      const coverage = report.coverage[name];
      return <article className="coverage" key={name}><div className="coverage-title"><h3>{translatedLabel(locale, name)}</h3><span className={`status ${coverage.status}`}>{translatedLabel(locale, coverage.status)}</span></div><p>{coverage.scanned_files} {t(locale, "scannedFiles")}</p>
        {Object.entries(coverage.languages ?? {}).map(([language, counts]) => <div className="coverage-section" key={language}><b>{translatedLabel(locale, language)}</b><small>{translatedLabel(locale, counts.status)} · {counts.files} {t(locale, "files")} · {counts.functions} {translatedLabel(locale, "functions")}</small><Limitations items={counts.limitations} locale={locale} />{Object.entries(counts.capabilities ?? {}).map(([capability, receipt]) => <div key={capability}><p><code>{translatedLabel(locale, capability)}</code>: {translatedLabel(locale, receipt.status)}</p><Limitations items={receipt.limitations} locale={locale} /></div>)}</div>)}
        <Limitations items={coverage.limitations} locale={locale} />
        {Object.entries(coverage.rules ?? {}).map(([ruleName, rule]) => <div className="coverage-section" key={ruleName}><code>{ruleName}</code><small>{translatedLabel(locale, rule.maturity)} · {translatedLabel(locale, rule.enabled_source)} · {translatedLabel(locale, rule.status)}</small>{rule.observations?.map(item => <small key={item.name}>{translatedLabel(locale, item.name)}: {item.count} {translatedLabel(locale, item.unit)}</small>)}<Limitations items={rule.limitations} locale={locale} /></div>)}
      </article>;
    })}</div>
  </details>;
}

const hashIssue = () => {
  const match = window.location.hash.match(/^#issue=(.*)$/);
  if (!match) return "";
  try { return decodeURIComponent(match[1]); } catch { return ""; }
};

function ReportView({ report, locale, onLocaleChange }: { report: Report; locale: Locale; onLocaleChange: (locale: Locale) => void }) {
  const [query, setQuery] = useState("");
  const [analysis, setAnalysis] = useState("");
  const [kind, setKind] = useState("");
  const [maturity, setMaturity] = useState("");
  const [baseline, setBaseline] = useState("");
  const [selectedId, setSelectedId] = useState(hashIssue);
  const detailHeading = useRef<HTMLHeadingElement>(null);
  const analyses = Object.keys(report.coverage).sort();
  const issueMaturity = (issue: Issue) => issue.evidence.map(evidence => report.coverage[issue.analysis]?.rules?.[evidence.rule]?.maturity).find(Boolean) ?? "unknown";
  const issueBaseline = (issue: Issue) => report.baseline_comparison?.issues[issue.id]?.state ?? "";
  const baselineRank: Record<string, number> = { new: 0, updated: 1, unknown: 2, unchanged: 3, absent: 4, "": 5 };
  const issues = useMemo(() => report.issues.filter(issue => JSON.stringify(issue).toLowerCase().includes(query.trim().toLowerCase()) && (!analysis || issue.analysis === analysis) && (!kind || issue.kind === kind) && (!maturity || issueMaturity(issue) === maturity) && (!baseline || issueBaseline(issue) === baseline)).sort((left, right) => Number(left.kind !== "policy") - Number(right.kind !== "policy") || baselineRank[issueBaseline(left)] - baselineRank[issueBaseline(right)] || left.analysis.localeCompare(right.analysis) || left.family.localeCompare(right.family) || subjectLabel(left.subject, locale).localeCompare(subjectLabel(right.subject, locale)) || left.id.localeCompare(right.id)), [report, query, analysis, kind, maturity, baseline, locale]);
  const selected = issues.find(issue => issue.id === selectedId) ?? issues[0];
  const partial = analyses.some(name => !["observed", "not_applicable"].includes(report.coverage[name].status));
  const filtersActive = Boolean(query || analysis || kind || maturity || baseline);

  useEffect(() => {
    const next = selected?.id ?? "";
    if (next !== selectedId) setSelectedId(next);
    const url = new URL(window.location.href);
    url.hash = next ? `issue=${encodeURIComponent(next)}` : "";
    window.history.replaceState(null, "", url);
  }, [selected?.id, selectedId]);
  useEffect(() => {
    const syncHash = () => setSelectedId(hashIssue());
    window.addEventListener("hashchange", syncHash);
    return () => window.removeEventListener("hashchange", syncHash);
  }, []);

  const choose = (id: string, moveToDetail = false) => {
    setSelectedId(id);
    if (moveToDetail && window.matchMedia("(max-width: 760px)").matches) requestAnimationFrame(() => { detailHeading.current?.focus(); detailHeading.current?.scrollIntoView({ behavior: "smooth", block: "start" }); });
  };
  const clearFilters = () => { setQuery(""); setAnalysis(""); setKind(""); setMaturity(""); setBaseline(""); };
  const navigateList = (event: React.KeyboardEvent, index: number) => {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const nextIndex = event.key === "Home" ? 0 : event.key === "End" ? issues.length - 1 : Math.max(0, Math.min(issues.length - 1, index + (event.key === "ArrowDown" ? 1 : -1)));
    choose(issues[nextIndex].id);
    document.getElementById(`issue-option-${nextIndex}`)?.focus();
  };

  return <main className="report-shell">
    <header className="topbar"><div className="brand"><span className="brand-mark" aria-hidden="true" /><div><b>REFORGE</b><span>{t(locale, "reportEyebrow")}</span></div></div><div className="workspace"><span>{t(locale, "workspace")}</span><strong>{report.target.workspace_identity}</strong></div><LanguagePicker locale={locale} onChange={onLocaleChange} /></header>
    <section className="report-summary"><div><p>{report.producer.name} {report.producer.version}</p><h1>{t(locale, "title")}</h1></div><dl><div><dt>{t(locale, "issuesCard")}</dt><dd>{report.summary.issue_count}</dd></div><div><dt>{t(locale, "evidenceCard")}</dt><dd>{report.summary.evidence_count}</dd></div><div><dt>{t(locale, "files")}</dt><dd>{report.summary.scanned_files}</dd></div><div><dt>{t(locale, "suppressed")}</dt><dd>{report.suppression.evidence_count}</dd></div></dl></section>
    {partial ? <div className="coverage-alert" role="status"><span />{t(locale, "coveragePartial")}: {analyses.filter(name => !["observed", "not_applicable"].includes(report.coverage[name].status)).map(name => translatedLabel(locale, name)).join(", ")}</div> : null}
    <section className="workbench">
      <aside className="issue-browser" aria-label={t(locale, "issueList")}><div className="browser-heading"><div><h2>{t(locale, "list")}</h2><span aria-live="polite">{issues.length} {t(locale, "results")}</span></div><input aria-label={t(locale, "filter")} placeholder={t(locale, "search")} type="search" value={query} onChange={event => setQuery(event.target.value)} /><div className="filters"><select aria-label={t(locale, "analysis")} value={analysis} onChange={event => setAnalysis(event.target.value)}><option value="">{t(locale, "allAnalyses")}</option>{analyses.map(value => <option value={value} key={value}>{translatedLabel(locale, value)}</option>)}</select><select aria-label={t(locale, "kind")} value={kind} onChange={event => setKind(event.target.value)}><option value="">{t(locale, "allKinds")}</option><option value="policy">{t(locale, "policy")}</option><option value="advisory">{t(locale, "advisory")}</option></select><select aria-label={t(locale, "maturity")} value={maturity} onChange={event => setMaturity(event.target.value)}><option value="">{t(locale, "allMaturities")}</option>{["stable", "preview", "experimental"].map(value => <option value={value} key={value}>{translatedLabel(locale, value)}</option>)}</select><select aria-label={t(locale, "baselineState")} value={baseline} onChange={event => setBaseline(event.target.value)} disabled={!report.baseline_comparison}><option value="">{report.baseline_comparison ? t(locale, "allBaseline") : t(locale, "baselineUnavailable")}</option>{["new", "updated", "unknown", "unchanged", "absent"].map(value => <option value={value} key={value}>{translatedLabel(locale, value)}</option>)}</select></div></div>
        <div className="issue-list" role="listbox" aria-label={t(locale, "issueList")}>{issues.map((issue, index) => <button id={`issue-option-${index}`} role="option" aria-selected={issue.id === selected?.id} className="issue-option" key={issue.id} onClick={() => choose(issue.id, true)} onKeyDown={event => navigateList(event, index)}><span className="option-meta"><b className={issue.kind}>{translatedLabel(locale, issue.kind)}</b>{issueBaseline(issue) && <i>{translatedLabel(locale, issueBaseline(issue))}</i>}<small>{translatedLabel(locale, issue.analysis)}</small></span><strong>{issue.title}</strong><span>{subjectLabel(issue.subject, locale)}</span><code>{issue.evidence[0]?.rule ?? issue.family}</code></button>)}</div>
        {!issues.length ? <div className="empty"><strong>{report.issues.length ? t(locale, "noMatchingIssues") : t(locale, "noReportedIssues")}</strong>{filtersActive && <button onClick={clearFilters}>{t(locale, "clearFilters")}</button>}</div> : null}
      </aside>
      <section className="detail-pane">{selected ? <IssueDetail issue={selected} maturity={issueMaturity(selected)} baseline={issueBaseline(selected)} locale={locale} headingRef={detailHeading} /> : <div className="empty-detail"><p>{t(locale, "selectIssue")}</p></div>}</section>
    </section>
    <Coverage report={report} analyses={analyses} locale={locale} />
    <footer>{report.target.workspace_identity} · {t(locale, "absence")}</footer>
  </main>;
}

export function App() {
  const parsed = useMemo(parseEmbeddedReport, []);
  const [locale, setLocale] = useState<Locale>(initialLocale);
  useEffect(() => { document.documentElement.lang = locale; document.title = locale === "zh-CN" ? "Reforge 分析报告" : "Reforge analysis report"; }, [locale]);
  const changeLocale = (next: Locale) => { persistLocale(next); setLocale(next); };
  if (parsed.error) return <main className="error"><div className="brand"><span className="brand-mark" /><b>REFORGE</b></div><LanguagePicker locale={locale} onChange={changeLocale} /><h1>{t(locale, "reportUnavailable")}</h1><p>{t(locale, "errorLead")}</p><details><summary>{t(locale, "details")}</summary><p>{parsed.error}</p></details></main>;
  return <ReportView report={parsed.report!} locale={locale} onLocaleChange={changeLocale} />;
}
