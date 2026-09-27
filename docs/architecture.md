# Architecture

- `tools/reforge` is a thin CLI and configuration boundary.
- `crates/reforge-engine` owns workspace indexing, execution planning,
  Codebase and Dataflow analysis, evidence aggregation, and report creation.
- `crates/reforge-schema` owns the public `Report`, stable identities,
  typed witnesses, coverage, and baseline comparison.
- `crates/reforge-output` owns human, JSON, YAML, SARIF, and embedded HTML
  rendering.
- `web/report-app` owns the offline HTML interface.

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
Issues; schema projection alone creates stable Evidence IDs.

The engine returns the public `Report` directly. Debug metrics and Flow IR take
separate explicit sidecar paths and never enter the report. Flow IR is only
materialized when `--flow-ir-output` is requested.


Comment inventory and cleanup live behind `reforge_engine::api::comments`.
They reuse source-scope discovery and language parsers, with separate versioned
inventory/patch structures rather than changing the analysis report schema.
The optional Codebase `comment_hygiene` rule shares the extractor over indexed
trees. Only the explicit comment-apply API writes source files; normal analysis
and comment preview remain read-only.
