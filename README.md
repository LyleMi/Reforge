<p align="center">
  <img src="assets/reforge-logo.png" alt="Reforge" width="144">
</p>

<h1 align="center">Reforge</h1>

<p align="center">
  Evidence-backed structural analysis for codebases changing faster than they can be reviewed.
</p>

<p align="center">
  <a href="https://github.com/LyleMi/Reforge/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/LyleMi/Reforge/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/LyleMi/Reforge/releases/latest"><img alt="Release" src="https://img.shields.io/github/v/release/LyleMi/Reforge"></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/github/license/LyleMi/Reforge"></a>
  <a href="https://lylemi.github.io/Reforge/"><img alt="Documentation" src="https://img.shields.io/badge/docs-read-1556ad"></a>
</p>

Reforge is a local CLI that shows maintainers where a codebase is becoming
harder to change. It finds duplicated implementations, oversized
responsibilities, dependency tangles, and architecture drift across the whole
repository—including patterns introduced gradually by coding agents.

It does not assign an opaque health score or ask you to trust a generated
summary. Every finding includes the source locations, measurements, and rule
that produced it. Every report also records what Reforge could and could not
analyze.

<p align="center">
  <a href="https://lylemi.github.io/Reforge/playground/"><strong>Try the bilingual Playground / 在线体验 →</strong></a>
  &nbsp;·&nbsp;
  <a href="https://lylemi.github.io/Reforge/sample/"><strong>Explore Reforge's self-analysis report →</strong></a>
</p>

## See the evidence behind a finding

This finding came from Reforge analyzing its own report application:

```text
Function readability: ReportView in web/report-app/src/reportApp.tsx

  Rule:        reforge.codebase.complex_function
  Location:    web/report-app/src/reportApp.tsx:33
  Measurement: estimated complexity 15 (threshold 14)
  Guidance:    Reduce the function to a clear sequence of named responsibilities.
```

The location makes the finding inspectable. The measurement explains why it
was reported. The threshold can be tuned, and a legitimate exception can be
suppressed with its reason preserved. Reforge makes the case for review; it
does not pretend that a measurement can decide the refactor for you.

## Get started

Install the latest release on Linux or macOS:

```sh
curl -fsSL https://raw.githubusercontent.com/LyleMi/Reforge/main/scripts/install.sh | sh
```

On Windows PowerShell:

```powershell
$installer = Join-Path $env:TEMP "install-reforge.ps1"
irm https://raw.githubusercontent.com/LyleMi/Reforge/main/scripts/install.ps1 -OutFile $installer
& $installer
```

Rust users can alternatively build and install the command from crates.io:

```sh
cargo install reforge-cli --locked
```

The crates.io package installs the `reforge` binary only. Use the verified
release installer above when you also want the bundled `reforge-analyze` Codex
skill.

Reforge runs Codebase analysis by default. Its rules begin as opt-in previews,
so adopting Reforge does not immediately impose someone else's definition of
maintainability. Initialize a configuration, then enable a small starter set:

```sh
reforge init
```

In the generated `reforge.toml`, start with:

```toml
[rules]
enable = [
  "reforge.codebase.large_file",
  "reforge.codebase.long_function",
  "reforge.codebase.dependency_cycle",
  "reforge.codebase.similar_functions",
]
```

Run the review in your repository or generate a standalone HTML report:

```sh
reforge analyze .
reforge analyze . --output html --output-file reforge-report.html
```

## Built for repository-level review

| Need | What Reforge provides |
| --- | --- |
| Find change pressure beyond style errors | Project-wide signals for responsibilities, duplication, dependencies, and drift |
| Verify why something was flagged | Source locations, measurements, thresholds, and rule provenance |
| Trust an empty report appropriately | Coverage receipts and explicit analysis limitations |
| Keep an accepted refactor from regressing | Reproducible baselines and CI gates for new or changed findings |
| Keep source code private | Local analysis with no uploads or telemetry |

Reforge complements compilers, linters, and security scanners. Its job is not
to prove correctness or find vulnerabilities; it identifies structural
pressure that deserves a maintainer's judgment before the next refactor.

## What Codebase finds

| Area | Examples |
| --- | --- |
| **Responsibilities** | Large files, long or complex functions, deep nesting, broad public surfaces |
| **Duplication** | Similar functions, repeated literals, repeated test setup, overlapping type shapes |
| **Architecture drift** | Dependency cycles, generic buckets, parallel implementations, boundary bypasses |
| **Repository consistency** | Naming drift, stale compatibility paths, TODO/FIXME clusters |

Each finding points to a concrete subject and includes the rule, source locations, and measurements that produced it. Coverage shows which languages and capabilities were actually observed. Reforge does not turn these signals into a health score, severity, or defect prediction—the decision stays with the reviewer.

Codebase supports Rust, JavaScript, TypeScript/TSX, Vue, Python, Go, Java, C#, Kotlin, PHP, Ruby, Bash, and PowerShell. Dependency rules also recognize C and C++.

Analysis runs locally. Reforge does not upload source code or collect telemetry.

## Use it in CI

Keep configuration in `reforge.toml`, review a JSON report as a baseline, then gate new or changed policy findings:

```sh
reforge analyze . --output json --output-file current.json \
  --baseline reforge-baseline.json --gate new --reproducible
```

Rules begin as opt-in previews. This keeps adoption deliberate: enable the signals that fit your codebase, review their evidence, and enforce only the policies your team has accepted.

## Learn more

- [Documentation](https://lylemi.github.io/Reforge/) — start here for installation, configuration, and report interpretation
- [Codebase guide](docs/analyses.md) — understand what is analyzed and how to review findings
- [Rule reference](docs/rule-cards.md) — see every available signal and its intended limits
- [Configuration reference](docs/configuration.md) — tune scope, thresholds, policies, and suppressions
- [Contributing](docs/contributing.md) — build and test Reforge locally

Reforge also includes an advanced, opt-in [Dataflow analysis](docs/dataflow.md) for exact value-path and boundary-policy inspection.

## Development

Reforge is a Rust 2024 workspace. Run the full validation suite with:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
```
