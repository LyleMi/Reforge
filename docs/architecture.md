# Architecture

Reforge is a single Rust CLI crate. The code is organized around a scan
pipeline that collects raw metrics first, then runs detectors, ranks hotspots,
scores findings, and renders reports.

## Module Boundaries

- `src/main.rs`: CLI entrypoint, progress/color/output routing, file writing,
  and broken-pipe handling.
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
- `src/output/mod.rs`: human, HTML, JSON, and YAML rendering.

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
5. Run structural, agent-drift, similar-function, and documentation detectors.
6. Merge structural raw metrics into the report model.
7. Collect git churn when enabled.
8. Summarize raw metrics into percentiles.
9. Rank hotspots with the chosen model.
10. Finalize finding metrics, priority, confidence, severity, and ranking
    explanations.
11. Render human, HTML, JSON, or YAML output to stdout or `--output-file`.

## Data Flow

`ScanArgs` is the input configuration. `scan_report` produces a `ScanReport`
with schema version `8`. Detectors emit `Finding` values with metrics and
related locations. Scoring later enriches those findings with dimensions,
normalized values, percentiles, `priority_factors`, `priority`, `severity`, and
`rank_explanation`.

Raw metrics remain available in reports so consumers can build their own
ranking or dashboards without relying only on findings.

## Parser Integration

Tree-sitter support is routed through `LanguageAdapter`. Structural and
similarity analysis currently supports Rust, JavaScript, TypeScript/TSX,
Python, and Go.

Files with parse errors are skipped for Tree-sitter detectors but can still
contribute basic file metrics and debt-marker findings. Broad source discovery
includes more extensions than Tree-sitter supports so simple file and directory
signals still work on mixed repositories.

## Progress and Output

Progress is abstracted behind `ProgressSink`. `NoopProgress` is used when
progress is disabled. `StderrProgress` writes either dynamic terminal progress
or coarser line-oriented progress, depending on whether stderr is a TTY.

Human and HTML output are rendered from the same `ScanReport` as JSON and
YAML. The terminal-oriented renderer lives in `src/output/human.rs`, the
static visual report renderer lives in `src/output/html.rs`, and
`src/output/mod.rs` keeps the format entry points and JSON/YAML writers. Color
is applied only to human output.

## Extension Points

To add a detector:

1. Add a `FindingKind` and display metadata.
2. Implement the detector in `src/detectors/` or extend an existing detector
   family.
3. Add metrics with meaningful names, units, thresholds, and related
   locations.
4. Wire the detector into `scan_report`.
5. Update scoring dimensions, confidence, impact, and actionability when the
   new kind needs custom scoring.
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
4. Add or update report tests that pin important fields.
