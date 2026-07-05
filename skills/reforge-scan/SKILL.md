---
name: reforge-scan
description: Run the Reforge Rust CLI against source repositories to detect refactoring signals, code drift, similar functions, large files, structural complexity, TODO/FIXME clusters, mixed naming styles, and agent-written-code drift. Use when Codex is asked to audit a repository for maintainability issues, identify cleanup priorities, produce a JSON/YAML/human Reforge report, compare refactoring risk across modules, or recommend concrete refactors from Reforge findings.
---

# Reforge Scan

## Workflow

Use Reforge as an evidence-gathering pass before proposing broad cleanup or refactoring work.

1. Locate the target repository. Default to the current working directory unless the user provides a path.
2. Prefer an installed `reforge` binary when available:

```bash
reforge scan <target-repo> --output json --output-file reforge-report.json --progress never
```

3. If Reforge is not installed, run it from the source checkout that contains this skill:

```bash
cargo run --manifest-path <reforge-repo>/Cargo.toml -- scan <target-repo> --output json --output-file reforge-report.json --progress never
```

4. Read the report and group findings by user impact, blast radius, and confidence. Treat high-severity threshold findings and large repeated groups as stronger signals than one-off info findings.
5. Recommend scoped refactors. Prefer local, behavior-preserving cleanup unless the report shows cross-module duplication or drift.
6. If making code changes, run the relevant project tests after edits. Reforge findings are signals, not proof that a refactor is safe.

## Running Reforge

Pass additional scan flags directly after the target path:

```bash
reforge scan . --output json --output-file reforge-report.json --progress never --max-file-lines 600 --function-similarity 0.9
```

For quick human review:

```bash
reforge scan . --output human --progress never
```

For CI or agent-to-agent handoff, prefer JSON or YAML with progress disabled:

```bash
reforge scan . --output yaml --output-file reforge-report.yaml --progress never
```

## Option Guidance

- Keep generated and dependency directories excluded by default. Add `--include-generated` only when the user explicitly wants generated output scanned.
- Keep tests out of similar-function analysis by default. Add `--include-test-similarity` when repeated test setup or test helper extraction is the goal.
- Add `--include-test-structure` when structural issues in tests are in scope.
- Lower `--max-file-lines`, `--max-function-lines`, or `--max-function-complexity` for mature codebases with strict maintainability budgets.
- Raise similarity strictness with `--function-similarity 0.9` when noisy duplication reports would slow the user down.

## Interpreting Findings

Prioritize findings in this order:

1. Critical findings that exceed thresholds by a wide margin.
2. Repeated drift patterns that cross files or modules, especially shadowed abstractions, duplicate data shapes, adapter boundary bypasses, and parallel implementations.
3. Similar functions with enough body tokens to indicate real duplication.
4. Large directories, mixed naming styles, and TODO/FIXME clusters as navigation and ownership signals.
5. Info-level findings as backlog candidates unless they cluster around the same subsystem.

When reporting results, include the command used, output file path if any, top findings, and suggested next actions. Avoid claiming Reforge found bugs; describe findings as maintainability or refactoring signals.
