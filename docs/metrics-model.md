# Metrics and Evidence Model

Reforge separates observations, detector evidence, issue decision units, and
coverage. Schema 22 does not emit a quality score, priority, severity, hotspot
rank, defect probability, or refactor-readiness score.

## Raw Metrics

Raw metrics are direct observations grouped by entity:

- Directory: direct source-file count.
- File: lines, imports, public/exported items, test classification, and optional
  churn.
- Function: lines, estimated cyclomatic complexity, nesting depth, parameter
  count, and test classification.
- Type: lines, member count, and test classification.
- Churn: commits touched, lines added/deleted, distinct authors, and recency-
  weighted churn.

Every metric definition in `raw_metric_manifest` declares a stable dotted ID,
entity scope, unit, scale, direction, and description. A
`higher_is_more_pressure` direction means larger values may support detector
evidence; it does not define a universal quality threshold. `context_only`
metrics provide interpretation context without independently voting for a
finding.

## Percentiles and Finding Metric Context

`metrics_summary` reports p50, p75, p90, p95, and max for directory, file,
function, type, and churn families. Percentiles are relative to the current
scan, so they help explain how unusual an observation is inside that project;
they are not a cross-project grade.

Finding metrics contain:

- `name`: canonical dotted metric ID.
- `value`: observed value.
- `threshold`: configured detector boundary, when applicable.
- `unit`: metric unit.
- `excess_ratio`: value divided by threshold for threshold findings.
- `normalized`: bounded metric context combining threshold excess and project
  percentile.
- `percentile`: project-relative position when enough observations exist.

The normalized value is interpretation context. It does not order findings or
turn a heuristic into higher-confidence evidence.

## Findings and Issues

A finding is atomic detector evidence with a stable `rf3-...` ID. It records
its detector kind, primary and related locations, metrics, maintainability
construct, observable mechanism, message, and recommendation.

An issue is a stable `ri3-...` decision unit. Compatible atomic findings are
clustered by issue family and canonical subject so correlated evidence can be
reviewed together without discarding detector-level details. An issue records
its refactor action and all member finding IDs; findings remain available for
baseline comparison and detector filtering.

Issue order is deterministic presentation order, not a priority ranking.
Choose work based on repository goals, affected behavior, metric excess,
evidence spread, test reachability, detector precision risk, and maintainer
judgment.

## Constructs, Mechanisms, and Actions

Every detector maps evidence to an ISO/IEC 25010-aligned maintainability
construct:

- modularity;
- reusability;
- analysability;
- modifiability;
- testability.

The mechanism explains the source-observable maintenance pressure, while the
action describes the intended refactoring direction. These classifications
organize evidence; they are not measurements of product quality.

See [Metric Ontology](metric-ontology.md) for the complete mechanism and action
vocabulary and [Detector Reference](detectors.md) for detector mappings.

## Coverage and Execution Receipts

An absent finding is meaningful only when the corresponding analysis could run.
Schema 22 therefore records:

- `coverage_manifest`: expected mechanism/entity-scope cells and their runtime
  status;
- `coverage_summary`: detected languages, analyzed entities, parse failures,
  unresolved dependency edges, and unobservable reasons;
- `detector_execution`: one receipt per detector, including completed
  zero-finding runs;
- `raw_metric_coverage`: whether each canonical raw metric was observed,
  unavailable, unsupported, or not applicable.
- `flow_analysis`: exact/unresolved Rust edge counts, bounded-path truncation,
  and capability-specific support.

Read these before interpreting a quiet report. `partial` or unavailable
coverage means absence of evidence, not evidence of absence.

The `flow.*` IDs are finding-context metrics rather than project-wide raw
metrics: `flow.module_hops`, `flow.call_edges`, `flow.path_steps`,
`flow.unresolved_edges`, `flow.policy_conforming_paths`, and
`flow.policy_bypass_paths`. They explain one exact witness and policy
comparison; they are not combined into a flow score.

## Agent Evidence

`agent_evidence` projects repository context for files and issues:

- context-closure file and line counts;
- unresolved local dependency counts;
- evidence file, directory, and language dispersion;
- direct and reachable tests;
- nearest test distance and representative paths;
- coverage and path-truncation status.

This data helps an agent or maintainer estimate the inspection and verification
surface. It is deliberately not collapsed into a readiness score. A small
closure can still hide runtime coupling, while a reachable test is not proof
that the affected behavior is asserted.

## Churn

`--churn auto` collects git history when available and records a degraded reason
otherwise. `--churn on` requires history; `--churn off` skips it. The window and
maximum commit-size settings determine which history contributes to raw churn
metrics.

Churn is context for maintainers and downstream consumers. Schema 22 does not
combine it with structural observations into a hotspot or finding score.
Disabled or unavailable churn is recorded as unavailable coverage, never as
observed zero change pressure.

## Threshold Selection

Built-in `strict`, `balanced`, and `relaxed` presets are operational starting
points. Project configuration and per-threshold CLI flags can override them.

Lower absolute thresholds generally increase recall and review volume.
Similarity behavior depends jointly on minimum function tokens, group size,
and similarity percentage. Repetition detectors depend on minimum occurrence
counts. Unity thresholds are project-calibrated review budgets rather than
public benchmark limits.

Do not tune a threshold merely to force a zero-finding scan. Review a sample,
record whether the evidence and recommendation are useful, change a parameter
only for a repeatable pattern, and validate it on another representative
repository.

## Interpreting Empty Findings

`findings=0` means no unsuppressed findings remain after detector filters and
suppressions. It does not prove:

- complete language or detector coverage;
- absence of raw metric outliers;
- code quality or refactor safety;
- absence of bugs;
- presence of adequate tests.

Always report coverage limitations, churn status, parse failures, unresolved
dependency context, and nonzero suppressions with an empty finding list.

## Evaluating the Model

Evaluate Reforge on multiple representative projects and keep measurement,
detection, action usefulness, and workflow outcomes separate. Useful review
questions include:

- Did the detector observe the construct it claims to observe?
- Does the issue cluster preserve the relevant atomic evidence?
- Is the recommendation locally actionable?
- Are coverage limitations visible?
- Do stable IDs survive harmless ordering and message changes?
- Did an accepted refactor preserve behavior under the project's tests?

Maintainer labels can calibrate detector precision and recommendation utility,
but they should not be converted into a universal codebase score without a
separate, validated policy and explicit product decision.
