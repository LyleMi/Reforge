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
size, complexity, and coupling dimensions.

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

## Metric Dimensions

Metrics are assigned one of these dimensions:

- `size`: file, directory, function, type, and unused-function signals.
- `complexity`: complexity, nesting, parameter, and combined readability-risk
  signals.
- `coupling`: imports, public surfaces, and adapter bypass signals.
  Dependency graph findings also use this dimension for direct fan-in/fan-out,
  transitive reach, dependency depth, cycle edges, and cycle density.
- `duplication`: similar functions, repeated literals, repeated error
  patterns, data clumps, and duplicate type shapes.
- `drift`: naming, directory, parallel implementation, abstraction, config,
  fixture, generic bucket, adapter boundary, and compatibility-path drift.
- `test_risk`: repeated setup and happy-path-only test risk.
- `documentation`: missing or stale documentation.

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

Static risk uses threshold excess and percentile risk for size, complexity,
coupling, duplication, drift, and test-risk signals. Threshold-based static
risk uses the same effective scan thresholds that findings use after applying
configuration and CLI overrides.

Churn risk uses project percentiles for:

- `commits_touched`
- `recent_weighted_churn`
- `authors_count`

Function and type churn is inherited from file churn only when the scoped item
already has meaningful static risk. File-level churn pressure is capped for
line-level findings unless there is an exact function/type hotspot match.

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
