import React from "react";
import type { EvidenceReport } from "./EvidenceTypes";
import type { ViewState } from "../viewState";
import { layerValue, type FileInspector, type FileView } from "../reportModel";
import { Card, label } from "./Common";

type MapProps = { files: FileView[]; inspector: FileInspector | null; state: ViewState; go: (patch: Partial<ViewState>) => void };
export function MapView({ files, inspector, state, go }: MapProps) {
  const maximum = Math.max(1, ...files.map(item => layerValue(item, state.layer)));
  return <section>
    <div className="panel toolbar map-head"><div><h2>Repository evidence map</h2><p>Tile area follows LOC; color intensity follows the selected evidence layer.</p></div>{(["findings", "issues", "churn", "coverage"] as const).map(layer => <button className={state.layer === layer ? "active" : ""} key={layer} onClick={() => go({ layer })}>{label(layer)}</button>)}</div>
    <div className="map" role="group" aria-label={`${state.layer} repository map`}>{files.map(file => { const ratio = layerValue(file, state.layer) / maximum; return <button key={file.path} style={{ flexGrow: Math.max(1, file.loc), background: `hsl(${state.layer === "coverage" ? 175 : 205} 45% ${94 - ratio * 48}%)` }} onClick={() => go({ file: file.path })}><b>{file.path}</b><span>{file.issues} issues · {file.findings} findings · churn {file.churn}</span></button>; })}</div>
    {inspector && <aside aria-label="File inspector"><button className="close" onClick={() => go({ file: null })}>Close</button><h2>{inspector.path}</h2><div className="cards"><Card label="Issues" value={inspector.issues}/><Card label="Findings" value={inspector.findings}/><Card label="LOC" value={inspector.loc}/><Card label="Coverage" value={label(inspector.coverageStatus)}/></div><h3>Metrics</h3><p>{inspector.imports} imports · {inspector.publicItems} public items · churn {inspector.churn}</p><h3>Related locations</h3>{inspector.relatedLocations.map(item => <code key={item}>{item}</code>)}<h3>Dependency closure</h3><p>Incoming: {inspector.incoming.join(", ") || "none observed"}</p><p>Outgoing: {inspector.outgoing.join(", ") || "none observed"}</p><h3>Test reachability</h3><p>{inspector.agent?.reachable_test_file_count ?? 0} reachable tests · nearest distance {inspector.agent?.nearest_test_distance ?? "unavailable"}</p>{inspector.agent?.nearest_test_paths.map(item => <code key={item}>{item}</code>)}<h3>Coverage limitations</h3><p>{inspector.agent?.unresolved_local_dependencies ?? 0} unresolved local dependencies{inspector.agent?.paths_truncated ? " · paths truncated" : ""}</p></aside>}
  </section>;
}

export function MetricsView({ report, files, openFile }: { report: EvidenceReport; files: FileView[]; openFile: (path: string) => void }) {
  return <section className="columns"><section className="panel"><h2>File measurements</h2>{files.map(file => <button className="file-row" key={file.path} onClick={() => openFile(file.path)}><b>{file.path}</b><span>{file.loc} LOC · {file.imports} imports · {file.publicItems} public items · churn {file.churn}</span><small>{file.issues} issues · {file.findings} findings</small></button>)}</section><section className="panel"><h2>Percentiles</h2>{Object.entries(report.metrics_summary).flatMap(([scope, metrics]) => Object.entries(metrics).map(([name, percentiles]) => <article className="compact" key={`${scope}-${name}`}><b>{scope}.{name}</b><span>p50 {percentiles.p50} · p90 {percentiles.p90} · p95 {percentiles.p95} · max {percentiles.max}</span></article>))}</section></section>;
}
