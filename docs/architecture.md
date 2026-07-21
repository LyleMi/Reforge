# Architecture

Reforge is a single Rust CLI crate. The scan pipeline observes source trees,
collects raw metrics, runs detectors, groups atomic findings into decision
units, projects coverage and agent context, and renders schema 22 reports.

## Module Boundaries

- `src/main.rs`: CLI entrypoint, config commands, progress/color/output routing,
  baseline loading, new-finding gate evaluation, and broken-pipe handling.
- `src/cli/`: Clap commands, scan argument groups, thresholds, output inference,
  progress/color/churn modes, and value parsers.
- `src/scan/`: scan orchestration, config discovery, source walking, git churn,
  finding controls, coverage projection, agent evidence, and final
  `ScanReport` assembly.
- `src/lang/`: Tree-sitter language adapters and shared syntax classification.
- `src/model/`: schema 22 report data, finding/issue identity, coverage,
  evidence subjects, raw metrics, dependency data, and Unity report data.
- `src/detectors/`: structural, similarity, unused-function, dependency,
  concept-drift, documentation, and detector-owned exact Rust data-flow
  analysis plus their manifest contracts.
- `src/evidence_analysis.rs`: metric percentile context and compatible-finding
  clustering without a ranking model.
- `src/workflow.rs`: strict resumable artifacts, approval snapshots, direct
  checks, rescans, and workflow state transitions.
- `src/baseline.rs`: schema 22 baseline validation, stable-ID diffing,
  `--show` selection, and new unsuppressed finding selection.
- `src/unity.rs` and `src/unity/`: Unity project planning, text asset/GUID
  indexing, asmdef analysis, and Unity-aware C# signals.
- `src/output/`: human, HTML, JSON, YAML, and issue-oriented SARIF rendering.
- `web/report-app`: React + TypeScript source for the embedded offline HTML
  report.

`src/main.rs` re-exports some modules under compatibility names used by inline
tests. These aliases are not public library API; Reforge is currently a binary
crate.

## Scan Flow

1. Parse `reforge scan [OPTIONS] [PATH]` with Clap.
2. Resolve the scan root and merge CLI values with an optional discovered or
   explicit `reforge.toml`.
3. Build a source plan with default generated/hidden/git-ignore exclusions,
   explicit ignored paths, and test-scope controls.
4. Read source files and collect basic file/directory observations plus parsed
   Tree-sitter sources where supported.
5. Run structural, unused-function, dependency-graph, optional exact Rust
   data-flow, concept-drift, similar-function, documentation, and applicable
   Unity detectors. `off` skips graph construction entirely.
6. Merge structural function/type metrics and the resolved source dependency
   graph into the observation model.
7. Collect git churn when enabled. Missing history in `auto` mode degrades the
   churn observation rather than failing the scan.
8. Summarize raw metrics into project percentiles and attach normalized metric
   context to findings.
9. Remove composite summaries, apply detector filters and suppressions, and
   record `suppression_summary`.
10. Cluster compatible atomic findings into stable `ri3-...` issues while
    retaining their stable `rf3-...` evidence IDs.
11. Build `agent_evidence` from dependency closure, unresolved local edges,
    evidence dispersion, and direct/reachable tests.
12. Project coverage manifests, run-specific coverage summary, detector
    execution receipts, and raw metric coverage.
13. Render human, HTML, JSON, YAML, or SARIF output to stdout or
    `--output-file`.
14. When a schema 22 baseline is present, compute issue display diffs. If
    `--fail-on-findings` is enabled, fail after writing the report when current
    unsuppressed finding IDs are absent from the baseline.

## Data Contract

`ScanArgs` is the resolved execution input. `scan_report` produces a schema 22
`ScanReport`. The serialized layers are:

- observations: `stats`, `raw_metrics`, `metrics_summary`,
  `raw_metric_manifest`, and `raw_metric_coverage`;
- topology and project context: `dependency_graph`, `agent_evidence`, and
  `unity_project`;
- observability: `coverage_manifest`, `coverage_summary`, `flow_analysis`, and
  `detector_execution`;
- decisions and evidence: `issues`, `findings`, `detector_manifest`, and
  `suppression_summary`.

Findings carry metric values, thresholds, normalized context, construct,
mechanism, message, recommendation, and related locations. Issues carry a
stable canonical subject, refactor action, family, and member evidence IDs.
Flow findings additionally carry a compact typed witness; the full internal
graph is never serialized.

## Exact Rust Data-Flow Boundary

`src/detectors/data_flow/` owns deterministic nodes/edges, Rust free-function
and module resolution, lexical def-use extraction, bounded call composition,
policy matching, and capability receipts. The analysis retains only assignment,
argument-to-parameter, and return-to-result edges it can prove. Call/return
search state tracks call sites, so recursive components are bounded without
enumerating cyclic paths. At most one shortest deterministic path is retained
per source/sink/policy tuple.

The layer is deliberately not a whole-program taint engine. It does not model
heap aliases, fields, methods, dynamic dispatch, external crates, runtime
middleware, persistence, queues, or service hops. Unsupported constructs add
coverage reasons and cannot appear in a conservative finding witness.

Schema 22 does not serialize priority, severity, hotspot ranking, scoring
policy, or reliability scores. Those legacy model and CLI fields are not kept
in the Rust implementation. Consumers choose work using the evidence,
coverage, project goals, and their own policy.

See [Report Schema](report-schema.md) for the complete serialized contract and
[Agent Workflows](agent-workflows.md) for the boundary between scanner
facts and proposed agent orchestration.

## Parser Integration

Tree-sitter support is routed through language adapters. Structural and
similarity analysis currently covers Rust, JavaScript, TypeScript/TSX, Vue SFC
script blocks, Python, Go, Java, C#, Kotlin, PHP, Ruby, Bash, and PowerShell.
Bash and PowerShell are structure/similarity-only in the first version; the
dependency graph and unused-function detectors intentionally avoid script call
semantics.

Files with parse failures can still contribute language-neutral file,
directory, debt-marker, dependency, and documentation evidence. Coverage
receipts record parse failures and unobservable reasons so consumers do not
mistake unsupported or partial analysis for an observed absence.

## Stable Identity and Baselines

Finding IDs use detector kind, metric names, and canonicalized evidence
locations. Issue IDs use issue family and canonical subject. They intentionally
exclude messages, metric values, and traversal order so the same evidence can
survive harmless rendering or ordering changes.

Schema 22 baselines select `new` or `all` evidence. They do not compare
severity or priority because those values are not part of the serialized
contract. `--fail-on-findings` requires a baseline and gates only new current
unsuppressed finding IDs. Human display diffs operate on issue IDs and can show
new, same, and resolved decision units.

## Progress and Output

Progress is abstracted behind `ProgressSink`. `NoopProgress` is used when
progress is disabled. `StderrProgress` writes dynamic terminal progress or
coarser line-oriented progress depending on whether stderr is a TTY.

Human, HTML, and SARIF output derive from the same `ScanReport` as JSON and
YAML. JSON and YAML preserve atomic findings and issues. SARIF emits issue
decision units at `note` level with stable issue fingerprints. Color applies
only to human output.

## HTML Report App

`--output html` and `.html`/`.htm` output-file extensions produce one offline
artifact. The packaging flow is:

1. The Rust scanner builds a schema 22 `ScanReport`.
2. HTML output serializes the report as JSON into an HTML shell.
3. The shell inlines the compiled React bundle and CSS.
4. The browser renders locally without a server or network dependency.

Frontend source lives under `web/report-app`. After UI changes run:

```powershell
cd web\report-app
npm ci
npm run build
```

Commit frontend source together with refreshed `assets/report-app.js` and
`assets/report-app.css`. Update the report schema documentation and frontend
types in the same change when serialized fields change.

## Extension Points

To add a detector:

1. Add a `FindingKind` and stable serialized name.
2. Implement the detector in the owning detector family.
3. Emit declared metrics with meaningful IDs, units, thresholds, and related
   locations.
4. Add manifest construct, mechanism, action, entity scope, approach,
   supported-language, precision-risk, issue-family, and evidence-role data.
5. Wire execution and coverage receipts into the scan.
6. Add focused detector, issue-clustering, output, and documentation tests.

To add a language:

1. Add the Tree-sitter dependency and language adapter.
2. Define function, type, import, public-item, complexity, and test constructs.
3. Update supported-language manifest data and coverage expectations. Keep
   detectors out of the language manifest when the language has only partial
   semantic support.
4. Add parsing, metric, detector, dependency-resolution, and failure tests.

To change report shape:

1. Update `SCAN_REPORT_SCHEMA_VERSION`.
2. Update serializable model types and baseline validation.
3. Update JSON/YAML, human, SARIF, and HTML consumers as applicable.
4. Update `docs/report-schema.md`, this architecture guide, and agent skills.
5. Add zero-finding and nonzero-finding report contract tests.
