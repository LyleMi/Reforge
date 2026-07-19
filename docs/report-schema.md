# Report Schema

Schema 21 separates atomic evidence (`findings`) from decision units (`issues`)
and adds `agent_evidence` for context closure and test reachability. It retains
measurement, detector-execution, coverage, dependency, suppression, and Unity
provenance without serializing a priority, severity, hotspot, scoring-policy,
or readiness model.

JSON and YAML reports use schema version `21`. Older reports and baselines,
including v20, are rejected and must be regenerated. The same Rust data model
is serialized for both formats. SARIF output is a separate SARIF 2.1.0 document
whose results represent issue decision units.

## Top-Level Shape

```json
{
  "schema_version": 21,
  "summary": {},
  "stats": {},
  "metrics_summary": {},
  "raw_metrics": {},
  "raw_metric_manifest": [],
  "dependency_graph": {},
  "agent_evidence": {},
  "unity_project": {},
  "suppression_summary": {},
  "coverage_manifest": [],
  "coverage_summary": {},
  "detector_execution": [],
  "raw_metric_coverage": [],
  "issues": [],
  "detector_manifest": [],
  "findings": []
}
```

Top-level fields:

- `schema_version`: report schema version. Current value is `21`.
- `coverage_manifest`: normative 7 mechanisms × 6 entity scopes matrix with expectation, runtime status, detectors, entity count, and unobservable reasons.
- `detector_execution`: one execution receipt per detector, including completed zero-finding runs.
- `raw_metric_coverage`: observation state for all 18 canonical raw metrics. Disabled churn is `unavailable`, never observed zero pressure.
- `summary`: scan totals, duration, and churn status.
- `stats`: source files, directories, and function candidates counted.
- `metrics_summary`: percentile distributions for raw metrics.
- `raw_metrics`: directory, file, function, type, and churn measurements.
- `raw_metric_manifest`: scale, unit, scope, direction, and meaning of every
  raw metric family.
- `dependency_graph`: resolved source-file dependency graph snapshot.
- `agent_evidence`: context closure, unresolved local dependency, evidence
  dispersion, and test-reachability evidence for files and issues.
- `unity_project`: Unity detection status, Editor/serialization metadata, asset statistics, asmdef graph, problem references, and Unity-specific coverage.
- `suppression_summary`: aggregate counts for findings removed by
  suppressions.
- `issues`: compatible atomic evidence grouped into stable human-facing
  refactoring issues.
- `detector_manifest`: coverage and classification metadata for every finding
  kind.
- `findings`: atomic detector evidence with metrics and related locations.

Reports contain maintainability and refactoring signals. They are not a
quality score, health score, bug detector, or defect probability model.
`findings` is the post-filter, post-suppression list; an empty list does not
mean raw metrics or suppressed signals were absent.

## `summary`

Fields:

- `scanned_files`: number of source files scanned.
- `finding_count`: number of findings emitted after filters and suppressions.
- `issue_count`: findings after clustered secondary facets are counted once.
- `similar_function_group_count`: number of similar-function findings.
- `duration_ms`: scan duration in milliseconds.
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

`metrics_summary` contains maps for `directories`, `files`, `functions`,
`types`, and `churn`. Each metric has:

- `p50`
- `p75`
- `p90`
- `p95`
- `max`

Directory metrics include `source_files`. Each directory contributes exactly
one observation, independent of the number of files it contains.

File metrics include `loc`, `imports`, and `public_items`.

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
- `is_test`
- `churn`

`raw_metrics.directories` entries:

- `path`
- `source_files`

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

## `agent_evidence`

`agent_evidence.files` records one context entry per relevant source file:

- `path`
- `coverage_status`: `observed`, `partial`, `unsupported`, or
  `not_applicable`.
- `context_closure_files`
- `context_closure_loc`
- `unresolved_local_dependencies`
- `direct_test_files`
- `reachable_test_files`
- `reachable_test_file_count`
- `nearest_test_distance`
- `nearest_test_paths`
- `paths_truncated`

`agent_evidence.issues` uses the same closure and test-reachability fields and
adds:

- `issue_id`: stable `ri3-...` issue identity.
- `evidence_dispersion.evidence_files`
- `evidence_dispersion.evidence_directories`
- `evidence_dispersion.evidence_languages`

These fields describe the likely context and verification surface. They do not
claim that a change is safe. `partial` means the dependency closure is
incomplete, while `paths_truncated` means only a bounded set of representative
test paths was serialized.

## `unity_project`

- `status` is `not_detected`, `disabled`, `observed`, or `partially_observed`.
- `assemblies` and `assembly_edges` form an independent Unity assembly graph; they do not replace the source-file `dependency_graph`.
- `problem_references` contains only broken or missing-script references, keeping normal asset graphs out of large reports.
- `coverage` and `degraded_reasons` explain unavailable PackageCache identity data or binary serialization.

## `findings`

Finding fields:

- `kind`: detector-specific finding kind.
- `id`: stable evidence identifier in the form `rf3-<hex>`.
- `path`: primary path.
- `line`: primary line or `null`.
- `metrics`: finding-specific measurements.
- `construct`: primary ISO/IEC 25010-aligned maintainability construct.
- `mechanism`: primary source-observable maintenance mechanism.
- `issue_id`: owning issue ID or `null`.
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

`related_locations` entries contain:

- `path`
- `line`
- `name`

Very large `similar_functions` groups serialize at most 50 related locations
to keep reports bounded.

Finding IDs are deterministic for the same evidence identity. The `rf3-` ID
uses the finding kind, metric names, and the normalized, sorted, deduplicated
set of primary and related path/line locations. It intentionally does not
include the representative location choice, related-location order, names,
message text, or metric values. Baseline comparison therefore recognizes the
same evidence group when detector traversal order or ranking changes.

## `issues`

Issues contain `id`, `family`, `summary`, `construct`, `mechanism`, `action`,
`path`, `line`, `primary_finding_id`, `finding_ids`, `kinds`, and `subject`.
Finding `id` values are stable `EvidenceId` values (`rf3-...`). Issue `id`
values are stable `IssueKey` values (`ri3-...`) derived only from the issue
family and canonical subject, not from evidence membership or input order.
Every compatible atomic evidence group emits an issue. Member findings remain
in `findings` for baselines and detector-specific filtering.

## `detector_manifest`

Each entry contains `kind`, `construct`, `mechanism`, `action`, `entity_scope`,
`approach`, `supported_languages`, `precision_risk`, typed `input_metrics`,
`issue_family`, `evidence_role`, and `constituent_kinds`. Consumers can
distinguish unsupported analysis from an observed absence of findings.

## `raw_metric_manifest`

Each entry contains a stable dotted `name`, `entity_scope`, `unit`, `scale`,
`direction`, and `description`. `higher_is_more_pressure` means larger values
may contribute to detector evidence; `context_only` metrics remain
observable but do not independently vote for maintenance pressure. A metric
definition describes an observation, not a universal threshold or quality
grade.

`findings=0` means no unsuppressed findings were emitted. Consumers should
avoid presenting that as proof that the scanned code is healthy or bug-free.

## `suppression_summary`

Fields:

- `suppressed_count`: number of findings removed by suppressions.
- `suppressed_by_kind`: map of finding kind to suppressed count.

Suppressions remove matching entries from `findings` before report emission
and CI gate selection. The suppression summary is report context, not a
finding: its purpose is to show that findings were intentionally removed and
whether an empty finding list means zero unsuppressed findings rather than zero
observed signals.

Suppressed finding bodies are not serialized in `findings`. Consumers should
render `suppression_summary` near
`summary.finding_count` and avoid counting suppressed findings as gate
failures.

## SARIF Output

`--output sarif` and `.sarif` output files emit SARIF version `2.1.0`.
The SARIF log contains one run with Reforge as the tool driver. Rules are keyed
by issue `family`, and each issue result contains:

- `ruleId`: issue family.
- `ruleIndex`: index into the run's rule table.
- `level`: `note`; schema 21 does not assign severity.
- `message.text`: issue summary.
- `locations[].physicalLocation`: primary path and line.
- `partialFingerprints.reforgeIssueId`: stable issue `id`.
- `properties.id`: stable issue `id`.
- `properties.family`, `construct`, `mechanism`, and `action`.
- `properties.evidence_ids`: member finding IDs.

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
- `unity_assembly_cycle`, `unity_assembly_hub`, `unity_unresolved_assembly_reference`, `unity_runtime_editor_dependency`
- `unity_duplicate_guid`, `unity_missing_meta`, `unity_orphan_meta`, `unity_broken_asset_reference`, `unity_missing_script`
- `unity_non_text_serialization`, `unity_scene_build_drift`, `unity_large_scene`, `unity_large_prefab`
- `unity_serialized_field_bloat`, `unity_lifecycle_overload`, `unity_expensive_frame_call`, `unity_editor_api_in_runtime`, `unity_unbalanced_event_subscription`

## Compatibility Notes

Consumers should check `schema_version` before assuming field shape. Schema
version `21` adds `agent_evidence` and removes serialized priority, severity,
hotspots, scoring policy, and reliability fields. It also changes baseline
gating to stable-ID selection and SARIF output to issue decision units.
In particular, schema 21 no longer emits `detection_reliability`,
`interpretation_reliability`, `priority_factors`, or `rank_explanation` on
findings; consumers must not infer replacements for those removed fields.
Baselines from older schemas are rejected and should be regenerated. Schema
version `20` added `unity_project`, Unity detector records, and Unity asset
paths without changing the source dependency graph. Schema version `19`
retains `rf3-`
EvidenceIds and `ri3-` IssueKeys over canonical
subjects. Issue identity is independent of alternative evidence membership and
input ordering. Schema
version `16` gives every finding metric a canonical dotted ID, adds directory
raw metrics and percentile summaries, and removes repeated parent-directory
counts from file raw metrics. Schema
version `14` adds finding constructs and mechanisms, issue clusters, detector
manifests, and `summary.issue_count`; it removes metric `dimension`. Schema
version `13` stopped emitting the legacy v4 fields `score`, `score_breakdown`,
and `rank_reason`. Schema version `13` includes stable finding `id`, per-finding
`recommendation`, the `dependency_graph` snapshot, and `suppression_summary`.
Schema version `12` included stable finding IDs, recommendations, and
`dependency_graph`, but did not include suppression summary context. Schema
version `11` included stable finding IDs and recommendations, but did not
include `dependency_graph`. Reports without IDs should be regenerated before
being used as baselines.

New finding kinds may be added in future schema versions. Consumers should
handle unknown `kind` values gracefully when possible.
