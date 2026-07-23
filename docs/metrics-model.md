# Measurements and evidence

Measurements are typed values attached to Evidence. Each records a stable name,
numeric value, optional numeric threshold, and unit. Evidence adds a rule, message, locations,
and an optional typed Dataflow witness.

A measurement is evidence for a detector decision, not a quality score. Reforge
does not combine measurements into grades, normalized health scores, or
cross-rule rankings.

Issues are the baseline, gate, and SARIF decision unit. Evidence explains why an
Issue exists; changing evidence prose, measurements, ordering, or adding
same-family evidence does not change the Issue ID.

The compact report does not contain the raw Codebase metric inventory. Use
`--metrics-output PATH` for detector development or calibration. That sidecar
is deliberately outside the stable report contract.
