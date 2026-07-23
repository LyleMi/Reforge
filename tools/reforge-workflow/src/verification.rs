use std::collections::{BTreeMap, BTreeSet};

use reforge_schema::{CoverageStatus, Report};

use crate::{PlanArtifact, RunArtifact, VerificationOutcome};

pub(crate) struct IssueChanges {
    pub resolved: Vec<String>,
    pub remaining: Vec<String>,
    pub new_issues: Vec<String>,
}

pub(crate) fn issue_changes(
    run: &RunArtifact,
    plan: &PlanArtifact,
    fresh: &[Report],
) -> IssueChanges {
    let current = fresh
        .iter()
        .flat_map(|report| report.issues.iter().map(|issue| issue.id.clone()))
        .collect::<BTreeSet<_>>();
    let initial = run
        .initial_issue_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let selected = plan
        .selected_issue_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    IssueChanges {
        remaining: selected.intersection(&current).cloned().collect(),
        resolved: selected.difference(&current).cloned().collect(),
        new_issues: current.difference(&initial).cloned().collect(),
    }
}

pub(crate) fn missing_producers(run: &RunArtifact, fresh: &[Report]) -> Vec<String> {
    let available = fresh
        .iter()
        .map(|report| report.producer.name.as_str())
        .collect::<BTreeSet<_>>();
    run.initial_producers
        .iter()
        .filter(|producer| !available.contains(producer.as_str()))
        .map(|producer| format!("verification report from producer {producer} is missing"))
        .collect()
}

pub(crate) fn outcome(
    mut failed: Vec<String>,
    needs_input: Vec<String>,
) -> (VerificationOutcome, Vec<String>) {
    if !failed.is_empty() {
        failed.extend(needs_input);
        (VerificationOutcome::Failed, failed)
    } else if !needs_input.is_empty() {
        (VerificationOutcome::NeedsInput, needs_input)
    } else {
        (VerificationOutcome::Pass, Vec::new())
    }
}

pub(crate) fn evaluate_coverage(
    initial: &[Report],
    fresh: &[Report],
    plan: &PlanArtifact,
    failed: &mut Vec<String>,
    needs_input: &mut Vec<String>,
) {
    let initial_map = coverage_by_producer(initial);
    let fresh_map = coverage_by_producer(fresh);
    let selected_coverage = initial
        .iter()
        .flat_map(|report| {
            report
                .issues
                .iter()
                .filter(|issue| plan.selected_issue_ids.contains(&issue.id))
                .map(|issue| (report.producer.name.clone(), issue.analysis.clone()))
        })
        .collect::<BTreeSet<_>>();
    for ((producer, analysis), before) in initial_map {
        if !selected_coverage.contains(&(producer.clone(), analysis.clone())) {
            continue;
        }
        match fresh_map.get(&(producer, analysis.clone())).copied() {
            None | Some(CoverageStatus::Unsupported | CoverageStatus::NotApplicable) => {
                needs_input.push(format!("coverage for {analysis} is not observable"));
            }
            Some(after) if after.rank() < before.rank() => failed.push(format!(
                "coverage for {} degraded from {:?} to {:?}",
                analysis, before, after
            )),
            Some(_) => {}
        }
    }
}

fn coverage_by_producer(reports: &[Report]) -> BTreeMap<(String, String), CoverageStatus> {
    reports
        .iter()
        .flat_map(|report| {
            report.coverage.iter().map(move |(analysis, coverage)| {
                (
                    (report.producer.name.clone(), analysis.clone()),
                    coverage.status,
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_all_terminal_verification_outcomes() {
        assert_eq!(outcome(Vec::new(), Vec::new()).0, VerificationOutcome::Pass);
        assert_eq!(
            outcome(vec!["failed".into()], Vec::new()).0,
            VerificationOutcome::Failed
        );
        assert_eq!(
            outcome(Vec::new(), vec!["input".into()]).0,
            VerificationOutcome::NeedsInput
        );
    }
}
