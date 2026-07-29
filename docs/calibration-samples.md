# Calibration samples

Schema 27 calibration uses the frozen 15-repository manifest in
`calibration/corpus.toml` and the offline protocol in
`calibration/README.md`. Exact commits, commands, language, and license are
committed. Third-party source trees and complete reports are not committed;
complete reports are retained as CI calibration artifacts.

The earlier five-site Dataflow observations are historical regression context
only. They do not satisfy the current minimum of 40 candidates across five
repositories, 20 quiet sites across three repositories, repository
concentration limits, Wilson lower bounds, two-reviewer agreement, fixture
recall, and unsupported-coverage honesty. They therefore cannot justify stable
maturity or a default rule.

`reforge-calibrate` generates anonymous packets, validates reviewer JSON, and
produces per-rule/per-language summaries. Reviewers cannot see maturity,
promotion targets, thresholds, or the other reviewer's labels. Stored
provenance names reviewer type, model, version, prompt digest, timestamp, and
adjudication.

No retained rule currently has an audited summary meeting every promotion
gate, so every core rule remains preview/off. Self-analysis is regression data,
not promotion evidence or a threshold-selection sample.

Before using a retained summary for promotion, validate the frozen input and
report set:

```sh
reforge-calibrate corpus validate --manifest calibration/corpus.toml
reforge-calibrate corpus matrix --manifest calibration/corpus.toml
reforge-calibrate verify-reports --help
reforge-calibrate verify-promotion --corpus calibration/corpus.toml
```

The last command succeeds with `promotion_candidates: 0` while every registry
entry remains preview/off. Once a rule is proposed as stable or default, the
same command requires complete audit manifests and an eligible summary for
each promoted rule/language pair.
