# Metrics Model

Reforge separates measurement from interpretation. The scanner first collects
raw file, function, type, and churn metrics, then derives summaries, hotspots,
and findings from that model. The model reports maintainability and
refactoring signals; it is not a quality score, health score, bug detector, or
defect probability model.

## Raw Metrics

File metrics:

- `loc`: total line count.
- `imports`: top-level import/use declarations for supported Tree-sitter
  languages.
- `public_items`: public or exported top-level items.
- `directory_source_files`: number of direct source files in the parent
  directory.
- `is_test`: whether the path looks like a test file.
- `churn`: git churn metrics when enabled.

Function metrics:

- `loc`: function line span.
- `complexity`: estimated cyclomatic complexity.
- `nesting_depth`: maximum nested control-flow depth.
- `parameter_count`: parameter count.
- `is_test`: whether the function belongs to a test file.

Type metrics:

- `loc`: type line span.
- `member_count`: fields, variants, methods, signatures, or equivalent member
  constructs.
- `is_test`: whether the type belongs to a test file.

Churn metrics:

- `commits_touched`
- `lines_added`
- `lines_deleted`
- `authors_count`
- `recent_weighted_churn`

## Percentiles

`metrics_summary` records `p50`, `p75`, `p90`, `p95`, and `max` for each metric
category. Percentiles help rank hotspots relative to the scanned project, not
against a universal standard.

Finding metrics may include a `percentile` value when at least five values are
available for that metric. Percentiles are combined with threshold excess for
raw metrics that have a matching project distribution.

## Finding Priority

`priority` is a refactoring priority score from 0 through 100. It is not a
defect probability, quality grade, or health score.

Priority factors:

- `impact`: how important the detector's signal usually is.
- `intensity`: how far the strongest metric exceeds its threshold or
  normalized baseline.
- `spread`: how broadly related locations cross files.
- `change_pressure`: churn pressure from matching hotspots.
- `actionability`: how directly the signal suggests a refactoring action.
- `confidence`: detector confidence multiplier.

The weighted priority formula is:

```text
((impact * 0.30)
 + (intensity * 0.30)
 + (spread * 0.15)
 + (change_pressure * 0.15)
 + (actionability * 0.10))
* confidence
```

Severity bands:

- `info`: priority 0 through 34.
- `warning`: priority 35 through 69.
- `critical`: priority 70 through 100.

The bands are workflow labels for triage and CI policy. They do not claim that
a file is defective or that a change is safe.

## Constructs, Mechanisms, and Issue Clusters

Each finding declares one ISO/IEC 25010-aligned maintainability `construct`
and one source-observable `mechanism`. These classifications replace the old
metric-dimension label, which mixed measurements, symptoms, and quality
outcomes at different abstraction levels.

Correlated atomic findings remain available for filtering, baselines, and CI,
but `issue_clusters` combine evidence that describes the same entity,
mechanism, and likely action. Human and HTML output present the cluster's
highest-priority finding as the issue and retain member IDs for auditability.
See [Metric Ontology](metric-ontology.md) for definitions and invariants.

## Confidence

Threshold-based structural findings generally use confidence `1.0`. Combined
readability risk uses confidence `0.90` because the measured evidence is
objective, but the readability interpretation is still a review prompt.
Heuristic detectors use lower values when false positives are more likely. For
example, repeated literals can be weaker in tests or report text, and
happy-path-only test risk is intentionally conservative.

## Hotspots

Hotspots rank files, functions, and types independently from findings. They are
retained when `priority >= 35`.

`static_risk` and `churn_risk` are floating-point scores from 0 through 100.
Hotspot `priority` applies the selected model, rounds the result to an integer,
and clamps it to the same 0-100 range.

Static risk is the strongest applicable structural signal for the location,
not a blend of every detector mechanism:

- File risk considers the file-LOC threshold, import threshold at 80% weight,
  public-item threshold at 80%, direct directory file-count threshold at 65%,
  and file-LOC percentile at 35%.
- Function risk considers line and complexity thresholds, nesting at 85%,
  parameter count at 75%, and function-LOC percentile at 35%.
- Type risk considers line and member-count thresholds plus type-LOC
  percentile at 35%.

Threshold-based inputs use the same effective scan thresholds as findings
after configuration and CLI overrides. Reforge takes the maximum weighted
input and clamps it to 0-100.

Churn risk likewise takes the strongest of these project-percentile inputs:

- `commits_touched`
- `recent_weighted_churn`
- `authors_count` at 70% weight

Function and type churn is inherited from file churn only when the scoped item
has `static_risk >= 35`; otherwise its `churn_risk` is zero. File-level churn
pressure is capped for line-level findings unless there is an exact
function/type hotspot match.

Hotspot models:

- `static`: `priority = static_risk`
- `churn`: `priority = churn_risk`
- `hybrid`: `priority = static_risk * 0.65 + churn_risk * 0.35`

Hotspots are a review watchlist. They help identify places where static
maintenance pressure and churn overlap, but they are not findings and should
not be used as a hard CI gate by themselves.

## Interpreting Empty Findings

`findings=0` means no unsuppressed findings remain after scoring, filters, and
suppressions. It does not mean the project has no maintainability risk, no
hotspots, no raw metric outliers, or no bugs. Review `raw_metrics`,
`metrics_summary`, `hotspots`, and suppression summary context before treating
an empty finding list as a clean refactoring backlog.

Suppression summaries are audit context. They should explain how many findings
were intentionally removed and why, so an empty finding list is not confused
with an absence of measured signals.

## Calibration

Calibrate thresholds and priority expectations with multiple real projects,
not a single repository or synthetic fixture set.

1. Pick a representative sample, such as a small library, a service, a
   frontend-heavy project, and a test-heavy project.
2. Run stable reports with the same settings across the sample:

```powershell
cargo run -- scan D:\path\to\project --churn off --hotspot-model static --output json --progress never
```

3. Compare `metrics_summary` percentiles, top findings, and hotspots across
   projects. Look for detectors that are consistently noisy, consistently
   silent, or only useful for one project shape.
4. Review high-priority findings with maintainers who know the codebase. A
   calibrated model should surface plausible refactoring work, not force every
   mature project toward zero findings.
5. Tune thresholds or detector filters only when the same pattern repeats
   across the sample. Keep `priority` as an ordering signal, not an absolute
   quality score.
6. Validate the tuned settings on a holdout project before enabling a blocking
   CI gate. Prefer a baseline gate such as `new-or-worse` so unchanged legacy
   findings remain visible without blocking every change.

## Churn Collection

When enabled, Reforge runs git with `--no-merges`, `--numstat`, and the
configured time window. Binary numstat rows, paths outside the scan root, and
commits above `--churn-max-commit-lines` are ignored.

`--churn auto` falls back gracefully when git history is unavailable.
`--churn on` fails the scan if churn cannot be collected. `--churn off` skips
git entirely.
