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
5. Read the report and group findings by user impact, blast radius, and confidence. Treat high-severity threshold findings and large repeated groups as stronger signals than one-off info findings.
6. Recommend scoped refactors. Prefer local, behavior-preserving cleanup unless the report shows cross-module duplication or drift.
7. If making code changes, run the relevant project tests after edits. Reforge findings are signals, not proof that a refactor is safe.

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
- Use `--config <path>` when the repository has a `reforge.toml`, or rely on default discovery from the scan root upward.
- Use `reforge init`, `reforge config validate`, and `reforge config show` when the task is to create, check, or inspect configuration without scanning source files.
- Keep generated and dependency directories excluded by default. Add `--include-generated` only when the user explicitly wants generated output scanned.
- Keep hidden files excluded by default. Add `--include-hidden` only when dotfiles or hidden source trees are in scope.
- Keep tests scanned by default. Add `--exclude-tests` when the user wants production-source-only results or when test fixture volume would drown out application signals.
- Keep tests out of similar-function analysis by default. Add `--include-test-similarity` when repeated test setup or test helper extraction is the goal.
- Keep tests out of general structural analysis by default. Add `--include-test-structure` when structural issues in tests are in scope.
- Use `--baseline` with `--baseline-mode new-or-worse` and `--fail-on warning` for pull request gates that should not fail on unchanged legacy findings.
- Lower `--max-file-lines`, `--max-function-lines`, or `--max-function-complexity` for mature codebases with strict maintainability budgets.
- Raise similarity strictness with `--function-similarity 0.9` when noisy duplication reports would slow the user down.
- Tune `--min-repeated-literal-occurrences` and `--min-data-clump-occurrences` when repeated literals, repeated error handling, data clumps, or test setup duplication are too noisy or too sparse.

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

Reports contain `raw_metrics`, `metrics_summary`, `hotspots`, and `findings`. Human output sorts findings by descending `priority`; JSON and YAML expose `priority`, `confidence`, `priority_factors`, and `rank_explanation` for downstream ranking.

Prioritize findings in this order:

1. Critical findings that exceed thresholds by a wide margin.
2. High-priority hotspots, especially when static risk and churn risk point to the same file, function, or type.
3. Structural hotspots such as long functions, high complexity, deep nesting, many parameters, large types, import-heavy files, and large public surfaces.
4. Repeated drift patterns that cross files or modules, especially shadowed abstractions, duplicate data shapes, adapter boundary bypasses, and parallel implementations.
5. Similar functions with enough body tokens to indicate real duplication.
6. Repeated literals, repeated error patterns, data clumps, and test setup duplication when they cluster around the same subsystem.
7. Large directories, directory drift, mixed naming styles, and TODO/FIXME clusters as navigation and ownership signals.
8. Info-level findings as backlog candidates unless they cluster around the same subsystem.

Use `priority`, `confidence`, `priority_factors`, `rank_explanation`, and related locations when explaining why a finding matters. Avoid claiming Reforge found bugs; describe findings as maintainability or refactoring signals.

When reporting results, include the command used, output file path if any, churn status, hotspot model, top findings, and suggested next actions.
