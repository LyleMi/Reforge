# Data-flow Evidence Execution Plan

> **Status:** Implemented decision record. Normative behavior now lives in the
> architecture, detector, configuration, metrics, and schema documentation.
> This file is retained only to audit the research and rollout rationale.
>
> **Decision:** Conditional go for a bounded, exact-edge Rust prototype. No-go
> for a generic `abnormal_data_flow` detector or whole-program taint engine.
>
> **Last reviewed:** 2026-07-22.

The supporting evidence and candidate-signal review are in the
[research report](report.md). Structured research inputs and results remain in
this directory so the conclusions can be audited and regenerated.

## Outcome

Build a lightweight, explainable data-flow evidence layer that can prove a
small set of project-local value transfers and state exactly where analysis is
partial or unsupported. Use the layer first to validate a policy-declared
adapter-flow bypass. Do not infer general architectural intent, assign an
abnormality score, or prescribe an automatic refactoring.

The first public behavior is acceptable only when it can answer all of these
questions from one report:

1. Which source value was observed?
2. Which exact assignments, arguments, returns, calls, and module crossings
   form the witness?
3. Which configured boundary should have owned the crossing?
4. Which edges were exact, unresolved, truncated, or unsupported?
5. Which concrete, behavior-preserving refactoring seam should a maintainer
   inspect?

## Product boundary

### In scope

- Rust lexical definitions and uses for parameters and local variables.
- Direct, project-local free-function calls with explicit
  `crate`, `self`, `super`, or module qualification.
- Argument-to-parameter, assignment, and return-to-result flow.
- Bounded interprocedural summary composition, initially four call edges.
- Ordered witness paths and capability-specific coverage receipts.
- Explicit, repository-owned adapter policies.
- An opt-in `adapter_flow_bypass` finding only after instrumentation and corpus
  gates pass.
- Existing filters, suppressions, baselines, issue clustering, human output,
  JSON/YAML, SARIF, and HTML report behavior once the feature becomes public.

### Out of scope for the first implementation

- A finding named `abnormal_data_flow` or any universal path-length rule.
- Whole-program security taint analysis or vulnerability claims.
- Heap, alias, pointer, field-sensitive, or points-to analysis.
- Rust methods, trait/interface dispatch, macro expansion, escaping closures,
  async task causality, unsafe pointer flow, or external crate models.
- Dynamic-language interprocedural flow.
- Queues, persistence, network hops, service-to-service flow, or runtime
  middleware inference.
- Automatic Move Method, DTO consolidation, middle-layer deletion, or source
  edits.
- Priority, severity, readiness, defect probability, or aggregate flow scores.

## Design principles

- **Witness before interpretation.** Record exact flow facts before deriving a
  maintainability finding.
- **Intent must be declared.** A path becomes a boundary violation only when
  project configuration names the protected paths and allowed adapter.
- **Absence requires coverage.** No finding means only that no in-scope exact
  witness was observed.
- **No speculative edge in a conservative finding.** Unresolved or ambiguous
  edges may appear in coverage, never in a conservative witness.
- **Metrics explain; they do not vote.** Hop count, module span, and path shape
  remain finding context rather than an aggregate score.
- **Language support is capability-based.** Local def-use, direct calls,
  fields, dispatch, library models, and generated-code visibility are reported
  separately.
- **Existing evidence remains atomic.** `adapter_boundary_bypass`, data clumps,
  dependency hubs, and the new flow evidence stay independently filterable and
  may be clustered only through explicit detector relations.

## Proposed internal architecture

Keep the prototype inside one detector-owned subtree until a second consumer
justifies a shared top-level analysis package:

```text
src/detectors/data_flow/
  mod.rs                 orchestration and feature gate
  model.rs               nodes, edges, summaries, paths, capability status
  local.rs               lexical scopes and intraprocedural def-use
  compose.rs             bounded summary composition and SCC handling
  coverage.rs            capability and unresolved-reason accounting
  policy.rs              parsed effective boundary policy
  rust/
    mod.rs               Rust syntax classification
    symbols.rs           module/free-function resolution
    summaries.rs         parameter, argument, assignment, and return summaries
  tests.rs               focused fixtures and contract tests
```

Do not expose this layout as a library API. Revisit ownership after the Rust
prototype and before adding a second language.

### Core model

The prototype should use deterministic IDs and ordered edges:

```text
FlowNode
  id
  kind: parameter | local | argument | return | call_result
  path, line, function, module

FlowEdge
  from, to
  kind: assignment | argument_to_parameter | return_to_result
  resolution: exact | unresolved | unsupported
  path, line

FunctionFlowSummary
  function_id
  parameter_to_call_arguments
  parameter_to_return
  return_sources
  unresolved_reasons

FlowWitness
  source
  ordered_steps
  sink
  module_hops
  truncated
```

Collections must use deterministic ordering. IDs must not depend on traversal
order, rendered messages, metric values, or allocation addresses.

### Rust resolution subset

Resolve only when one project-local target is provable:

- free-function definitions in parsed Rust modules;
- `crate::`, `self::`, `super::`, and explicit module-qualified calls;
- unambiguous imports and re-exports added deliberately to the resolver;
- positional argument-to-parameter binding;
- direct returns and lexical aliases.

Record, but do not guess, method calls, associated functions with ambiguous
types, trait calls, macro calls, function values, closures, external crates,
and unresolved imports. A later milestone may add uniquely resolved associated
functions as a separate capability.

### Composition rules

- Default maximum interprocedural depth: four call edges.
- Condense recursive call components before calculating depth.
- Compose summaries, not raw AST paths.
- Keep at most one shortest deterministic exact witness per equivalent
  source/sink/policy tuple for the MVP.
- Report truncation and candidate counts so bounded search is visible.
- Never enumerate all paths in a cyclic graph.

## Draft policy configuration

Configuration remains absent from generated `reforge.toml` until the Rust graph
passes Phase 2. The provisional shape follows existing kebab-case config rules:

```toml
[data-flow]
mode = "off" # off | observe | policy
max-hops = 4

[[data-flow.boundaries]]
name = "http-client"
protected-paths = ["src/domain", "src/application"]
adapter-paths = ["src/adapters/http"]
sink-symbols = ["crate::transport::send"]
exempt-paths = ["src/bin", "src/migrations"]
```

Semantics:

- `off` is the default and performs no flow work.
- `observe` emits internal benchmark/debug artifacts during development. It
  must not add findings, baseline entries, or CI failures.
- `policy` evaluates only configured boundaries and remains opt-in until its
  promotion gate passes.
- Unknown keys, empty names, invalid globs, duplicate names, invalid modes, a
  zero hop limit, or policies without protected, adapter, and sink definitions
  fail configuration validation.
- CLI values, if added, override configuration consistently with existing
  precedence rules. CI-only switches remain CLI-only.

The exact symbol syntax and whether `observe` becomes a supported public mode
are Phase 2 decisions, not assumptions to bake into Phase 1.

## Public evidence contract

### Before schema publication

Phases 0 through 2 keep graph structures internal and expose them only through
tests and retained calibration artifacts. Do not add a `FindingKind`,
`MetricId`, report field, CLI flag, or config key during the instrumentation
spike.

### Proposed schema 22 shape

If Phase 2 passes, make the public change intentionally and bump the report
schema. Do not serialize the full graph. Add a compact flow-analysis summary:

```text
flow_analysis
  status
  functions_analyzed
  exact_edges
  unresolved_edges
  truncated_paths
  capabilities[]
    language
    local_def_use
    direct_calls
    fields
    dynamic_dispatch
    library_models
    status and reasons
```

The proposed finding is `adapter_flow_bypass`, separate from the existing
heuristic `adapter_boundary_bypass`. This avoids changing existing finding IDs
or semantics. Both may share the dependency-coupling issue family and related
refactor action, but they remain independently selectable.

Candidate finding metrics:

- `flow.module_hops` — exact module transitions in the witness;
- `flow.call_edges` — exact interprocedural call edges;
- `flow.path_steps` — all ordered witness steps;
- `flow.unresolved_edges` — must be zero for the finding;
- `flow.policy_conforming_paths` — comparable exact paths through the adapter;
- `flow.policy_bypass_paths` — grouped exact paths avoiding it.

Use ordered related locations with step names for the first renderer. Before
schema publication, decide whether a typed optional witness structure is
necessary. Do not encode a large or lossy graph into the finding message.

## Work plan and gates

### Phase 0 — freeze the research decision

Deliverables:

- this temporary plan and retained structured research;
- a detector semantics note with positive and negative examples;
- a frozen Rust fixture catalog;
- benchmark manifest fields for repository revision, command, duration, RSS,
  edge labels, coverage, and reviewer labels.

Gate:

- maintainers agree on the in-scope Rust subset, policy meaning, proposed
  refactoring action, and explicit non-goals;
- every example can be labeled as instrumentation, detection, action, coverage,
  or outcome rather than a mixed judgment.

Stop if the first use case cannot be described without security or behavioral
claims.

### Phase 1 — local Rust flow facts

Implementation:

- add Rust lexical scopes for parameters and locals;
- extract direct definitions and uses from assignments, arguments, and returns;
- build deterministic local nodes and edges;
- record syntax errors and unsupported constructs;
- keep all output internal to unit tests.

Tests:

- direct aliases, shadowing, reassignment, branches, early returns, tuples,
  destructuring, references, nested blocks, and parse failures;
- negative fixtures for fields, methods, macros, closures, and external calls;
- metamorphic fixtures for formatting, comments, and identifier renaming.

Gate:

- at least 95% exact-edge precision and 90% recall on in-scope holdout fixture
  edges;
- 100% correct unsupported/partial classification on deliberately out-of-scope
  fixtures;
- byte-identical graph snapshots across repeated runs.

Stop or narrow the syntax subset if exact-edge precision misses the gate.

### Phase 2 — direct calls and bounded summaries

Implementation:

- index project-local Rust modules and free functions;
- resolve the explicit target subset;
- bind arguments to parameters and returns to call results;
- compose summaries to the configured depth;
- condense recursion and expose deterministic witness paths in test artifacts;
- measure runtime and memory on small, medium, and large Rust repositories.

Tests:

- `crate`, `self`, `super`, imports, re-exports, nested modules, recursion,
  mutual recursion, ambiguous names, unresolved modules, and depth truncation;
- additions of unrelated files must not change existing exact paths;
- adding an identity wrapper changes the path but not its endpoints;
- adding a real transform or unsupported call ends exact composition.

Gate:

- at least 90% manually adjudicated path agreement with one semantic reference
  engine for paths inside the declared abstraction;
- deterministic output and correct truncation/unresolved receipts;
- median scan-time overhead no greater than 15%, p95 overhead no greater than
  25%, and peak RSS no greater than 1.25 times the source-only baseline on the
  retained corpus. These are provisional product budgets, not universal
  performance claims.

Stop before schema work if path precision, coverage honesty, or the performance
budget fails after one bounded optimization pass.

### Phase 3 — policy prototype and maintainer labeling

Implementation:

- parse the draft policy in a prototype-only harness;
- identify exact protected-to-sink paths that omit every allowed adapter;
- retain one conforming comparison path when available;
- label candidates independently for instrumentation correctness, detection
  usefulness, refactoring action, exception category, and coverage accuracy;
- sample negative/quiet paths to estimate missed evidence.

Corpus:

- Rust libraries and services with explicit adapter modules;
- CLIs with composition roots;
- middleware and layered services;
- generated-client users;
- functional pipelines and event-driven systems as confounders;
- at least one whole-repository holdout not used for rule design.

Gate:

- lower 95% confidence bound of finding precision at least 80% on the holdout;
- median maintainer actionability at least 3 on a five-point scale;
- at least 60% of accepted candidates map to a concrete consolidation or
  routing seam;
- no finding path contains an unresolved or speculative edge.

If the instrumentation is accurate but actionability fails, keep flow facts as
internal research and do not publish the detector.

### Phase 4 — opt-in public feature

Implementation batch A — contracts:

- add resolved CLI/config models and strict validation;
- add schema 22 flow coverage types;
- add canonical metric IDs and manifests;
- add `FindingKind::AdapterFlowBypass` and its construct, mechanism, action,
  scope, approach, precision risk, issue family, and detector relations;
- define stable finding identity and baseline behavior.

Implementation batch B — scan integration:

- wire parsing and flow analysis after parsed-source collection and before
  finding controls;
- make `off` allocate no graph and perform no extra resolution;
- build the exact witness, policy comparison, coverage summary, detector
  receipt, and related locations;
- apply existing filters and suppressions without hiding raw coverage.

Implementation batch C — output:

- update human and JSON/YAML rendering;
- emit one SARIF result per clustered issue with witness locations;
- update the report-app TypeScript schema and path presentation;
- regenerate and commit `assets/report-app.js` and `assets/report-app.css` with
  their source changes.

Implementation batch D — documentation:

- update configuration, detector reference, metrics model, ontology,
  architecture, report schema, user guide, calibration protocol, and sample
  output;
- describe exact non-coverage and the difference between boundary and flow
  bypass detectors;
- replace or retire this temporary plan.

Gate:

- all unit, contract, baseline, output, config, and frontend tests pass;
- zero- and nonzero-finding reports remain valid;
- formatting/comment-only edits preserve at least 99% of unaffected finding
  IDs;
- Reforge self-scan remains at zero unsuppressed findings;
- `off` mode matches the previous JSON report except for the intentional schema
  version and new inactive flow-coverage contract;
- no performance regression outside the accepted Phase 2 budget.

### Phase 5 — promotion or containment

Run the opt-in feature across retained repositories for at least one calibration
cycle. Track accepted, rejected, suppressed, unresolved, timed-out, and
unobservable candidates separately.

Promotion to a non-experimental heuristic requires:

- two independently labeled holdouts;
- the precision and actionability gates to remain satisfied;
- stable performance and finding identities;
- documented exceptions and configuration migration;
- an explicit maintainer decision.

Promotion to a conservative detector additionally requires every emitted edge
to be exact and all required source/sink surfaces to meet the closed-world
coverage contract. Otherwise keep it opt-in. Do not lower gates merely to ship.

## Validation matrix

| Surface | Required validation |
| --- | --- |
| Instrumentation | Hand-labeled nodes, exact edges, summaries, and paths |
| Resolution | Exact, ambiguous, unresolved, external, recursive, truncated |
| Detection | Positive policy bypasses and conforming comparison paths |
| Exceptions | Facades, middleware, composition roots, generated code, tests, pipelines |
| Coverage | Parse failures, unsupported syntax, excluded paths, depth limits |
| Identity | Reordering, comments, formatting, unrelated additions, message changes |
| Configuration | Discovery, precedence, unknown keys, invalid globs, duplicates |
| Baseline | New, same, resolved, suppressed, and unobservable findings |
| Output | Human, JSON, YAML, SARIF, HTML, zero-finding contracts |
| Performance | Wall time, RSS, edge count, summary count, truncation count |
| Outcome | Accepted refactor preserves declared tests and removes or explains evidence |

## Risks and controls

| Risk | Control |
| --- | --- |
| Scope expands into a semantic engine | Enforce the in-scope edge list and phase stop gates |
| Dynamic or framework behavior is guessed | Emit unsupported/unresolved coverage only |
| Long paths become false smells | Require explicit policy and exact edges; no hop-only finding |
| Existing detector IDs change | Add a separate finding kind after validation |
| Reports become too large | Serialize summaries and bounded witnesses, never the full graph |
| Runtime grows with all-pairs reachability | Seed from configured policy endpoints and compose summaries |
| Config becomes security policy | Describe architecture ownership only; make no vulnerability claim |
| One language dictates a false abstraction | Revisit model ownership before the second adapter |
| Calibration overfits fixtures | Split by repository/framework and retain whole-project holdouts |
| Quiet reports imply completeness | Require capability-specific coverage in every run |

## Operational rollout and rollback

- Default the public feature to `off` for its first release.
- Do not include experimental findings in presets.
- Require explicit configuration before `policy` mode can emit findings.
- Allow standard `--exclude-detector`, `--only`, inline suppression, config
  suppression, and baselines once the finding is public.
- Retain a versioned schema and reject incompatible baselines normally.
- If a regression appears, users can set `mode = "off"`; maintainers can
  disable the detector without deleting retained policy configuration.
- A rollback must not silently reinterpret old finding IDs or report an
  unsupported analysis as observed zero.

## Required commands before each implementation handoff

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features
cargo run -- scan . --progress never
python3 docs/research/data-flow-signals/generate_report.py
python3 /home/ubuntu/sample/Deep-Research-skills/skills/research-codex-zh/research/validate_json.py \
  -f docs/research/data-flow-signals/fields.yaml \
  -d docs/research/data-flow-signals/results
scripts/build-docs.sh /tmp/reforge-docs-data-flow
```

Use a task-specific temporary output path. Do not commit generated scan reports
or documentation build output.

## Resolved implementation decisions

1. Sink syntax accepts fully qualified `crate::...` free-function symbols only.
2. `observe` is a supported config mode that emits coverage but no findings.
3. Schema 22 uses ordered related locations and a typed optional
   `flow_witness`; the internal graph is not serialized.
4. A declared adapter is sufficient. A conforming comparison path is retained
   when available but is not required to emit an exact bypass.
5. Policy configuration belongs in `reforge.toml` and remains config-owned.
6. The implementation is experimental and opt-in. Inline frozen Rust fixtures,
   report contracts, self-scan, and benchmark checks are release gates; broader
   independently labeled corpora remain a prerequisite for Phase 5 promotion,
   not for retaining the opt-in detector.
