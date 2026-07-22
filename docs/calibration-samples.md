# Maintainer Calibration Samples

The previous July 2026 calibration notes were produced with the pre-schema 21
priority, severity, hotspot, and scoring-policy model. Those aggregate reports
are no longer compatible with the current CLI or serialized contract and are
not a normative basis for current behavior. Raw reports and source identities
were never committed, so the old result tables cannot be regenerated or
audited from this repository.

Schema 23 calibration must start from new reports. Historical observations may
still motivate which repository shapes to include, but historical ranks,
severity bands, hotspot counts, and policy fits must not be compared with
current issue/finding output.

## Current Collection Protocol

For each frozen source snapshot, record the Reforge commit/version, target
revision, configuration fingerprint, command, duration, schema version,
coverage summary, suppressions, and parse/unresolved-dependency context.

Use source-only JSON for a reproducible detector pass:

```bash
reforge scan <sample> --output json --output-file <artifact>.json --progress never --churn off --reproducible
```

Run a separate `--churn auto` pass when repository-history behavior is part of
the research question. Do not mix unavailable churn with observed zero churn.

The sample set should include:

- small libraries and services with expected low finding volume;
- large CLI/TUI orchestration repositories;
- TypeScript/JavaScript monorepos;
- test-heavy Python or framework repositories;
- repositories with dependency cycles and unresolved alias/framework imports;
- mixed-language repositories and controlled parse failures;
- representative Unity projects with text and binary serialization states;
- generated-code-heavy projects to verify default exclusions;
- a holdout group not used while tuning thresholds.

## Labels

Keep label questions independent:

- Instrumentation: is the raw observation correct?
- Detection: does the finding describe the source evidence it claims?
- Action: is the recommendation suitable for this repository and location?
- Clustering: does the issue group compatible evidence without hiding a
  materially separate decision?
- Coverage: are unsupported, partial, unavailable, and excluded surfaces
  represented accurately?
- Agent context: do closure and test-reachability fields include the files a
  maintainer actually needed to inspect?
- Outcome: after an accepted refactor, did declared tests pass and did the
  selected evidence remain, disappear, become unobservable, or change identity?

Schema 23 does not have a ranking-gold dataset because it does not emit a
priority order. If a downstream consumer wants ranking, that policy and its
validation data must remain external to the Reforge evidence contract.

## Required Aggregate Report

For each anonymous sample, publish at least:

- source files, directories, functions, types, test files, and duration;
- findings and issues by detector kind/family;
- suppressions by kind;
- metric percentile summaries;
- coverage cell status, detector execution receipts, parse failures, and
  unobservable reasons;
- unresolved dependency edges;
- agent-evidence closure/test-reachability distributions;
- maintainer label counts and agreement, without source-identifying paths.

Do not publish a single health grade. Detector volume is strongly affected by
repository shape and configured thresholds.

## Historical Lessons Worth Revalidating

Earlier anonymous passes suggested several useful hypotheses:

- File and function sizes have long tails, so absolute thresholds must remain
  configurable.
- Parameter counts and test-duplication volume vary sharply by project shape.
- Similarity cost and output vary with language mix and minimum token/group
  settings.
- Framework metadata and language syntax can create instrumentation errors
  that should be fixed before threshold tuning.
- Dependency analysis must operate on condensed strongly connected components
  rather than enumerate paths in cyclic graphs.
- Unity thresholds require Unity-specific samples and should remain operational
  heuristics until those samples exist.

These are hypotheses for a new schema 23 run, not current benchmark results.

## Acceptance Before Policy Changes

Before changing built-in presets or detector defaults:

1. Confirm instrumentation on focused fixtures.
2. Review a stratified finding sample with maintainers.
3. Check issue clustering and coverage receipts separately.
4. Propose a change from repeatable noise or missed evidence, not from a desire
   to reach zero findings.
5. Validate the change on the holdout repositories.
6. Record any language or repository-shape regression.
7. Keep any agent-workflow evaluation separate from detector calibration.

New aggregate results should be appended only when their source revisions and
schema 23 artifacts are reproducible from retained metadata.
