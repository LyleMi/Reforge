use super::*;
use reforge_schema::{BaselineState, CoverageStatus, Issue, IssueKind, RuleActivation};

/// Explicit repository acceptance rules can include preview advisories. This
/// does not promote them to calibrated policy issues or change report kinds.
pub(super) fn selected_rule_failures(
    report: &Report,
    baseline: Option<&Report>,
    selected: &[String],
    mode: Option<GateArg>,
) -> Vec<String> {
    if selected.is_empty() {
        return Vec::new();
    }
    let rules = selected.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let mut failures = BTreeSet::new();
    if report.summary.scanned_files == 0 {
        failures.insert("no source files were analyzed".into());
    }
    for rule in &rules {
        check_rule_coverage(report, rule, &mut failures);
        if matches!(mode, Some(GateArg::New))
            && let Some(before) = baseline
        {
            check_baseline_contract(report, before, rule, &mut failures);
        }
    }
    check_current_issues(report, baseline, &rules, mode, &mut failures);
    if matches!(mode, Some(GateArg::New))
        && let Some(before) = baseline
    {
        check_absent_unknown(report, before, &rules, &mut failures);
    }
    failures.into_iter().collect()
}

fn check_current_issues(
    report: &Report,
    baseline: Option<&Report>,
    rules: &BTreeSet<&str>,
    mode: Option<GateArg>,
    failures: &mut BTreeSet<String>,
) {
    for issue in &report.issues {
        if issue.kind == IssueKind::Policy || !is_selected(issue, rules) {
            continue;
        }
        let blocks = match mode {
            Some(GateArg::All) => true,
            Some(GateArg::New) => selected_issue_changed(issue, baseline, rules),
            None => false,
        };
        if blocks {
            failures.insert(format!("{}: {}", issue.id, issue.title));
        }
    }
}

fn selected_issue_changed(
    issue: &Issue,
    baseline: Option<&Report>,
    rules: &BTreeSet<&str>,
) -> bool {
    let Some(previous) =
        baseline.and_then(|report| report.issues.iter().find(|before| before.id == issue.id))
    else {
        return true;
    };
    selected_fingerprint(issue, rules) != selected_fingerprint(previous, rules)
}

fn selected_fingerprint(issue: &Issue, rules: &BTreeSet<&str>) -> String {
    let mut selected = issue.clone();
    selected
        .evidence
        .retain(|evidence| rules.contains(evidence.rule.as_str()));
    reforge_schema::content_fingerprint(&selected)
}

fn is_selected(issue: &Issue, rules: &BTreeSet<&str>) -> bool {
    issue
        .evidence
        .iter()
        .any(|evidence| rules.contains(evidence.rule.as_str()))
}

fn check_rule_coverage(report: &Report, rule: &str, failures: &mut BTreeSet<String>) {
    let receipt = report
        .coverage
        .values()
        .find_map(|analysis| analysis.rules.get(rule));
    match receipt {
        None => {
            failures.insert(format!("{rule}: rule was not evaluated"));
        }
        Some(receipt)
            if matches!(
                receipt.enabled_source,
                RuleActivation::Disabled | RuleActivation::Internal
            ) =>
        {
            failures.insert(format!(
                "{rule}: explicitly enable this rule in rules.enable"
            ));
        }
        Some(receipt)
            if !matches!(
                receipt.status,
                CoverageStatus::Observed | CoverageStatus::NotApplicable
            ) || !receipt.limitations.is_empty() =>
        {
            failures.insert(format!(
                "{rule}: incomplete coverage ({:?})",
                receipt.status
            ));
        }
        _ => {}
    }
}

fn check_baseline_contract(
    report: &Report,
    baseline: &Report,
    rule: &str,
    failures: &mut BTreeSet<String>,
) {
    if report.provenance.scope_digest != baseline.provenance.scope_digest {
        failures
            .insert("scan scope or suppressions changed; review and refresh the baseline".into());
    }
    if report.provenance.rules.get(rule) != baseline.provenance.rules.get(rule) {
        failures.insert(format!("{rule}: rule configuration or semantics changed"));
    }
    let analysis = rule.split('.').nth(1).unwrap_or_default();
    if report.provenance.analyses.get(analysis) != baseline.provenance.analyses.get(analysis) {
        failures.insert(format!("{rule}: analysis configuration or policy changed"));
    }
    let mut baseline_failures = BTreeSet::new();
    check_rule_coverage(baseline, rule, &mut baseline_failures);
    failures.extend(
        baseline_failures
            .into_iter()
            .map(|failure| format!("baseline {failure}")),
    );
}

fn check_absent_unknown(
    report: &Report,
    baseline: &Report,
    rules: &BTreeSet<&str>,
    failures: &mut BTreeSet<String>,
) {
    let Some(comparison) = &report.baseline_comparison else {
        return;
    };
    for issue in &baseline.issues {
        if is_selected(issue, rules)
            && !report.issues.iter().any(|current| current.id == issue.id)
            && comparison
                .issues
                .get(&issue.id)
                .is_some_and(|entry| entry.state == BaselineState::Unknown)
        {
            failures.insert(format!(
                "{}: absent baseline issue remains unknown",
                issue.id
            ));
        }
    }
}
