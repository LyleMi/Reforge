# Measurements and evidence

Measurements are typed values attached to Evidence. Each records a stable name,
numeric value, optional numeric threshold, and unit. Evidence adds a rule, message, locations,
and an optional typed Dataflow witness.

A measurement is evidence for a detector decision, not a quality score. Reforge
does not combine measurements into grades, normalized health scores, or
cross-rule rankings.

Issues are the baseline, gate, and SARIF decision unit. Evidence explains why an
Issue exists. Prose and ordering do not change identity; measurements,
thresholds, evidence-set changes, and substantive witnesses update the content
fingerprint while the same typed subject keeps its Issue ID.

Coverage records the observed denominator, rule activation and maturity, and
language capability limitations. An unsupported or unresolved semantic surface
is never inferred as an exact edge.

The compact report does not contain the raw Codebase metric inventory. Use
`--metrics-output PATH` for detector development or calibration. That sidecar
is deliberately outside the stable report contract.
