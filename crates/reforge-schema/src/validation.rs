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
        validate_limitations(&language.limitations)?;
    }
    Ok(())
}

fn validate_rule_coverage(coverage: &AnalysisCoverage) -> Result<()> {
    for (rule, execution) in &coverage.rules {
        validate_namespace("coverage rule", rule)?;
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
    if issue.id != issue_id(&issue.family, &issue.subject) {
        bail!("issue {} has an invalid stable ID", issue.id);
    }
    if issue.evidence.is_empty() {
        bail!("issue {} has no evidence", issue.id);
    }
    for evidence in &issue.evidence {
        validate_issue_evidence(issue, evidence, evidence_ids)?;
    }
    Ok(())
}

fn validate_issue_evidence(
    issue: &Issue,
    evidence: &Evidence,
    evidence_ids: &mut BTreeSet<String>,
) -> Result<()> {
    validate_namespace("evidence rule", &evidence.rule)?;
    if !evidence.id.starts_with("re6-") {
        bail!("evidence {} is not a schema 26 evidence ID", evidence.id);
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
    stable_id("ri6", &[family, &subject.identity()])
}

pub fn evidence_id(rule: &str, semantic_anchor: &str) -> String {
    stable_id("re6", &[rule, semantic_anchor])
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

fn canonical_member(member: &str) -> String {
    match member.split_once('#') {
        Some((path, symbol)) => format!("{}#{symbol}", canonical_path(path)),
        None => canonical_path(member),
    }
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
