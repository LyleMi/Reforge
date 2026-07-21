import React from "react";
import type { EvidenceReport } from "./EvidenceTypes";
import { label } from "./Common";

export function CoverageView({ report }: { report: EvidenceReport }) {
  return <section className="columns">
    <section className="panel"><h2>Data-flow coverage</h2><article className="compact"><b>Status</b><span className={`status ${report.flow_analysis.status}`}>{label(report.flow_analysis.status)}</span><p>{report.flow_analysis.functions_analyzed} Rust functions · {report.flow_analysis.exact_edges} exact edges · {report.flow_analysis.unresolved_edges} unresolved · {report.flow_analysis.truncated_paths} truncated paths</p></article>{report.flow_analysis.capabilities.map(capability => <article className="compact" key={capability.language}><b>{label(capability.language)}</b><p>Local def-use: {label(capability.local_def_use)} · direct calls: {label(capability.direct_calls)} · fields: {label(capability.fields)} · dispatch: {label(capability.dynamic_dispatch)} · library models: {label(capability.library_models)}</p>{capability.reasons.map(reason => <small key={reason}>{reason}</small>)}</article>)}</section>
    <section className="panel"><h2>Coverage audit</h2>{report.coverage_manifest.map(cell => <article className="compact" key={`${cell.mechanism}-${cell.entity_scope}`}><b>{label(cell.mechanism)} · {label(cell.entity_scope)}</b><span className={`status ${cell.status}`}>{label(cell.status)}</span><p>{cell.reason}</p><small>{cell.completed_detectors.length}/{cell.detectors.length} detectors completed · {cell.entity_count} entities</small></article>)}</section>
    <section className="panel"><h2>Detector receipts</h2>{report.detector_execution.map(item => <article className="compact" key={item.kind}><b>{label(item.kind)}</b><span>{label(item.status)}</span><small>{item.analyzed_entities} entities · {item.candidate_groups} candidates · {item.unobservable_count} unobservable</small></article>)}</section>
  </section>;
}
