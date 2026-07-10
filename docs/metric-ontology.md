# Metric Ontology

Reforge separates the quality property being discussed from the mechanism
observed in source code. This prevents convenient measurements such as line
count from being treated as maintainability itself and prevents correlated
signals from receiving multiple votes merely because several detectors can
describe them.

The ontology is scoped to maintainability signals that Reforge can observe
statically. It is not a claim that source analysis completely measures
maintainability.

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

## Measurement Layers

Reforge uses four layers:

1. Raw metrics record observations such as LOC, complexity, fan-out, and churn.
2. Findings interpret observations through one detector.
3. Issue clusters join overlapping findings that describe the same entity,
   mechanism, and likely refactoring action.
4. Priority ranks the resulting evidence for review; it is not a quality score
   or defect probability.

For example, a function that exceeds complexity and nesting thresholds retains
both atomic findings for filtering, baselines, and auditability. The report
also emits one `cognitive_load` issue cluster and human-facing output displays
the highest-priority member as the primary issue.

## Detector Manifest Contract

Every finding kind has an entry in the report-level `detector_manifest` with:

- its primary `construct` and `mechanism`;
- detection `approach`;
- supported languages or repository scope;
- qualitative `precision_risk`;
- optional `parent_kind` for composite detectors;
- `overlaps_with` relationships for detectors that can observe the same cause.

Adding a detector requires adding its manifest entry. Tests enforce one entry
per finding kind. Unsupported languages mean “not observed,” not “no issue.”

## Orthogonality Rules

- One finding has one primary construct and one primary mechanism.
- One raw observation may appear as evidence in multiple atomic findings, but
  issue clustering prevents it from becoming multiple human-facing issues.
- Parent and child findings remain traceable; they are not summed.
- Detector confidence represents interpretation uncertainty. It does not
  compensate for missing language coverage.
- Threshold and percentile evidence are normalized within a finding by taking
  the strongest facet rather than summing correlated facets.

## Validation Expectations

The ontology establishes completeness relative to declared constructs, not to
all possible maintenance work. Calibration should separately test detector
precision, recall on seeded and reviewed examples, ranking agreement with
maintainers, language coverage, and correlations among raw metrics.
