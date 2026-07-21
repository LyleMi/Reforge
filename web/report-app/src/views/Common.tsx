import React from "react";
import type { EvidenceFinding } from "./EvidenceTypes";

export const fmt = (value: number) => new Intl.NumberFormat().format(value);
export const label = (value: string) => {
  const words = value.replace(/_/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
};
export const location = (path: string, line?: number | null) => `${path}${line ? `:${line}` : ""}`;

export function Card({ label: caption, value, meta }: { label: string; value: number | string; meta?: string }) {
  return <article className="card"><span>{caption}</span><strong>{typeof value === "number" ? fmt(value) : value}</strong>{meta && <small>{meta}</small>}</article>;
}

export function FindingRow({ finding, onFile }: { finding: EvidenceFinding; onFile: (path: string) => void }) {
  return <article className="evidence-row">
    <div><b>{label(finding.kind)}</b><button onClick={() => onFile(finding.path)}>{location(finding.path, finding.line)}</button><p>{finding.message}</p><small>{label(finding.mechanism)} · {finding.id}</small></div>
    <details><summary>Evidence</summary>
      {finding.metrics.map(metric => <p key={metric.name}><code>{metric.name}</code> {metric.value} {metric.unit}{metric.threshold != null ? ` · threshold ${metric.threshold}` : ""}</p>)}
      <p>{finding.recommendation}</p>
      {finding.flow_witness && <div className="flow-witness"><p><b>Policy:</b> {finding.flow_witness.policy} · {finding.flow_witness.path_steps} exact steps · {finding.flow_witness.module_hops} module hops</p><ol>{finding.flow_witness.ordered_steps.map((step, index) => <li key={`${step.from}:${step.to}:${index}`}><code>{label(step.kind)}</code> {step.name} <button onClick={() => onFile(step.path)}>{location(step.path, step.line)}</button></li>)}</ol>{finding.flow_witness.conforming_path?.length ? <p>A conforming comparison path was also observed.</p> : null}</div>}
      {finding.related_locations.map(item => <button key={`${item.path}:${item.line}`} onClick={() => onFile(item.path)}>{location(item.path, item.line)}</button>)}
    </details>
  </article>;
}
