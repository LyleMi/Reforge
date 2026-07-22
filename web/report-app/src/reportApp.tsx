import React, { useEffect, useMemo, useState } from "react";
import { deriveFiles, deriveInspector, parseEmbeddedReport, toDisplayReport } from "./reportModel";
import { defaultViewState, parseViewState, serializeViewState, type ViewName, type ViewState } from "./viewState";
import { CoverageView, EvidenceView, MapView, MetricsView, OverviewView } from "./views/ViewExports";

function locationHash() {
  return window.location.hash || serializeViewState(defaultViewState);
}

export function App() {
  const parsed = useMemo(parseEmbeddedReport, []);
  if (parsed.error) return <main className="error"><h1>Report unavailable</h1><p>{parsed.error}</p></main>;
  const report = useMemo(() => toDisplayReport(parsed.report!), [parsed.report]);
  const [state, setState] = useState<ViewState>(() => parseViewState(locationHash(), report));
  useEffect(() => {
    const sync = () => setState(parseViewState(locationHash(), report));
    addEventListener("hashchange", sync);
    return () => removeEventListener("hashchange", sync);
  }, [report]);
  const go = (patch: Partial<ViewState>) => {
    const next = { ...state, ...patch };
    history.replaceState(null, "", serializeViewState(next));
    setState(next);
  };
  const files = useMemo(() => deriveFiles(report, state.sort), [report, state.sort]);
  const selected = files.find(file => file.path === state.file);
  const inspector = selected ? deriveInspector(report, selected) : null;
  const openFile = (path: string) => go({ view: "map", file: path });
  const views: [ViewName, string][] = [["overview", "Overview"], ["evidence", "Evidence"], ["map", "Code map"], ["metrics", "Metrics"], ["coverage", "Coverage"]];

  return <main>
    <header>
      <div><span className="eyebrow">Schema 24 evidence report</span><h1>Refactoring evidence</h1><p>Deterministic observations, coverage receipts, provenance, and repository context. No inferred ranking or quality score.</p></div>
      <div className="scan-meta"><b>{report.summary.scanned_files} files</b><span>churn {report.summary.churn.status}</span><span>{report.summary.duration_ms} ms</span></div>
    </header>
    <nav role="tablist">{views.map(([key, name]) => <button role="tab" aria-selected={state.view === key} key={key} onClick={() => go({ view: key, file: key === "map" ? state.file : null })}>{name}</button>)}</nav>
    {state.view === "overview" && <OverviewView report={report} />}
    {state.view === "evidence" && <EvidenceView report={report} state={state} go={go} openFile={openFile} />}
    {state.view === "map" && <MapView files={files} inspector={inspector} state={state} go={go} />}
    {state.view === "metrics" && <MetricsView report={report} files={files} openFile={openFile} />}
    {state.view === "coverage" && <CoverageView report={report} />}
    <footer>Schema {report.schema_version} · {report.suppression_summary.suppressed_count} suppressed · coverage must be read before absence is interpreted.</footer>
  </main>;
}
