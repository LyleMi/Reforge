# Cross-module abnormal data flow as a Reforge refactoring signal

> **Status:** Research evidence, not a committed detector specification.
> See the [temporary execution plan](execution-plan.md) for the gated implementation proposal.

## Table of contents

1. [Boundary policy escape](#boundary-policy-escape) - Confidence Tier: Experimental for inferred policy; conservative only when an explicit project policy names boundaries, sources/sinks, and allowed exceptions.
2. [Mutable state fan-out and cyclic journeys](#mutable-state-fan-out-and-cyclic-journeys) - Confidence Tier: conservative for explicit shared-state writer dispersion; experimental for cyclic journeys
3. [Ownership leakage and feature envy](#ownership-leakage-and-feature-envy) - Confidence Tier: experimental
4. [Pass-through chains and data clumps](#pass-through-chains-and-data-clumps) - Confidence Tier: experimental
5. [Reforge feasibility and validation design](#reforge-feasibility-and-validation-design) - Confidence Tier: Experimental detector tier; raw metrics and coverage receipts may ship before actionable findings. Promotion to heuristic requires labeled-corpus precision and usefulness targets; promotion to conservative requires exact-edge-only findings and stable cross-version validation.
6. [Representation churn and schema diffusion](#representation-churn-and-schema-diffusion) - Confidence Tier: experimental

## Boundary policy escape

### Conclusion

#### Item Name

Boundary policy escape

#### Thesis

Boundary policy escape is a useful architectural refactoring signal only when the repository provides independent evidence that a particular adapter, validator, sanitizer, authorizer, or redaction component is the intended crossing point. A single source-to-sink path that lacks a policy step is primarily a security or correctness warning; repeated or newly introduced paths that route equivalent data around an established boundary are maintainability evidence because policy knowledge and change responsibility have become dispersed.

#### Refactoring Relevance

The behavior-preserving structural opportunity is to route callers through the existing port or adapter, extract a shared boundary interface, or move equivalent validation/redaction logic into one owned crossing point without changing accepted inputs or outputs. This reduces the number of places that must change when protocols, schemas, authorization rules, privacy policy, logging format, or infrastructure libraries evolve. If the proposed repair adds a check, rejects previously accepted data, changes authorization, or redacts previously emitted fields, it is not refactoring alone and must be reported as a security/correctness change. Reforge should therefore describe the structural escape and possible consolidation, not claim that adding a missing policy preserves behavior.

### Evidence

#### Foundational Sources

- title: Software Reflexion Models: Bridging the Gap Between Source and High-Level Models | year: 1995 | claim: An engineer-supplied high-level model can be mapped to source and compared with the implementation to expose convergences, divergences, and absences; applications included a 250 KLOC NetBSD subsystem and experimental reengineering of Microsoft Excel. | url: https://www.cs.ubc.ca/~murphy/papers/rm/fse95.html
- title: Discovery of Architectural Layers and Measurement of Layering Violations in Source Code | year: 2009 | claim: Layer organization can be discovered semi-automatically and non-conformance to layered design principles can be quantified, establishing forbidden cross-layer dependencies as measurable architecture evidence. | url: https://doi.org/10.1016/j.jss.2009.10.023
- title: P/Taint: Unified Points-to and Taint Analysis | year: 2017 | claim: Taint and points-to analysis can share a formal implementation basis, including explicit treatment of sanitization, supporting source-to-sink reachability as a program-analysis primitive. | url: https://doi.org/10.1145/3133926

#### Industrial Precedents

- tool: CodeQL | practice: Global data-flow configurations define sources, sinks, and barriers/sanitizers and compute paths across calls and fields that do not pass through a barrier; its call graph distinguishes declared from possible concrete callees. | url: https://codeql.github.com/docs/codeql-language-guides/codeql-library-for-go/
- tool: Semgrep | practice: Taint rules specify sources, sinks, propagators, and sanitizers. Official documentation explicitly distinguishes per-function, per-file, and interfile coverage and states that unmodeled propagators cannot be inferred by intraprocedural analysis. | url: https://docs.semgrep.dev/writing-rules/glossary
- tool: ArchUnit | practice: Architecture tests express allowed package access, layered architecture, and onion/ports-and-adapters constraints using method-call, constructor-call, and field-access dependencies. | url: https://www.archunit.org/userguide/html/000_Index.html
- tool: OWASP Benchmark | practice: Ground-truth true and false test cases plus TPR and FPR scoring demonstrate that flow-analysis capability must be validated explicitly; OWASP warns that its microcases are simpler than real applications. | url: https://owasp.org/www-project-benchmark/

#### Conflicting Evidence

Architecture rules require intent: a direct dependency or sink call can be deliberately allowed, and ports-and-adapters introduce navigation and indirection costs. Taint reachability proves only a possible value path, not that a particular project intended a named component to dominate that path. Sanitizers may be partial, conditional, order-sensitive, policy-specific, or duplicated intentionally for defense in depth. Static call graphs, alias analysis, dynamic dispatch, reflection, framework injection, serialization, persistence, queues, callbacks, and generated routing create both false paths and missed paths. OWASP notes that benchmark programs are substantially simpler than real applications, while Semgrep documents that intraprocedural analysis needs explicit propagator models. These limitations rule out treating an inferred missing barrier as an automatic refactoring recommendation.

### Detectability

#### Observable Pattern

Without assuming design intent, observe: (1) a value originating at a parameter, deserializer, environment/config read, persistence read, request/event boundary, or sensitive-data constructor; (2) a path through assignments, fields, returns, calls, callbacks, or route registration to an external or cross-module sink; (3) zero occurrences on that path of a candidate boundary function or module; and (4) one or more comparable source-to-sink paths in the same repository that do pass through that candidate boundary. Stronger observations are multiple bypass paths from distinct modules, direct imports of infrastructure clients from otherwise inner modules, repeated local policy-like code, or a recently growing bypass count. The source observation is a path difference; calling it a violation requires declared configuration or corroborated repository convention.

#### Graph Model

nodes:   - expression and storage locations
  - function parameters and returns
  - fields and object properties
  - call sites and resolved callees
  - route or event registrations
  - source, sink, boundary, module, and layer nodes; edges:   - assignment and def-use
  - argument-to-parameter
  - return-to-call-result
  - field read/write and alias
  - call and callback registration
  - import/dependency
  - route-to-handler
  - module containment; labels_and_summaries: Attach data kind, policy kind, boundary role, source/sink category, sanitizer transform or guard semantics, call confidence, and coverage status. Store function summaries mapping tainted/formal inputs to outputs, sinks, guards, and returned policy state. Aggregate paths by source module, sink module, policy kind, required boundary, actual first crossing edge, and path equivalence class.; decision: A declared-policy finding exists when a source-to-sink path avoids every allowed boundary node. An inferred-convention observation compares avoiding paths with repository paths that consistently pass through the same boundary and must remain heuristic. Dominance is useful for validators/authorizers that must execute on every feasible route; simple reachability is insufficient.

#### Analysis Requirements

An authoritative detector needs parsing and symbol/name resolution; an interprocedural call graph; CFG and dominance/guard reasoning; def-use/SSA-like flow; field-sensitive and at least allocation-site points-to reasoning; conservative dynamic-dispatch resolution; function summaries; context sensitivity sufficient to distinguish wrapper calls; and models for framework sources, sinks, callbacks, route registration, serializers, storage, queues, and library sanitizers. Policy-specific validation needs path sensitivity and a lattice richer than binary taint because validation, authorization, sanitization, and redaction are not interchangeable. A lower-cost architectural detector can instead use resolved imports/calls plus an explicit policy file defining modules, forbidden dependencies, and required boundary symbols, but must report dependency bypass rather than value-flow bypass.

#### Tree Sitter Feasibility

Tree-sitter can reliably locate declarations, imports, direct calls, member calls, route-registration syntax, assignments, returns, and path/name annotations in successfully parsed files. Reforge can conservatively build intra-function syntactic def-use for simple local variables and direct-call evidence, and can reuse its file dependency graph to show cross-module crossings. Tree-sitter alone cannot reliably resolve overloads, aliases, virtual/interface targets, higher-order calls, reflection, DI containers, heap aliases, field identity, framework-generated routes, or whether a custom function truly sanitizes/authorizes/redacts. The existing line-pattern adapter-boundary detector can cheaply corroborate an escape but cannot prove that the same value crossed the boundary or that a policy step was absent. Therefore an MVP must use explicit configured boundary/sink symbols or report only syntax-level candidates with partial coverage.

#### Language Constraints

Rust: Direct calls, modules, Result-based validation, and many trait calls are syntactically visible, but trait-object dispatch, macros, re-exports, proc-macro routing, lifetimes, and interior mutability limit syntax-only precision.; JavaScript_TypeScript_Vue: Dynamic properties, callbacks, promises, re-exports, framework routers, middleware order, decorators, and untyped JavaScript make interfile flow difficult; TypeScript type information is unavailable from Tree-sitter alone. Vue template-to-script flow requires additional modeling.; Python_Ruby_PHP: Monkey patching, dynamic attributes, metaprogramming, decorators, framework magic, and dynamic imports make call and field resolution incomplete.; Go: Interfaces, goroutines, channels, method values, and middleware chains need whole-program/type-aware models; direct package calls are comparatively tractable.; Java_CSharp_Kotlin: Overloading, inheritance/interfaces, annotations, reflection, DI, ORM persistence, coroutines/tasks, and framework middleware require type and framework models; compiled metadata would improve precision.; C_Cpp: Headers, macros, preprocessing, function pointers, templates, aliasing, and manual memory operations make source-only call and points-to analysis expensive and configuration-dependent.; Bash_PowerShell: Dynamic command construction, pipelines, dot sourcing, environment mutation, and external processes make value flow largely unsupported; Reforge already limits scripts to structure/similarity for some analyses.

#### Complexity Cost

Tier 1, explicit forbidden import/direct-call rules, is near linear in syntax nodes and resolved dependency edges and has low memory cost. Tier 2, intraprocedural def-use plus direct-call summaries, is roughly linear in AST/CFG size per function and moderate to implement across language adapters. Tier 3, whole-program context-, field-, and points-to-sensitive flow can approach cubic worst-case behavior, requires large graph storage and extensive library/framework models, and is a major multi-language subsystem. Path enumeration must be bounded and summarized to avoid exponential output.

### Signal Quality

#### False Positives

- Defense in depth intentionally repeats authorization or sanitization at several trust boundaries.
- An entry point performs equivalent validation inline or relies on a stronger typed constructor, schema decoder, database constraint, or framework middleware.
- A direct infrastructure call occurs in composition roots, migrations, probes, diagnostics, bootstrap code, tests, scripts, adapters, or generated code.
- Different sinks require different encodings or redaction policies; passing through a similarly named sanitizer would be wrong.
- Read-only data, constants, already-normalized domain values, or public/non-sensitive fields do not require the candidate boundary.
- A boundary wrapper exists for convenience rather than as mandatory architecture.
- Static analysis misses a guard, callback middleware, alias, framework interceptor, or runtime policy enforcement.
- A possible call-graph edge is infeasible in the relevant configuration or execution context.

#### Precision Boosters

- An explicit reforge.toml policy names source/sink categories, allowed crossing modules, and required symbols or annotations.
- Architecture tests, CODEOWNERS, ADRs, module visibility, lint rules, or existing annotations independently declare the boundary.
- At least three bypass occurrences in at least two non-boundary production modules, or one newly introduced bypass against an otherwise universal convention.
- Comparable paths of the same data/policy kind overwhelmingly traverse the candidate boundary.
- The bypassing module imports a concrete infrastructure client while neighboring modules depend on the port/interface.
- Repeated local validation/redaction/authorization code or parallel wrapper implementations indicate dispersed policy ownership.
- Dependency hub/cycle, high fan-out, churn, debt markers, or shadowed abstraction evidence overlaps the bypass locations.
- A bounded source-to-sink witness shows the first module crossing, omitted boundary, sink, and one conforming comparison path.
- Test fixtures demonstrate that routing through the boundary preserves current behavior.

#### Legitimate Exceptions

- Small scripts, one-shot migrations, build tools, and composition roots where abstraction cost exceeds expected change cost.
- Pipes-and-filters or event-driven systems where policy is intentionally enforced by infrastructure outside the source-visible path.
- Microservices where gateways, service meshes, brokers, database policies, or platform middleware own the boundary.
- Generated clients, serializers, ORM layers, and framework convention code.
- Performance-critical paths with an approved direct fast path and equivalent invariant checks.
- Multiple bounded contexts that intentionally own different adapters or policy implementations.
- Defense-in-depth architectures with deliberately repeated checks or redaction.

#### Confidence Tier

Experimental for inferred policy; conservative only when an explicit project policy names boundaries, sources/sinks, and allowed exceptions.

### Reforge Integration

#### Existing Capabilities

Reforge already emits adapter_boundary_bypass when a path-named HTTP, config, filesystem, or logging boundary exists and at least four line-pattern direct calls occur across at least three files outside boundary files. It skips tests and operational entry-point paths for this check. Current patterns cover APIs such as fetch/axios/requests/reqwest, environment reads, filesystem calls, and console/print/log calls. Reforge also has resolved source-file dependency edges, fan-in/fan-out, transitive context closures, unresolved-edge counts, parse/coverage receipts, related locations, git churn metrics, config-key drift, repeated literals/error patterns, parallel implementation, shadowed abstraction, dependency cycle/hub, and stable finding identity. These can corroborate a boundary escape, but the current adapter detector is line/name based and does not establish same-value flow, dominance, sanitizer semantics, authorization, or redaction.

#### Proposed Metrics

- name: boundary_escape_path_count | definition: Distinct summarized source-to-sink paths that avoid all required boundary nodes. | unit: paths
- name: escaping_module_count | definition: Distinct non-exempt source modules containing the first crossing edge of an escape path. | unit: modules
- name: boundary_conformance_ratio | definition: Comparable observed paths that traverse an allowed boundary divided by all comparable observed paths. | unit: ratio
- name: unresolved_path_segment_count | definition: Calls, imports, callbacks, or framework transitions on candidate paths that could not be resolved. | unit: segments
- name: direct_concrete_dependency_count | definition: Imports or calls from protected modules to configured infrastructure implementations instead of allowed ports. | unit: dependencies
- name: duplicate_policy_site_count | definition: Distinct sites performing similar validator, sanitizer, authorization, or redaction operations outside the owned boundary. | unit: sites
- name: escape_churn | definition: Recent weighted added plus deleted lines for files containing escape evidence, using Reforge's configured churn window. | unit: weighted lines

#### Proposed Finding Kinds

- name: declared_boundary_policy_escape | issue_family: modularity | mechanism: dependency_propagation | action: reduce_dependency_coupling | scope: finding_group | precision_risk: Low to moderate when policy and path coverage are explicit; never claim a security vulnerability.
- name: inferred_boundary_policy_drift | issue_family: modularity | mechanism: responsibility_dispersion | action: consolidate boundary handling | scope: finding_group | precision_risk: High because intent and semantic equivalence are inferred; keep experimental and require multiple corroborators.
- name: direct_boundary_dependency_bypass | issue_family: modularity | mechanism: dependency_propagation | action: reduce_dependency_coupling | scope: file or finding_group | precision_risk: Moderate; this is an extension or specialization of existing adapter_boundary_bypass and does not require value-flow claims.

#### Explanation Path

Show one shortest or lowest-uncertainty witness: source location and category; value/argument identity; each assignment, call, field, route, and module crossing; the first forbidden direct dependency or crossing; sink location and category; and an explicit statement that no configured boundary node appears on the modeled path. Also show the required boundary declaration and its source (configuration, architecture test, annotation, or inferred convention), one conforming comparison path for heuristic findings, all duplicate-policy locations, overlapping dependency/churn findings, and unresolved/truncated segments. Related locations must include the candidate boundary implementation, every grouped escape origin, and relevant tests. Do not imply runtime exploitability or behavioral equivalence from static reachability.

#### Coverage Contract

observed: All files and supported constructs on the displayed path parsed; direct symbols/imports resolved; configured source, sink, and boundary models applied; no unresolved segment lies between source and sink.; partial: Some calls, fields, imports, callbacks, aliases, framework transitions, generated code, or external libraries are unmodeled; findings may be shown but absence is not meaningful.; unsupported: Reflection-heavy, runtime-generated, cross-process, persistence-mediated, queue-mediated, macro/generated, shell pipeline, or language constructs without models; do not evaluate policy escape.; unresolved: Report exact unresolved call/import/route segments, parse failures, skipped paths, excluded generated code, and whether path output was truncated.; negative_result_rule: Report 'no observed escape under configured models', never 'all data passes the policy boundary', unless the supported closed-world contract is explicitly satisfied.

#### Mvp Scope

Add a policy-driven, dependency/call-level extension rather than general taint analysis. Let reforge.toml define protected module globs, concrete sink/import patterns, allowed adapter or boundary globs, and exemptions. Use existing file discovery, Tree-sitter call/import extraction, resolved dependency graph, coverage receipts, related locations, and churn. Emit direct_boundary_dependency_bypass only for resolvable production files, grouping repeated crossings and showing the intended boundary plus direct edge. Optionally add simple intraprocedural local-variable flow for direct calls in Rust, JavaScript/TypeScript, Python, Go, Java, and C#. Defer interprocedural heap flow, sanitizer semantics, authorization guard dominance, redaction-field tracking, framework middleware, persistence/queue flow, automatic intent inference, and cross-service analysis.

### Validation

#### Fixtures

positive:   - A controller sends a request body directly to a repository sink while all peer controllers call the configured validator adapter.
  - Three domain modules import a concrete HTTP client around an existing gateway port.
  - A route calls a sink on one branch without the configured authorizer, while the authorizer dominates all conforming routes.
  - Sensitive DTO fields reach a configured logger directly while peer paths traverse a redactor.; negative:   - Every path crosses the configured boundary, including through a wrapper summary.
  - A composition root wires a concrete adapter under an explicit exemption.
  - Two bounded contexts use separately configured validators for different schemas.
  - Framework middleware enforces authorization and a supplied model makes that transition observed.
  - A value is public or constructed as an already-validated type and its policy category does not require the boundary.; metamorphic:   - Rename files and symbols while preserving explicit policy identifiers: result and finding identity should remain stable modulo locations.
  - Extract a direct call into an unmodeled wrapper: coverage must degrade rather than silently clear the finding.
  - Route the same call through the configured boundary without changing transformations: escape count decreases by one.
  - Add dead or infeasible bypass code: a path-sensitive implementation should not add a finding; a path-insensitive MVP must label its limitation.
  - Move a fixture from production to a recognized test/script exemption: it should no longer contribute to production grouping.
  - Add an unrelated sanitizer of a different policy kind: the escape must remain.

#### Corpus Strategy

Build a stratified corpus with (a) repositories that declare ArchUnit or equivalent architecture rules, ADRs, module visibility, or custom lint policies; (b) mature web/service projects in each supported language with validators, middleware, repositories, gateways, and logging/redaction wrappers; (c) security benchmark cases for flow-engine capability, explicitly separated from maintainability labels; and (d) Reforge's own repository for regression of the existing adapter_boundary_bypass behavior. Mine commits and review discussions that introduce, reject, or consolidate direct infrastructure/policy calls. Split by repository, organization, framework, and policy family so near-duplicate patterns do not cross train/holdout boundaries. Reserve whole frameworks and at least one language family as holdouts, and evaluate explicit-policy and inferred-policy modes separately.

#### Oracle And Labels

Provide maintainers with the displayed path, configured or inferred boundary evidence, comparison paths, and coverage gaps, but hide detector confidence during first-pass labeling. Independently label: instrumentation correctness (source/sink/boundary category), path feasibility, policy applicability, intended architectural crossing, legitimate exception, maintainability concern, refactoring actionability, and whether the smallest remedy preserves behavior. A security reviewer may separately label vulnerability/correctness impact; that label must not substitute for maintainability intent. Use two maintainers or an architect plus maintainer, measure agreement per dimension, adjudicate disagreements, and follow accepted changes to record whether callers were routed, policy duplicated, exception documented, or no action taken.

#### Open Questions

- Should Reforge require an explicit boundary policy for findings and expose inferred conventions only as raw observations?
- How should policy kinds compose when validation, authorization, sanitization, normalization, and redaction must occur in a particular order?
- Is the first product increment an extension of adapter_boundary_bypass or a separate declared_boundary_policy_escape kind?
- Which language and framework models provide enough value before whole-program type information is available?
- Can conforming-path prevalence infer intent without merely codifying an accidental majority pattern?
- How should approved exceptions be versioned, owned, expired, and distinguished from suppressions?
- Should churn and repeated escapes affect only displayed evidence or detector gating, given Reforge's policy of not emitting severity or priority?
- How can refactoring equivalence be verified when centralizing a policy operation may change exception order, error messages, or observability?


## Mutable state fan-out and cyclic journeys

### Conclusion

#### Item Name

Mutable state fan-out and cyclic journeys

#### Thesis

Write dispersion around shared or imported mutable state is a plausible and measurable refactoring signal: a state location written by several modules creates common coupling, widens the potential impact set, and suggests consolidating mutation behind an owner, service, or explicit state transition API. A value journey that leaves a module and later returns can be useful path evidence only when the returning lineage is mutated or semantically transformed across several owners; a graph cycle by itself is not a validated smell. The best product shape is therefore a conservative shared-state-writer finding and an experimental round-trip-flow observation, both calibrated against change history rather than presented as universal predictors.

#### Refactoring Relevance

The behavior-preserving restructuring hypothesis is concrete for dispersed mutation: introduce a single owner or mutation API, move update logic next to the state, encapsulate a module global, replace writeable exports with commands or immutable values, or split unrelated state when one storage location represents several responsibilities. These transformations can retain observable behavior while reducing the number of modules allowed to establish a new state. For a suspicious round trip, candidate treatments include moving transformations into the owning module, extracting a pipeline, introducing a typed intermediate representation, or replacing an out-and-back callback protocol with an explicit boundary. No refactor follows from topology alone: the analysis must show the abstract state location or value lineage, actual write/transform edges, and boundary crossings. Pure request-response, visitor callbacks, middleware, immutable pipelines, event loops, parsers, and recursive algorithms often contain legitimate returns or cycles.

### Evidence

#### Foundational Sources

- On the Criteria To Be Used in Decomposing Systems into Modules (D. L. Parnas, 1972): argues that modules should hide design decisions likely to change, providing the ownership rationale for hiding mutable representation behind a stable interface. https://doi.org/10.1145/361598.361623
- Software Structure Metrics Based on Information Flow (Sallie Henry and Dennis Kafura, 1981): defines structure complexity from informational fan-in and fan-out, including global-data access, establishing information flow as an inter-module complexity measure. https://doi.org/10.1109/TSE.1981.231113
- Program Slicing (Mark Weiser, 1984): defines automatic program decomposition from control and data flow; forward slices provide the semantic basis for a potential change-impact set from a state definition or write. https://doi.org/10.1109/TSE.1984.5010248
- The Program Dependence Graph and Its Use in Optimization (Jeanne Ferrante, Karl J. Ottenstein, and Joe D. Warren, 1987): unifies control and data dependences in a graph representation suitable for reachability, strongly connected components, and path explanations. https://doi.org/10.1145/24039.24041
- Interprocedural Slicing Using Dependence Graphs (Susan Horwitz, Thomas Reps, and David Binkley, 1990): gives a context-sensitive system-dependence-graph method for interprocedural slicing, demonstrating why precise cross-call influence needs more than syntax traversal. https://doi.org/10.1145/77606.77608

#### Empirical Sources

- Examining the Effects of Global Data Usage on Software Maintainability (Selby, Ruffell, Giesbrecht, and Godfrey, WCRE 2007): analyzed seven binaries/releases from Emacs, GCC, GDB, Make, PostgreSQL, and Vim using link-time true-global references and CVS maintenance data. Global-reference counts had statistically significant positive correlations with file revisions and lines changed; reported coefficients were 0.12-0.44 for revisions and 0.08-0.39 for changed lines, with revision correlations stronger in every binary. The authors explicitly say this is association, not causation, and note hotspot/feature-growth confounding. https://plg2.cs.uwaterloo.ca/~migod/papers/2007/wcre07.pdf
- Assessing the Impact of Global Variables on Program Dependence and Dependence Clusters (Binkley, Harman, Hassoun, Islam, and Li, 2010): assessed 849 globals in 21 programs totaling just over 50 KLOC. More than half of programs contained an individual global with significant impact on overall dependence; in one quarter, a single global solely caused a dependence cluster. This supports inspecting high-impact shared state, but the outcome is dependence structure rather than observed change propagation. https://doi.org/10.1016/j.jss.2009.03.038
- Mining Metrics to Predict Component Failures (Nagappan, Ball, and Zeller, ICSE 2006): studied post-release defects in five Microsoft systems. Module-level maximum WriteCoupling correlations ranged from 0.011 to 0.618, total WriteCoupling from -0.128 to 0.629, and ProcCoupling from 0.000 to 0.579. No single metric correlated in all five projects, so the paper recommends project-specific calibration and multivariate models. https://doi.org/10.1145/1134285.1134349
- A Study of Cyclic Dependencies on Defect Profile of Software Components (Oyetoyan, Conradi, Cruzes, 2013): evaluated Apache Camel, ActiveMQ, Lucene, Eclipse, openPDC, and one commercial application in Java/C#. Components in or depending on structural dependency cycles were more defect-prone. The labels and edges were class/package dependency relations, not value lineages, so this is corroboration for a cycle context signal rather than direct validation of cyclic data journeys. https://doi.org/10.1016/j.jss.2013.07.039
- Modularity, Dependence and Change (Markus M. Geipel, 2012): empirical analysis of 35 software architectures reported that dependency relations seldom caused change propagation and that higher architectural dependency correlated negatively with large change events. This directly cautions against treating static reachability or cycles as automatic change propagation. https://doi.org/10.1142/S021952591250083X

#### Industrial Precedents

- SciTools Understand exposes informational fan-out that counts called subprograms plus parameters and global variables set/modified, showing that production analysis tools treat global writes as outputs rather than only counting imports. https://docs.scitools.com/manuals/pdf/metrics.pdf
- CodeQL supports global interprocedural data-flow and taint-tracking configurations, path queries, call targets, fields, and explicit source/sink/barrier models. Its documentation warns that global analysis is less precise and materially more expensive than local analysis, and that all-program global flow is generally infeasible without a focused configuration. https://codeql.github.com/docs/codeql-language-guides/analyzing-data-flow-in-javascript-and-typescript/
- Semgrep documents cross-file taint analysis through arbitrarily many files using sources, sinks, propagators, and sanitizers. This demonstrates deployable summary-based interfile flow, while also showing that real tools scope flow to configured questions rather than constructing every possible journey. https://semgrep.dev/docs/writing-rules/glossary/
- SonarQube documents automatic circular-dependency findings for supported languages and explains maintainability and unintended-side-effect risks. This is a precedent for cycle reporting, but it concerns reference dependencies, not identity-preserving or transformed value flow. https://docs.sonarsource.com/sonarqube-server/design-and-architecture/cycle-detection

#### Conflicting Evidence

Static dependency is not equivalent to runtime influence, maintenance propagation, or bad ownership. Geipel's 35-architecture study found that dependencies seldom caused change propagation and associated higher dependency with fewer large change events. Nagappan et al. found no universally predictive metric across five systems; WriteCoupling ranged from negligible to moderate/strong depending on project. Selby et al. did not establish causality, did not normalize revision counts by file size, studied mainly GNU C systems, and acknowledged that frequently changed global-heavy files may be successful feature hotspots. Binkley et al. measured dependence connectivity, not developer effort or co-change. Ordinary dependency-cycle evidence cannot be transferred directly to data-flow cycles because loops, recursion, callbacks, state machines, two-way protocols, and request-response naturally generate cyclic paths. A static may-flow graph can also create spurious cycles by merging aliases, call contexts, object instances, or mutually exclusive branches. These limits argue for separate raw metrics, local baselines, explicit uncertainty, and history-backed validation.

### Detectability

#### Observable Pattern

Observe declarations of module/static/global state, exported or imported bindings, fields of singleton-like objects, and explicit reads and writes. Resolve each write site to an abstract storage location and aggregate its distinct writer functions, files, directories/modules, and packages. A dispersed-write candidate requires at least two independently owned modules writing the same non-test, non-generated mutable location; read-only consumers do not count as writers. For round trips, start from a value definition, parameter, returned object, or mutable location in module A; follow assignment, argument-to-parameter, return-to-call, field load/store, alias, and configured transform steps across distinct modules; report only a simple module path A -> B -> ... -> A where the lineage returns to an A-owned sink after at least two external module transitions. Keep path length, transform count, mutation count, alias uncertainty, and whether the returned value is the same abstract object, a derived value, or merely taint-influenced. Do not infer a problem from import cycles or control-flow loops alone.

#### Graph Model

Use a layered graph. Nodes: module/file/package; function/method; declaration or abstract storage location; value/SSA definition; parameter and return slot; optional commit. Edges: declares/owns, imports, calls, actual-to-formal, return-to-call-site, defines, reads, writes, aliases, field-load, field-store, captures, and transforms/influences. Edge labels need source location, language, direct-versus-modeled status, read/write kind, field path, resolution confidence, and call context. Function summaries map input positions and global/field reads to output positions, global/field writes, and escaped aliases. Collapse value edges to a module multigraph while retaining state identity and witness paths. Writer fan-out is the number of distinct owning modules with write edges to one abstract location. Round trips are simple cycles in the lineage-labeled module graph whose first and last module match, not arbitrary SCCs; canonicalize rotations and keep the shortest bounded witnesses. Optional history edges connect commits to touched modules for downstream predictive validation.

#### Analysis Requirements

A useful shared-state detector requires project-level declaration and import/name resolution; lexical scope; assignment and mutation recognition; module ownership; and distinction between reads, writes, initialization, and atomic/interior mutation. Precision beyond explicit module globals requires a CFG, reaching definitions or SSA-like def-use, interprocedural call/return matching, function summaries, points-to/alias analysis, field sensitivity, closure capture, heap abstraction, and escape analysis. Dynamic dispatch, callbacks, async scheduling, reflection, dependency injection, framework containers, serialization, and native/FFI boundaries need conservative models or unresolved receipts. Context sensitivity is especially important for round trips: context-insensitive call graphs can fabricate return paths by matching a callee's return to the wrong caller. Object sensitivity prevents writes to separate instances from collapsing into one shared state. Transform lineage needs a declared policy distinguishing value-preserving flow from influence/taint; otherwise nearly every calculation can join unrelated values. Git validation additionally needs rename-aware entity mapping, commit-size filtering, and time-ordered training/holdout splits.

#### Tree Sitter Feasibility

Tree-sitter can reliably locate module-level/static declarations, export/import syntax, assignment/update expressions, explicit member writes, function boundaries, parameters, returns, calls, closures, and precise source locations for Reforge's parsed languages. With a lightweight symbol index, Reforge can conservatively resolve direct writes to locally declared module state and some explicitly imported mutable bindings; this is enough for a useful syntax-tier writer-dispersion MVP. Tree-sitter alone cannot establish runtime mutability, alias identity, receiver types, which dynamic callee executes, whether an accessor mutates hidden state, actual-to-formal/return flow across overloads, instance sharing, framework lifecycle, macro/generated code, or whether a transformed result preserves a semantic lineage. It can enumerate syntactic path candidates but cannot reliably prove cyclic journeys. Therefore Tree-sitter-only round trips must be labeled partial/heuristic, and unresolved aliases or calls must never be interpreted as absence of flow.

#### Language Constraints

Java, C#, and Kotlin provide nominal static fields and class members, but properties, reflection, DI, extension functions, annotations/code generation, overloads, and virtual dispatch complicate writes. Rust makes top-level `static mut` rare and unsafe, but `Mutex`, `RwLock`, atomics, `OnceLock`, interior mutability, `lazy_static!`, macros, and shared `Arc` state require API and macro models; its borrow checker improves local alias information but Tree-sitter does not expose compiler ownership facts. Go package variables and explicit assignments are tractable, while pointer aliases, interfaces, goroutines, channel-mediated state, init functions, and generated code complicate semantics. JavaScript/TypeScript/Vue have live module bindings and exported objects; property mutation, re-exports, structural typing, closures, proxies, and bundler aliases reduce precision, and CommonJS can export mutable singleton objects. Python, Ruby, and PHP permit module/class globals but dynamic attributes, monkey patching, magic methods, metaprogramming, and import hooks make resolution conservative. Bash and PowerShell variables are heavily scope- and environment-dependent, dot-sourcing changes namespaces, and subprocess boundaries change sharing; support should initially be limited to explicit same-process script variables or marked unsupported. C/C++ currently lack Tree-sitter structural support in Reforge beyond basic source metrics, so source-level global-write flow should be unsupported until parsers and pointer/alias handling exist.

#### Complexity Cost

Direct declaration/write indexing is O(N + E) time and memory in AST nodes and resolved references and should be inexpensive. SCC or bounded shortest-cycle analysis over an already aggregated module graph is O(V + E), but enumerating all simple cycles is exponential and must not be attempted. Interprocedural def-use with summaries is medium-to-high implementation cost and typically near-linear for a fixed abstraction, yet points-to sets and polymorphic calls can enlarge E substantially. Field-, object-, and context-sensitive heap analysis can be superlinear in practice and is high memory. Whole-program all-pairs lineage is infeasible for a lightweight multi-language CLI; use focused sources (shared locations and boundary outputs), cached function summaries, maximum path/module depth, witness caps, and per-language budgets.

### Signal Quality

#### False Positives

- Intentional process-wide registries, feature-flag/configuration state, caches, metrics counters, tracing context, intern pools, connection pools, and dependency-injection containers.
- Atomic counters, locks, concurrent queues, actor runtimes, schedulers, event buses, and framework-owned lifecycle state where multiple writers are the designed API.
- Generated code, test fixtures, mocks, build scripts, migrations, plugin registration, application bootstrap, and compatibility shims.
- Request-response calls, callback completion, visitor double dispatch, middleware chains, parser/serializer pairs, codecs, adapters, and anti-corruption layers that naturally leave and return.
- Immutable pipelines where each module returns a new value; shared lineage is not shared mutation.
- Recursion, loops, fixed-point solvers, state machines, reactive feedback, game loops, and control systems with intentional cycles.
- Alias imprecision that merges separate instances or fields, and context-insensitive call/return matching that fabricates a round trip.
- A central state module that intentionally exposes a narrow transaction API: many caller modules initiate changes but only one module performs the write.

#### Precision Boosters

- Count physical write owners separately from callers of an owner-provided mutation API; the latter is ordinary fan-in, not dispersed mutation.
- Require a resolved in-project state declaration, two or more distinct external writer modules, and at least one non-initialization write per writer.
- Weight direct writes, address/alias escape, and read-modify-write more than initialization, reset, test, or generated writes; display raw counts independently.
- Require independent corroboration such as high file churn, dependency hub/cycle membership, many authors, data clump/duplicate type shape, or historical co-change among writer modules.
- Prefer project-relative writer-dispersion percentiles and persistence across releases over a universal absolute threshold.
- For round trips, require at least three distinct modules, two semantic transformation or mutation steps, a bounded shortest witness, and a return to an A-owned sink; suppress pure call/return paths.
- Keep identity-preserving alias flow separate from derived-value/taint influence and report confidence for every hop.
- Use context-sensitive call/return matching, field-sensitive storage identities, library/framework summaries, and an unresolved-edge budget.
- Recognize and downgrade registries, caches, metrics, tracing, configuration, event systems, tests, generated sources, serializers, adapters, and lifecycle/bootstrap code.
- Validate that writer dispersion predicts future multi-module changes after controlling for size, churn, fan-in, complexity, age, and module centrality before raising severity.

#### Legitimate Exceptions

Expected designs include operating-system or embedded state, hardware registers, explicitly synchronized runtime state, metrics and tracing collectors, caches and pools, registries and plugin tables, configuration/feature flags, actor/event systems, ECS worlds, Redux-like stores with a single reducer boundary, database transaction coordinators, reactive feedback networks, parsers/codecs, request-response protocols, middleware, visitors, state machines, iterative numerical solvers, and application bootstrap. A state location can have many logical requesters but one encapsulated writer; this is often good centralization. Some low-level C or performance-critical systems deliberately trade encapsulation for memory/layout/latency. The report should distinguish these from unowned ad hoc writes and support suppression with a recorded reason.

#### Confidence Tier

conservative for explicit shared-state writer dispersion; experimental for cyclic journeys

### Reforge Integration

#### Existing Capabilities

Reforge already has Tree-sitter adapters for Rust, JavaScript/TypeScript/TSX/Vue, Python, Go, Java, C#, Kotlin, PHP, Ruby, Bash, and PowerShell; structural extraction supplies functions, types, parameters, imports, complexity, nesting, and locations. Its source dependency graph resolves a conservative subset of local imports/references and already computes file fan-in/out, transitive reach, depth, dependency hubs, SCC-based dependency cycles, and unresolved-edge counts. Git churn supplies file-level commits touched, lines added/deleted, author count, and recent weighted churn. Existing corroborators include dependency_cycle, dependency_hub, import_heavy_file, data_clump, duplicate_type_shape, generic_bucket_drift, adapter_boundary_bypass, large_file, complex_function, and parallel_implementation. Report infrastructure already supports typed metrics, related locations, detector manifests, coverage status, execution receipts, and suppression. Missing pieces are state-declaration/write extraction, symbol binding across imports, def-use/alias and call summaries, method-level co-change, and lineage-path storage.

#### Proposed Finding Kinds

- shared_state_writer_dispersion | issue family: state_coupling | mechanism: one resolved mutable location has direct writes in several owner modules | action: review single ownership, encapsulate mutation, expose commands/transitions, or split state | scope: state declaration plus writer locations | precision risk: medium for explicit bindings and high when aliases/properties are inferred.
- mutable_state_round_trip | issue family: state_coupling | mechanism: the same abstract object/location leaves its owner, is mutated by two or more foreign modules, and returns to an owner sink | action: review ownership and consolidate or make the transformation protocol explicit | scope: shortest bounded interprocedural witness | precision risk: high; experimental only.
- transformed_value_round_trip | issue family: flow_topology | mechanism: a derived value lineage crosses several modules and returns after multiple transformations | action: inspect for misplaced pipeline stages or redundant representation conversions | scope: value definition to returning sink | precision risk: very high because semantic lineage is policy-dependent; defer beyond MVP.

#### Explanation Path

A shared-state finding must show the declaration and inferred owner, mutability kind, each distinct writer module/function with one representative write location, total write/read-modify-write counts, reader count, initialization classification, alias escapes, unresolved references, and why callers through an owner API were not counted as writers. Display a small star path: writer module -> write site -> state declaration, plus dependency/churn corroborators and recognized exception status. A round-trip observation must show the ordered shortest witness with every source location and edge kind: origin definition/state -> boundary argument/return/store -> foreign transforms or mutations -> returning A-owned sink. Each hop must state resolved, modeled, or heuristic confidence; identity-preserving and influence-only paths must not be mixed. State explicitly that path feasibility, runtime ordering, design intent, and future propagation were not proven.

#### Coverage Contract

Observed for the MVP means a supported parsed file, a syntactically explicit mutable module/static declaration, direct write syntax, and a declaration/import binding resolved inside the scan root. Partial means some imports, aliases, member receivers, calls, macros, generated declarations, properties, dynamic dispatch, or history are unresolved; positive findings can use resolved edges, but a zero result cannot imply absence of shared mutation. Unsupported initially includes heap objects shared only through parameters/containers, reflection/metaprogramming, framework or native state without a model, C/C++ source flow, most Bash/PowerShell cross-script/environment flow, and semantic transformed-value journeys. Excluded test/generated/vendor paths and recognized exceptions require counts and reasons. Reports must expose analyzed declarations, resolved/unresolved reads and writes, skipped languages, parse failures, resolver tier, history availability, path caps, summary timeouts, and whether the result is writer dispersion or a round-trip heuristic.

#### Mvp Scope

Implement shared_state_writer_dispersion only. For JavaScript/TypeScript/Vue, Python, Go, Java, C#, Kotlin, PHP, Ruby, and Rust, extract explicit module/package/class static mutable declarations and explicit assignments/update operations. Resolve same-file writes first and conservative direct imported-name/re-export writes where language semantics permit; index declaration owner, writer file/module, initialization, and source locations. Emit only when at least two external modules directly write the same in-project state, at least three non-initialization write sites exist overall, the state is not test/generated, and either churn or an existing dependency hub/cycle supplies corroboration. Report readers, unresolved references, and exceptions but do not trace arbitrary heap aliases. Defer entropy-based severity, automatic refactoring, call-through mutator inference, object/field points-to, callbacks/async, history prediction, and all cyclic/transformed journey findings. After the writer detector is validated, prototype identity-preserving mutable round trips in one statically typed language behind an experimental flag.

### Validation

#### Fixtures

- Positive: Java or C# public static mutable state declared in module A and directly read-modify-written from B, C, and D; the finding names A and all three writers.
- Positive: TypeScript exports a mutable object and two external modules assign the same resolved property; direct property identity is retained.
- Positive experimental: A creates a mutable request context, B mutates it, C mutates it, and it returns to an A sink with a context-correct call/return witness.
- Negative: many modules call A.setState(), but the physical write occurs only inside A; writer module count is one.
- Negative: an exported immutable constant or read-only binding has many readers and no writes.
- Negative: cache, metrics counter, registry, feature flag, test fixture, generated code, and framework lifecycle examples with the same raw writer count are downgraded or suppressed with receipts.
- Negative: A calls B and receives B's return without external mutation or multi-module transformation; ordinary call/return is not a journey finding.
- Negative: recursion, event loop, visitor callback, middleware, request-response, codec round trip, and state-machine feedback examples.
- Metamorphic: formatting, comment, import ordering, and identifier renaming with preserved binding do not change state identity, counts, or finding ID.
- Metamorphic: move all direct writes behind an A-owned setter and update callers; external writer count falls to zero while call fan-in may increase.
- Metamorphic: duplicate the state into two separate instances; an object-sensitive tier separates them, while a syntax tier must mark partial instead of merging confidently.
- Metamorphic: insert a value-preserving helper module into a journey; hop count changes but transform count and semantic classification do not.
- Metamorphic: add an unrelated dependency cycle; data-flow finding must remain unchanged.

#### Corpus Strategy

Use a stratified longitudinal corpus by language and architecture: statically typed services/libraries, dynamic web applications, CLI tools, compilers, IDEs, embedded/game code, event-driven systems, state-store architectures, and framework/plugin ecosystems. Include explicit hard-negative strata for registries, metrics, caches, DI containers, Redux-like stores, ECS, actors, callbacks, codecs, and generated code. For writer dispersion, mine state declarations at releases t and measure future commits/change tasks over a fixed window; sample matched controls by language, repository, file size, prior churn, fan-in, complexity, age, authors, and centrality. Use time-ordered repository-level splits and reserve whole organizations/projects as holdouts. For round trips, build a smaller manually traced corpus and compare syntax, context-insensitive, and context-sensitive paths. Avoid random file splits and avoid labeling current high churn as future propagation. Publish fixture sources, extraction receipts, and de-identified labels where licensing allows.

#### Oracle And Labels

At least two experienced maintainers or language reviewers independently label four dimensions. Instrumentation: declaration identity, mutability, write/read classification, owner module, alias/call resolution, and every journey hop. Design interpretation: unowned shared mutation, intentional shared infrastructure, centralized owner API, suspicious round trip, legitimate protocol, or ambiguous. Actionability: keep/suppress, encapsulate mutation, move update logic, split state, introduce immutable transition, simplify pipeline, or needs architectural context. Outcome: after any accepted change, tests and behavior contracts pass, direct writer modules decrease without merely hiding aliases, dependency/churn locality changes, and no new cycle or contention is introduced. Reviewers see raw evidence and code context but not the score or future-change outcome. Report agreement and adjudication, preserve ambiguous cases, and keep prediction labels (future multi-module changes) separate from smell/action labels.


## Ownership leakage and feature envy

### Conclusion

#### Item Name

Ownership leakage and feature envy

#### Thesis

Ownership leakage is a useful but non-conclusive refactoring signal when a function accesses, traverses, or transforms data belonging to one foreign type or module substantially more than data belonging to its lexical owner. The strongest interpretation is the classic Feature Envy / misplaced-responsibility case; broader module-level aggregation can also expose Broken Modularization. The detector should recommend inspection and a candidate destination, not automatically prescribe Move Method or Move Field.

#### Refactoring Relevance

The observation has a direct behavior-preserving restructuring hypothesis: move all or an extracted portion of the behavior closer to the data it predominantly uses, or move a field only when ownership and change evidence show that the field itself belongs with the consuming abstraction. Fowler's Feature Envy treatment and the JDeodorant literature connect concentrated foreign access to Move Method. Broken Modularization generalizes the same responsibility-placement problem to data or methods split across abstractions and admits Move Method or Move Field. The intended benefit is to replace repeated cross-boundary knowledge with a local operation, reducing inter-module knowledge and localizing future changes. A move is not justified when the source function is legitimate orchestration, a query/report over intentionally passive data, a Visitor/Strategy implementation, serialization/mapping code, or when the target would acquire an incoherent responsibility.

#### Evidence Strength

moderate. The conceptual link, operational metrics, behavior-preserving Move Method mechanics, and multiple research/industrial precedents are strong. Empirical evidence also shows that history and static structure are complementary. However, expert evaluations of Move Method recommenders report low precision in realistic systems, smell definitions and tools disagree, and the direct extension from object-oriented fields to cross-language module-owned data is not yet validated for Reforge. Therefore the proposed Reforge detector should remain experimental and require corroboration.

### Evidence

#### Foundational Sources

- title: Refactoring: Improving the Design of Existing Code | year: 1999 | claim: Introduced Feature Envy as behavior that is more interested in another class's data and associates it principally with Move Method, sometimes after Extract Method. | url: https://martinfowler.com/books/refactoring.html
- title: Detection Strategies: Metrics-Based Rules for Detecting Design Flaws | year: 2004 | claim: Established metrics-based detection strategies. The later canonical Feature Envy strategy operationalizes foreign data access through ATFD, LAA, and FDP rather than a raw coupling count. | url: https://doi.org/10.1109/ICSM.2004.1357820
- title: Identification of Move Method Refactoring Opportunities | year: 2009 | claim: Formalized candidate target classes from accessed members and combined detection with refactoring preconditions intended to preserve behavior; this work underlies JDeodorant's Feature Envy analysis. | url: https://doi.org/10.1109/TSE.2009.1
- title: Refactoring for Software Design Smells: Managing Technical Debt | year: 2014 | claim: Defines Broken Modularization as data and/or methods that should be localized in one abstraction being separated across abstractions, connecting responsibility dispersion to Move Method and Move Field treatments. | url: https://www.sciencedirect.com/book/9780128013977/refactoring-for-software-design-smells

#### Empirical Sources

- title: Mining Version Histories for Detecting Code Smells | year: 2015 | systems_and_labels: Eight systems with manually validated smell instances; Feature Envy was found in five systems, totaling 42 affected methods. | relevant_results: HIST detected 34 instances (81% recall) with 71% precision, versus JDeodorant's 25 instances (60% recall) and 68% precision. After applying JDeodorant move preconditions, HIST obtained 74% recall and precision. Only 39% of correct Feature Envy instances overlapped; 41% were unique to history and 20% unique to JDeodorant, supporting combined static and co-change evidence. | url: https://doi.org/10.1109/TSE.2014.2372760
- title: JMove: A novel heuristic and tool to detect move method refactoring opportunities | year: 2018 | systems_and_labels: 195 synthesized Move Method opportunities in ten open-source systems, plus expert assessment in two industrial-strength systems. | relevant_results: JMove precision ranged from 21% to 32% and median recall from 21% to 60%; JDeodorant precision ranged from 5% to 15%. In one expert-evaluated system without misplaced methods, JDeodorant and inCode produced 43 and 50 false recommendations; in another, JMove precision was 24% versus 6% and 5%. This strongly limits any uncorroborated detector. | url: https://doi.org/10.1016/j.jss.2017.11.038
- title: Finding needles in a haystack: Leveraging co-change dependencies to recommend refactorings | year: 2019 | systems_and_labels: Evaluation of Draco against REsolution, JDeodorant, and JMove using fine-grained historical dependencies and developer-oriented assessment. | relevant_results: Fine-grained entities that frequently change together across classes supported feasible Move Method and Move Field recommendations and revealed design improvements, while the authors also report developer resistance to recommendations that challenge the existing design. It supports change localization as corroboration, not as proof. | url: https://doi.org/10.1016/j.jss.2019.110420
- title: Comparing and experimenting machine learning techniques for code smell detection | year: 2016 | systems_and_labels: 74 systems and 1,986 manually validated examples across Data Class, Large Class, Feature Envy, and Long Method. | relevant_results: Cross-validation accuracy exceeded 96% for some algorithms, but performance varied on the full imbalanced dataset; the study emphasizes subjective smell interpretation and incompatible tool results. Accuracy alone is therefore insufficient for a rare detector finding. | url: https://doi.org/10.1007/s10664-015-9378-4

#### Industrial Precedents

- tool: JDeodorant | practice: An Eclipse research plug-in identifies Feature Envy by finding executable Move Method opportunities and checking semantic preconditions, demonstrating a stronger contract than merely counting foreign accesses. | url: https://users.encs.concordia.ca/~nikolaos/jdeodorant/files_JDeodorant/ICSM_2007.pdf
- tool: Designite | practice: The current C#/Java product documents both Feature Envy and Broken Modularization as detectable design smells and reports causes as refactoring clues. | url: https://www.designite-tools.com/docs/features_cs_new.html
- tool: IntelliJ IDEA | practice: Its official Move refactoring documentation explicitly gives a method or field used more in another class than in its own as a motivating case, and limits instance-method targets to parameter or field types while checking conflicts and updating usages. | url: https://www.jetbrains.com/help/idea/move-refactorings.html

#### Conflicting Evidence

The signal has substantial limits. JMove's expert evaluation found realistic precision as low as 5-32% for structural recommenders and dozens of false suggestions in a system with no misplaced methods. Comparative tool studies report context-dependent precision/recall because Feature Envy's informal definition admits different operationalizations. History-only analysis misses new or rarely changed code, while static analysis misses latent evolutionary coupling; HIST observed only 39% overlap between correct historical and JDeodorant results. Co-change itself may reflect commits, tests, cross-cutting requirements, or tangled changes rather than shared ownership. Access to a foreign data carrier may be intentionally external in Visitor, Strategy, functional core/imperative shell, report/query, serializer, mapper, adapter, anti-corruption-layer, and ECS/data-oriented designs. These findings limit the signal to a review aid and argue against automatic moves or universal thresholds.

### Detectability

#### Observable Pattern

For each named function or method, observe syntactically resolvable reads, writes, method calls, destructuring, pattern matching, constructor/reconstruction operations, and chained member traversals. Classify each referenced member/type/module as local, foreign, library/external, or unresolved without inferring intent. A candidate exists when (1) foreign member references materially exceed local member references or local access ratio is low; (2) most foreign references concentrate on one target owner rather than many providers; and preferably (3) the function reconstructs or transforms that owner's representation, repeatedly accesses its members, or co-changes more with the target than its lexical owner. Module aggregation should report several source functions reaching into the same foreign owner or a split cluster of mutually dependent data and behavior. Raw import count or parameter use alone is not Feature Envy.

#### Graph Model

nodes:   - module/file
  - type or data declaration
  - function/method
  - field/property/member
  - parameter/local value
  - optional commit/change-set; edges:   - declares/lexically-owns
  - imports/references-type
  - reads-member
  - writes-member
  - calls-method
  - constructs/reconstructs
  - passes/returns
  - co-changes-with; labels_and_summaries: Edges need source locations, read/write/call kind, direct versus accessor-mediated status, resolution status, and confidence. Function summaries need distinct local members, distinct foreign members, access occurrences, distinct foreign providers, dominant provider, chains, writes, and unresolved accesses. Aggregate function-to-owner weights into module-to-owner matrices, retaining individual paths so a large module cannot hide a single envious function.; candidate_path: lexical module -> function -> access/call/construct edge -> member or type -> declaring type/module; optionally function -> commit <- target member/method for historical corroboration

#### Analysis Requirements

A high-precision implementation requires project-level name and type resolution, receiver-type inference, member-declaration lookup, import/alias resolution, inheritance and extension-method awareness, and distinction between self/local members and foreign members. Accessor-mediated ATFD requires call resolution or library/project summaries. Writes and reconstruction need CFG-aware def-use; closures, returned aliases, heap objects, fluent chains, and indirect mutation require points-to/alias analysis. Candidate Move Method safety further needs call graph and override analysis, visibility/access rules, target signature collision checks, dependency-cycle effects, and language-specific refactoring preconditions. Context and field sensitivity improve precision but are not necessary for a conservative syntax-only MVP if unresolved accesses are excluded and coverage is explicit. Historical corroboration requires entity tracking across renames/moves and commit-size/noise filtering.

#### Tree Sitter Feasibility

Tree-sitter can reliably extract lexical owners, named functions, type/field declarations, parameters, explicit self/this accesses, member-access syntax, method-call syntax, destructuring, constructors, imports, and precise locations. It can conservatively classify explicit self/this members as local and some receivers whose types are syntactically declared as foreign. It can count unresolved receiver/member patterns and dominant receiver identifiers. It cannot by itself reliably resolve inferred or generic receiver types, overloaded or virtual calls, accessors to underlying fields, aliases, re-exports, traits/extensions, dynamic dispatch, reflection, monkey patching, or ownership through containers. Therefore syntax-only results must be labeled partial, must not equate an identifier spelling with a type owner, and should exclude unresolved edges from the positive numerator while displaying them as a coverage limitation.

#### Language Constraints

Java and C# offer the best initial precision because nominal types, explicit members, this-access, and established Move Method semantics are available, although overloads, inheritance, properties, extension methods, Lombok/source generation, and dependency injection still require resolution. Kotlin adds extension functions, properties, implicit receivers, delegation, and top-level functions. Rust has explicit ownership syntax but responsibility does not map one-to-one to memory ownership; traits, deref coercions, associated functions, macros, and module privacy complicate receiver ownership. Go methods and structs are tractable, but interfaces, embedding, package-level functions, and pointer aliases complicate targets. TypeScript is partial when compiler types are unavailable; structural typing, unions, re-exports, decorators, and Vue SFC boundaries reduce certainty. JavaScript, Python, Ruby, and PHP need conservative receiver heuristics because dynamic attributes, monkey patching, metaprogramming, magic methods, and duck typing make provider resolution incomplete. Bash and PowerShell lack a stable class/member model for this smell and should be unsupported initially. C/C++ are only basic source extensions in current Reforge and should also be unsupported until parsed semantic ownership exists.

#### Complexity Cost

Syntax extraction is linear in AST size, O(N), with per-function maps proportional to observed member accesses. Building and resolving a project symbol/type index is approximately O(N + E) time and O(N + E) memory, but language-specific resolution dominates implementation effort. A lightweight historical co-change pass is O(C * K) for C commits touching K tracked entities if large commits are capped; naive all-pairs co-change is O(C * K^2) and must be bounded. Whole-program points-to, context-sensitive call graphs, and automated move-safety checking are high-cost and unsuitable for the MVP. Overall implementation complexity is high across all Reforge languages but moderate for a Java/C# syntax-plus-explicit-type pilot.

### Signal Quality

#### False Positives

- Visitor, Strategy, Comparator, formatter, presenter, report/query, and analytics functions intentionally operate on foreign passive data.
- DTO, record, schema, ORM entity, protobuf, serialization, mapper, adapter, anti-corruption-layer, and API-boundary code intentionally reconstruct representations.
- Application services and use-case orchestrators legitimately coordinate several domain owners without owning their data.
- Functional and data-oriented architectures place behavior outside data types by design; ECS systems intentionally process component data from systems.
- Tests, fixtures, builders, migrations, compatibility shims, generated code, and glue code often traverse foreign representations.
- Fluent APIs and immutable value transformations create many syntactic foreign accesses without leaking mutable representation.
- A tiny source method with zero local state can have an extreme ratio from only one or two harmless accesses.
- Receiver-name heuristics can merge unrelated values or misclassify aliases and accessor calls.

#### Precision Boosters

- Require a minimum evidence floor, such as at least three distinct resolved foreign members or repeated accesses, before evaluating ratios.
- Require one dominant target provider and display ATFD, local-access ratio, foreign-provider count, and dominant-provider share independently.
- Prefer direct reads/writes and repeated deep traversals over ordinary public method calls; weight writes and representation reconstruction separately.
- Require resolvable receiver and declaring-owner types; exclude external libraries and unresolved edges from the decision.
- Corroborate with function-to-target call/entity dependency similarity, duplicate type shape, data clump, dependency hub/cycle, or adapter-boundary evidence.
- Add method-level co-change localization: the source function should co-change more with target-owner entities than with source-owner entities, with minimum support and large-commit filtering.
- Check that a concrete Move Method or Extract-then-Move target exists and that basic visibility, signature, override, and cycle risks are not violated.
- Suppress or downgrade recognized tests, generated code, mappers/serializers/adapters, visitors, DTOs, and report/query namespaces while showing the reason.
- Use project-relative percentiles and require stability across harmless formatting/renaming transformations rather than imposing a universal ratio alone.

#### Legitimate Exceptions

Expected cases include anemic or intentionally passive domain models; functional programming and data-oriented/ECS designs; CQRS read models and reporting/analytics; Visitor, Strategy, Comparator, and pattern-matching dispatch; serializers, mappers, adapters, anti-corruption layers, API clients, ORMs, and schema conversion; UI presenters/view models; migrations and compatibility shims; generated clients/models; test fixtures and assertions; and application-service orchestration. A project may deliberately forbid behavior on data classes or preserve dependency direction, so moving behavior into the envied owner could violate architectural policy or introduce a cycle.

#### Confidence Tier

experimental

### Reforge Integration

#### Existing Capabilities

Reforge already parses named functions, types, parameters, imports, and public items with Tree-sitter across Rust, JavaScript/TypeScript/TSX/Vue, Python, Go, Java, C#, Kotlin, PHP, Ruby, Bash, and PowerShell. Its structural pipeline supplies function/type spans, members, parameters, complexity, nesting, and test classification. The source-file dependency graph supplies resolved local imports, fan-in/out, transitive reach, depth, hubs, cycles, and unresolved-edge receipts. Churn supplies file-level commits, authors, additions/deletions, and recency context, but not yet method-level entity co-change. Relevant corroborators include data_clump, duplicate_type_shape, similar_functions/parallel_implementation, import_heavy_file, dependency_hub, dependency_cycle, adapter_boundary_bypass, and generic_bucket_drift. The report model already supports atomic findings, related locations, typed metrics, detector manifests, coverage statuses, execution receipts, suppressions, and agent-evidence context closure. Missing capabilities are a project symbol/type index, member-use extraction and resolution, function-level history tracking, and refactoring-safety analysis.

#### Proposed Finding Kinds

- name: ownership_leakage | issue_family: responsibility_placement | mechanism: behavior depends disproportionately on data owned by one foreign abstraction | action: review Extract Method plus Move Method, introduce an owner operation, or preserve as intentional boundary code | scope: function with source and target type/module related locations | precision_risk: high for syntax-only analysis; medium when receivers resolve, a dominant target exists, exceptions are filtered, and historical or structural evidence corroborates
- name: broken_modularization_cluster | issue_family: responsibility_placement | mechanism: multiple functions and data declarations that appear to form one responsibility are dispersed across modules | action: review Move Method/Move Field or consolidate an abstraction | scope: cross-module cluster | precision_risk: high; defer beyond MVP because clustering and architectural intent are ambiguous

#### Explanation Path

Each finding must show: source module/type and function location; lexical owner; dominant foreign target type/module and its declaration location; at least three representative access locations with edge kind (read, write, call, reconstruction) and member name; local versus foreign counts; local-access ratio; provider count; dominant-provider share; unresolved-access count; and applicable exception/filter status. If history is used, show commit support counts and representative co-change paths without treating commit adjacency as causality. Related locations should include the target members/methods, relevant import edge, and any dependency-cycle consequence. The recommendation must explain why the target is proposed and state that move safety and architectural intent were not proven.

#### Coverage Contract

Observed means the function, lexical owner, relevant receiver types, and declaring members were parsed and resolved within the scanned project, with the detector executed and no parse failure on the evidence path. Partial means some member accesses, aliases, generated declarations, dependencies, history, or dynamic dispatch remain unresolved; positive findings may use only resolved edges, while an absent finding cannot be read as absence of leakage. Unsupported applies initially to Bash, PowerShell, C/C++, dynamic-only ownership cases, reflection/metaprogramming, macro-generated members, and languages without the detector's required resolver tier. Excluded cases include tests/generated/dependencies according to scan settings and recognized exception categories; exclusion counts and reasons must be reported. Reports must expose analyzed function/access totals, resolved/unresolved edge counts, supported language tier, parse failures, history availability, and whether refactoring preconditions were checked.

#### Mvp Scope

Implement a read-only Java and C# pilot for named instance methods with explicit lexical class, explicit this/self accesses, parameters or fields with syntactically declared nominal types, and member accesses resolvable to project declarations. Compute distinct/occurrence foreign access, local-access ratio, provider count, dominant-provider share, unresolved count, and related access paths. Emit only ownership_leakage when there is a minimum evidence floor, one dominant in-project target, no recognized test/generated/mapper/serializer/visitor exception, and at least one independent Reforge corroborator or optional history signal. Do not perform or guarantee a move. Defer inferred/generic receiver types, inheritance/extension semantics, alias/points-to analysis, accessor-mediated field inference, automatic Move Method safety, method-level git tracking, dynamic languages, Rust/Go-specific ownership models, and Broken Modularization clustering until corpus results justify expansion.

### Validation

#### Fixtures

positive:   - A Java OrderPrinter method in ReportService reads four distinct Order fields, calls two Order-derived calculations, uses no ReportService state, and has Order as the single resolved provider.
  - A C# method destructures and reconstructs one Customer value across repeated accesses, with target-owner co-change support and no mapper/serializer marker.
  - Several source-module functions each traverse the same foreign type and one extracted fragment can be moved without using source state.; negative:   - Visitor, Comparator, Strategy, serializer, ORM mapper, DTO assembler, CQRS query handler, report renderer, and anti-corruption-layer examples with identical raw ratios.
  - A coordinator accesses one member from each of five providers: high foreign count but no dominant owner.
  - A two-line helper with one foreign getter and no local fields; below the evidence floor.
  - A method where moving to the target would invert an architectural dependency or create a file cycle.
  - Dynamic or reflected member access marked partial rather than clean.; metamorphic:   - Renaming identifiers and reformatting must not change resolved metrics or finding identity.
  - Replacing repeated receiver text with a same-typed local alias should preserve a semantic-tier finding and downgrade syntax-only coverage rather than change ownership.
  - Adding unrelated imports or local variables must not affect the signal.
  - Moving the complete method to the dominant owner and updating calls should remove the finding if behavior and access resolution remain equivalent.
  - Adding a second equally used foreign provider should reduce dominant-provider share and suppress or downgrade the finding.
  - Annotating the fixture as generated/test/intentional mapper should change exclusion/suppression receipts, not raw metrics.

#### Corpus Strategy

Build a stratified corpus with Java and C# repositories from domain applications, libraries, frameworks, compilers, IDEs, web services, data pipelines, and GUI systems. Deliberately include Visitor-heavy, DTO/mapping, CQRS, ORM, functional-style, and generated-code projects as hard negatives. Mine real Move Method/Move Field commits using a refactoring miner, then manually verify pre- and post-change responsibility intent; keep synthetic moved-method fixtures only for sensitivity tests. Sample detector negatives as well as positives, stratified by method size, provider count, project, and exception category. Split train/calibration and holdout by repository and organization, never by method, and reserve at least one language/project family for external validation. Compare syntax-only, resolved-static, history-only, and combined variants; evaluate module aggregation separately from method Feature Envy.

#### Oracle And Labels

Use at least two maintainers or experienced language reviewers independently. Separate four labels: (1) instrumentation correctness: are lexical owner, target owner, member edges, and coverage right; (2) smell interpretation: is behavior disproportionately concerned with the proposed target; (3) actionability: keep, extract only, move method, move field, introduce target operation/facade, or intentional exception; and (4) outcome: if applied, did tests preserve behavior and did dependency/co-change localization improve without creating a cycle or policy violation. Reviewers must see evidence paths and code context but not the detector score or each other's labels. Resolve disagreements by adjudication, report agreement (Cohen's kappa or Krippendorff's alpha), and retain an ambiguous label instead of forcing consensus. Real refactoring commits count as positive outcomes only after manual intent and behavior checks; reverted or later-undone moves are tracked separately.

#### Success Criteria

For the Java/C# MVP, require at least 95% precision for owner/member instrumentation on resolved edges and explicit coverage for 100% of analyzed functions. On repository-disjoint holdout data, target finding precision >= 80% with a 95% confidence-interval lower bound >= 70%; report recall against manually verified real opportunities, with >= 40% as an initial useful floor rather than optimizing recall at the cost of review volume. At least 70% of true findings should receive a maintainer label of actionable or useful-for-inspection, and fewer than 10% should recommend a target that violates a known dependency rule or creates a cycle. Finding IDs and metrics must remain identical under formatting/order fixtures and >= 95% stable under identifier-renaming fixtures. Added scan time should be <= 25% and peak memory <= 1.5x on the calibration corpus for the syntax/resolution pass. Every finding and zero-finding run must carry complete execution and coverage receipts; all accepted refactors must pass the repository's existing tests, while test passage is reported as outcome evidence rather than proof of semantic equivalence.


## Pass-through chains and data clumps

### Conclusion

#### Item Name

Pass-through chains and data clumps

#### Thesis

Pass-through chains are useful only as experimental composite evidence of misplaced orchestration or weak abstraction boundaries. Hop count alone is not a finding. A candidate should require a value or repeated parameter group to cross multiple resolved functions and modules while remaining mostly identity-preserving, receiving little local use or transformation, and being corroborated by an independent signal such as DataClump or boundary bypass. The result should invite inspection for an aggregate parameter object, a direct dependency, or relocation of orchestration; it should not prescribe an automatic refactoring.

#### Refactoring Relevance

A value forwarded unchanged through several layers suggests a behavior-preserving structural hypothesis: eliminate a redundant delegate, shorten an unnecessary message chain, move orchestration to the boundary that owns the operation, or replace a recurring primitive parameter group with a typed parameter object. The hypothesis is strongest when intermediate functions neither validate, authorize, transform, branch on, store, nor combine the forwarded values and when the chain crosses module ownership boundaries. The behavior-preservation claim applies only after retaining required ordering, error mapping, instrumentation, transaction, security, and lifecycle semantics. A long path may instead express an intentional facade, port/adapter boundary, pipeline, middleware stack, CQRS command/query route, event choreography, typed-newtype transition, or generated client forwarding, in which case shortening it can damage the architecture.

#### Evidence Strength

moderate for the underlying concepts and weak-to-speculative for a cross-language Reforge detector. Program dependence graphs provide a strong formal basis for tracking value dependences, and Data Clumps, Message Chains, and Middle Man are established smell labels. Hall et al. provide empirical evidence that some smells have statistically significant but small fault effects, which supports prioritization but not arbitrary refactoring. CodeQL demonstrates practical local/global data-flow analysis while warning that global flow costs more and is less precise. No supplied evidence directly validates hop-count-based pass-through detection or Reforge-specific thresholds, so the proposed finding must remain experimental and corroborated.

### Evidence

#### Foundational Sources

- title: The Program Dependence Graph and Its Use in Optimization | year: 1987 | authors: Jeanne Ferrante, Karl J. Ottenstein, and Joe D. Warren | claim: Introduced the program dependence graph as a unified representation of control and data dependences. It supplies the formal foundation for tracing whether definitions and uses propagate through procedures, but does not itself equate a long dependence path with a design defect. | url: https://doi.org/10.1145/24039.24041
- title: Refactoring: Improving the Design of Existing Code | year: 1999 | claim: Established Data Clumps, Message Chains, and Middle Man as refactoring smells and connects them to parameter objects, hiding delegates, and removing unnecessary intermediaries. These labels motivate candidate treatments but remain contextual design judgments. | url: https://martinfowler.com/books/refactoring.html

#### Empirical Sources

- title: Some Code Smells Have a Significant but Small Effect on Faults | year: 2014 | authors: Tracy Hall et al. | systems_and_labels: Empirical study of code-smell occurrence and fault outcomes; relevant labels include Data Clumps, Message Chains, and Middle Man. | relevant_results: The study reports that some smell-fault relationships are statistically significant but their effects are small. This supports using smells as weak prioritization evidence and directly limits the inference that an arbitrary detected chain deserves refactoring. | url: https://doi.org/10.1145/2629648

#### Industrial Precedents

- tool: CodeQL | practice: Official data-flow documentation distinguishes local from global flow, models value-preserving flow and non-value-preserving taint separately, and requires explicit source/sink modeling for useful global queries. It warns that global flow is more expensive and less precise, supporting bounded, question-specific analysis rather than enumerating every long path. | url: https://codeql.github.com/docs/writing-codeql-queries/about-data-flow-analysis/
- tool: Reforge | practice: The existing DataClump detector identifies repeated parameter-name groups, while the file dependency graph supplies import/dependency paths. Together they can corroborate a future pass-through prototype, although neither currently proves value flow through calls. | url: repository-local implementation

#### Conflicting Evidence

The strongest supplied empirical result is cautionary: Hall et al. found effects on faults that were significant in some cases but small, so smell presence does not establish harmfulness or expected refactoring payoff. The smell categories also describe different mechanisms: repeated parameters, excessive delegation, and client traversal can coincide but are not interchangeable. CodeQL's official guidance states that global flow is more expensive and less precise and should be constrained by sources and sinks. Architecturally meaningful facades, middleware, pipelines, adapters, CQRS/event routes, typed wrappers, dependency-injection seams, telemetry/error boundaries, and generated forwarders all produce long identity-looking paths. These limits rule out raw hop thresholds, automatic deletion of delegates, and claims that zero findings prove the absence of pass-through chains.

### Detectability

#### Observable Pattern

Starting at a named parameter, argument expression, field read, or return value, observe a sequence of resolved call arguments and callee parameters/returns. A candidate path spans at least several functions and preferably multiple files/modules. At each intermediate step, the tracked value or tracked group is passed onward with the same resolved identity or a transparent projection/wrapper; the intermediate function performs little or no local use, branching, validation, transformation, persistence, or combination involving it. For a group candidate, the same two-or-more parameter names or resolved origins travel together across edges. The observable evidence must distinguish exact identity, simple renaming, field projection, transparent wrapping/unwrapping, and unknown transformation. A long call chain without a traced value, or a repeated parameter group without forwarding, is not this signal.

#### Graph Model

nodes:   - module/file
  - function/method
  - call site
  - formal parameter
  - argument/value definition
  - return value
  - field or typed wrapper; edges:   - module declares function
  - caller invokes callee
  - actual argument binds to formal parameter
  - definition reaches use
  - parameter flows to argument
  - value flows to return
  - field projected from value
  - wrapper constructed or unwrapped
  - file depends on file; labels_and_summaries: Every flow edge needs a source location, resolution status, language, flow kind, identity-preservation class, and confidence. Each intermediate function needs counts of local uses, forwarding uses, transformations, control predicates, writes, and outgoing calls. Function summaries should map input positions to output call positions and return positions. Module aggregation must retain the concrete function path and collapse recursive cycles safely rather than treating repeated traversal as extra hops.; candidate_path: source definition -> caller argument -> callee formal -> zero or more local aliases/projections -> next call argument or return -> ... -> sink/use, with function and module ownership attached at every interprocedural boundary

#### Analysis Requirements

A defensible detector needs project-level symbol and call resolution, actual-to-formal parameter binding, a control-flow graph, and def-use/SSA-like value tracking within each function. Interprocedural summaries must record which inputs can reach call arguments or returns and whether the transfer is identity-preserving. Dynamic dispatch, callbacks, higher-order functions, overloads, generics, default/named/variadic arguments, closures, async boundaries, re-exports, and indirect calls require conservative call-graph modeling. Alias and points-to analysis are needed when values pass through fields, mutable containers, object properties, or receiver state. Field sensitivity helps distinguish forwarding one member from forwarding the whole value; context sensitivity prevents unrelated call sites from being merged. Library/framework models are needed for transparent wrappers, middleware APIs, futures/promises, serialization, and generated clients. A source/sink-scoped global traversal with depth/state budgets is preferable to unrestricted all-pairs flow.

#### Tree Sitter Feasibility

Tree-sitter can reliably extract functions, formal parameter names and positions, call expressions, actual argument syntax, returns, simple assignments, member accesses, imports, modules, and precise source locations. It can conservatively recognize direct forwarding such as f(x) calling g(x), returning g(x), or passing the same pair of identifiers to another call; it can also join those observations to Reforge's repeated parameter-name groups. Syntax alone cannot reliably resolve callees, bind named/default/variadic parameters, distinguish shadowed identifiers across scopes without a symbol layer, identify aliases through branches and fields, prove that a wrapper is transparent, or account for dispatch, callbacks, macros, decorators, reflection, and generated code. A Tree-sitter-only prototype may produce path candidates with partial coverage, but must not call them proven interprocedural value flow.

#### Language Constraints

Rust, Java, C#, Go, Kotlin, and typed TypeScript offer the best pilot surface when calls and explicit parameters can be resolved, but traits/interfaces, virtual dispatch, extension methods, generics, implicit receivers, macros, and async lowering still limit precision. Rust ownership types can make explicit newtype conversions meaningful transformations rather than redundant hops. JavaScript, Python, Ruby, PHP, and untyped TypeScript have dynamic dispatch, decorators/metaprogramming, monkey patching, flexible arguments, and runtime property mutation, so syntax-only paths are often partial. Go interfaces, embedding, and goroutines complicate interprocedural destinations; Java/C# reflection and dependency injection do likewise. Vue component boundaries add template/event flow not represented by ordinary calls. Bash and PowerShell pipelines and dynamic command resolution need separate semantics and should be unsupported initially. Cross-process RPC, queues, databases, and event buses terminate the static call/def-use graph unless explicit framework models exist.

#### Complexity Cost

AST candidate extraction is approximately O(N) in syntax size. Intraprocedural CFG and def-use construction is roughly O(N + E) per project representation. A resolved call graph and cached input/output summaries are O(V + E) for storage, but propagation can revisit summary states until a fixed point and grow substantially with call contexts, aliases, and fields. Unrestricted all-pairs global flow is expensive and imprecise; source/sink scoping, path-length limits, per-function summaries, strongly connected component condensation, and state budgets are required. Implementation complexity is high because Reforge currently lacks a call graph and def-use graph; a direct-identifier pilot is moderate but offers only partial coverage.

### Signal Quality

#### False Positives

- Facades deliberately stabilize a public API while delegating implementation.
- Pipelines and middleware intentionally pass a request, context, or event through many stages.
- CQRS handlers, event choreography, actors, and message buses encode explicit routing boundaries.
- Ports/adapters, anti-corruption layers, RPC clients, repositories, and controllers preserve dependency direction while forwarding values.
- Authentication, authorization, tracing, metrics, transactions, retries, cancellation, error translation, and resource-lifecycle code may use a value semantically even when syntax shows little transformation.
- Typed newtypes, validation wrappers, capability tokens, and ownership/lifetime transitions may look identity-preserving but enforce invariants.
- Generated clients, proxies, bindings, mocks, compatibility shims, and framework glue are forwarding by design.
- Builders and configuration propagation naturally carry repeated parameter groups.
- Identifier equality can conflate shadowed or unrelated values; field projection and reconstruction can hide real transformations.
- Recursive calls and cycles can inflate a naive hop count.

#### Precision Boosters

- Require resolved actual-to-formal bindings and a concrete inspectable path rather than matching identifier text alone.
- Require mostly identity-preserving forwarding across the path and report the exact proportion and any unknown edge.
- Require cross-module span and at least two semantically empty intermediate functions, while treating the hop threshold as calibration data rather than proof.
- Require low local-use and transformation counts at intermediate nodes, including checks for control predicates, writes, validation, wrapping, logging, error mapping, authorization, and lifecycle actions.
- Require an independent corroborator such as Reforge DataClump, adapter-boundary bypass, duplicate wrapper signatures, dependency indirection, or repeated parallel paths.
- Prefer repeated parameter groups whose resolved value origins remain together across multiple calls over name coincidence.
- Suppress generated, test, facade, middleware, pipeline, adapter, CQRS, event, proxy, and typed-newtype contexts when recognized; otherwise display the exception risk.
- Bound analysis to selected source categories and sinks, following CodeQL's source/sink-scoped global-flow precedent.
- Show unresolved edges and exclude them from positive identity-preservation claims.
- Check that the proposed shortening would not violate module dependency direction or remove required policy behavior.

#### Legitimate Exceptions

Expected and often beneficial cases include public facades, layered APIs, request/response pipelines, middleware, interceptors, decorators, CQRS command/query handlers, domain/event choreography, actors, ports-and-adapters and anti-corruption layers, controllers and application services, repositories, RPC and SDK clients, proxies, compatibility shims, dependency-injection wiring, telemetry/security/transaction boundaries, typed newtypes and capability wrappers, immutable context propagation, generated forwarding code, mocks, and framework glue. These structures may intentionally trade more hops for stable interfaces, policy enforcement, testability, observability, or dependency inversion.

#### Confidence Tier

experimental

### Reforge Integration

#### Existing Capabilities

Reforge already detects repeated parameter-name groups as DataClump, which can identify groups worth tracing and serve as independent corroboration. Its source-file dependency graph provides resolved local file edges, module span, transitive paths, hubs, cycles, and boundary-oriented context. Tree-sitter adapters already expose named functions, parameters, calls/import syntax, and locations across supported languages, and the report system can present related locations, metrics, coverage, and suppressions. Relevant corroboration includes adapter_boundary_bypass and structural dependency findings. Reforge does not currently have a resolved call graph, actual-to-formal binding, CFG-backed def-use graph, interprocedural summaries, or points-to analysis, so its dependency graph must not be presented as value flow.

#### Proposed Finding Kinds

- name: pass_through_chain | issue_family: data_journey_shape | mechanism: a resolved value is forwarded mostly unchanged through low-use intermediaries across module boundaries | action: inspect whether to remove a redundant delegate, expose a direct operation, or relocate orchestration while preserving policy boundaries | scope: one source-to-sink value path with every function and flow edge as related evidence | precision_risk: high; emit only as experimental composite evidence with corroboration
- name: forwarded_data_clump | issue_family: parameter_grouping | mechanism: the same resolved group of parameters repeatedly travels together, mostly unchanged, across functions/modules | action: inspect Introduce Parameter Object or a domain-specific typed value, or keep the group explicit if it represents a boundary contract | scope: parameter group and interprocedural path | precision_risk: medium-to-high because name groups and boundary DTOs can be intentional

#### Explanation Path

A finding must show the source definition and terminal use/sink; every call site, caller, callee, actual argument, and bound formal parameter in order; module/file transitions; and whether each edge is direct identity, alias, projection, transparent wrapper, transformed, or unresolved. For every intermediate function, show forwarding-use, local-use, transformation, branch/predicate, write, and policy-action counts plus representative locations. Display function hops, SCC-condensed path, module span, identity-preservation ratio, unresolved-edge count, group width/cohesion where applicable, and the independent DataClump/boundary/dependency corroborator. Also show recognized exception markers and explain that architectural intent and behavior-preserving removal were not proven.

#### Coverage Contract

Observed means all reported functions parsed successfully, every displayed caller-to-callee edge and actual-to-formal binding resolved within the project, and every positive flow step has an explicit identity/transformation classification. Partial means any call target, dispatch alternative, alias, wrapper, generated declaration, macro expansion, callback, field flow, or dynamic edge is unresolved; partial paths may be displayed as candidates but cannot support a high-confidence conclusion, and a zero result does not mean absence. Unsupported initially includes Bash/PowerShell pipelines, reflection/metaprogramming, runtime message routing, cross-process queues/RPC without models, macro-generated call graphs, and languages without the selected resolver tier. Excluded generated, dependency, test, and recognized architecture categories must have counts and reasons. Reports must include parsed-function totals, call-site totals, resolved/unresolved bindings, summarized-flow totals, truncated searches, source/sink selection, budgets, supported languages, parse failures, and detector version.

#### Mvp Scope

Build a read-only prototype for direct named-function forwarding in one statically tractable language already parsed by Reforge. Index functions and unambiguous project-local calls, bind positional arguments to formal parameters, and recognize only direct identifier aliases and direct parameter-to-call/return flows within straight-line function bodies. Seed traversal only from existing DataClump groups or explicit boundary-bypass candidates, require cross-file/module span, low local use, mostly identity-preserving edges, and at least one independent corroborator. Emit an experimental path with complete coverage receipts; do not auto-refactor or claim semantic redundancy. Defer virtual/dynamic dispatch, callbacks, fields/heap aliases, closures, exceptions, complex branches, async/event flow, wrapper inference, points-to/context-sensitive analysis, unrestricted all-pairs flow, and broad multi-language rollout.

### Validation

#### Fixtures

positive:   - A three-value DataClump is passed unchanged from an HTTP handler through two project modules to a service, while both intermediaries only call the next function.
  - A single request value crosses three resolved functions in different modules; two intermediaries have zero local use and an independent boundary-bypass signal identifies an avoidable layer.
  - Two parameters are locally renamed but their resolved definitions remain together across the path, demonstrating semantic rather than spelling-based flow.; negative:   - A facade delegates one hop to preserve a stable API.
  - Authentication middleware forwards a request after checking credentials and adding policy context.
  - A pipeline transforms the value at every stage.
  - A CQRS command passes through dispatcher and handler boundaries.
  - A typed newtype wrapper validates a primitive before forwarding.
  - Generated proxy and RPC client methods mirror signatures.
  - A long call chain in which no single value is tracked end to end.
  - The same parameter names occur in unrelated scopes but definitions do not connect.; metamorphic:   - Identifier renaming and formatting must preserve resolved paths and finding IDs.
  - Introducing a semantically equivalent local alias must preserve the path.
  - Adding a real validation, branch, or transformation must increase local-use/transformation metrics and suppress or downgrade the finding.
  - Adding an unrelated call or import must not affect the path.
  - Replacing one redundant intermediary with a direct call must shorten or remove the finding without changing terminal behavior.
  - Moving a path into generated code must change exclusion receipts while leaving raw instrumentation explainable.
  - Adding recursion must not inflate hop count after SCC condensation.

#### Corpus Strategy

Construct a stratified corpus spanning service applications, layered enterprise systems, libraries, compilers, CLI tools, web frameworks, data pipelines, and SDK/client generators. Oversample hard negatives: middleware-heavy frameworks, CQRS/event-driven projects, hexagonal architecture, typed-newtype Rust, RPC clients, proxies, and generated code. Mine commits that introduce parameter objects, remove middle men, hide delegates, collapse chains, or relocate orchestration; manually verify intent and preserve pre-change snapshots as candidate positives. Also sample detector negatives and random flow paths to estimate missed opportunities. Split calibration and holdout by repository and organization, not by function, and keep architecture families represented in both while reserving at least one family for external validation. Compare hop-only, identity-only, DataClump-seeded, boundary-seeded, and composite variants to measure the value of each corroborator.

#### Oracle And Labels

At least two experienced reviewers should independently label four layers: (1) instrumentation correctness—call target, argument binding, def-use edge, identity/transformation classification, and coverage; (2) design interpretation—redundant pass-through, necessary architectural boundary, ambiguous, or false path; (3) actionability—keep, remove delegate, hide delegate/direct call, relocate orchestration, introduce parameter object/typed value, or another treatment; and (4) outcome—tests preserved behavior and the change reduced avoidable coupling/parameter repetition without violating dependency or policy constraints. Reviewers should see full path context and exception evidence but not detector scores or each other's labels. Report agreement and retain ambiguity after adjudication. A historical refactoring is not automatically a positive unless its intent and subsequent survival are verified.


## Reforge feasibility and validation design

### Conclusion

#### Item Name

Reforge feasibility and validation design

#### Thesis

Data movement can become a useful Reforge refactoring signal if the product detects only source-observable, explainable value-transfer structures and reports semantic gaps explicitly. The recommended architecture is a lightweight, language-adapted flow graph derived from Tree-sitter, not a claim of whole-program semantic equivalence to CodeQL or Joern. Begin with exact lexical def-use, direct calls, parameter-to-return summaries, and module aggregation; treat dynamic dispatch, heap flow, reflection, macros, generated code, and unmodeled libraries as unresolved coverage rather than inferred facts.

#### Refactoring Relevance

A flow observation is refactoring-relevant when it shows that a value crosses more module boundaries, visits more pass-through functions, fans out to more consumers, or bypasses a named boundary more than nearby flows, and when a behavior-preserving structural action is plausible: move transformation closer to ownership, introduce or restore an adapter, replace a long pass-through chain with a stable data contract, split an over-broad transfer object, or consolidate duplicate mapping. Flow alone does not establish abnormality or intent. Reforge should require structural context such as dependency direction, adapter naming, repeated transformation, churn, data clumps, or multiple independent paths before emitting an actionable issue.

#### Evidence Strength

Moderate. Program dependence graphs, slicing, information-flow metrics, and production semantic engines establish that data-flow structure is analyzable and useful. Reforge already has parsers, dependency aggregation, evidence paths, precision-risk metadata, and coverage receipts that lower integration cost. However, there is no validated public benchmark for 'abnormal module-to-module data movement as a refactoring smell,' and syntax-only interprocedural resolution is intrinsically incomplete. The product thesis therefore requires a staged benchmark and maintainer labeling before default-on findings are justified.

### Evidence

#### Foundational Sources

- Ferrante, Ottenstein, and Warren, 'The Program Dependence Graph and Its Use in Optimization' (1987), DOI 10.1145/24039.24041. The paper makes control and data dependence explicit in one intermediate representation and demonstrates its use for program transformations. https://doi.org/10.1145/24039.24041
- Weiser, 'Program Slicing' (1981), ICSE 5, pp. 439-449. It defines a data-flow-based approximation of the statements affecting a value at a program point, which is the conceptual basis for inspectable predecessor paths. https://dl.acm.org/doi/10.5555/800078.802557
- Henry and Kafura, 'Software Structure Metrics Based on Information Flow' (1981), DOI 10.1109/TSE.1981.231113. It proposes fan-in/fan-out-derived measures intended to expose structural flaws in large systems, supporting module-level flow aggregation while not validating Reforge's proposed thresholds. https://doi.org/10.1109/TSE.1981.231113
- Yamaguchi, Golde, Arp, and Rieck, 'Modeling and Discovering Vulnerabilities with Code Property Graphs' (2014), DOI 10.1109/SP.2014.44. It unifies AST, CFG, and program-dependence information as a queryable graph, demonstrating the richer semantic reference model against which a lightweight Reforge graph can be compared. https://doi.org/10.1109/SP.2014.44

#### Empirical Sources

- Boland and Black, 'The Juliet 1.1 C/C++ and Java Test Suite' (2012), DOI 10.1109/MC.2012.345. Juliet contains more than 81,000 synthetic programs covering 181 CWEs, with flawed and similar non-flawed variants and control/data-flow variants. It is useful for instrumentation and path-coverage tests, but its security labels are not an oracle for refactoring actionability. https://www.nist.gov/publications/juliet-11-cc-and-java-test-suite
- OWASP Benchmark provides expected-result labels, true and false cases, and automated TP/FN/TN/FP scoring. It is useful for differential flow-engine experiments in Java and Python, but its sink-oriented vulnerability oracle must remain separate from Reforge smell labels. https://owasp.org/www-project-benchmark/
- Feng et al., 'An Empirical Study of Untangling Patterns of Two-Class Dependency Cycles' (2023), analyzed 38 open-source projects and manually inspected 587 successful and 69 unsuccessful cycle-untangling cases; five patterns covered 91.3% of successful cases, and design context affected the chosen repair. This supports labeling context and actionability independently from graph detection. https://arxiv.org/abs/2306.10599
- Tsantalis, Ketkar, and Dig, 'RefactoringMiner 2.0' (2022), DOI 10.1109/TSE.2020.3007722, reports a curated refactoring oracle and high precision/recall for mined Java refactorings. Its commit candidates can seed before/after flow studies, but RefactoringMiner labels refactoring operations rather than whether a pre-change flow was abnormal. https://doi.org/10.1109/TSE.2020.3007722
- Gnoyke et al., 'Evolution patterns of software-architecture smells' (2024), DOI 10.1016/j.jss.2024.112170, studies 485 releases of 14 open-source systems and finds that dependency smells evolve differently and can become tangled. This supports longitudinal evaluation and relative baselines, but the study analyzes dependency smells rather than value-flow smells. https://doi.org/10.1016/j.jss.2024.112170

#### Industrial Precedents

- CodeQL models semantic data-flow nodes and edges distinct from AST nodes, offers local and global flow, and documents that global flow includes calls and properties but costs substantially more time and memory. It also documents unavoidable challenges from unavailable libraries, runtime call targets, aliasing, and graph size. https://codeql.github.com/docs/writing-codeql-queries/about-data-flow-analysis/
- Semgrep documents sources, sinks, sanitizers, and explicit propagators. Its documentation states that intraprocedural analysis cannot infer call propagation without propagator models and that interfile analysis is proprietary, illustrating both the usefulness of a rule-oriented lightweight UX and the boundary of its OSS engine. https://docs.semgrep.dev/writing-rules/glossary
- Joern generates layered code property graphs, supplies static taint analysis and extensible passes, and has language-specific frontends. It is the closest open semantic comparison engine, but its official large-codebase example warns about separate JVMs and demonstrates 30 GB frontend and 80-100 GB query-process heaps for an old Linux-kernel import. https://docs.joern.io/ and https://docs.joern.io/installation/
- Tree-sitter officially provides incremental concrete syntax trees, error recovery, and a dependency-free runtime. It supplies robust syntax locations but not name resolution, CFG, def-use, call targets, points-to facts, or library behavior by itself. https://tree-sitter.github.io/tree-sitter/

#### Conflicting Evidence

- The strongest tools do not derive whole-program flow from syntax alone: CodeQL requires semantic databases and library models; Joern adds multiple CPG passes and language frontends; Semgrep requires rule-specified propagation and reserves interfile analysis for its proprietary engine. This limits any claim that a common Tree-sitter adapter can provide equal recall.
- A high boundary-crossing count can reflect deliberate architecture: request pipelines, middleware, event buses, ETL, serializers, DTO layers, ports-and-adapters, CQRS, telemetry, and audit chains intentionally move data through modules.
- Static may-flow over-approximations can inflate paths through aliases and dynamic dispatch; syntax-only under-approximations can silently miss paths. Reforge must not turn absence of an observed path into evidence that no path exists.
- Juliet and OWASP Benchmark validate security-flow reachability, not maintainability or behavior-preserving refactoring. Good performance on them is necessary for graph correctness experiments but insufficient for product validity.
- The cycle-untangling study shows that graph shape alone does not determine the repair; neighboring design context matters. Flow findings therefore need contextual corroboration and human actionability labels.

### Detectability

#### Observable Pattern

For each supported function, observe syntactic value origins (parameters, local definitions, literals, field/property reads, and selected external-return sources), value-preserving transfer steps (assignment, destructuring, argument binding, return, and conservative container construction), direct statically resolvable calls, and module-boundary crossings. Aggregate only witnessed paths. Candidate pressure patterns are: values crossing an unusually high number of internal module boundaries; functions whose dominant role is passing unchanged values between modules; one source value fanning out across many modules; many unrelated source values converging on a module that does not appear to own them; parallel conversion chains between the same module pair; and direct flows that bypass an observed adapter path. 'Unusual' must be repository-relative and separately corroborated, never inferred from a universal path-length threshold alone.

#### Graph Model

Use a compact layered graph. Nodes: value occurrences, lexical definitions, parameters, returns, direct call sites, functions, fields only when exactly named, files, and normalized modules/directories. Edges: defines, reads, assigns, argument-to-parameter, return-to-call-result, value-preserving derives, contains, imports/resolves-to, and crosses-module. Labels: language, path/line/range, transfer kind, exact-versus-heuristic resolution, transformation flag, test/generated status, and unresolved reason. Compute intraprocedural def-use first; summarize each function as parameter-or-source to return-or-sink relations; compose summaries only across exact direct calls under a bounded depth; collapse recursive SCCs; and aggregate distinct witnessed value paths onto module edges without discarding the underlying locations. Keep the existing file dependency graph separate so a finding can show both a value-flow path and its import/dependency context.

#### Analysis Requirements

Required for MVP: lexical scopes; declaration/use binding without type inference; basic block/branch-aware reaching definitions sufficient to avoid obvious use-before-definition mistakes; direct free/static call resolution; argument/parameter and return mapping; conservative function summaries; module mapping; and deterministic path reconstruction. Stronger phases require full CFG joins and loop handling, interprocedural call graphs, virtual/trait/interface dispatch, overload resolution, points-to/alias analysis, heap and field sensitivity, context sensitivity, exception/async flow, macro or generated-code expansion, and framework/library summaries. CodeQL should be the semantic reference for languages it supports; Joern should be a second reference where a mature frontend exists; Semgrep should benchmark rule ergonomics and configured propagation rather than serve as the sole oracle.

#### Tree Sitter Feasibility

Reliable from current Reforge syntax trees: function boundaries, parameters, returns, assignments, many direct call expressions, imports, paths/locations, branch/loop syntax, and parse-error detection. Conservative with adapter work: lexical scopes, local def-use, value-preserving expression edges, direct package/module-qualified calls, named imports, and parameter-to-return summaries. Not derivable from Tree-sitter alone: semantic symbol identity, type-driven overload resolution, virtual dispatch, trait/interface targets, aliases through heap objects or pointers, reflection/eval, macro expansion semantics, framework injection, serialization/RPC behavior, or unavailable-library flow. Every composed edge must therefore carry a resolution class, and paths containing heuristic edges must not produce default actionable findings.

#### Language Constraints

- Tier 1A, first implementation: Rust direct free-function paths with explicit crate/self/super/module qualification, lexical locals, assignments, arguments, and returns. Exclude method dispatch, trait calls, macro-expanded flow, unsafe pointer flow, closures escaping scope, and field-sensitive heap flow. Rust is the self-hosting choice for Reforge fixtures and profiling, not a claim that Rust is semantically easiest.
- Tier 1B, next adapters: Go package-qualified functions and Java/C#/Kotlin static or uniquely indexed calls. Go interface dispatch and closures, and JVM/.NET overloads, inheritance, annotations, generated members, and dependency injection remain unresolved unless semantic metadata is added.
- Tier 2, heuristic/experimental only: JavaScript/TypeScript and Vue script, Python, PHP, and Ruby. Named imports and direct lexical calls can be witnessed, but monkey patching, dynamic imports, decorators, metaprogramming, prototype changes, framework wiring, and runtime module behavior prevent completeness. TypeScript type information is not available from Tree-sitter.
- Tier 3, local observations only: Bash and PowerShell. Pipelines, string expansion, command discovery, dot-sourcing, environment state, and dynamic invocation make cross-file value identity too weak for actionable module-flow findings without shell-specific abstract interpretation.
- Repository-language tier must be capability-based rather than a single supported/unsupported boolean: local lexical flow, direct-call composition, field flow, dynamic dispatch, library models, and generated-code visibility each receive their own observed/partial/unsupported receipt.

### Signal Quality

#### False Positives

- Deliberate layered pipelines, compiler passes, middleware chains, ETL/data-engineering flows, parsers, render pipelines, and workflow orchestrators.
- Ports-and-adapters, anti-corruption layers, serialization boundaries, DTO mapping, RPC clients, event buses, CQRS, and message routing where translation and boundary crossings are intentional.
- Cross-cutting observability, logging, metrics, tracing, auditing, authorization context, cancellation tokens, and request metadata.
- Facade and compatibility modules intentionally forwarding data while shielding callers from churn.
- Repository layouts where directories are deployment or ownership units rather than semantic modules.
- Test builders and fixtures that intentionally fan out shared values, and generated binding/client code that repeats transformations.

#### Precision Boosters

- Require all actionable paths to contain only exact lexical and direct-call edges; show heuristic paths as experimental evidence only.
- Require at least two independent signals: extreme repository-relative path length or fan-out plus dependency hub/cycle, adapter-boundary bypass, data clump, repeated mapping shape, churn hotspot, or duplicate type shape.
- Compare against peers of the same language and module role; use robust percentiles and a minimum population rather than global fixed thresholds.
- Down-rank or exclude tests, generated code, vendor code, migrations, serializers, CLI wiring, and files with parser errors or unresolved imports.
- Require repeated paths or multiple distinct values; a single long path is evidence to inspect, not a finding.
- Display transformations and named boundaries so reviewers can distinguish intentional conversion from passive forwarding.
- Use longitudinal evidence: persistence across releases and co-change/churn near the path increase priority, while stable intentional pipelines reduce it.
- Let maintainers configure module roots, allowed boundary pairs, adapter patterns, and expected infrastructure flows.

#### Legitimate Exceptions

Expected architectures include pipeline and streaming systems, compilers, interpreters, middleware, event-driven systems, ETL, message brokers, workflow engines, ports-and-adapters, CQRS/event sourcing, serialization/RPC layers, compatibility facades, telemetry/audit infrastructure, dependency-injection composition roots, and test-data factories. These should be suppressible by directory/module role and by named allowed flows, and should remain visible in raw metrics when users request them.

#### Confidence Tier

Experimental detector tier; raw metrics and coverage receipts may ship before actionable findings. Promotion to heuristic requires labeled-corpus precision and usefulness targets; promotion to conservative requires exact-edge-only findings and stable cross-version validation.

### Reforge Integration

#### Existing Capabilities

Reforge already parses Rust, JavaScript/TypeScript/Vue script, Python, Go, Java, C#, Kotlin, PHP, Ruby, Bash, and PowerShell through Tree-sitter adapters (`src/lang/mod.rs`); extracts function structure and parameters; records parse failures; builds a file-level dependency graph with nodes, edges, fan-in/out, SCC cycles, hubs, depth, and unresolved-edge counts (`src/detectors/dependency_graph.rs` and `resolution.rs`); emits related locations and normalized metrics; classifies findings by quality construct, signal mechanism, action, entity scope, detection approach, precision risk, and evidence role (`src/detectors/manifest.rs`); and produces coverage manifests, detector-execution receipts, raw-metric coverage, unsupported-language status, and unobservable reasons (`src/model/coverage.rs`, `src/scan/coverage.rs`). Existing corroborators include DependencyCycle, DependencyHub, ImportHeavyFile, DataClump, DuplicateTypeShape, AdapterBoundaryBypass, ConfigKeyDrift, churn summaries, and the issue/evidence aggregation model. Gaps are semantic scopes, reusable CFG/def-use IR, call summaries, method/call resolution across most languages, and a function/module entity scope capable of storing flow-path evidence.

#### Proposed Metrics

- flow.observed_value_edges: count of witnessed value-preserving edges included in the graph.
- flow.unresolved_edges: count of candidate transfers not composed because symbol, call target, field, macro, dynamic dispatch, or library semantics were unresolved; also break down by reason.
- flow.direct_call_resolution_percent: exact resolved direct internal calls divided by eligible direct internal call expressions times 100.
- flow.module_crossings: number of internal module-boundary edges on a witnessed source-to-use path.
- flow.pass_through_functions: count of functions on a path whose observed summary transfers an input to output without an observed semantic transformation.
- flow.distinct_modules_reached: count of unique internal modules reached by a witnessed origin under the configured depth bound.
- flow.value_fan_out_modules: unique destination modules receiving values derived from the same origin.
- flow.module_pair_paths: number of distinct witnessed value paths between an ordered module pair.
- flow.transformation_steps: count of explicitly observed non-identity construction/call operations along a path; context-only until adapters can classify transformations reliably.
- flow.path_exactness_percent: exact-resolution edges divided by all edges in the displayed path times 100; actionable MVP paths require 100%.
- flow.covered_functions and flow.eligible_functions: counts used to disclose coverage denominator rather than report only findings.
- flow.summary_cache_hits and flow.invalidated_summaries: operational metrics for later incremental scanning, not quality signals.

#### Proposed Finding Kinds

- LongDataTransit: family `data_flow_coupling`; mechanism `dependency_propagation`; action `reduce_dependency_coupling`; scope `finding_group` anchored at the origin; reports repeated exact paths with unusually many module crossings/pass-through functions. Precision risk: high until corpus-calibrated, then medium.
- DiffuseDataFanOut: family `data_flow_coupling`; mechanism `responsibility_dispersion` or `dependency_propagation`; action `decompose_responsibility`; scope `function` or `finding_group`; reports one origin reaching an extreme number of internal modules with corroborating hub/churn evidence. Precision risk: high.
- AdapterFlowBypass: family `boundary_integrity`; mechanism `dependency_propagation`; action `reduce_dependency_coupling`; scope `finding_group`; requires an observed normal adapter route plus a direct exact bypass carrying a comparable value shape. Precision risk: medium and the best candidate for the first actionable detector.
- ParallelDataTransformation: family `duplication_consolidation`; mechanism `duplication_divergence`; action `consolidate_duplication`; scope `finding_group`; reports repeated module-pair mapping chains corroborated by similar functions or duplicate type shapes. Precision risk: medium-high.
- PassThroughModulePressure: family `responsibility_decomposition`; mechanism `responsibility_dispersion`; action `decompose_responsibility`; scope `file` or `directory`; reports a module dominated by unchanged forwarding and supported by import-heavy/hub evidence. Precision risk: high; raw metric only in MVP.

#### Explanation Path

Each finding must include: origin path, line, symbol, and origin kind; every intermediate assignment/call/parameter/return with path and line; module boundary labels; destination use; exact or heuristic resolution on every edge; transformation labels; the shortest witnessed path plus the number of additional distinct paths; threshold, repository percentile, and peer population; all corroborating finding IDs; excluded/unresolved alternatives that could change the conclusion; and a concrete inspection statement such as 'value from A::request crosses modules A -> B -> C -> D through three unchanged parameter/return summaries before use at D'. Related locations should include each boundary and the proposed refactoring seam, not merely every node in a large reachability set.

#### Coverage Contract

Reports must separate: Observed-exact (syntax parsed and all displayed edges lexically/directly resolved); Observed-heuristic (a path exists but contains explicitly labeled approximate edges); Partial (eligible functions or calls were skipped due to parser errors, ambiguous symbols, dispatch, macros, heap flow, external libraries, generated files, or path limits); Unsupported (the requested capability is not implemented for that language); and NoEntities. Add a per-language/per-capability matrix for local def-use, direct calls, methods/dispatch, fields/heap, libraries, macros/generated code, and async/exception flow. Detector receipts must contain analyzed functions, eligible calls, resolved calls, candidate groups, emitted findings, truncated paths, unresolved counts by reason, and configured depth/path limits. The human and machine-readable reports must state: 'No observed path is not proof of no runtime flow.' Existing parse-failure and unresolved-dependency receipts should feed this matrix rather than be duplicated.

#### Mvp Scope

Phase 0 instrumentation: define graph/receipt schemas and emit local Rust def-use metrics without findings. Phase 1 useful MVP: Rust-only lexical flow for parameters, locals, assignments, direct free-function calls with explicit crate/self/super/module resolution, returns, bounded summary composition (default depth 4), SCC condensation, module aggregation, JSON/human path evidence, and raw metrics. Emit only experimental AdapterFlowBypass candidates when every edge is exact and an existing adapter observation plus dependency context corroborates the bypass; keep LongDataTransit and fan-out as raw metrics. Phase 2: add Go package-qualified calls, fixtures, repository-relative baselines, caching by file-content and summary hash, and differential comparisons against CodeQL/Joern. Defer method/trait/interface dispatch, heap/field flow, alias/points-to analysis, closures escaping scope, async/exceptions, macros, reflection/eval, framework DI, external-library models, Bash/PowerShell interfile flow, automated refactoring, and default-on thresholds.

### Validation

#### Fixtures

- Positive exact path: Rust parameter assigned locally, passed through two explicit crate-qualified free functions in different modules, and returned/used; assert exact path order and crossings.
- Positive adapter bypass: most callers pass a value through `adapter::map`, while one exact direct call sends the same declared shape to the downstream module; assert corroboration and locations.
- Positive parallel mapping: two modules independently construct the same target shape from the same source shape; require duplicate-shape/similarity corroboration.
- Negative intentional pipeline: parser -> validate -> normalize -> persist, with each step transforming the value; assert metrics exist but no pass-through issue.
- Negative facade/compatibility layer and negative telemetry/context propagation; assert configured role exemptions suppress findings without deleting raw coverage.
- Negative shadowed names, unreachable branch, reassignment, loop-carried definition, recursion SCC, and same-name functions in different modules; assert no incorrect exact edge.
- Partial cases: unresolved trait method, macro-produced call, external crate function, closure escape, ambiguous glob import, parser ERROR/MISSING node, and configured depth/path truncation; assert a reasoned receipt and no actionable finding.
- Metamorphic: alpha-renaming locals preserves the graph; formatting/comments preserve metrics; inserting an identity local assignment changes edge count but not module path; moving a pass-through function within the same module preserves crossing count; inserting a named adapter changes route evidence; adding an unrelated file does not change existing paths; source order does not change deterministic output.

#### Corpus Strategy

Use four layers. (1) Micro-fixtures for each adapter and coverage state. (2) Seeded multi-module programs with paired good/bad variants inspired by Juliet/OWASP flow variants but labeled for graph instrumentation, not smell validity. (3) A version-pinned open-source corpus stratified by language, size, architecture, and domain: initial Rust self-scan plus approximately 10-15 Rust repositories for development, then a holdout of at least 10 additional Rust repositories; add equivalent Go and JVM/.NET strata only when adapters exist. Exclude forks and near-duplicates and freeze commits before threshold tuning. (4) Before/after commits mined from RefactoringMiner and repository histories where data-transfer code moved, adapters were introduced, DTOs split, or dependency cycles untangled. Split by repository, never by file or commit, to avoid leakage. Run CodeQL and Joern where supported and Semgrep rules on the same revision, retaining engine version, configuration, build success, runtime, memory, path endpoints, and unsupported cases. Use security suites only for reachability discrimination; use maintainer review and actual refactoring outcomes for product validity.

#### Oracle And Labels

Use independent labels with at least two reviewers and adjudication. Instrumentation label: is each node/edge and source-to-destination path semantically possible under the stated abstraction? Detection label: does the implementation match its declared exact/partial/unsupported contract? Pattern label: does the measured module-flow shape exist and is its percentile/context computed correctly? Actionability label: would a maintainer inspect or prioritize this, and what behavior-preserving refactoring seam is plausible? Outcome label for historical commits: did the refactoring reduce the targeted path/crossing/fan-out while preserving tests/API behavior, and was that reduction intentional? Reviewers must see coverage gaps and path evidence but should be blinded to detector score during initial labeling. CodeQL/Joern agreement is corroborating evidence, not ground truth; disagreements are manually classified as Reforge miss, semantic-engine miss/model difference, abstraction difference, or indeterminate. Report Cohen's kappa or Krippendorff's alpha per label dimension.

#### Success Criteria

Instrumentation gate: >=95% precision for individual exact edges and >=90% recall on in-scope fixture edges, with 100% correct partial/unsupported receipts for deliberately unobservable cases. Differential gate: for endpoints within the declared MVP abstraction, >=90% path agreement with at least one semantic reference engine after manual adjudication; never count out-of-scope paths as false negatives without displaying them. Product gate before heuristic findings: lower 95% confidence bound of precision >=80% on the repository holdout, median maintainer actionability >=3 on a 5-point scale, and at least 60% of accepted findings mapped to a concrete refactoring seam. Stability: deterministic byte-identical JSON across repeated scans, >=99% unchanged finding IDs under formatting/comment-only metamorphic edits, and <5% metric drift under unrelated-file additions. Performance gate: meet the proposed wall-time/RSS budgets in `complexity_cost` on small, medium, and large corpus strata, with explicit timeout/truncation receipts. Self-scan gate: Reforge remains at 0 default actionable findings; experimental/raw flow metrics may be non-zero. Promotion to conservative additionally requires two language adapters, two independently labeled holdouts, 100% exact edges in emitted paths, and no severity based solely on flow length.


## Representation churn and schema diffusion

### Conclusion

#### Item Name

Representation churn and schema diffusion

#### Thesis

Representation churn is a useful composite refactoring signal when one logical payload is repeatedly copied between near-equivalent records, objects, dictionaries, or wire shapes across modules with little semantic transformation. The source-observable fact is repeated structural correspondence, not shared business meaning. Reforge should flag dispersed, mostly identity-preserving conversion chains for review and recommend consolidating ownership or mapping, while treating deliberate boundary models as exceptions rather than defects.

#### Refactoring Relevance

A high-confidence instance suggests a behavior-preserving structural change such as reusing an existing model inside one bounded context, extracting a named value object, replacing repeated ad hoc construction with one owned mapper, moving mapping code to an adapter, or generating equivalent representations from one schema. The behavior-preserving precondition is that fields, defaults, validation, nullability, serialization names, units, and compatibility behavior remain unchanged. The signal is valuable because every duplicated representation and conversion site is another place that must co-evolve when the fact changes. It is not sufficient to recommend collapsing persistence, domain, API, event, and UI models: those boundaries may intentionally isolate change, security, or vocabulary. A detector should therefore identify representation families and repeated conversion evidence, then let maintainers choose consolidation, centralization, generation, or an intentional-boundary disposition.

### Evidence

#### Foundational Sources

- title: Refactoring: Improving the Design of Existing Code / Introduce Parameter Object | year: 1999 | claim: Recurring groups of parameters can be replaced by a named object, establishing repeated co-traveling fields as a classic refactoring cue rather than a defect proof. | url: https://refactoring.com/catalog/introduceParameterObject.html
- title: Data Transfer Object | year: 2003 | claim: A DTO intentionally carries serializable data across a remote boundary and is commonly assembled from domain objects; this establishes both the representation/mapping pattern and a major legitimate exception. | url: https://martinfowler.com/eaaCatalog/dataTransferObject.html
- title: Survey of Research on Software Clones | year: 2007 | claim: Software redundancy and similarity admit multiple clone types and detection techniques; near-miss structural copies require more than exact text matching. | url: https://doi.org/10.4230/DagSemProc.06301.13
- title: Bounded Context | year: 2014 | claim: Different bounded contexts may deliberately use different models for common concepts and translate between them; structural overlap across a real context boundary does not by itself justify model unification. | url: https://martinfowler.com/bliki/BoundedContext.html

#### Empirical Sources

- title: Evaluating Code Duplication to Identify Rich Business Objects from Data Transfer Objects | year: 2010 | systems_or_labels: An industrial Java enterprise content-management system; DTOs, invoking classes, calls, and code-duplication edges were visualized as DTO Constellations. | relevant_result: Manual inspection found three logical data groups and three duplication categories. Duplication between DTOs exposed shared data that could be merged or factored, while duplication between DTO clients exposed business logic that could be moved or factored. This is direct but single-case, exploratory evidence. | url: https://scg.unibe.ch/archive/papers/Peri10dDTOs.pdf
- title: An Empirical Analysis of the Co-evolution of Schema and Code in Database Applications | year: 2013 | systems_or_labels: Ten popular open-source database applications totaling more than 160,000 revisions; database schema changes and application-code changes. | relevant_result: Schemas evolved frequently, schema changes induced significant code-level modifications, and co-change analysis appeared viable for assisting evolution. This supports displaying historical propagation around representation families, though it studies database schemas rather than arbitrary DTO chains. | url: https://doi.org/10.1145/2491411.2491431
- title: Some Code Smells Have a Significant but Small Effect on Faults | year: 2014 | systems_or_labels: Eclipse, ArgoUML, and Apache Commons; five smells including Data Clumps, with repository-derived fault data and negative-binomial models. | relevant_result: Data Clumps had inconsistent direction across systems and every significant smell effect was under 10 percent. The study cautions against using a structural smell alone as a fault-reduction mandate. | url: https://doi.org/10.1145/2629648
- title: Characteristics and Automated Detection and Refactoring of Data Clumps | year: 2026 | systems_or_labels: More than eight million line-level data clumps across 23 open-source projects and 3,290 timestamps, plus more than 100,000 UML class diagrams. | relevant_result: Data clumps occurred predominantly in parameters in source code, were mostly local to classes, increased over project lifetimes, and correlated positively with faults in many projects. The work supplies scalable recurring-field-group evidence but does not evaluate cross-language conversion chains. | url: https://doi.org/10.48693/930

#### Industrial Precedents

- tool: CodeQL | practice: Its data-flow libraries model value nodes and directed flow steps, support local and global flow, field/call paths, and path explanations. This demonstrates the semantic infrastructure needed to connect a source field read to a destination field write, while the documentation warns that global flow is costlier and less precise. | url: https://codeql.github.com/docs/writing-codeql-queries/creating-path-queries/
- tool: MapStruct | practice: The compile-time Java mapper validates source and target property coverage and can warn or fail on unmapped properties. This demonstrates that explicit type-pair field correspondence and rename/conversion declarations are practical build artifacts. | url: https://mapstruct.org/documentation/stable/reference/html/
- tool: AutoMapper | practice: The .NET mapper validates that destination members have corresponding source members and treats source renames as a typical configuration failure. This is practical evidence that mapping pairs and field coverage are inspectable, not evidence that mapping is itself a smell. | url: https://docs.automapper.io/en/stable/Configuration-validation.html
- tool: SonarQube | practice: It reports token/statement-based duplicated blocks and density across supported languages. Clone evidence can corroborate repeated hand-written mapper bodies, although ordinary duplication thresholds miss short or renamed field mappings. | url: https://docs.sonarsource.com/sonarqube-community-build/user-guide/code-metrics/metrics-definition
- tool: PMD | practice: Its Java DataClass and ExcessiveParameterList rules operationalize passive data holders and ungrouped related parameters using explainable metrics. These are adjacent ingredients, not a representation-chain detector. | url: https://pmd.github.io/pmd/pmd_rules_java_design.html

#### Conflicting Evidence

The most important conflict is architectural: DTOs, anti-corruption layers, bounded contexts, persistence entities, events, public API versions, read models, and view models deliberately represent overlapping facts differently so that one schema does not become shared coupling. Fowler's DTO and bounded-context descriptions make translation an expected cost at real boundaries. Empirically, Hall et al. found small and system-dependent fault effects for Data Clumps and warned that arbitrary refactoring may not reduce faults. The Perin and Girba DTO result is one exploratory industrial case without precision, recall, controlled maintenance outcomes, or generalization beyond Java enterprise code. Structural equality also does not establish semantic equality: identically named fields can differ in units, privacy, validation, lifecycle, ownership, or optionality, while renamed fields can be the same fact. These limits require conservative wording and independent evidence before recommending unification.

### Detectability

#### Observable Pattern

Observe declarations and operations without inferring intent: (1) two or more declared or anonymous record shapes share at least three normalized fields or have a recoverable one-to-one field correspondence; (2) functions read fields from one shape and construct, assign, return, serialize, or pass a second shape; (3) most correspondences are direct copies, simple renames, wrapping/unwrapping, or type-preserving serialization rather than calculations; and (4) the same shape family or conversion is repeated across modules, or a value traverses two or more successive near-equivalent reconstructions. Concrete syntax includes object/struct literals, constructors/builders, destructuring followed by rebuilding, getter-to-setter assignments, dictionary/hash projection, spread/copy plus overrides, serializer annotations, JSON key literals, and mapper declarations. The finding is stronger when conversion logic is dispersed outside named adapter modules, reverse mappings disagree, or the representation family co-changes historically.

#### Graph Model

nodes:   - named type, record, class, struct, interface, data class, and schema declarations
  - anonymous object, dictionary, hash, tuple, and serialized payload shapes
  - field/property/key declarations and access-path slots
  - construction, destructuring, assignment, setter, mapper, serialization, and conversion operations
  - functions, files, modules, packages, and configured boundaries; edges:   - type-contains-field and module-contains-entity
  - value/field read to destination field write via local def-use
  - argument-to-parameter and return-to-call-result
  - constructs, destructures, serializes, deserializes, maps-to, and calls
  - field-correspondence with exact-name, normalized-name, declared-annotation, positional, or resolved-flow provenance
  - import/dependency and optional historical co-change; labels_and_summaries: Label shapes with language, module, visibility, nominal name, normalized field name, declared type text, nullability/default, serialization alias, annotation/tag, and generated/test/boundary status. Label correspondences with confidence, copy/rename/coerce/compute/drop/add kind, direction, and location. Summarize each conversion as source shape, target shape, copied fields, renamed fields, transformed fields, dropped/added fields, coverage, and unresolved operations. Collapse conversions into representation-family components and value-path chains; aggregate by module pair and boundary crossing.; decision_model: Candidate shape edges require minimum field support and high weighted correspondence. Candidate churn requires either the same conversion implemented at multiple independent sites or a path with repeated near-identity conversion edges. Schema diffusion is a representation-family component spread across multiple non-boundary modules. Do not equate transitive shape similarity with semantic identity; preserve every edge's direct evidence and split components when correspondence is weak or contradictory.

#### Analysis Requirements

A high-quality detector needs language-aware parsing; declaration and import/name resolution; type resolution for constructors, fields, overloads, generic substitutions, and inferred records; local CFG and def-use/SSA-like reasoning to follow aliases from a source access into a destination construction; call-graph and function summaries for helper mappers; field-sensitive access paths; and modest context sensitivity to distinguish the same generic mapper at different type pairs. Points-to analysis is needed for mutable aliases, setters, collections, and heap-carried payloads. Library/framework models are required for serializers, ORMs, mapping frameworks, builders, reflection, generated clients, protocol buffers, database rows, and messaging APIs. A syntax-only tier can instead infer correspondences inside one expression or function from direct field accesses, identifier provenance, names, tags, and positional arguments; it must not claim whole-program value flow. Historical co-change is optional corroboration and needs entity matching across renames and generated/noisy commit filtering.

#### Tree Sitter Feasibility

Tree-sitter can reliably locate successfully parsed declarations, fields, parameters, object/struct literals, constructors, destructuring patterns, assignments, returns, direct calls, member accesses, dictionary keys, annotations/tags, and source locations. Reforge can conservatively extract named shape signatures and direct mapping pairs such as Target(a = source.a, b = source.b), {a: source.a}, or target.setA(source.getA()) within one function. It can also recognize serializer/mapping annotations and combine this with file dependencies. Tree-sitter alone cannot reliably resolve nominal or inferred types, overloads, aliases, spread values, generic builders, macro/proc-macro expansion, operator conversions, reflection, generated members, framework serializers, dynamic keys, or semantic equivalence of names and types. It cannot prove that two copied values denote the same business fact. Therefore an MVP should report explicit correspondences and unresolved counts, and treat shape-name similarity without value provenance only as corroboration.

#### Language Constraints

Rust: Struct declarations/literals, field shorthand, destructuring, From/Into/TryFrom implementations, and serde rename attributes are promising. Macros, proc-macro-generated serializers, trait dispatch, tuple structs, update syntax, deref coercion, and generic conversion traits limit syntax-only resolution.; JavaScript_TypeScript_Vue: Object literals, destructuring, spreads, serializers, and mapped fields are visible, but structural typing, inferred anonymous types, dynamic keys, prototype mutation, re-exports, callback chains, and framework transforms make identity and flow uncertain. TypeScript compiler information would materially improve precision; Vue template payloads need separate modeling.; Python: Dataclasses, TypedDict, Pydantic-style models, keyword construction, dict literals/comprehensions, and ** unpacking are visible. Duck typing, decorators, dynamic attributes, serializers, and metaclasses make nominal families and generated mappings incomplete.; Go: Struct declarations/literals, explicit selectors, conversions, and JSON tags are tractable. Positional literals, embedding, interfaces, reflection, map[string]any, generated protobuf code, and custom Marshal methods need special handling.; Java_CSharp_Kotlin: Nominal DTOs, records/data classes, constructors, builders, properties, object initializers, annotations, and mapper methods provide strong syntax. Overloads, inheritance, extension methods, Lombok/source generators, reflection, ORM proxies, nullability conventions, and dependency injection require semantic models. Java and C# are the best initial resolved-language pilots; Kotlin adds implicit receivers and destructuring components.; PHP_Ruby: Associative arrays/hashes and constructors are visible, but dynamic fields, magic accessors, monkey patching, symbols versus strings, metaprogramming, and framework hydration sharply reduce precision. Restrict an MVP to explicit literal-to-literal or accessor-to-constructor mappings.; Bash_PowerShell: Shell variables, pipelines, environment records, hashtables, and external JSON tools can transform data, but stable type and field identity are usually absent. Mark representation-family analysis unsupported initially except for explicit PowerShell hashtable literals or configured schemas.

#### Complexity Cost

AST extraction and direct intra-function mapping recognition are O(N) in syntax nodes with O(N) stored occurrences. Building shape signatures is O(F), where F is total fields. Candidate generation should use an inverted index by normalized field/token; naive all-pairs comparison is O(T^2 * average_fields) for T shapes, while indexed candidates are closer to the sum of squared posting-list sizes and need caps for ubiquitous names such as id or name. Local def-use is roughly linear in CFG size per function. Representation-family clustering is O(V + E) after candidate edges exist. Interprocedural, field-sensitive points-to and context-sensitive summaries are high-cost in time, memory, and language engineering; unrestricted path enumeration can be exponential and must be summarized and bounded. The syntax-only multi-language MVP is moderate-to-high implementation work; a semantic whole-program version is a major subsystem.

### Signal Quality

#### False Positives

- API request/response DTOs intentionally decouple public compatibility from domain evolution.
- Persistence entities, ORM rows, database migrations, and domain objects intentionally differ in lifecycle and constraints.
- Bounded contexts and anti-corruption layers intentionally translate overlapping concepts with different vocabularies.
- CQRS commands, events, projections, read models, and UI view models optimize different use cases.
- Security/privacy projections deliberately drop secrets, authorization state, internal identifiers, or regulated fields.
- Versioned messages and compatibility shims intentionally retain near-equivalent old and new schemas.
- Generated protobuf/OpenAPI/GraphQL clients, serializers, ORM code, and mapping-framework output duplicate shapes mechanically.
- Unit, currency, timezone, locale, normalization, encryption, validation, and null/default conversions can look syntactically trivial while changing semantics.
- Builders, copy constructors, immutable with-methods, cloning, and snapshot/audit code legitimately rebuild the same shape.
- Tests, fixtures, factories, fuzz inputs, and assertions repeat payload construction for clarity and isolation.
- Generic object-copy or serialization utilities create apparent flows that cannot be attributed to a stable business shape.
- Common generic fields such as id, name, status, created_at, and metadata create accidental shape overlap.

#### Precision Boosters

- Require at least three high-information corresponding fields and discount ubiquitous names with inverse-document-frequency weighting.
- Require direct source-field-to-destination-field provenance for most matched fields, not names alone.
- Require a high identity-preserving ratio and report calculated, dropped, added, defaulted, and unresolved fields separately.
- Require either at least two independent conversion sites, at least three successive representations, or the same representation family in at least three non-boundary modules.
- Prefer evidence where the same source/target pair is mapped in both directions or multiple implementations disagree on coverage, defaults, or renames.
- Use declared types, serialization aliases, annotations, tags, mapper configurations, and exact value flow as independent correspondence evidence.
- Downgrade recognized adapter, mapper, serializer, generated, migration, test, DTO, event, command, projection, persistence, and versioned namespaces; display the reason instead of silently deleting raw metrics.
- Corroborate with Reforge duplicate_type_shape, data_clump, similar_functions, repeated_literal, adapter_boundary_bypass, dependency hub/cycle, or churn/co-change evidence.
- Require all candidate types to be project-owned and exclude standard-library or third-party shapes unless explicitly configured.
- Show whether consolidation would cross a declared module/bounded-context boundary or create a dependency cycle.
- Use project-relative calibration and human feedback rather than one universal shape-overlap threshold.

#### Legitimate Exceptions

Expected and often beneficial cases include ports-and-adapters and anti-corruption layers; bounded contexts; public API and message versioning; remote DTOs; persistence/domain separation; CQRS commands, events, and read models; UI view models; privacy/security projections; generated schemas and clients; serializer/ORM/mapping code; migrations and compatibility shims; immutable copies and snapshots; test fixtures; ETL/data pipelines where each stage has a declared schema; and integrations that normalize units or vendor-specific conventions. The smell is dispersion or redundant near-identity work within an ownership boundary, not the mere existence of multiple representations.

#### Confidence Tier

experimental

### Reforge Integration

#### Existing Capabilities

Reforge already has several strong inputs. data_clump finds repeated parameter groups; duplicate_type_shape extracts types with at least three fields and groups shapes at roughly 0.75 field-name overlap; similar_functions and parallel_implementation expose repeated mapping bodies; repeated_literal can expose duplicated wire keys; adapter_boundary_bypass and stale_compatibility_path provide boundary/compatibility context; file dependencies provide resolved local import edges, fan-in/out, hubs, cycles, transitive context, and unresolved-edge counts; structural analysis supplies named functions, parameters, type size/public surface, imports, and precise spans; git churn supplies file-level commits, authors, additions/deletions, and recency; issue clustering and related locations can combine corroborating findings. The current duplicate-type-shape extraction is largely lexical/line-oriented and Reforge does not yet have field-level def-use, constructor/type resolution, mapper summaries, anonymous-shape identity, schema-family graphs, or method/type-level co-change.

#### Proposed Finding Kinds

- name: representation_churn | issue_family: data_modeling | mechanism: one value path repeatedly reconstructs near-equivalent shapes with predominantly identity-preserving field correspondences | action: review eliminating an internal hop, reusing a model within the boundary, or centralizing/generating the conversion | scope: conversion chain with source, intermediate, and destination locations | precision_risk: high with syntax-only flow; medium when direct field provenance, resolved types, and boundary classification are available
- name: schema_diffusion | issue_family: data_modeling | mechanism: a near-equivalent representation family and its conversion responsibility are dispersed across multiple non-boundary modules | action: review a single owning schema/value object, generated derivatives, or one explicit adapter per real boundary | scope: representation family aggregated by modules | precision_risk: high because structural overlap does not prove shared meaning
- name: duplicated_mapping | issue_family: duplication | mechanism: the same source/target field correspondence is hand-implemented at multiple sites | action: extract or generate one owned mapper and add mapping-coverage tests | scope: source/target pair with repeated site locations | precision_risk: medium when both types and field provenance resolve; high when inferred from body similarity
- name: mapping_drift | issue_family: concept_drift | mechanism: independent mappings for the same representation pair disagree about fields, aliases, defaults, or conversion behavior | action: reconcile intentional differences or consolidate mapping ownership | scope: mapping pair with conflicting decisions | precision_risk: medium; differences may be operation-specific and intentional

#### Explanation Path

Each finding should show the representation family, declaration/module location of every involved shape, and one shortest evidence path. For each conversion edge, show the conversion function/call location; source and destination types or stable shape IDs; representative source-read to destination-write pairs; exact-copy, rename, transform, add, and drop labels; identity-preserving ratio; unresolved operations; and boundary/generated/test classification. For duplicated mappings, show at least two implementation locations and a correspondence diff. For schema diffusion, show the module aggregation and only the strongest direct edges rather than implying that every transitive family member is semantically equal. Include relevant Reforge corroborators, dependency edges/cycle risk, and coverage receipts. The recommendation must state that semantic equivalence and safe model unification were not proven.

#### Coverage Contract

Observed means the files parsed successfully and the reported shape declarations, conversion syntax, field accesses, and source-to-destination correspondences are directly present; for semantic-tier findings, constructor/callee and field owners also resolved. Partial means any type, alias, spread, builder step, helper call, serializer, generated declaration, dynamic key, dependency, or path segment is unresolved. Positive findings may be based only on supported edges, while absence of a finding never implies absence of churn under partial coverage. Unsupported initially includes Bash, most PowerShell pipelines, reflective/dynamic mappings, runtime-generated schemas, external service transformations, database/broker transformations, templates, macros that hide fields, and languages outside the implemented adapter tier. Excluded generated, dependency, test, migration, or configured boundary paths must have counts and reasons. Reports must expose analyzed shapes/conversions, resolved and unresolved correspondence counts, supported language/tier, parse failures, candidate-pair caps, history availability, and whether boundaries came from configuration or naming heuristics.

#### Mvp Scope

Implement a read-only syntax-tier detector first for Rust struct literals, Go keyed struct literals, Java/C#/Kotlin constructor/object-initializer or getter-to-setter mappers, and JavaScript/TypeScript/Python explicit object/dict literals. Reuse duplicate_type_shape candidates, but require at least three direct source-field-to-destination-field correspondences inside one named function and a high identity-preserving share. Emit duplicated_mapping only when the same explicit source/target shape pair occurs at two or more production sites, and representation_churn only when a statically connected chain contains at least three shapes. Add mapping sites and field pairs as related locations; expose unresolved operations and suppress/downgrade recognized tests, generated files, serializers, migrations, and designated adapter/boundary directories. Defer semantic schema-family unification, cross-function aliases, setters across statements where ownership is unresolved, reflection/framework hydration, points-to analysis, anonymous-shape interning across functions, historical co-change gating, schema_diffusion as a default finding, and mapping_drift defaults until corpus validation.

### Validation

#### Fixtures

positive:   - A Java controller maps ApiOrder to ServiceOrder field-for-field, a service maps ServiceOrder to RepositoryOrder, and a repository rebuilds the same fields; all three types have the same six fields and no boundary annotation between the internal hops.
  - Two C# modules independently implement CustomerDto to Customer mappings with the same five direct assignments.
  - Two TypeScript mapping functions copy the same object keys, but one omits postalCode; emit mapping disagreement with both sites.
  - A Rust chain implements `From<WireUser>` for `AppUser` and `From<AppUser>` for `DbUser` with direct copies for all but one field.
  - A Go conversion family repeats JSON-tag-equivalent fields under different nominal struct names across three internal packages.; negative:   - A domain object maps once to a public API DTO at a configured remote boundary.
  - An anti-corruption layer renames vendor fields, converts cents to Money, validates status, and drops vendor-only metadata.
  - A versioned event v1-to-v2 upcaster preserves old wire compatibility.
  - A privacy projection copies public fields but deliberately removes email and internal identifiers.
  - A generated protobuf model and generated mapper are excluded with receipts.
  - Two unrelated types share id, name, status, and metadata but have no mapping/value-flow edge.
  - A test fixture rebuilds a record for assertion clarity.
  - A copy constructor and immutable with-method rebuild the same type, not a distinct representation family.; metamorphic:   - Formatting, field-order changes in keyed constructions, and local-variable renames must not change finding identity or metrics.
  - Replacing direct source.field expressions with same-value local aliases should preserve a semantic-tier result and mark the syntax tier partial rather than invent a different correspondence.
  - Adding an unrelated common field named id to both shapes must not alone cross the threshold because common-name weighting is low.
  - Extracting repeated mapping into one shared mapper should reduce conversion_site_count and remove duplicated_mapping without changing observed outputs.
  - Marking the same directories as configured adapters should preserve raw metrics but change disposition/confidence.
  - Adding a non-trivial unit conversion should decrease identity_preserving_ratio and increase transformed_field_count.
  - Changing a serialization alias while preserving the declared field name should update the wire-shape correspondence but not the in-memory shape signature.
  - Deleting an intermediate identity mapping and passing the existing representation through should shorten the chain and remove representation_churn.

#### Corpus Strategy

Create a stratified corpus across Reforge's supported ecosystems: Rust services and CLIs using serde; Go services with API/domain/database structs; Java/Kotlin and C# enterprise applications with DTOs, ORMs, MapStruct/AutoMapper, records/data classes, and generated clients; TypeScript/Vue frontends and Node services; Python applications using dataclasses/Pydantic-like models; and smaller PHP/Ruby samples for explicit mapping coverage. Deliberately oversample hard negatives: bounded contexts, anti-corruption layers, CQRS/events, public API versioning, privacy projections, migrations, generated protobuf/OpenAPI code, ETL pipelines, and tests. Mine candidate commits that introduce/extract mappers, merge duplicate models, generate schemas, or remove intermediate DTOs; manually verify intent and preserved behavior. Label detector negatives as well as positives. Split calibration and holdout by repository and organization, with no forks or generated siblings across splits. Compare shape-only, direct-flow, clone-corroborated, boundary-aware, and history-aware variants, and publish per-language/per-exception performance rather than one pooled score.

#### Oracle And Labels

Use at least two maintainers or experienced language reviewers independently and separate four decisions. Instrumentation: are shapes, fields, type owners, correspondences, conversion edges, and coverage correct? Interpretation: do the shapes represent the same or deliberately related fact, or is similarity accidental/unknown? Actionability: keep separate, centralize mapper, generate from schema, reuse a type within one boundary, extract value object, remove a hop, or needs design discussion? Outcome: if changed, did characterization/golden tests preserve serialized payloads and behavior, did mappings remain complete, and did conversion sites/dependency spread decrease without collapsing a valid boundary? Reviewers see code and explanation paths but not detector score or each other's answers. Preserve ambiguous and intentional-boundary labels, adjudicate disagreements, report Cohen's kappa or Krippendorff's alpha, and never use passing tests alone as proof of semantic equivalence.
