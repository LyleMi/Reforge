import React from "react";
import type { EvidenceReport } from "./EvidenceTypes";
import { label } from "./Common";

export function CoverageView({ report }: { report: EvidenceReport }) {
  return <section className="columns">
    <section className="panel"><h2>Coverage audit</h2>{report.coverage_manifest.map(cell => <article className="compact" key={`${cell.mechanism}-${cell.entity_scope}`}><b>{label(cell.mechanism)} · {label(cell.entity_scope)}</b><span className={`status ${cell.status}`}>{label(cell.status)}</span><p>{cell.reason}</p><small>{cell.completed_detectors.length}/{cell.detectors.length} detectors completed · {cell.entity_count} entities</small></article>)}</section>
    <section className="panel"><h2>Detector receipts</h2>{report.detector_execution.map(item => <article className="compact" key={item.kind}><b>{label(item.kind)}</b><span>{label(item.status)}</span><small>{item.analyzed_entities} entities · {item.candidate_groups} candidates · {item.unobservable_count} unobservable</small></article>)}</section>
  </section>;
}
