<div class="docs-masthead">
  <img src="assets/reforge-logo.png" alt="Reforge logo">
  <div>
    <h1>Reforge</h1>
    <p>A source-tree scanner for maintainability evidence, refactoring issues, and codebase drift.</p>
    <div class="docs-actions">
      <a href="user-guide.html">Start scanning</a>
      <a href="sample/index.html">Open the sample report</a>
      <a href="https://github.com/LyleMi/Reforge">View source</a>
    </div>
  </div>
</div>

Reforge collects directory, file, function, type, dependency, and optional git churn
metrics before deriving issues, findings, coverage, and agent evidence. It is designed for local audits,
CI gates, and evidence-led refactoring work. It is not a quality score, bug
detector, or defect probability model.

<div class="signal-key">
  <div class="findings">
    <strong>Findings</strong>
    <span>Threshold and detector signals that merit review.</span>
  </div>
  <div class="coverage">
    <strong>Coverage</strong>
    <span>Execution receipts and explicit observed, partial, or unavailable analysis.</span>
  </div>
  <div class="metrics">
    <strong>Metrics</strong>
    <span>Raw measurements and project percentile context.</span>
  </div>
</div>

## Quick Start

Reforge requires Rust 1.85 or newer.

```powershell
cargo build --release
cargo run -- scan .
```

Generate the same self-contained visual report published as this site's sample:

```powershell
cargo run -- scan . --output-file reforge-report.html --progress never
```

## Documentation Map

### Use Reforge

- [User Guide](user-guide.md): install the CLI, run scans, choose output, and
  troubleshoot common failures.
- [Configuration](configuration.md): configure thresholds, exclusions,
  suppressions, churn, exact Rust adapter-flow policies, and precedence.
- [Report Schema](report-schema.md): consume JSON/YAML schema 22 and SARIF 2.1.0.
- [HTML Report](report-app.md): build and use the self-contained visual report.

### Understand Results

- [Metrics Model](metrics-model.md): interpret raw metrics, percentiles,
  findings, issues, coverage, and agent evidence.
- [Detector Reference](detectors.md): review every detector family and its
  thresholds or heuristics.
- [Calibration Samples](calibration-samples.md): inspect the sample set used to
  sanity-check report volume.

### Maintain Reforge

- [Architecture](architecture.md): follow the scan pipeline, module boundaries,
  and extension points.
- [Agent Workflow Research](agent-workflows.md): review the current agent
  integration boundary, reference-workflow findings, and phased implementation
  plan for safe, resumable refactoring assistance.
- [Data-flow Signal Research](research/data-flow-signals/report.md): review the
  evidence, false-positive boundaries, and candidate cross-module flow signals.
- [Data-flow Execution Plan](research/data-flow-signals/execution-plan.md):
  inspect the temporary, gated implementation RFC before any source work begins.
- [Contributing](contributing.md): set up local development and validate changes.
- [Release](release.md): prepare and package a release.

## Supported Languages

Tree-sitter structural and similar-function analysis covers Rust, JavaScript,
TypeScript/TSX, Vue SFC script blocks, Python, Go, Java, C#, Kotlin, PHP, and
Ruby, plus Bash (`.sh`, `.bash`) and PowerShell (`.ps1`, `.psm1`) scripts.
Basic source-tree metrics also include supported C and C++ source extensions.
