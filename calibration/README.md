# Calibration protocol

Calibration is offline development infrastructure. The release binary neither
contains nor calls an LLM API. `reforge-calibrate packet` turns normalized site
records into anonymous review packets. Two isolated automated reviewers label
the packets independently; `validate` rejects incomplete labels and missing
reviewer provenance. An optional third label file records adjudication.
`summarize` computes Wilson 95% lower bounds, raw agreement, Cohen's kappa,
repository concentration, fixture recall, and unsupported-case coverage.

Review packets intentionally omit rule maturity, promotion targets, thresholds,
and the other reviewer's labels. Label provenance includes reviewer type, model,
version, prompt digest, timestamp, and—when present—a separate adjudicator.
Labels record instrumentation correctness, detection-claim correctness,
usefulness for inspection, legitimate exceptions, suggested-action suitability,
clustering correctness, and coverage honesty independently.

Promotion is evaluated per rule and language. A stable advisory requires at
least 40 candidate sites from five repositories (no repository over 25%), 20
quiet/negative sites from three repositories, detection correctness Wilson
lower bound at least 0.90, usefulness lower bound at least 0.80, raw agreement
at least 0.80, Cohen's kappa at least 0.60, fixture recall at least 0.90, and
100% honest unsupported coverage. Insufficient samples remain preview/off.
Automated calibration can never make a rule a policy; policy requires the
target repository to list a stable rule under `rules.enforce`.

The frozen corpus is declared in `corpus.toml`. Source trees and complete
reports are not committed. CI retains complete reports as calibration
artifacts; this directory contains only manifests, anonymous site anchors,
labels, normalized summaries, and report hashes.

`reforge-calibrate corpus validate` rejects a wrong manifest version, any
schema other than 27, duplicate repositories, non-40-character commits,
missing license data, and loss of any of the five frozen language strata.
`corpus matrix` emits only typed repository/language/commit/license fields; it
never executes the informational command strings in the manifest.

`verify-reports` consumes the three analyses and their repeated copies plus
repeated metrics and Flow IR sidecars. It verifies byte-identical repetition,
Codebase/Dataflow isolation, combined union, matching coverage,
workspace/revision identity, and sidecar JSON, then writes an audit manifest
with SHA-256 digests. `summarize` requires the current corpus and report-audit
digests. `verify-promotion` requires a complete set of corpus audits in release
CI and rejects stable/default rules unless an eligible per-rule/per-language
summary is bound to those exact digests.

The reusable GitHub Actions workflow performs report production and audit
verification only. Review packets, two isolated label files, and optional
adjudication remain inputs from the external audit process; CI does not invent
or substitute them.
