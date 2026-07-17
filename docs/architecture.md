# Architecture

Reforge is a single Rust CLI crate. The code is organized around a scan
pipeline that collects raw metrics first, then runs detectors, ranks hotspots,
scores findings, and renders reports.

## Module Boundaries

- `src/main.rs`: CLI entrypoint, progress/color/output routing, file writing,
  baseline gate evaluation, and broken-pipe handling.
- `src/cli.rs`: Clap command definitions, scan arguments, output inference,
  progress modes, color modes, churn modes, and hotspot models.
- `src/scan/mod.rs`: scan orchestration, config discovery, source walking,
  file-level findings, git churn collection, progress reporting, and final
  `ScanReport` assembly.
- `src/lang/mod.rs`: Tree-sitter language adapters and shared node-kind
  constants.
- `src/model/mod.rs`: serializable report model, finding kinds, raw metrics,
  summaries, hotspots, severities, and schema version.
- `src/detectors/`: similarity, structure, drift, and documentation detector
  implementations.
- `src/scoring/mod.rs`: metric summaries, priority scoring, severity mapping,
  and hotspot ranking.
- `src/baseline.rs`: schema 19 baseline loading, finding ID comparison, diff
  classification, and `--fail-on` gate selection.
- `src/output/mod.rs`: human, HTML, JSON, YAML, and SARIF output entry points.

`src/main.rs` re-exports internal modules under compatibility names such as
`scanner`, `report`, `similar_functions`, and `structural` so existing inline
tests and module references can stay stable while the implementation is split
into clearer directories.

## Scan Flow

1. Parse `reforge scan [OPTIONS] [PATH]` with Clap.
2. Resolve the scan root and load effective arguments from CLI plus optional
   `reforge.toml`.
3. Walk source files with default exclusions, explicit ignored paths, and
   hidden/generated controls.
4. Read each source file, collect line counts, file metrics, TODO/FIXME debt
   markers, and parsed Tree-sitter sources where supported.
5. Run structural, unused-function, dependency-graph, agent-drift,
   similar-function, and documentation detectors.
6. Merge structural raw metrics and the resolved dependency graph snapshot into
   the report model.
7. Collect git churn when enabled.
8. Summarize raw metrics into percentiles.
9. Rank hotspots with the chosen model.
10. Finalize finding metrics, priority, confidence, severity, and ranking
    explanations.
11. Apply finding filters and suppressions, recording `suppression_summary`
    for findings removed by suppressions.
12. Render human, HTML, JSON, YAML, or SARIF output to stdout or
    `--output-file`.
13. Apply `--fail-on` to all current unsuppressed findings or to the
    baseline-selected finding set after the report is written. Human output
    can also render baseline diff counts and `--show`-selected current
    findings.

## Data Flow

`ScanArgs` is the input configuration. `scan_report` produces a `ScanReport`
with schema version `19`. Detectors emit `Finding` values with metrics and
related locations. The dependency-graph detector also emits a resolved
source-file graph snapshot. Scoring later enriches findings with constructs and mechanisms,
normalized values, percentiles, `priority_factors`, `priority`, `severity`,
`rank_explanation`, and stable `rf3-` IDs. After filtering and suppression,
overlapping findings are grouped into issue clusters.

Raw metrics remain available in reports so consumers can build their own
ranking or dashboards without relying only on findings.

## Parser Integration

Tree-sitter support is routed through `LanguageAdapter`. Structural and
similarity analysis currently supports Rust, JavaScript, TypeScript/TSX, Vue
SFC script blocks, Python, Go, Java, C#, Kotlin, PHP, and Ruby.

Files with parse errors are skipped for Tree-sitter detectors but can still
contribute basic file metrics and debt-marker findings. Broad source discovery
includes more extensions than Tree-sitter supports so simple file and directory
signals still work on mixed repositories.

## Progress and Output

Progress is abstracted behind `ProgressSink`. `NoopProgress` is used when
progress is disabled. `StderrProgress` writes either dynamic terminal progress
or coarser line-oriented progress, depending on whether stderr is a TTY.

Human, HTML, and SARIF output are produced from the same `ScanReport` as JSON
and YAML. The terminal-oriented renderer lives in `src/output/human.rs`, SARIF
2.1.0 output lives in `src/output/sarif.rs`, and `src/output/mod.rs` keeps the
format entry points and JSON/YAML writers. Color is applied only to human
output.

## HTML Report App

`--output html` and output-file extensions `.html` or `.htm` produce a single
offline HTML artifact. The active HTML implementation is the React +
TypeScript report app.

The data and packaging flow is:

1. The Rust scanner builds a schema 19 `ScanReport`.
2. The HTML output path serializes that report as JSON.
3. Reforge writes an HTML shell containing the serialized report data.
4. The shell inlines the compiled React bundle and CSS.
5. The browser runs the embedded app locally with no network or server
   dependency.

Frontend source lives under `web/report-app`. Build the app there when the
visual report changes:

```powershell
cd web\report-app
npm ci
npm run build
```

The build is expected to refresh the checked-in report assets:

- `assets/report-app.js`
- `assets/report-app.css`

Keep those generated assets in sync with frontend source changes so Rust can
embed the current app into the offline report. Update `docs/report-schema.md`
when the `ScanReport` shape changes; the report app should read the documented
schema rather than private scanner internals.

## Extension Points

To add a detector:

1. Add a `FindingKind` and display metadata.
2. Implement the detector in `src/detectors/` or extend an existing detector
   family.
3. Add metrics with meaningful names, units, thresholds, and related
   locations.
4. Wire the detector into `scan_report`.
5. Add manifest classification, coverage, precision risk, parent, and overlap
   metadata; update confidence, impact, and actionability when needed.
6. Update report schema and detector docs.
7. Add focused unit tests next to the module being changed.

To add a language:

1. Add the Tree-sitter crate dependency.
2. Extend `LanguageFamily` and `adapter_for_path`.
3. Add function, type, import, public item, complexity, and test-case handling
   where applicable.
4. Add tests for parsing, metrics, and detector behavior.

To change report shape:

1. Update `SCAN_REPORT_SCHEMA_VERSION`.
2. Update serializable model types.
3. Update `docs/report-schema.md`.
4. Update the React report app when the visual report depends on the changed
   fields.
5. Add or update report tests that pin important fields.
