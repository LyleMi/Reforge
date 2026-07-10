# Report Schema

JSON and YAML reports use schema version `15`. The same Rust data model is
serialized for both formats. SARIF output is a separate SARIF 2.1.0 document
that carries the same finding IDs in result fingerprints.

## Top-Level Shape

```json
{
  "schema_version": 15,
  "summary": {},
  "stats": {},
  "metrics_summary": {},
  "raw_metrics": {},
  "raw_metric_manifest": [],
  "dependency_graph": {},
  "hotspots": [],
  "suppression_summary": {},
  "issue_clusters": [],
  "detector_manifest": [],
  "findings": []
}
```

Top-level fields:

- `schema_version`: report schema version. Current value is `15`.
- `summary`: scan totals, duration, hotspot model, and churn status.
- `stats`: source files, directories, and function candidates counted.
- `metrics_summary`: percentile distributions for raw metrics.
- `raw_metrics`: file, function, type, and churn measurements.
- `raw_metric_manifest`: scale, unit, scope, direction, and meaning of every
  raw metric family.
- `dependency_graph`: resolved source-file dependency graph snapshot.
- `hotspots`: ranked file, function, and type locations.
- `suppression_summary`: aggregate counts for findings removed by
  suppressions.
- `issue_clusters`: overlapping atomic findings grouped into human-facing
  refactoring issues.
- `detector_manifest`: coverage and classification metadata for every finding
  kind.
- `findings`: detector findings with priority, confidence, metrics, and
  related locations.

Reports contain maintainability and refactoring signals. They are not a
quality score, health score, bug detector, or defect probability model.
`findings` is the post-scoring, post-filter, post-suppression list; an empty
list does not mean raw metrics, hotspots, or suppressed signals were absent.

## `summary`

Fields:

- `scanned_files`: number of source files scanned.
- `finding_count`: number of findings emitted after filters and suppressions.
- `issue_count`: findings after clustered secondary facets are counted once.
- `hotspot_count`: number of hotspots emitted for the watchlist.
- `similar_function_group_count`: number of similar-function findings.
- `duration_ms`: scan duration in milliseconds.
- `hotspot_model`: `static`, `churn`, or `hybrid`.
- `churn`: churn collection details.

`summary.churn` fields:

- `mode`: requested churn mode, one of `auto`, `on`, or `off`.
- `enabled`: whether churn metrics were collected.
- `status`: `enabled`, `disabled`, or `unavailable`.
- `reason`: optional human-readable reason when churn is disabled or
  unavailable.
- `window_days`: configured git history window.
- `max_commit_lines`: configured max added+deleted lines per commit.

## `stats`

Fields:

- `source_files_scanned`: source files scanned.
- `directories_scanned`: directories visited.
- `function_candidates`: function bodies considered for similarity analysis.

## `metrics_summary`

`metrics_summary` contains maps for `files`, `functions`, `types`, and
`churn`. Each metric has:

- `p50`
- `p75`
- `p90`
- `p95`
- `max`

File metrics include `loc`, `imports`, `public_items`, and
`directory_source_files`.

Function metrics include `loc`, `complexity`, `nesting_depth`, and
`parameter_count`.

Type metrics include `loc` and `member_count`.

Churn metrics include `commits_touched`, `lines_added`, `lines_deleted`,
`authors_count`, and `recent_weighted_churn`.

## `raw_metrics`

`raw_metrics.files` entries:

- `path`
- `loc`
- `imports`
- `public_items`
- `directory_source_files`
- `is_test`
- `churn`

`raw_metrics.files[].churn` entries:

- `commits_touched`
- `lines_added`
- `lines_deleted`
- `authors_count`
- `recent_weighted_churn`

`raw_metrics.functions` entries:

- `path`
- `name`
- `line`
- `loc`
- `complexity`
- `nesting_depth`
- `parameter_count`
- `is_test`

`raw_metrics.types` entries:

- `path`
- `name`
- `line`
- `loc`
- `member_count`
- `is_test`

## `dependency_graph`

`dependency_graph` records the resolved source-file import graph used by the
dependency-cycle and dependency-hub detectors. External packages and unresolved
imports are not included.

`dependency_graph.nodes` entries:

- `path`: source file path.
- `fan_in`: number of resolved files that import or include this file.
- `fan_out`: number of resolved files imported or included by this file.

`dependency_graph.edges` entries:

- `from`: importing or including source file.
- `to`: resolved imported or included source file.

## `hotspots`

Hotspot fields:

- `level`: `file`, `function`, or `type`.
- `path`: location path.
- `line`: source line for function/type hotspots, otherwise `null`.
- `name`: function/type name, otherwise `null`.
- `priority`: 0 through 100 ranking score.
- `severity`: `info`, `warning`, or `critical`.
- `static_risk`: floating-point structural risk score from 0 through 100.
- `churn_risk`: floating-point git-churn risk score from 0 through 100.
- `reason`: short explanation of the ranking model and dominant risk.

The selected hotspot model converts those components into integer `priority`
from 0 through 100. Hotspots are retained when `priority >= 35` and sorted by
priority descending.
They are watchlist entries, not detector findings, and should not be treated as
hard CI gate failures by themselves.

## `findings`

Finding fields:

- `kind`: detector-specific finding kind.
- `id`: stable finding identifier in the form `rf1-<hex>`.
- `severity`: `info`, `warning`, or `critical`.
- `path`: primary path.
- `line`: primary line or `null`.
- `metrics`: finding-specific measurements.
- `construct`: primary ISO/IEC 25010-aligned maintainability construct.
- `mechanism`: primary source-observable maintenance mechanism.
- `issue_cluster_id`: owning issue cluster ID or `null`.
- `priority`: 0 through 100 refactoring priority.
- `confidence`: detector confidence from 0.0 through 1.0.
- `priority_factors`: scoring inputs.
- `rank_explanation`: short ranking explanation.
- `message`: human-readable summary.
- `recommendation`: concise refactoring hint computed from `kind`.
- `related_locations`: additional locations for grouped findings.

`metrics` entries contain:

- `name`
- `value`
- `threshold`
- `unit`
- `excess_ratio`
- `normalized`
- `percentile`

`construct` is one of `modularity`, `reusability`, `analysability`,
`modifiability`, or `testability`. `mechanism` is defined in the
[metric ontology](metric-ontology.md).

`priority_factors` contains:

- `impact`
- `intensity`
- `spread`
- `change_pressure`
- `actionability`
- `confidence`

`related_locations` entries contain:

- `path`
- `line`
- `name`

Very large `similar_functions` groups serialize at most 50 related locations
to keep reports bounded.

Finding IDs are deterministic for the same finding identity. The `rf1-` ID
uses the finding kind, normalized primary location, primary line, related
locations, and metric names. It intentionally does not include message text or
metric values, allowing baseline comparison to recognize an existing finding
whose priority or severity has changed.

## `issue_clusters`

Issue clusters contain `id`, `construct`, `mechanism`, `action`, `path`, `line`,
`primary_finding_id`, `finding_ids`, `kinds`, `priority`, and `severity`.
Only groups with at least two overlapping atomic findings are emitted. The
primary member is the highest-priority finding; member findings remain in
`findings` for baselines and detector-specific filtering.

## `detector_manifest`

Each entry contains `kind`, `construct`, `mechanism`, `action`, `entity_scope`,
`approach`, `supported_languages`, `precision_risk`, optional `parent_kind`,
and typed `relations`. A relation is either `facet_of` or
`alternative_evidence`; clustering consumes these relations instead of
inferring overlap from a shared mechanism alone. Consumers can distinguish
unsupported analysis from an observed absence of findings.

## `raw_metric_manifest`

Each entry contains a stable dotted `name`, `entity_scope`, `unit`, `scale`,
`direction`, and `description`. `higher_is_more_pressure` means larger values
may contribute to hotspot or finding intensity; `context_only` metrics remain
observable but do not independently vote for maintenance pressure. A metric
definition describes an observation, not a universal threshold or quality
grade.

`findings=0` means no unsuppressed findings were emitted. Consumers should
avoid presenting that as proof that the scanned code is healthy or bug-free.

## `suppression_summary`

Fields:

- `suppressed_count`: number of findings removed by suppressions.
- `suppressed_by_kind`: map of finding kind to suppressed count.
- `suppressed_by_severity`: map of severity to suppressed count.
- `highest_suppressed_priority`: highest suppressed finding priority, or
  `null` when no findings were suppressed.

Suppressions remove matching entries from `findings` before report emission
and CI gate selection. The suppression summary is report context, not a
finding: its purpose is to show that findings were intentionally removed and
whether an empty finding list means zero unsuppressed findings rather than zero
observed signals.

Schema version `13` does not serialize suppressed finding bodies in
`findings`. Consumers should render `suppression_summary` near
`summary.finding_count` and avoid counting suppressed findings as gate
failures.

## SARIF Output

`--output sarif` and `.sarif` output files emit SARIF version `2.1.0`.
The SARIF log contains one run with Reforge as the tool driver. Rules are keyed
by finding `kind`, and each result contains:

- `ruleId`: finding kind.
- `ruleIndex`: index into the run's rule table.
- `level`: `error` for `critical`, `warning` for `warning`, and `note` for
  `info`.
- `message.text`: finding message.
- `locations[].physicalLocation`: primary path and line.
- `relatedLocations`: related finding locations when present.
- `partialFingerprints.reforgeFindingId`: stable finding `id`.
- `properties.id`: stable finding `id`.
- `properties.recommendation`: concise refactoring hint.

## Finding Kinds

Current `kind` values:

- `large_file`
- `large_directory`
- `debt_marker`
- `similar_functions`
- `long_function`
- `complex_function`
- `deep_nesting`
- `many_parameters`
- `readability_risk`
- `large_type`
- `large_public_surface`
- `import_heavy_file`
- `function_proliferation`
- `unused_function`
- `repeated_literal`
- `repeated_error_pattern`
- `test_duplication`
- `happy_path_only_tests`
- `file_naming_drift`
- `directory_drift`
- `data_clump`
- `parallel_implementation`
- `shadowed_abstraction`
- `duplicate_type_shape`
- `config_key_drift`
- `fixture_factory_drift`
- `generic_bucket_drift`
- `adapter_boundary_bypass`
- `stale_compatibility_path`
- `missing_documentation_set`
- `missing_user_guide`
- `missing_report_schema_docs`
- `missing_metrics_model_docs`
- `missing_architecture_docs`
- `stale_cli_documentation`
- `stale_schema_documentation`
- `dependency_cycle`
- `dependency_hub`

## Compatibility Notes

Consumers should check `schema_version` before assuming field shape. Schema
version `14` adds finding constructs and mechanisms, issue clusters, detector
manifests, and `summary.issue_count`; it removes metric `dimension`. Schema
version `13` does not emit the legacy v4 fields `score`, `score_breakdown`, or
`rank_reason`; use `priority`, `priority_factors`, and `rank_explanation`
instead. Schema version `13` includes stable finding `id`, per-finding
`recommendation`, the `dependency_graph` snapshot, and `suppression_summary`.
Schema version `12` included stable finding IDs, recommendations, and
`dependency_graph`, but did not include suppression summary context. Schema
version `11` included stable finding IDs and recommendations, but did not
include `dependency_graph`. Reports without IDs should be regenerated before
being used as baselines.

New finding kinds may be added in future schema versions. Consumers should
handle unknown `kind` values gracefully when possible.
