# Maintainer Calibration Samples

## Dataflow GA calibration (schema 26, CLI 0.2.0)

Stable thresholds were registered before judgment through Dataflow configuration:
`min-function-hops = 2`, `min-sinks = 3`, `min-relay-percent = 90`, and
`max-path-steps = 12`. Only complete exact-edge paths can emit. The review unit
is a unique `(rule, source function)` site so multiple parameters from one
function do not inflate the denominator. “Worth checking” means the witness
exposes an ownership/propagation decision a maintainer can explain; it does not
require a code change.

| Project / revision | Family | Functions | Exact / unresolved edges | Findings | Review sites | Worth checking |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| BurntSushi/walkdir `6fd031c82ba5a4204b4ce6eae73dacb00dc072ec` | Rust | 60 | 96 / 648 | 0 | 0 | n/a |
| pallets/itsdangerous `672971d66a2ef9f85151e53283113f33d642dabd` | Python | 114 | 636 / 206 | 1 relay | 1 | 1 |
| sindresorhus/p-limit `df476048d023ff868cd45b35ee47f5fb0ca2b25a` | JS/TS | 10 | 129 / 40 | 1 relay | 1 | 1 |
| pallets/click `398f9154317f6c54bf98fe3359672ad5cb851585` | Python | 1195 | 8751 / 3004 | 2 fan-out | 1 | 1 |
| serde-rs/json `de8500740cdcabffb9734f503e4889def823cf10` | Rust | 311 | 1363 / 1619 | 3 relay | 2 | 2 |

All five unique sites were explainable and worth checking (100%, above the
registered 80% review-acceptance gate, which is distinct from the configured
90% relay threshold): signature input normalization, concurrency-wrapper
ownership, multi-backend stream options, Karatsuba forwarding, and
float-comparison propagation. Four were intentional designs and one was a
useful ownership review; calibration measures review value, not mandated
refactors. Unresolved dynamic/library behavior stayed in partial coverage and
never appeared inside an exact witness.

The checked-in DF-1 and DF-2 corpus has at least five positive and five negative microfixtures per rule. Positives cover Rust, JavaScript/TypeScript and Python with common measurements; negative, ambiguous, truncated, destructured and dynamic-dispatch cases emit no speculative stable Evidence. The revisions above and `reforge analyze PATH --analysis dataflow --reproducible` make the run repeatable. Raw reports remain local calibration artifacts and are not committed; the exact source revisions, thresholds, counts, judgment rule, and outcomes needed to reproduce the table are retained here.

## Analysis execution benchmark

The 0.2.0 release build was measured on the same checkout with five sequential reproducible JSON runs to `/dev/null`; values below are wall-clock medians. Codebase took 2.50 s, Dataflow 0.47 s, and combined took 2.64 s. Combined is 88.9% of the 2.97 s independent-analysis sum, meeting the registered 90% sharing gate. The umbrella CLI took 2.48 s for the same Codebase selection, so its path was 0.8% slower, well inside the registered 10% regression limit. Execution receipts also recorded one inventory walk and one parse per source/language for combined analysis.

The previous July 2026 calibration notes were produced with the pre-schema 21
priority, severity, hotspot, and scoring-policy model. Those aggregate reports
are no longer compatible with the current CLI or serialized contract and are
not a normative basis for current behavior. Raw reports and source identities
were never committed, so the old result tables cannot be regenerated or
audited from this repository.

Schema 26 calibration starts from new reports. Historical observations may
still motivate which repository shapes to include, but historical ranks,
severity bands, hotspot counts, and policy fits are not part of the current
Issue/Evidence contract.

## Current Collection Protocol

For each frozen source snapshot, record the Reforge commit/version, target
source revision, command, duration, schema version,
coverage summary, suppressions, and parse/unresolved-dependency context.

Use Codebase-only JSON for a reproducible detector pass:

```bash
reforge analyze <sample> --analysis codebase --output json --output-file <artifact>.json --reproducible --set codebase.churn=off
```

Run a separate `--churn auto` pass when repository-history behavior is part of
the research question. Do not mix unavailable churn with observed zero churn.

The sample set should include:

- small libraries and services with expected low Issue volume;
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
- Detection: does the Evidence describe the source observation it claims?
- Action: is the recommendation suitable for this repository and location?
- Clustering: does the issue group compatible evidence without hiding a
  materially separate decision?
- Coverage: are unsupported, partial, unavailable, and excluded surfaces
  represented accurately?
- Outcome: after an accepted refactor, did declared tests pass and did the
  selected evidence remain, disappear, become unobservable, or change identity?

Schema 26 does not emit a priority order. If a downstream consumer wants
ranking, that policy and its
validation data must remain external to the Reforge evidence contract.

## Required Aggregate Report

For each anonymous sample, publish at least:

- source files, directories, functions, types, test files, and duration;
- Issues and Evidence by rule/family;
- suppressions by rule;
- Coverage status, rule execution, parse failures, and limitations;
- unresolved dependency edges;
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

These are historical hypotheses to revalidate against schema 26, not current benchmark results.

## Acceptance Before Policy Changes

Before changing built-in presets or detector defaults:

1. Confirm instrumentation on focused fixtures.
2. Review a stratified Evidence sample with maintainers.
3. Check issue clustering and coverage receipts separately.
4. Propose a change from repeatable noise or missed evidence, not from a desire
   to reach zero Issues.
5. Validate the change on the holdout repositories.
6. Record any language or repository-shape regression.
7. Keep any agent-workflow evaluation separate from detector calibration.

New aggregate results should be appended only when their source revisions and
schema 26 commands/artifacts are reproducible from retained metadata.
