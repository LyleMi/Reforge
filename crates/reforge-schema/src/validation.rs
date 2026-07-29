fn coverage_is_downgraded(current: &AnalysisCoverage, previous: &AnalysisCoverage) -> bool {
    current.status.rank() < previous.status.rank()
        || previous.languages.iter().any(|(language, before)| {
            current
                .languages
                .get(language)
                .is_none_or(|after| after.status.rank() < before.status.rank())
        })
        || previous.rules.iter().any(|(rule, before)| {
            current
                .rules
                .get(rule)
                .is_none_or(|after| after.status.rank() < before.status.rank())
        })
}

fn validate_provenance(
    provenance: &ReportProvenance,
    coverage: &BTreeMap<String, AnalysisCoverage>,
) -> Result<()> {
    if provenance.identity_scheme != IDENTITY_SCHEME {
        bail!(
            "unsupported report identity scheme `{}`; expected `{IDENTITY_SCHEME}`",
            provenance.identity_scheme
        );
    }
    if provenance.scope_digest.trim().is_empty() {
        bail!("report provenance scope digest must not be empty");
    }
    for analysis in coverage.keys() {
        let entry = provenance
            .analyses
            .get(analysis)
            .with_context(|| format!("missing provenance for analysis `{analysis}`"))?;
        if entry.config_digest.trim().is_empty() || entry.policy_digest.trim().is_empty() {
            bail!("analysis provenance digests must not be empty for `{analysis}`");
        }
        for rule in coverage[analysis].rules.keys() {
            validate_rule_provenance(provenance, rule)?;
        }
    }
    Ok(())
}

fn validate_rule_provenance(provenance: &ReportProvenance, rule: &str) -> Result<()> {
    let entry = provenance
        .rules
        .get(rule)
        .with_context(|| format!("missing provenance for rule `{rule}`"))?;
    if entry.semantic_version.trim().is_empty() || entry.evaluation_digest.trim().is_empty() {
        bail!("rule provenance fields must not be empty for `{rule}`");
    }
    Ok(())
}

fn comparison_entry(
    current: &Report,
    baseline: &Report,
    issue: &Issue,
    comparable_state: BaselineState,
) -> BaselineEntry {
    match comparison_unknown_reason(ComparisonInputs {
        current,
        baseline,
        issue,
    }) {
        Some(reason) => BaselineEntry {
            state: BaselineState::Unknown,
            reason: Some(if issue.kind == IssueKind::Policy {
                format!("policy_issue:{reason}")
            } else {
                reason
            }),
        },
        None => BaselineEntry {
            state: comparable_state,
            reason: None,
        },
    }
}

struct ComparisonInputs<'a> {
    current: &'a Report,
    baseline: &'a Report,
    issue: &'a Issue,
}

fn comparison_unknown_reason(input: ComparisonInputs<'_>) -> Option<String> {
    if input.current.provenance.scope_digest != input.baseline.provenance.scope_digest {
        return Some("scope_changed".into());
    }
    analysis_unknown_reason(&input)
        .or_else(|| coverage_unknown_reason(&input))
        .or_else(|| rule_unknown_reason(&input))
}

fn analysis_unknown_reason(input: &ComparisonInputs<'_>) -> Option<String> {
    let Some(current_analysis) = input
        .current
        .provenance
        .analyses
        .get(&input.issue.analysis)
    else {
        return Some("analysis_not_evaluated".into());
    };
    let Some(baseline_analysis) = input
        .baseline
        .provenance
        .analyses
        .get(&input.issue.analysis)
    else {
        return Some("analysis_not_evaluated".into());
    };
    if current_analysis.config_digest != baseline_analysis.config_digest {
        return Some("analysis_config_changed".into());
    }
    if current_analysis.policy_digest != baseline_analysis.policy_digest {
        return Some("analysis_policy_changed".into());
    }
    None
}

fn coverage_unknown_reason(input: &ComparisonInputs<'_>) -> Option<String> {
    let current = input.current.coverage.get(&input.issue.analysis);
    let baseline = input.baseline.coverage.get(&input.issue.analysis);
    match (current, baseline) {
        (Some(current), Some(baseline)) if current == baseline => None,
        (Some(_), Some(_)) => Some("coverage_changed".into()),
        _ => Some("analysis_not_evaluated".into()),
    }
}

fn rule_unknown_reason(input: &ComparisonInputs<'_>) -> Option<String> {
    for rule in input
        .issue
        .evidence
        .iter()
        .map(|evidence| evidence.rule.as_str())
        .collect::<BTreeSet<_>>()
    {
        let current_rule = match input.current.provenance.rules.get(rule) {
            Some(value) => value,
            None => return Some(format!("rule_not_evaluated:{rule}")),
        };
        let baseline_rule = match input.baseline.provenance.rules.get(rule) {
            Some(value) => value,
            None => return Some(format!("rule_not_evaluated:{rule}")),
        };
        if current_rule.semantic_version != baseline_rule.semantic_version {
            return Some(format!("rule_semantics_changed:{rule}"));
        }
        if current_rule.evaluation_digest != baseline_rule.evaluation_digest {
            return Some(format!("rule_evaluation_changed:{rule}"));
        }
    }
    None
}

fn validate_coverage(coverage_by_analysis: &BTreeMap<String, AnalysisCoverage>) -> Result<()> {
    for (analysis, coverage) in coverage_by_analysis {
        validate_analysis_name(analysis)?;
        validate_language_coverage(coverage)?;
        validate_rule_coverage(coverage)?;
        validate_limitations(&coverage.limitations)?;
    }
    Ok(())
}

fn validate_analysis_name(analysis: &str) -> Result<()> {
    if matches!(
        analysis,
        ANALYSIS_CODEBASE | ANALYSIS_DATAFLOW | ANALYSIS_UNITY
    ) {
        Ok(())
    } else {
        bail!("unknown analysis `{analysis}` in coverage")
    }
}

fn validate_language_coverage(coverage: &AnalysisCoverage) -> Result<()> {
    for language in coverage.languages.values() {
        for (capability, receipt) in &language.capabilities {
            validate_code("coverage capability", capability)?;
            validate_limitations(&receipt.limitations)?;
        }
        validate_limitations(&language.limitations)?;
    }
    Ok(())
}

fn validate_rule_coverage(coverage: &AnalysisCoverage) -> Result<()> {
    for (rule, execution) in &coverage.rules {
        validate_namespace("coverage rule", rule)?;
        if !matches!(
            execution.maturity.as_str(),
            "experimental" | "preview" | "stable"
        ) {
            bail!(
                "coverage rule `{rule}` has unknown maturity `{}`",
                execution.maturity
            );
        }
        for observation in &execution.observations {
            validate_code("coverage observation name", &observation.name)?;
            if observation.unit.trim().is_empty() {
                bail!(
                    "coverage observation {} has an empty unit",
                    observation.name
                );
            }
        }
        validate_limitations(&execution.limitations)?;
    }
    Ok(())
}

fn validate_limitations(limitations: &[CoverageLimitation]) -> Result<()> {
    for limitation in limitations {
        validate_code("coverage limitation code", &limitation.code)?;
        if limitation.count == 0 {
            bail!("coverage limitation {} has a zero count", limitation.code);
        }
    }
    Ok(())
}

fn validate_issues(
    issues: &[Issue],
    coverage_by_analysis: &BTreeMap<String, AnalysisCoverage>,
) -> Result<()> {
    let mut issue_ids = BTreeSet::new();
    let mut evidence_ids = BTreeSet::new();
    for issue in issues {
        if !coverage_by_analysis.contains_key(&issue.analysis) {
            bail!(
                "issue {} names analysis `{}` which is absent from coverage",
                issue.id,
                issue.analysis
            );
        }
        validate_issue(issue, &mut evidence_ids)?;
        if !issue_ids.insert(&issue.id) {
            bail!("duplicate issue ID {}", issue.id);
        }
    }
    Ok(())
}

fn validate_issue(issue: &Issue, evidence_ids: &mut BTreeSet<String>) -> Result<()> {
    validate_code("issue analysis", &issue.analysis)?;
    validate_namespace("issue family", &issue.family)?;
    validate_subject(&issue.subject)?;
    if issue.id != issue_id(&issue.family, &issue.subject) {
        bail!("issue {} has an invalid stable ID", issue.id);
    }
    if issue.content_fingerprint != content_fingerprint(issue) {
        bail!("issue {} has an invalid content fingerprint", issue.id);
    }
    if issue.evidence.is_empty() {
        bail!("issue {} has no evidence", issue.id);
    }
    for evidence in &issue.evidence {
        validate_issue_evidence(issue, evidence, evidence_ids)?;
    }
    Ok(())
}

fn validate_subject(subject: &Subject) -> Result<()> {
    match subject {
        Subject::Repository => Ok(()),
        Subject::Directory { entity } | Subject::File { entity } => validate_entity(entity, false),
        Subject::Symbol { entity } => validate_entity(entity, true),
        Subject::Group { members } => {
            if members.is_empty() {
                bail!("group subject must contain at least one member");
            }
            let mut identities = BTreeSet::new();
            for member in members {
                validate_entity(member, false)?;
                if !identities.insert(member.identity()) {
                    bail!("group subject contains duplicate members");
                }
            }
            Ok(())
        }
    }
}

fn validate_entity(entity: &EntityRef, symbol_required: bool) -> Result<()> {
    if entity.key.trim().is_empty() {
        bail!("subject entity key must not be empty");
    }
    if entity.path.trim().is_empty() {
        bail!("subject entity path must not be empty");
    }
    if canonical_path(&entity.path) != entity.path {
        bail!("subject entity path must be canonical: `{}`", entity.path);
    }
    if symbol_required
        && entity
            .symbol
            .as_deref()
            .is_none_or(|symbol| symbol.trim().is_empty())
    {
        bail!("symbol subject must name a symbol");
    }
    Ok(())
}

fn validate_issue_evidence(
    issue: &Issue,
    evidence: &Evidence,
    evidence_ids: &mut BTreeSet<String>,
) -> Result<()> {
    validate_namespace("evidence rule", &evidence.rule)?;
    if !evidence.id.starts_with("re7-") {
        bail!("evidence {} is not a schema 27 evidence ID", evidence.id);
    }
    if issue.analysis != rule_analysis(&evidence.rule)? {
        bail!(
            "evidence {} belongs to a different analysis than issue {}",
            evidence.id,
            issue.id
        );
    }
    if !evidence_ids.insert(evidence.id.clone()) {
        bail!("duplicate evidence ID {}", evidence.id);
    }
    Ok(())
}

pub fn issue_id(family: &str, subject: &Subject) -> String {
    stable_id("ri7", &[family, &subject.identity()])
}

pub fn evidence_id(rule: &str, semantic_anchor: &str) -> String {
    stable_id("re7", &[rule, semantic_anchor])
}

pub fn content_fingerprint(issue: &Issue) -> String {
    let mut parts = vec![
        match issue.kind {
            IssueKind::Advisory => "advisory".to_owned(),
            IssueKind::Policy => "policy".to_owned(),
        },
        issue.analysis.clone(),
        issue.family.clone(),
        issue.subject.identity(),
    ];
    for evidence in &issue.evidence {
        parts.push(evidence.id.clone());
        let mut measurements = evidence.measurements.clone();
        measurements.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.unit.cmp(&right.unit))
                .then(left.value.total_cmp(&right.value))
                .then_with(|| {
                    left.threshold
                        .unwrap_or(f64::NAN)
                        .total_cmp(&right.threshold.unwrap_or(f64::NAN))
                })
        });
        for measurement in measurements {
            parts.push(format!(
                "measurement:{}:{}:{}:{}",
                measurement.name,
                measurement.value,
                measurement
                    .threshold
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                measurement.unit
            ));
        }
        if let Some(witness) = &evidence.witness {
            parts.push(format!(
                "flow:{}:{}:{}:{}:{}:{}",
                canonical_path(&witness.source.path),
                witness.source.symbol,
                canonical_path(&witness.sink.path),
                witness.sink.symbol,
                witness.function_hops,
                witness.module_hops
            ));
            for step in &witness.ordered_steps {
                parts.push(format!(
                    "step:{}:{}:{}:{:?}",
                    canonical_path(&step.path),
                    step.symbol,
                    step.operation,
                    step.resolution
                ));
            }
        }
    }
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    stable_id("rc7", &refs)
}

fn stable_id(prefix: &str, parts: &[&str]) -> String {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update(part.as_bytes());
        digest.update([0]);
    }
    let hash = format!("{:x}", digest.finalize());
    format!("{prefix}-{}", &hash[..20])
}

fn canonical_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn validate_namespace(label: &str, value: &str) -> Result<()> {
    if value.split_once('.').is_none_or(|(namespace, name)| {
        namespace.is_empty() || name.is_empty() || value.chars().any(char::is_whitespace)
    }) {
        bail!("{label} `{value}` must be namespaced (for example reforge.codebase.large_file)");
    }
    Ok(())
}

fn validate_code(label: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.chars().any(|character| {
            !(character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_')
        })
    {
        bail!("{label} `{value}` must use lowercase snake_case");
    }
    Ok(())
}

fn rule_analysis(rule: &str) -> Result<&'static str> {
    if rule.starts_with("reforge.codebase.") {
        Ok(ANALYSIS_CODEBASE)
    } else if rule.starts_with("reforge.dataflow.") {
        Ok(ANALYSIS_DATAFLOW)
    } else if rule.starts_with("reforge.unity.") {
        Ok(ANALYSIS_UNITY)
    } else {
        bail!("evidence rule `{rule}` has an unknown producer namespace")
    }
}
