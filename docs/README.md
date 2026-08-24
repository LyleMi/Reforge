<div class="docs-hero">
  <img src="assets/reforge-logo.png" alt="" class="docs-hero-logo">
  <div class="docs-eyebrow">Structural analysis for real codebases</div>
  <h1>See what deserves a refactor.<br><span>See the evidence first.</span></h1>
  <p>Reforge finds structural risks in human- and agent-written code, explains every finding, and makes gaps in analysis visible.</p>
  <div class="docs-actions">
    <a class="primary" href="user-guide.html">Get started</a>
    <a class="secondary" href="sample/">Explore a live report <span aria-hidden="true">→</span></a>
  </div>
</div>

<div class="quick-command" aria-label="Quick start command">
  <code>reforge analyze . --output html --output-file reforge-report.html</code>
</div>

## From signal to decision

Reforge does not hide structural observations behind a score. It gives reviewers the context needed to decide whether a change is worthwhile.

<div class="feature-grid">
  <div class="feature-card findings">
    <span class="feature-number">01</span>
    <h3>Find the pressure points</h3>
    <p>Surface duplication, oversized responsibilities, dependency tangles, naming drift, and difficult value paths.</p>
  </div>
  <div class="feature-card evidence">
    <span class="feature-number">02</span>
    <h3>Inspect the evidence</h3>
    <p>Trace each finding back to its rule, measurements, source locations, and exact value-flow witness when available.</p>
  </div>
  <div class="feature-card coverage">
    <span class="feature-number">03</span>
    <h3>Know the limits</h3>
    <p>See partial and unsupported analysis explicitly. No findings never means more coverage than Reforge actually observed.</p>
  </div>
</div>

## Choose the right analysis

<div class="analysis-grid">
  <div class="analysis-card">
    <div class="analysis-label">Default</div>
    <h3>Codebase</h3>
    <p>Review structure across files, functions, types, dependencies, naming, duplication, tests, and repository history.</p>
    <a href="analyses.html">Learn about Codebase →</a>
  </div>
  <div class="analysis-card">
    <div class="analysis-label">Opt in</div>
    <h3>Dataflow</h3>
    <p>Follow conservative value paths to inspect relay depth, fan-out, and configured boundary policies.</p>
    <a href="dataflow.html">Learn about Dataflow →</a>
  </div>
</div>

## Designed for review, not scoring

Findings are inspection prompts—not severity labels, priorities, or defect predictions. Reforge runs locally, uploads no source code, and collects no telemetry. Use it interactively, generate a standalone HTML report, or compare reviewed JSON baselines in CI.

<div class="next-links">
  <a href="user-guide.html"><strong>Install and run</strong><span>From first analysis to a readable report</span></a>
  <a href="configuration.html"><strong>Configure Reforge</strong><span>Scope, rules, thresholds, and policies</span></a>
  <a href="rule-cards.html"><strong>Browse the rules</strong><span>Claims, capabilities, and exceptions</span></a>
</div>
