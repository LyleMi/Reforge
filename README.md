<p align="center">
  <img src="assets/reforge-logo.png" alt="Reforge" width="144">
</p>

<h1 align="center">Reforge</h1>

<p align="center">
  Find structural risks in a codebase, understand the evidence, and keep reviewed refactors from drifting back.
</p>

<p align="center">
  <a href="https://github.com/LyleMi/Reforge/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/LyleMi/Reforge/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/LyleMi/Reforge/releases/latest"><img alt="Release" src="https://img.shields.io/github/v/release/LyleMi/Reforge"></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/github/license/LyleMi/Reforge"></a>
  <a href="https://lylemi.github.io/Reforge/"><img alt="Documentation" src="https://img.shields.io/badge/docs-read-1556ad"></a>
</p>

Reforge is a local codebase analyzer for maintainers and coding agents. It surfaces refactoring opportunities such as duplicated implementations, oversized responsibilities, dependency tangles, and naming drift.

Every finding includes the source locations and measurements that produced it. Reports also say what could not be analyzed, so an empty result is never presented as stronger evidence than it is.

## Get started

Install the latest release on Linux or macOS:

```sh
curl -fsSL https://raw.githubusercontent.com/LyleMi/Reforge/main/scripts/install.sh | sh
reforge analyze .
```

On Windows PowerShell:

```powershell
$installer = Join-Path $env:TEMP "install-reforge.ps1"
irm https://raw.githubusercontent.com/LyleMi/Reforge/main/scripts/install.ps1 -OutFile $installer
& $installer
reforge analyze .
```

Reforge runs the Codebase analysis by default. Generate a standalone HTML report to explore the findings:

```sh
reforge analyze . --output html --output-file reforge-report.html
```

<p align="center">
  <a href="https://lylemi.github.io/Reforge/sample/"><strong>Explore an example Codebase report →</strong></a>
</p>

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
