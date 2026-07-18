# Metric Ontology

Reforge separates the quality property being discussed from the mechanism
observed in source code. This prevents convenient measurements such as line
count from being treated as maintainability itself and prevents correlated
signals from receiving multiple votes merely because several detectors can
describe them.

The ontology is scoped to maintainability signals that Reforge can observe
statically. It is not a claim that source analysis completely measures
maintainability.

## Normative coverage matrix

Schema 20 declares coverage independently of detector registration. Its stable
42 cells comprise 12 required and 30 intentionally out-of-scope combinations.
Runtime status is `observed`, `partially_observed`, `unsupported`, `no_entities`,
`planned`, or `intentionally_out_of_scope`. Finding-group populations come from
group detector receipts and are not inferred from issue count.

## Quality Constructs

Findings use one primary maintainability construct, aligned with ISO/IEC 25010:

| Construct | Question answered |
| --- | --- |
| `modularity` | Will a change to one component propagate into others? |
| `reusability` | Is a concept represented once in a reusable form? |
| `analysability` | Can a maintainer locate, understand, and diagnose the change point? |
| `modifiability` | Can the code be changed locally without unnecessary work or regression risk? |
| `testability` | Can maintainers establish and execute effective verification criteria? |

Every finding has exactly one primary construct. A detector may provide
evidence relevant to other constructs, but those secondary interpretations do
not increase its score.

## Signal Mechanisms

Mechanisms describe how the observed evidence may create maintenance pressure:

| Mechanism | Representative evidence | Common confounders |
| --- | --- | --- |
| `cognitive_load` | function length, branching, nesting, parameters | generated dispatch, parsers |
| `dependency_propagation` | fan-in/out, cycles, imports, boundary bypasses | incomplete module resolution |
| `responsibility_dispersion` | oversized files/types/directories, mixed concepts | generated registries, declarative tables |
| `duplication_divergence` | similar implementations, repeated shapes or setup | intentional protocol symmetry |
| `change_pressure` | debt markers, compatibility paths, churn | migrations with explicit exit plans |
| `verification_difficulty` | missing negative or boundary evidence | tests stored outside the scan root |
| `knowledge_drift` | naming inconsistency, missing or stale documentation | project-specific conventions |

Mechanisms are mutually exclusive as primary classifications. They are not
assumed to be statistically independent. Correlated raw measurements remain
separate evidence facets and are not added together automatically.

## Goal and Measurement Layers

Reforge's measurement goal is to prioritize reviewable, behavior-preserving
refactoring opportunities from source trees and optional repository history.
The model follows a goal-question-metric direction: measurements are admitted
only when they answer a declared maintenance question and support a review or
refactoring decision.

Reforge uses five layers:

1. Raw metrics record observations such as LOC, complexity, fan-out, and churn.
2. Findings interpret observations through one detector.
3. Constructs and mechanisms explain the maintenance capability and causal
   pressure represented by the evidence.
4. Issue clusters join related findings that describe the same entity and
   typed refactoring `action`.
5. Priority ranks the resulting evidence for review; it is not a quality score
   or defect probability.

For example, a function that exceeds complexity and nesting thresholds retains
both atomic findings for filtering, baselines, and auditability. The report
also emits one `cognitive_load` issue cluster and human-facing output displays
the highest-priority member as the primary issue.

## Detector Manifest Contract

Every finding kind has an entry in the report-level `detector_manifest` with:

- its primary `construct` and `mechanism`;
- its typed refactoring `action` and `entity_scope`;
- detection `approach`;
- supported languages or repository scope;
- qualitative `precision_risk`;
- canonical `input_metrics`, dual reliability, impact, and actionability policy values;
- `issue_family`, `evidence_role`, and `constituent_kinds` for composition.

Adding a detector requires adding its manifest entry. Tests enforce one entry
per finding kind. Unsupported languages mean “not observed,” not “no issue.”

## Orthogonality Rules

- One finding has one primary construct and one primary mechanism.
- Human-facing uniqueness is defined by refactoring action and evidence
  identity, not by requiring raw metrics to be statistically independent.
- One raw observation may appear as evidence in multiple atomic findings, but
  issue clustering prevents it from becoming multiple human-facing issues.
- Parent and child findings remain traceable; they are not summed.
- Detector confidence represents interpretation uncertainty. It does not
  compensate for missing language coverage.
- Threshold and percentile evidence are normalized within a finding by taking
  the strongest facet rather than summing correlated facets.
- A detector cannot emit a metric outside its manifest-declared input set.
- Metric IDs are canonical and entity-qualified, so aliases cannot make one
  observation appear to be independent evidence.
- Clusters use complete-link compatibility: every member must be related to
  every other member. A chain of shared files cannot merge unrelated endpoint
  findings.
- Findings carry stable EvidenceIds. Clustering sorts by EvidenceId before
  grouping, and each IssueKey is derived from the sorted member EvidenceIds,
  so detector emission order cannot change cluster membership or identity.

## Raw Metric Contract

The report-level `raw_metric_manifest` defines the canonical metric ID, entity
scope, unit, scale, direction, and meaning of every raw metric family. Directory
observations are stored once per directory instead of being repeated on every
contained file. Boolean context fields are
not treated as numeric pressure. Counts remain ratio-scale observations, but
their thresholds are contextual policy rather than universal quality grades.

## Coverage Boundary

Completeness is relative to evidence Reforge declares observable: discovered
source paths, supported parsed syntax, the resolved dependency graph,
repository documentation, and optional Git history. Unsupported languages,
disabled history, excluded paths, and unresolvable dependencies mean “not
observed,” never “no maintenance pressure.” Detector manifest language scope
and raw metric definitions make this boundary machine-readable.

| Evidence surface | Coverage state | Explicit non-coverage |
| --- | --- | --- |
| Source paths and physical LOC | Language-neutral for discovered source files | Excluded, hidden, generated, and dependency paths follow scan configuration. |
| Parsed functions, types, structure, and similarity | Rust, JavaScript, TypeScript/TSX, Vue SFC script blocks, Python, Go, Java, C#, Kotlin, PHP, and Ruby as declared per detector | Parse failures and unsupported grammars are not observations. |
| Unused-function analysis | Rust, JavaScript, TypeScript/TSX, Vue SFC script blocks, Python, Go, and C# local functions | Dynamic and unresolved references can reduce recall. |
| Dependency graph | Rust, JavaScript, TypeScript/TSX, Vue SFC script blocks, Python, Ruby, C, and C++ | Unresolved external or framework-specific edges are omitted. |
| Repository documentation contract | Reforge repository scope | This is not a universal documentation policy for arbitrary projects. |
| Change history | Git history when churn is enabled and available | Disabled, unavailable, binary, out-of-root, and oversized commits are omitted. |

## Theoretical Basis

- ISO/IEC 25010:2023 supplies the product-quality reference model and
  maintainability constructs; it does not prescribe Reforge's detectors or
  weights: <https://www.iso.org/standard/78176.html>.
- ISO/IEC 25023:2016 defines product-quality measurement guidance and leaves
  rated ranges contextual to the product and user needs:
  <https://www.iso.org/standard/35747.html>.
- Goal Question Metric motivates deriving measurements from explicit goals and
  questions: <https://csis.pace.edu/~ogotel/teaching/CS777gqm.pdf>.
- Briand, Morasca, and Basili motivate explicit entities, attributes, and
  mathematical properties for software measures:
  <https://www.cs.umd.edu/~basili/publications/journals/J58.pdf>.
- ISO/IEC 25020:2019 supplies the quality-measurement reference model and
  guidance for selecting, constructing, validating, and documenting measures:
  <https://www.iso.org/standard/72117.html>.
- ISO/IEC/IEEE 15939:2017 supplies the measurement process for connecting
  information needs, measures, analysis, application, and validity checks:
  <https://www.iso.org/standard/71197.html>.

## Validation Expectations

The ontology establishes completeness relative to declared constructs, not to
all possible maintenance work. Calibration should separately test detector
precision, recall on seeded and reviewed examples, ranking agreement with
maintainers, language coverage, and correlations among raw metrics.
