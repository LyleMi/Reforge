# Agent Workflows

Reforge ships a deterministic, resumable workflow around schema 23 evidence. The scanner does not assign quality, priority, severity, readiness, or hotspot scores. Selection and refactoring judgment remain review decisions.

## Commands and phases

```text
scanned -> selected -> investigated -> planned -> approved -> applied -> verified
                                      |                     |          |
                                      +-> needs_input       +-> failed
```

```bash
reforge workflow start . --progress never
reforge workflow select .reforge/runs/run-... --issue ri3-... --goal "split the parser boundary"
reforge workflow status .reforge/runs/run-...
reforge workflow validate .reforge/runs/run-...
reforge workflow advance .reforge/runs/run-...
reforge workflow approve .reforge/runs/run-...
reforge workflow mark-applied .reforge/runs/run-...
reforge workflow check .reforge/runs/run-... --kind test -- cargo test
reforge workflow rescan .reforge/runs/run-...
reforge workflow confirm-lineage .reforge/runs/run-... --candidate rl1-...
reforge workflow confirm-lineage .reforge/runs/run-... --remediated ri3-...
reforge workflow finish .reforge/runs/run-...
```

`start` runs a complete schema 23 scan and stores the effective scan command and report, config, and source fingerprints. The default run directory is `.reforge/runs/run-<epoch>-<report-hash>/`; `--run-dir` selects an exact external or project-local directory.

`select` accepts only issue IDs present in `scan.json`. `advance` validates one immutable investigation per selected issue, then validates `plan.json`. `status` and `validate` never mutate artifacts.

`approve` is the only approval entry point. It freezes the canonical plan hash, target-relative write set, and a hash snapshot of the workspace. Invoking the apply skill is not approval. `mark-applied` compares the approved snapshot with the current workspace and rejects every changed file outside the write set.

`check` executes its program directly without a shell. It records arguments,
exit status, duration, timeout state, and a short redacted output summary. It
does not persist full command output. `rescan` reuses the stored effective scan
command, classifies selected evidence, and stores deterministic issue lineage
candidates. `confirm-lineage` writes immutable `lineage.json`: a candidate
creates a `supersedes` record, while `--remediated` records an observably
disappeared selected issue without a successor. Automatic candidates never
change Stable IDs or gate results.

`finish` requires unchanged approval scope, every required check to pass, a required test check, and a completed observable rescan. A failed check produces `failed`. Missing tests or degraded selected evidence produces `needs_input`.

## Artifacts

All workflow JSON uses artifact schema v2, rejects unknown fields, and is written through a temporary sibling followed by rename.

```text
run.json
scan.json
selection.json
investigations/ri3-<id>.json
plan.json
approval.json
application.json
rescan.json
lineage.json (optional)
verification.json
```

Paths in investigation and plan artifacts are target-relative. Existing paths are canonicalized; missing write targets are checked through their nearest existing ancestor. Parent traversal, absolute paths, and symlink escape are rejected.

Investigations separate repository facts, analysis, unknowns, and rejected alternatives. They declare inspected, read, and candidate write sets, coverage limitations, and checks. Plans declare the intended outcome, exact selected IDs, conflict-free batches, write set, behavior assumptions, checks, unresolved risks, and conflict graph.

The conflict graph includes shared evidence or write files, dependency boundaries, reachable tests, and Unity `.meta` or asmdef surfaces. Read-only investigation may use up to four `reforge-investigator` agents. The coordinator is the sole writer of shared workflow artifacts. Apply remains sequential.

## Skills and trust boundary

- `reforge-scan` collects and explains evidence.
- `reforge-plan` investigates selected issues and stops in `planned` without source edits.
- `reforge-apply` is explicit-only and requires an already `approved` run.
- `reforge-verify` records checks, rescan comparison, and the terminal result.

The project agent `.codex/agents/reforge-investigator.toml` is read-only, has no configured network MCP servers, and returns one investigation JSON object to the coordinator. It cannot edit source, configuration, suppressions, or workflow state.

No workflow command installs dependencies, changes thresholds, adds suppressions, commits, pushes, opens a pull request, or modifies project ignore files.

## Installation

```bash
scripts/install-agent-workflow.sh
scripts/install-agent-workflow.sh --skills-only --skip-cli
scripts/install-agent-workflow.sh --agent claude --project-dir /path/to/project --skip-cli
scripts/install-agent-workflow.sh --agent all --project-dir /path/to/project --skip-cli
```

PowerShell and batch equivalents are `install-agent-workflow.ps1` and `install-agent-workflow.bat`. All installers support custom destinations, `--skip-agent`, `--skip-cli`, and atomic `--force` updates. Use `--agent codex|claude|gemini|opencode|codebuddy|cursor|generic|all` to choose the target assistant. Without `--project-dir`, the installer uses that assistant's user root/config directory; with `--project-dir`, it writes project-local instruction files and skill directories where supported. Use `--root-dir` to override the inferred user root/config directory. The older `install-agent-skill.*` entry points install only `reforge-scan` through the compatibility path.

Run any installer with `-h` or `--help` to list its available options without
performing installation or validating destination paths.
