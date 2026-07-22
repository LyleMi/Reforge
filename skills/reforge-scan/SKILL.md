---
name: reforge-scan
description: Run the Reforge Rust CLI against source repositories to collect maintainability and refactoring evidence, including structural thresholds, similarity, dependency and concept drift, documentation drift, Unity project signals, coverage receipts, and agent context/test-reachability evidence. Use when Codex is asked to audit a repository, explain Reforge findings, produce human/HTML/JSON/YAML/SARIF output, tune reforge.toml, or configure schema 23 baseline gates.
---

# Reforge Scan

## Workflow

Use Reforge as an evidence-gathering pass before proposing broad cleanup or
refactoring work.

1. Locate the target repository. Default to the current working directory
   unless the user provides a path.
2. Prefer an installed `reforge` binary:

```bash
reforge scan <target-repo> --output json --output-file reforge-report.json --progress never
```

3. If Reforge is not installed and a source checkout is available, run it from
   that checkout:

```bash
cargo run --manifest-path <reforge-repo>/Cargo.toml -- scan <target-repo> --output json --output-file reforge-report.json --progress never
```

4. If neither is available, ask the user to install the CLI or provide a
   checkout path.
5. Check coverage and suppression context before interpreting an empty finding
   list. Use `issues` for decision units and `findings` for atomic evidence.
6. Scope recommendations with `agent_evidence`: context closure, unresolved
   local dependencies, evidence dispersion, and reachable tests describe the
   likely inspection and verification surface.
7. If the user asks for code changes, inspect the relevant code before editing
   and run the repository's tests plus a follow-up Reforge scan. A finding is a
   refactoring prompt, not proof of a bug or proof that a refactor is safe.

## Running Reforge

When `--output` is omitted, a recognized `--output-file` extension selects the
format:

```bash
reforge scan . --output-file reforge-report.json --progress never
```

Pass thresholds directly after the target path:

```bash
reforge scan . --output json --output-file reforge-report.json --progress never --max-file-lines 600 --function-similarity 0.9
```

Use a built-in threshold preset:

```bash
reforge scan . --preset strict --progress never
```

Produce quick human or static HTML output:

```bash
reforge scan . --output human --progress never
reforge scan . --output html --output-file reforge-report.html --progress never
```

Produce automation formats:

```bash
reforge scan . --output json --progress never
reforge scan . --output yaml --output-file reforge-report.yaml --progress never
reforge scan . --output sarif --output-file reforge-report.sarif --progress never
```

Disable git history for a reproducible source-only observation pass:

```bash
reforge scan . --churn off --reproducible --output json --progress never
```

For Unity projects, keep automatic root detection for normal audits:

```bash
reforge scan <unity-project-root> --unity auto --output json --progress never
```

Use `--unity on` when the path must be recognized as a Unity project and
`--unity off` when Unity analysis is deliberately out of scope.

Remove tests from the scan only when the requested scope requires it:

```bash
reforge scan . --exclude-tests --progress never
```

## Baseline Gates

Schema 23 gates compare stable finding IDs. A blocking gate requires a current
schema 23 JSON or YAML baseline:

```bash
reforge scan . --baseline baseline.json --baseline-mode new --fail-on-findings --output json --progress never
```

`--baseline-mode new` selects IDs absent from the baseline. Use
`--baseline-mode all` only when every current unsuppressed finding should be
selected. For human review, `--show new` limits the displayed finding list:

```bash
reforge scan . --baseline baseline.json --show new --output human --progress never
```

Schema 23 does not serialize priority, severity, a readiness score, or hotspot
ranking. Do not invent those values and do not use removed options such as
`--fail-on`, `--severity`, `--min-priority`, or `--hotspot-model`.

## Option Guidance

- Keep `--churn auto` for normal repository audits. Use `--churn on` only when
  missing git history should fail the scan, and `--churn off --reproducible`
  for byte-stable source-only comparisons.
- Use `--config <path>` for an explicit configuration, or rely on discovery of
  `reforge.toml` from the scan root upward.
- Use `reforge init`, `reforge config validate`, and `reforge config show` to
  create or inspect configuration without scanning.
- Start with `balanced` unless the repository has an agreed maintenance
  budget. `strict` increases review volume; `relaxed` reserves output for
  stronger threshold excesses.
- Keep generated, dependency, hidden, and git-ignored paths excluded unless
  the user explicitly expands scope.
- Keep tests scanned by default. Test files remain outside similar-function and
  general-structure analysis unless `--include-test-similarity` or
  `--include-test-structure` is supplied.
- Treat Unity thresholds as project-calibrated review budgets, not public Unity
  benchmark limits.
- Tune thresholds only after reviewing a representative sample. Do not change
  thresholds merely to force `findings=0`.

## Reading Schema 23

Read a JSON or YAML report in this order:

1. `summary`, `coverage_summary`, `coverage_manifest`,
   `detector_execution`, and `raw_metric_coverage`: establish what was scanned,
   what was observable, and which detectors completed.
2. `suppression_summary`: distinguish zero unsuppressed findings from zero
   observed signals.
3. `issues`: use stable `ri3-...` decision units, their refactor action,
   canonical subject, and member finding IDs to organize review.
4. `findings`: inspect stable `rf3-...` atomic evidence, metrics, messages,
   recommendations, and related locations.
5. `agent_evidence`: estimate inspection and verification scope from evidence
   dispersion, context closure size, unresolved local dependencies, and test
   reachability. `partial` coverage is a warning that the closure is incomplete.
6. `raw_metrics`, `metrics_summary`, `dependency_graph`, and `unity_project`:
   use measurement and project-specific context to validate the proposed work.

Choose what to address using user impact, affected scope, threshold excess,
cross-file evidence, test reachability, detector precision risk, and repository
constraints. Schema 23 intentionally does not collapse those judgments into a
single numeric rank.

`findings=0` means no unsuppressed findings remain after detector filters and
suppressions. It does not prove complete coverage, code quality, safety, or the
absence of bugs. When suppressions are nonzero, always report their count and
kind breakdown.

For Unity scans, inspect `unity_project.coverage` and `degraded_reasons` before
interpreting absent asset or reference findings. `partially_observed` is
incomplete evidence, not evidence of absence.

## Self-Debugging Reforge

When working inside the Reforge source repository:

```bash
cargo test
cargo run -- scan . --progress never
```

For a stable self-scan artifact:

```bash
cargo run -- scan . --output json --output-file reforge-self-report.json --progress never --churn off --reproducible
```

Report the command, artifact path, schema version, churn state, coverage or
degraded-reason context, suppression summary when nonzero, selected issues and
supporting findings, and suggested verification steps.
