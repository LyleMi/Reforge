# Codebase analysis

Codebase is Reforge's default analysis. It reviews repository structure and
produces evidence-backed findings for maintainers to inspect before deciding on
a refactor.

```sh
reforge analyze .
```

## What it examines

Codebase builds one project-wide index before applying any rule. That index
contains files, directories, declared functions and types, imports, local
dependencies, naming patterns, repeated syntax, test structure, and optional
Git history.

Rules use that shared view to look for four broad kinds of pressure:

| Area | Typical findings | Review question |
| --- | --- | --- |
| Responsibilities | Large files and types, long or complex functions, broad directories | Does this unit own more than one reason to change? |
| Duplication | Similar functions, repeated literals, repeated setup, overlapping type shapes | Is the repetition intentional, or is a shared concept missing? |
| Architecture | Dependency cycles and hubs, parallel implementations, boundary bypasses | Is ownership clear, and do dependencies point in the intended direction? |
| Consistency | Naming drift, generic buckets, stale compatibility paths, debt markers | Has a temporary or local convention spread beyond its original purpose? |

The complete list is in the [Rule Reference](rule-cards.md).

## Enable the rules you want to review

Rules start as opt-in previews. Running Codebase still records its coverage, but
a rule produces findings only after it is enabled in `reforge.toml`:

```toml
version = 2

[analysis]
enabled = ["codebase"]

[rules]
enable = [
  "reforge.codebase.large_file",
  "reforge.codebase.long_function",
  "reforge.codebase.dependency_cycle",
  "reforge.codebase.similar_functions",
]

[codebase]
max-file-lines = 600
max-function-lines = 80
```

Start with a small set whose meaning is easy to review in your repository.
Adjust a threshold when the evidence is consistently too broad or too narrow;
do not tune it merely to force a clean report.

## Read a finding

A finding is the unit to review. It names one file, symbol, repository, or
related group and contains one or more Evidence records. Evidence answers three
questions:

1. Which rule made the observation?
2. Where in the source was it observed?
3. Which measurement crossed the configured threshold?

Legitimate exceptions are expected. Generated facades, protocol signatures,
composition roots, test builders, and deliberate compatibility layers can all
look unusual for good reasons. Keep those decisions visible with a documented
suppression instead of weakening a useful rule globally.

## Check Coverage before trusting an empty result

Coverage records the files and languages seen by Codebase, the rules that ran,
and any limitations. An empty findings list means only that the enabled rules
found nothing within the observed surface. It does not prove that the codebase
is healthy or defect-free.

## Generate a report

Use the terminal output for quick review or create a standalone HTML file for a
larger repository:

```sh
reforge analyze .
reforge analyze . --output html --output-file reforge-report.html
reforge analyze . --output json --output-file reforge-report.json --reproducible
```

JSON is the appropriate format for reviewed baselines and CI. See the
[User Guide](user-guide.md#baselines-and-ci-gates) for the baseline workflow and
[Configuration](configuration.md) for scope, thresholds, and suppressions.

## Advanced value-path analysis

Codebase is sufficient for normal structural review. Reforge also offers an
opt-in [Dataflow analysis](dataflow.md) for teams that need conservative,
source-to-sink value paths or explicit adapter-boundary policies.
