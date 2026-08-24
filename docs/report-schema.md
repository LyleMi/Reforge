# Report format

The current report format uses `schema_version = 27`. `reforge_schema::Report`
contains `schema_version`, `producer`, `target`,
`provenance`, `summary`, `suppression`, `coverage`, `issues`, and optional
`baseline_comparison`. Unknown fields are rejected.

Provenance records identity scheme `reforge-identity-v7`, the evaluated scope
digest, per-analysis configuration and policy digests, and each evaluated
rule's semantic version and evaluation digest.

An Issue contains `kind = advisory | policy`, explicit analysis and family,
typed Subject, readable prose, Evidence, an `ri7-*` ID, and a versioned
`content_fingerprint` (`rc7-*`). Subject entities contain independent `key`,
`path`, and optional
`symbol` fields; groups contain structured entity members. Symbol keys use
language, qualified owner, declaration kind, name, and signature or stable
disambiguator. Prose, ordering, checkout location, comments, and line numbers
do not define identity.

Evidence has an `re7-*` ID derived from rule and semantic anchor. Measurements,
thresholds, evidence-set changes, and substantive witness changes update the
Issue content fingerprint. Flow witnesses expose typed source/sink symbols,
ordered steps, hop counts, and `exact`, `modeled`, `unresolved`, or
`unsupported` resolution. Only all-exact, value-preserving paths can be policy
witnesses.

Coverage is keyed by analysis and language. Language entries include capability
receipts for syntax, symbols, lexical scopes, local def-use, direct calls,
call/return composition, field flow, and dynamic dispatch. Rule entries print
once with maturity, activation source, status, observations, and limitations.
Zero Evidence never erases the observed denominator.

Baseline comparison maps every current or previous Issue ID to `new`,
`unchanged`, `updated`, `absent`, or `unknown`, with an optional reason. A
matching ID with a changed content fingerprint is `updated`. Scope, relevant
configuration/policy, rule semantics/evaluation, analysis availability, or
coverage changes make otherwise unprovable additions/disappearances `unknown`.
Workspace identity mismatch is an error. Producer name and identity scheme must
match, but producer versions and unrelated analysis sets may differ.

Older report formats are rejected rather than silently converted. Regenerate
them with the current analyzer so their findings and Coverage describe the same
analysis behavior.
