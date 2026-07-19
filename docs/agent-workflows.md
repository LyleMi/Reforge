# Agent Workflow Research and Implementation Plan

This document evaluates how Reforge should evolve from a CLI plus one scan
skill into a safe, resumable agent workflow. It is based on the local
`Deep-Research-skills` reference checkout inspected on 2026-07-21 and on the
current Reforge schema 21 implementation. The reference project is an input to
the design, not a runtime dependency.

The sections labeled **Current** describe shipped behavior. Sections labeled
**Proposed** are design decisions and must not be presented as available
features until their exit criteria are met.

## Decision

Reforge should remain a deterministic local evidence engine. Agent skills may
orchestrate scanning, issue investigation, planning, authorized edits, and
verification, but they must not move LLM judgment into the scanner's evidence
model or turn findings into a quality, severity, priority, or readiness score.

The recommended product shape is a small workflow bundle:

1. `reforge-scan` collects and explains evidence.
2. `reforge-plan` investigates user-selected issues and produces a reviewable
   plan without editing source.
3. `reforge-apply` implements an approved plan in conflict-free batches.
4. `reforge-verify` runs project checks, compares finding IDs, and records what
   changed and what remains unknown.

Only the first skill exists today. The other names describe the proposed
capability boundary, not committed commands.

## Research Questions and Method

The review asked four questions:

- Which parts of the reference workflow solve problems Reforge also has?
- Which parts would expand Reforge beyond its stated purpose?
- Which current Reforge report fields can make agent work safer and more
  reproducible?
- What is the smallest implementation path that can be tested before agents
  are allowed to modify code?

The review covered the reference project's five Codex skills, web-researcher
configuration, routing modules, JSON validator, installer, and installer tests.
It also checked Reforge's actual `scan --help`, schema 21 JSON, strict human
output, source model, installer scripts, and self-scan behavior. Runtime output
was treated as authoritative when it disagreed with prose documentation.

The review also found that the React report app accepts schema 21 but still
contains schema 20 ranking, severity, and hotspot concepts in its view state,
types, tests, and presentation. Because those fields are absent from current
reports, the app can display empty watchlists or zero-valued ranking UI. This is
a current compatibility gap, not a proposed workflow feature, and belongs in
Phase 0.

The documentation-drift detector and its fixtures also still require removed
schema 20 terms and CLI flags. Their token-presence checks can pass when a
compatibility note merely says a field was removed, so the default zero-finding
self-scan did not expose the contract mismatch. Phase 0 must update the
detector's expected field/flag set from authoritative schema and Clap metadata,
then add negative tests for removed options. Until then, self-scan is supporting
evidence rather than proof that documentation matches the executable.

## Current Reforge Capability

Reforge currently provides:

- A Rust 2024 CLI with `init`, `config`, and `scan` commands.
- Source discovery plus Tree-sitter structural analysis across the documented
  language set.
- Structural, similarity, dependency, drift, documentation, unused-function,
  test-risk, and Unity-specific detectors.
- Human, single-file HTML, JSON, YAML, and SARIF output.
- Stable atomic evidence IDs (`rf3-...`) and stable issue IDs (`ri3-...`).
- Schema 21 coverage manifests, detector execution receipts, raw metric
  coverage, dependency data, suppressions, issues, and findings.
- `agent_evidence` for file and issue context closure, unresolved local
  dependencies, evidence dispersion, and test reachability.
- Baseline comparison that can display or fail on new finding IDs.
- One installable `reforge-scan` skill and shell, PowerShell, and batch
  installers.

Current implementation debt relevant to agent workflows includes skipped
legacy model/CLI fields that produce dead-code warnings, no installer regression
suite in this repository, a single monolithic skill, no durable workflow
artifact contract, and no conflict-aware orchestration. These are gaps to stage,
not reasons to widen the scanner's core scope.

Schema 21 intentionally does not serialize hotspot ranking, priority,
severity, scoring policy, or reliability scores. Selection is therefore a
review decision based on evidence and repository goals, not a numeric order.

On the 2026-07-21 deterministic self-scan, Reforge scanned 91 source files and
emitted zero unsuppressed issues or findings under repository configuration.
Churn was disabled and four findings were suppressed: three
`similar_functions` and one `repeated_literal`. A strict exploratory scan
emitted 117 issues and 134 atomic findings. This contrast demonstrates why
threshold settings and suppression context must travel with any agent plan.

## What the Reference Workflow Contributes

The reference project implements a three-stage research pipeline through five
skills:

1. Build an item outline and field schema.
2. Optionally add items or fields with human confirmation.
3. Run per-item research agents in batches, validate their JSON, and resume by
   skipping completed outputs.
4. Convert structured results into a report.

Its strongest architectural ideas are independent of web research:

| Reference mechanism | General value | Reforge adaptation |
| --- | --- | --- |
| Separate outline and field definitions | Separates scope from output contract | Separate selected issue IDs from investigation/verification schemas |
| One output file per item | Avoids shared concurrent writes | One investigation result per stable `ri3-...` issue ID |
| Batch size and items per agent | Bounds concurrency and context | Bound by issue count, context-closure size, and overlapping files |
| Resume by completed output | Makes long work interruptible | Resume only validated artifacts whose source report fingerprint matches |
| Dedicated read-only researcher | Keeps broad investigation away from the coordinator | Use a read-only local issue investigator; no web access by default |
| Scenario routing modules | Loads specialized instructions only when relevant | Route by detector family: structure, duplication/drift, dependency, docs, Unity |
| Human checkpoints | Prevents scope from silently expanding | Confirm selection and implementation boundary before source edits |
| Structured validation | Detects incomplete agent output | Validate IDs, paths, evidence coverage, checks, and state transitions |
| Installer regression test | Treats packaging as product behavior | Test fresh install, update, same-source, invalid source, and no-CLI modes |

## What Should Not Be Copied

The reference is optimized for generic internet research. Reforge should not
copy the following behavior:

- Broad live-web search as a default. Refactoring evidence should come from the
  target repository, its build/test tools, and explicitly supplied context.
- A single hard-coded model, home-directory layout, or agent feature toggle.
  Host-specific integration must be optional and versioned.
- Validation that checks field presence only. A syntactically complete plan
  can still cite the wrong report, escape the repository root, overlap another
  write batch, or omit verification.
- Slug-derived filenames. Stable Reforge IDs already provide collision-free
  artifact keys.
- Agents deciding that work is complete merely because they wrote a file.
  Completion requires validation and, for edits, execution of declared checks.
- Generating a new report-conversion program for every run. Reforge should own
  reusable serializers or templates when an artifact becomes part of its
  contract.
- Concurrent source edits by issue count alone. Issues may share files,
  dependencies, public APIs, or tests even when their IDs differ.

## Product Boundary

### CLI responsibilities

The Rust CLI owns facts that can be reproduced without a language model:

- file discovery and parsing;
- raw measurements and detector output;
- stable evidence and issue identity;
- coverage, suppression, dependency, Unity, and test-reachability evidence;
- baseline comparison and machine-readable serialization;
- validation of any future workflow artifact schema.

### Skill responsibilities

Skills own host-level orchestration:

- choosing commands that match user intent;
- explaining coverage limitations;
- asking the user to select scope when several materially different paths are
  possible;
- creating plans from repository inspection;
- applying explicitly authorized changes;
- invoking project-specific formatting, tests, and a follow-up scan.

### Investigator responsibilities

A proposed specialist agent may inspect one or more non-overlapping issues in
read-only mode. It may describe likely ownership boundaries, relevant tests,
unknowns, and candidate refactors. It may not edit source, change thresholds,
add suppressions, mark findings resolved, or use the internet unless the user
explicitly expands scope.

### Explicit non-goals

- Autonomous repository-wide cleanup from all findings.
- A universal maintainability or code-quality score.
- Bug, defect, or refactor-safety prediction.
- Hidden threshold tuning to obtain a clean report.
- Automatic commits, pushes, pull requests, or issue creation.
- Replacing repository tests or maintainer review with a second scan.
- A generic deep-research product embedded in Reforge.

## Proposed Workflow

The workflow should expose visible phase boundaries:

```text
scan -> select -> investigate -> approve -> apply -> verify
  |        |           |            |         |        |
  +--------+-----------+------------+---------+--------+
             validated, resumable run artifacts
```

### 1. Scan

Create schema 21 JSON with `--progress never`. Record the exact command,
effective configuration, schema version, churn status, coverage limitations,
and suppression summary. A run with partial coverage remains usable but must
carry its degraded reasons into every later phase.

### 2. Select

Choose issue IDs using user goals and evidence, not an implicit numeric rank.
Useful selection dimensions include:

- requested subsystem or behavior;
- detector action and issue family;
- metric threshold excess;
- number and dispersion of evidence locations;
- context-closure files and lines;
- unresolved local dependencies;
- direct and reachable tests;
- detector `precision_risk`;
- dirty-worktree overlap and expected blast radius.

Changing detector thresholds or suppressions is a separate configuration
decision and must not be folded into issue selection silently.

### 3. Investigate

Route each selected issue to the smallest relevant instruction module:

- `structure`: file/function/type responsibility and public surface;
- `duplication-drift`: similarity, repeated patterns, data shapes, and concept
  drift;
- `dependency`: cycles, hubs, adapter boundaries, and context closure;
- `documentation`: CLI/schema/document-set drift;
- `unity`: asmdef, asset identity, serialization, scene/prefab, lifecycle, and
  Editor/runtime boundaries.

Each investigator reads the issue, member findings, related locations,
`agent_evidence`, source, tests, and repository instructions. It returns facts,
unknowns, alternatives, affected files, verification commands, and a
recommended smallest change. It does not write source.

### 4. Approve

The coordinator merges compatible investigations into a plan and presents:

- the selected issue IDs and user-visible outcome;
- files expected to change and files inspected only for context;
- behavior-preservation assumptions;
- checks to run;
- unresolved questions and rollback boundary;
- batches that cannot run concurrently because their write or API surfaces
  overlap.

Approval is unnecessary when the user's original request already clearly
authorizes that exact implementation scope. It is required when investigation
reveals a materially broader or different change.

### 5. Apply

Apply the smallest approved batch. Preserve unrelated worktree changes. Do not
alter `reforge.toml`, add suppressions, or widen detector exclusions merely to
make the follow-up scan clean. Parallel implementation is permitted only when
write sets and shared contracts do not overlap.

### 6. Verify

Run the repository's formatter, focused tests, broader tests proportional to
risk, and a follow-up Reforge scan. Compare stable IDs and report four outcomes
separately:

- selected evidence no longer observed;
- selected evidence still observed;
- new evidence observed;
- observation unavailable or degraded.

The workflow must not claim behavior preservation when tests were absent,
skipped, failed, or could not observe the affected path.

## Proposed Artifact Contract

Long-running workflows need durable state, but generated run data should not be
committed by default. Use a project-local ignored workspace such as
`.reforge/runs/<run-id>/` only after the user has initiated a workflow; allow an
explicit external directory for repositories that prohibit local artifacts.

```text
.reforge/runs/<run-id>/
  run.json
  scan.json
  selection.json
  investigations/
    ri3-<hex>.json
  plan.json
  verification.json
```

`run.json` should contain an artifact schema version, target root, source report
fingerprint, Reforge version, scan command, configuration fingerprint, phase,
and timestamps. It must not contain source contents or secrets.

Every investigation should contain:

- `issue_id` and member `finding_ids`;
- source report fingerprint;
- inspected files and relevant locations;
- coverage status and copied degraded reasons;
- facts tied to repository paths;
- unknowns and rejected alternatives;
- proposed read and write sets;
- verification commands and expected observations;
- terminal status: `complete`, `needs_input`, or `failed`.

Artifacts should move through a strict state machine:

```text
created -> scanned -> selected -> investigated -> approved -> applied -> verified
                                      |              |           |
                                      +-> needs_input+-> failed <-+
```

Resume is valid only when the artifact schema is supported, the report
fingerprint matches, every referenced ID exists, paths remain under the target
root, and completed outputs pass validation. Write artifacts atomically by
creating a temporary sibling and renaming it after validation.

## Concurrency Model

The reference project's `batch_size` is necessary but insufficient for source
changes. Reforge should construct a conflict graph where two issues conflict if
they share any of the following:

- evidence or proposed-write files;
- context-closure files that define a shared contract;
- dependency graph nodes on a changed public boundary;
- direct or nearest tests whose fixtures cannot run concurrently;
- Unity assets or asmdef assemblies with shared GUID/reference surfaces.

Read-only investigation can run in parallel across conflict groups because
each agent writes one ID-named artifact. Application batches must be graph
independent sets, and the coordinator remains the sole writer of `run.json`,
`selection.json`, `plan.json`, and `verification.json`.

Start conservatively: one issue per investigator, at most four concurrent
investigators, and sequential application. Measure artifact size and review
quality before increasing either limit.

## Trust and Safety Requirements

- Scan and planning phases are read-only with respect to source.
- Source edits require clear user authorization for the selected outcome.
- Agents must read repository instructions before investigation or editing.
- Dirty worktrees are preserved; unrelated changes are neither reverted nor
  included in the plan.
- Paths are canonicalized and checked against the target root before reads or
  writes.
- No automatic network access, package installation, commits, pushes, or PRs.
- Tool output is evidence, while agent conclusions are labeled analysis.
- Partial coverage, unresolved dependencies, missing tests, and failed checks
  remain visible in the final result.
- A rescan is supporting evidence only; tests and human review remain required.

## Packaging Proposal

The proposed repository layout is:

```text
skills/
  reforge-scan/
  reforge-plan/
  reforge-apply/
  reforge-verify/
agents/
  reforge-investigator.toml
  reforge-modules/
    structure.md
    duplication-drift.md
    dependency.md
    documentation.md
    unity.md
scripts/
  install-agent-workflow.sh
  install-agent-workflow.ps1
  install-agent-workflow.bat
```

Keep agent configuration optional. Skill installation and CLI installation
must remain independently selectable. A manifest should list bundle version,
skills, optional agents, modules, required CLI schema, and target host so an
installer can validate the whole package before replacing an existing install.

The existing `install-agent-skill.*` entry points should remain compatible
during migration. They may delegate to the bundle installer after its behavior
is covered by tests.

## Implementation Roadmap

### Phase 0: repair the current contract

Deliverables:

- Make README, user guide, report schema, architecture, configuration, report
  app, and `reforge-scan` skill agree with actual schema 21 and `scan --help`.
- Remove examples of hidden/removed scoring, severity, hotspot, and CI options.
- Replace token-presence documentation checks with authoritative current-field
  and current-option expectations, including negative tests for removed terms.
- Remove or isolate skipped legacy fields once compatibility tests no longer
  need them, eliminating misleading dead-code warnings.
- Add a documentation contract test that checks documented long options against
  Clap help and a golden schema 21 report.

Exit criteria:

- Documentation contains no command rejected by the current parser.
- JSON field documentation matches a generated golden report.
- Default self-scan remains zero unsuppressed findings with suppression context
  reported.

### Phase 1: read-only scan and plan

Deliverables:

- Add `reforge-plan` and detector-family reference modules.
- Specify `run.json`, `selection.json`, `investigation`, and `plan` schema v1.
- Implement an artifact validator in Rust or a checked-in reusable script.
- Keep execution single-agent and source-read-only.

Exit criteria:

- Invalid IDs, stale fingerprints, root-escaping paths, incomplete coverage
  declarations, and illegal state transitions are rejected.
- A plan can be stopped and resumed without repeating a valid investigation.
- The plan clearly separates facts, analysis, unknowns, and authorization.

### Phase 2: optional parallel investigation and packaging

Deliverables:

- Add the read-only investigator configuration and module routing.
- Partition investigations with the conflict graph.
- Add a bundle manifest and cross-platform installer updates.
- Add installer tests modeled on the reference project: fresh install, update,
  missing source, same source/target, custom destination, and CLI opt-out.

Exit criteria:

- Parallel and sequential investigation produce the same selected IDs and no
  shared artifact writes.
- Interrupted batches resume idempotently.
- Installation never partially replaces a valid bundle.

### Phase 3: opt-in apply and verify

Deliverables:

- Add `reforge-apply` and `reforge-verify` skills.
- Enforce approved write sets and sequential application initially.
- Record formatter/test/rescan commands and outcomes in `verification.json`.

Exit criteria:

- Edits outside the approved root or write set are rejected.
- Failed checks cannot produce a `verified` terminal state.
- Remaining, resolved, new, and unobservable evidence are reported separately.
- No workflow path commits, pushes, or changes suppressions without an explicit
  user request.

### Phase 4: evaluate usefulness before expanding autonomy

Use representative repositories and maintainer review to measure:

- proportion of investigations judged factually grounded;
- proportion of plans accepted without scope correction;
- false resume hits and stale-artifact rejection;
- verification command success and missing-test frequency;
- wall time and context saved by parallel investigation;
- new findings or regressions introduced by applied plans.

These are workflow evaluation measures, not a codebase quality score. Do not
increase concurrency or default autonomy until the previous phase meets an
explicitly documented acceptance threshold.

## Required Test Matrix

| Area | Minimum cases |
| --- | --- |
| CLI/documentation | every documented flag parses; removed flags are absent |
| Report contract | schema 21 zero-finding and nonzero-finding golden reports |
| Coverage | observed, partial, unsupported, and not-applicable states |
| Artifact validation | valid run, stale report, unknown ID, path escape, duplicate output, illegal transition |
| Resume | complete, partial, corrupt, interrupted, and source-report-changed runs |
| Concurrency | disjoint issues, shared file, shared API, shared test, shared Unity asset |
| Worktree safety | clean tree, unrelated dirty files, overlap with user edits |
| Verification | pass, fail, missing command, timeout, degraded rescan, new evidence |
| Installation | fresh, update, force required, same source/target, custom root, no CLI, missing dependency |

## Open Decisions

The following should be resolved during Phase 1 with prototypes rather than
guessed in prompts:

- Whether workflow artifacts belong under `.reforge/runs`, an OS state
  directory, or a user-provided path by default.
- Whether artifact validation should be a new CLI subcommand or a small
  standalone binary/script.
- Which report/config fingerprint inputs are stable across absolute checkout
  paths.
- How much dependency closure is enough before an issue is forced into
  `needs_input` rather than planned automatically.
- Which host agent formats can be supported without editing global user config.
- Whether report-app support for plan and verification artifacts is valuable
  enough to justify a second UI contract.

Until these are settled, the safe next implementation is Phase 0 followed by a
single-agent, read-only Phase 1 prototype.
