# Metric Ontology

Reforge separates the maintainability property being discussed from the
mechanism observed in source. A convenient measurement such as line count is
evidence for review, not maintainability itself. The ontology covers signals
that Reforge can observe statically; it does not claim complete measurement of
software quality.

## Normative Coverage Matrix

Schema 21 declares coverage independently of detector registration. Its stable
7 mechanisms × 6 entity scopes form 42 cells: required combinations plus
explicitly out-of-scope combinations. Runtime status distinguishes observed,
partially observed, unsupported, no-entity, planned, and intentionally out-of-
scope analysis. Finding-group populations come from detector execution receipts
and are not inferred from issue count.

## Quality Constructs

Each finding uses one primary ISO/IEC 25010-aligned construct:

| Construct | Question answered |
| --- | --- |
| `modularity` | Will a change to one component propagate into others? |
| `reusability` | Is a concept represented once in a reusable form? |
| `analysability` | Can a maintainer locate, understand, and diagnose the change point? |
| `modifiability` | Can the code be changed locally without unnecessary work or regression risk? |
| `testability` | Can maintainers establish and execute effective verification criteria? |

A detector may be relevant to other constructs, but the report does not add
secondary interpretations together or turn them into a score.

## Signal Mechanisms

Mechanisms describe how observable evidence may create maintenance pressure:

| Mechanism | Representative evidence | Common confounders |
| --- | --- | --- |
| `cognitive_load` | function length, branching, nesting, parameters | generated dispatch, parsers |
| `dependency_propagation` | fan-in/out, cycles, imports, boundary bypasses | incomplete module resolution |
| `responsibility_dispersion` | oversized files/types/directories, mixed concepts | generated registries, declarative tables |
| `duplication_divergence` | similar implementations, repeated shapes or setup | intentional protocol symmetry |
| `change_pressure` | debt markers, compatibility paths, churn | migrations with explicit exit plans |
| `verification_difficulty` | missing negative/boundary evidence, distant tests | tests outside the scan root |
| `knowledge_drift` | naming inconsistency, missing or stale documentation | project-specific conventions |

Mechanisms are mutually exclusive as primary classifications, not assumed to
be statistically independent.

## Evidence Layers

Reforge uses these layers:

1. Raw metrics record observations such as LOC, complexity, fan-out, and churn.
2. Findings interpret observations through one detector and keep stable
   `rf3-...` evidence identity.
3. Constructs and mechanisms explain the maintainability concern represented
   by the evidence.
4. Issue clusters join compatible findings by issue family, canonical subject,
   and typed refactoring action under stable `ri3-...` identity.
5. Coverage and agent evidence explain what could be observed and the likely
   inspection/test surface.

There is no priority or readiness layer in schema 21. For example, complexity
and nesting findings on one function remain independently filterable while a
compatible issue presents their shared decision surface. Neither observation
automatically outweighs another issue.

## Detector Manifest Contract

Every finding kind has one `detector_manifest` entry with:

- primary `construct` and `mechanism`;
- typed refactoring `action` and `entity_scope`;
- detection `approach`;
- supported languages or repository scope;
- qualitative `precision_risk`;
- canonical `input_metrics`;
- `issue_family`, `evidence_role`, and `constituent_kinds`.

Tests enforce one entry per finding kind and reject metrics outside a detector's
declared input set. Unsupported languages mean “not observed,” not “no issue.”

## Orthogonality and Identity Rules

- One finding has one primary construct and mechanism.
- One raw observation may support multiple atomic findings, but issue
  clustering prevents compatible facets from becoming duplicate decision
  units.
- Parent, child, and composite relationships remain traceable; evidence values
  are not summed.
- Threshold and percentile values provide context inside a finding, not votes
  in a cross-finding score.
- Metric IDs are canonical and entity-qualified.
- Complete-link compatibility prevents a chain of shared files from merging
  unrelated endpoint findings.
- Finding IDs derive from kind, metric names, and canonical evidence
  locations. Issue IDs derive from issue family and canonical subject.
  Traversal order, rendering text, and metric value changes do not define
  identity.

## Raw Metric Contract

`raw_metric_manifest` defines the stable metric ID, entity scope, unit, scale,
direction, and meaning of every raw metric family. Directory observations are
stored once per directory. Boolean context fields are not treated as numeric
pressure. Count thresholds are contextual review policy, not universal quality
grades.

## Coverage Boundary

Completeness is relative to evidence Reforge declares observable: discovered
paths, supported parsed syntax, resolved local dependencies, repository
documentation, Unity project data, and optional git history.

| Evidence surface | Coverage | Explicit non-coverage |
| --- | --- | --- |
| Source paths and physical LOC | Language-neutral for discovered source files | Excluded, hidden, generated, dependency, and ignored paths follow scan settings. |
| Parsed functions, types, structure, and similarity | Declared per detector for supported Tree-sitter languages | Parse failures and unsupported grammars are not observations. |
| Unused-function analysis | Conservative supported-language local/private functions | Dynamic and unresolved references reduce recall. |
| Dependency graph | Declared language adapters and resolvable local edges | External, dynamic, alias, or framework-specific unresolved edges are omitted. |
| Repository documentation contract | Reforge repository scope | Not a universal documentation policy for arbitrary projects. |
| Unity project model | Recognized roots and supported text-serialized assets | Binary serialization, missing packages, and unresolved references degrade coverage. |
| Change history | Git history when enabled and available | Disabled, unavailable, binary, out-of-root, and oversized commits are omitted. |

`coverage_manifest`, `coverage_summary`, `detector_execution`, and
`raw_metric_coverage` make these boundaries machine-readable. `agent_evidence`
adds context closure and test reachability but does not claim behavioral
coverage or refactor safety.

## Theoretical Basis

- ISO/IEC 25010:2023 supplies the product-quality reference model and
  maintainability constructs; it does not prescribe Reforge thresholds.
- ISO/IEC 25023 and ISO/IEC 25020 provide quality-measurement guidance while
  leaving rated ranges contextual to product and user needs.
- Goal Question Metric motivates deriving measurements from explicit goals and
  questions.
- ISO/IEC/IEEE 15939 connects information needs, measures, analysis,
  application, and validity checks.

## Validation Expectations

The ontology establishes completeness relative to declared constructs, not all
possible maintenance work. Evaluation should keep instrumentation accuracy,
detector precision/recall, action usefulness, coverage, stable identity, and
workflow outcomes separate. Maintainer labels can improve detector policy but
must not be presented as a universal codebase score.
