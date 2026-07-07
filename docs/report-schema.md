# Report Schema

JSON and YAML reports use schema version `9`. The same Rust data model is
serialized for both formats. SARIF output is a separate SARIF 2.1.0 document
that carries the same finding IDs in result fingerprints.

## Top-Level Shape

```json
{
  "schema_version": 9,
  "summary": {},
  "stats": {},
  "metrics_summary": {},
  "raw_metrics": {},
  "hotspots": [],
  "findings": []
}
```

Top-level fields:

- `schema_version`: report schema version. Current value is `9`.
- `summary`: scan totals, duration, hotspot model, and churn status.
- `stats`: source files, directories, and function candidates counted.
- `metrics_summary`: percentile distributions for raw metrics.
- `raw_metrics`: file, function, type, and churn measurements.
- `hotspots`: ranked file, function, and type locations.
- `findings`: detector findings with priority, confidence, metrics, and
  related locations.

## `summary`

Fields:

- `scanned_files`: number of source files scanned.
- `finding_count`: number of findings emitted.
- `hotspot_count`: number of hotspots emitted.
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

## `hotspots`

Hotspot fields:

- `level`: `file`, `function`, or `type`.
- `path`: location path.
- `line`: source line for function/type hotspots, otherwise `null`.
- `name`: function/type name, otherwise `null`.
- `priority`: 0 through 100 ranking score.
- `severity`: `info`, `warning`, or `critical`.
- `static_risk`: static risk score.
- `churn_risk`: churn risk score.
- `reason`: short explanation of the ranking model and dominant risk.

Hotspots are retained when `priority >= 35` and sorted by priority descending.

## `findings`

Finding fields:

- `kind`: detector-specific finding kind.
- `id`: stable finding identifier in the form `rf1-<hex>`.
- `severity`: `info`, `warning`, or `critical`.
- `path`: primary path.
- `line`: primary line or `null`.
- `metrics`: finding-specific measurements.
- `priority`: 0 through 100 refactoring priority.
- `confidence`: detector confidence from 0.0 through 1.0.
- `priority_factors`: scoring inputs.
- `rank_explanation`: short ranking explanation.
- `message`: human-readable summary.
- `related_locations`: additional locations for grouped findings.

`metrics` entries contain:

- `name`
- `value`
- `threshold`
- `unit`
- `excess_ratio`
- `dimension`
- `normalized`
- `percentile`

`dimension` is one of `size`, `complexity`, `coupling`, `duplication`,
`drift`, `test_risk`, or `documentation`.

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

## Compatibility Notes

Consumers should check `schema_version` before assuming field shape. Schema
version `9` does not emit the legacy v4 fields `score`, `score_breakdown`, or
`rank_reason`; use `priority`, `priority_factors`, and `rank_explanation`
instead. Schema version `9` adds stable finding `id`; reports without IDs
should be regenerated before being used as baselines.

New finding kinds may be added in future schema versions. Consumers should
handle unknown `kind` values gracefully when possible.
