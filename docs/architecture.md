# Architecture

- `tools/reforge` is a thin CLI and configuration boundary.
- `crates/reforge-engine` owns workspace indexing, execution planning,
  Codebase and Dataflow analysis, evidence aggregation, and report creation.
- `crates/reforge-unity-engine` independently owns Unity scanning, rules,
  coverage, and report construction; it depends on the shared schema, not the
  core engine.
- `crates/reforge-schema` owns the strict schema 26 `Report`, stable identities,
  typed witnesses, coverage, and baseline comparison.
- `crates/reforge-output` owns human, JSON, YAML, SARIF, and embedded HTML
  rendering.
- `web/report-app` owns the offline HTML interface.
- `tools/reforge-unity` is an experimental CLI over the independent Unity
  engine and emits producer `reforge.unity`.
- `tools/reforge-workflow` is an optional report consumer; it is not part of
  analyzer execution.

The engine builds one shared workspace index. Each selected source is walked,
read, language-classified, and parsed once; Codebase and Dataflow consume the
same indexed sources. The typed `Config` selects either or both analyses and
owns scope, thresholds, policies, and suppressions.

The public model starts at the report. An analysis is an execution selection
and a Coverage key, not a wrapper around the report:

```text
Report
├── Coverage by analysis
│   ├── language counts
│   ├── rule execution
│   └── limitations
└── Issue
    └── Evidence
        ├── Measurement
        ├── Location
        └── optional Flow witness
```

Detectors produce `DetectedEvidence` with a semantic anchor and no internal
report ID. One static `RuleSpec` registry supplies analysis ownership,
aggregation family, output subject kind, input observation source, language
support, measurements, and a rule-specific description. Families
are an aggregation and identity mechanism, not an additional user workflow:
after suppression, the engine groups Evidence by family and Subject into
schema 26 Issues; schema projection alone creates `re6-*` Evidence IDs.

The engine returns the public `Report` directly. Debug metrics and Flow IR take
separate explicit sidecar paths and never enter the report. Flow IR is only
materialized when `--flow-ir-output` is requested.
