# Reforge

Reforge turns source analysis into a compact chain:

`analyze → issue → evidence → measurement → coverage`

It is designed for local audits and CI gates. It does not emit a health score,
severity, priority, or defect prediction.

## Quick start

Reforge requires Rust 1.85 or newer.

```sh
cargo run -p reforge -- analyze .
cargo run -p reforge -- analyze . --output html --output-file reforge-report.html
cargo run -p reforge -- rules
```

With no `--analysis`, Codebase runs alone. Select Dataflow explicitly, or use
`--analysis codebase --analysis dataflow` to run both over one workspace index.

## Core documentation

- [User Guide](user-guide.md)
- [Analyses](analyses.md)
- [Dataflow](dataflow.md)
- [Configuration](configuration.md)
- [Report Schema](report-schema.md)
- [Upgrading from 0.1 to 0.2](upgrading-to-0.2.md)
- [HTML Report](report-app.md)
- [Architecture](architecture.md)
- [Contributing](contributing.md)
- [Release](release.md)

Codebase supports Rust, JavaScript, TypeScript/TSX, Vue SFC script blocks,
Python, Go, Java, C#, Kotlin, PHP, Ruby, Bash, and PowerShell. Dataflow currently
builds exact conservative paths for Rust, JavaScript/TypeScript/TSX, and Python.
