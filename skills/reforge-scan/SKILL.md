---
name: reforge-scan
description: Run the Reforge Rust CLI against source repositories to detect refactoring signals, code drift, similar functions, large files, structural complexity, TODO/FIXME clusters, mixed naming styles, documentation drift, churn-backed hotspots, and agent-written-code drift. Use when Codex is asked to audit a repository for maintainability issues, identify cleanup priorities, produce a human, HTML, JSON, YAML, or SARIF Reforge report, compare refactoring risk across modules, tune a reforge.toml scan configuration, configure CI gates or baselines, or recommend concrete refactors from Reforge findings.
---

# Reforge Scan

## Workflow

Use Reforge as an evidence-gathering pass before proposing broad cleanup or refactoring work.

1. Locate the target repository. Default to the current working directory unless the user provides a path.
2. Prefer an installed `reforge` binary when available:

```bash
reforge scan <target-repo> --output json --output-file reforge-report.json --progress never
```

3. If Reforge is not installed and the current working directory or a known checkout is the Reforge source repository, run it from that checkout:

```bash
cargo run --manifest-path <reforge-repo>/Cargo.toml -- scan <target-repo> --output json --output-file reforge-report.json --progress never
```

4. If no installed binary or source checkout is available, ask the user to install the CLI or provide a Reforge checkout path before scanning.
5. Read the report and group issues and findings by user impact, blast radius, detection reliability, and interpretation reliability. Treat high-severity threshold findings and large repeated groups as stronger signals than one-off info findings.
6. Recommend scoped refactors. Prefer local, behavior-preserving cleanup unless the report shows cross-module duplication or drift.
7. If making code changes, run the relevant project tests after edits. Reforge findings are maintainability and refactoring signals, not proof that a refactor is safe, that code is low quality, or that a bug exists.

## Running Reforge

When `--output` is omitted, `--output-file` extensions `.json`, `.yaml`, and `.yml` select machine-readable output automatically:

```bash
reforge scan . --output-file reforge-report.json --progress never
```

Pass additional scan flags directly after the target path:

```bash
reforge scan . --output json --output-file reforge-report.json --progress never --max-file-lines 600 --function-similarity 0.9
```

To tighten structural checks:

```bash
reforge scan . --progress never --max-function-lines 60 --max-function-complexity 10 --max-nesting-depth 3
```

To use built-in threshold defaults before per-threshold overrides:

```bash
reforge scan . --preset strict --progress never
```

For quick human review:

```bash
reforge scan . --output human --progress never
```

For a static visual report:

```bash
reforge scan . --output html --output-file reforge-report.html --progress never
```

For CI code-scanning integrations:

```bash
reforge scan . --output sarif --output-file reforge-report.sarif --progress never
```

To remove test files and test directories from the scan entirely:

```bash
reforge scan . --exclude-tests --progress never
```

For CI or agent-to-agent handoff, prefer JSON or YAML with progress disabled:

```bash
reforge scan . --output yaml --output-file reforge-report.yaml --progress never
```

For deterministic static-only output, disable git churn and use the static hotspot model:

```bash
reforge scan . --churn off --hotspot-model static --output json --progress never
```

For a Unity project, keep automatic root detection for normal audits:

```bash
reforge scan <unity-project-root> --unity auto --output json --progress never
```

Use `--unity on` when the supplied path must be a Unity project and failure to
recognize it should stop the scan. Use `--unity off` when Unity assets are
intentionally outside the audit.

To fail CI on current warning or critical findings:

```bash
reforge scan . --output json --progress never --fail-on warning
```

To compare against a prior baseline and gate only new or worse findings:

```bash
reforge scan . --baseline baseline.json --baseline-mode new-or-worse --fail-on warning --output json --progress never
```

## Option Guidance

- Keep `--churn auto` and `--hotspot-model hybrid` for normal repository audits; use `--churn on` only when missing git history should fail the scan.
- Use `--churn off --hotspot-model static` for reproducible CI snapshots or when comparing output across machines.
- Keep `--unity auto` for normal scans. Use `--unity on` to require a recognizable Unity root and `--unity off` to exclude Unity project analysis deliberately.
- Treat `--max-unity-assembly-dependencies`, `--max-unity-scene-objects`, `--max-unity-prefab-objects`, `--max-unity-serialized-fields`, and `--max-unity-lifecycle-methods` as project-calibrated review budgets. The built-in values are operational heuristics, not public Unity benchmark limits.
- Use `--config <path>` when the repository has a `reforge.toml`, or rely on default discovery from the scan root upward.
- Use `reforge init`, `reforge config validate`, and `reforge config show` when the task is to create, check, or inspect configuration without scanning source files.
- Use `--preset strict`, `--preset balanced`, or `--preset relaxed` to start from built-in threshold sets. Threshold precedence is CLI per-threshold flags, CLI `--preset`, `reforge.toml` per-threshold values, `reforge.toml` `preset`, then built-in `balanced`.
- Keep generated and dependency directories excluded by default. Add `--include-generated` only when the user explicitly wants generated output scanned.
- Keep hidden files excluded by default. Add `--include-hidden` only when dotfiles or hidden source trees are in scope.
- Keep tests scanned by default. Add `--exclude-tests` when the user wants production-source-only results or when test fixture volume would drown out application signals.
- Keep tests out of similar-function analysis by default. Add `--include-test-similarity` when repeated test setup or test helper extraction is the goal.
- Keep tests out of general structural analysis by default. Add `--include-test-structure` when structural issues in tests are in scope.
- Use `--baseline` with `--baseline-mode new-or-worse` and `--fail-on warning` for pull request gates that should not fail on unchanged legacy findings.
- Treat hotspots as a watchlist, not a hard CI gate. `--fail-on` evaluates selected findings, not raw metrics or hotspot entries by themselves.
- Lower `--max-file-lines`, `--max-function-lines`, or `--max-function-complexity` for mature codebases with strict maintainability budgets.
- Raise similarity strictness with `--function-similarity 0.9` when noisy duplication reports would slow the user down.
- Tune `--min-repeated-literal-occurrences` and `--min-data-clump-occurrences` when repeated literals, repeated error handling, data clumps, or test setup duplication are too noisy or too sparse.

## Choosing Parameters

Start with `balanced` unless the repository already has an agreed maintenance
budget. Use `strict` when maintainers accept more review prompts to surface
smaller or earlier signals. Use `relaxed` for an initial rollout or when
framework-heavy and orchestration code creates known benign outliers.

Do not tune a threshold merely to make a report reach zero findings. Review a
sample of the affected findings, record whether the evidence is correct and
the recommendation is useful, change parameters only for a repeated pattern,
and validate the change on another representative repository.

Interpret parameter families by the tradeoff they control:

- File, directory, function, type, complexity, nesting, import, and public-item limits are absolute review budgets. Lower values increase recall and review volume; higher values reserve findings for stronger outliers.
- Similarity depends jointly on minimum body tokens, minimum group size, and similarity percentage. Lower token or group minima increase sensitivity; a higher similarity percentage requires closer matches and reduces noise. Evaluate the three together.
- Repetition minima control evidence strength. Lower occurrence counts find local repetition but are more vulnerable to fixtures, protocol constants, and framework conventions.
- Churn window and maximum commit size are operational filters. Short windows emphasize current activity; long windows help low-activity repositories. The commit limit filters bulk mechanical changes that could dominate percentiles.
- Unity thresholds represent assembly fan-out, serialized asset size, component state breadth, and lifecycle responsibility. Calibrate them on representative project assets before making them CI-blocking.

ISO/IEC 25010 informs Reforge's maintainability classification, not its numeric
thresholds. Treat built-in thresholds and ranking weights as documented policy
priors unless an accepted scoring policy and representative calibration data
show otherwise.

## Self-Debugging Reforge

When working inside the Reforge source repository, use Reforge against itself after code or documentation changes:

```bash
cargo test
cargo run -- scan . --progress never
```

If the human scan output reports warnings introduced by the current change, address the smallest local cause first. Common self-debug fixes include splitting large option structs with Clap `flatten`, moving broad test fixtures into narrower modules, documenting new CLI flags in `README.md` and `docs/`, and using `--exclude-tests` only when the user explicitly wants to ignore test-maintenance signals.

For a stable self-debug artifact:

```bash
cargo run -- scan . --output json --output-file reforge-self-report.json --progress never --churn off --hotspot-model static
```

## Interpreting Findings

Current schema 20 reports separate measurement, coverage, atomic evidence, and
decision units. Read them in this order:

1. Check `summary`, `coverage_summary`, `detector_execution`, and
   `suppression_summary` so missing observation or suppressed evidence is not
   mistaken for a clean scan.
2. Use `issues` as the primary decision list. Each issue selects a primary
   finding and retains related evidence without adding correlated signals more
   than once.
3. Inspect `findings` for atomic evidence, metrics, related locations, stable
   IDs, reliability, and recommendations.
4. Use `hotspots` as a separate watchlist and `raw_metrics` plus
   `metrics_summary` for measurement context.
5. For Unity scans, inspect `unity_project.coverage` and `degraded_reasons`
   before interpreting absent asset or reference findings. A
   `partially_observed` project is incomplete evidence, not evidence of absence.

JSON and YAML also expose `coverage_manifest`, `raw_metric_coverage`,
`scoring_policy`, and `detector_manifest` for auditing what could be observed
and how ranking was produced. Human output sorts issues and findings by
descending `priority`. `priority` is refactoring priority, not defect
probability, a quality grade, or a health score.

Within the issue and finding lists, prioritize evidence in this order:

1. Critical findings that exceed thresholds by a wide margin.
2. Repeated drift patterns that cross files or modules, especially shadowed abstractions, duplicate data shapes, adapter boundary bypasses, and parallel implementations.
3. Similar functions with enough body tokens to indicate real duplication.
4. Repeated literals, repeated error patterns, data clumps, and test setup duplication when they cluster around the same subsystem.
5. Large directories, directory drift, mixed naming styles, and TODO/FIXME clusters as navigation and ownership signals.
6. Info-level findings as backlog candidates unless they cluster around the same subsystem.

Use hotspots after findings as watchlist context, especially when static risk and churn risk point to the same file, function, or type. Do not report hotspots as CI failures unless a separate project policy explicitly says so.

`findings=0` means no unsuppressed atomic findings remain after scoring,
filtering, and suppressions. It does not prove code quality, complete coverage,
or absence of bugs. If suppressions are used, include suppression summary
context so reviewers can see that the result means zero unsuppressed findings.

Use `priority`, `detection_reliability`, `interpretation_reliability`,
`priority_factors`, `rank_explanation`, and related locations when explaining
why a finding matters. Avoid claiming Reforge found bugs; describe findings as
maintainability or refactoring signals.

When reporting results, include the command used, output file path if any,
churn status, hotspot model, coverage or degraded-reason context, suppression
summary when nonzero, top issues and supporting findings, and suggested next
actions.
