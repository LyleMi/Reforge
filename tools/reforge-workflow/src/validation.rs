use std::collections::BTreeSet;

use anyhow::{Result, bail};

use crate::{
    CheckKind, Phase, PlanArtifact, RunArtifact, normalize_write_path, path_allowed,
    validate_artifact_version,
};

pub(crate) fn plan(run: &RunArtifact, plan: &PlanArtifact) -> Result<()> {
    validate_artifact_version(plan.artifact_schema_version)?;
    if plan.goal.trim().is_empty() || plan.goal != run.goal {
        bail!("plan goal must exactly match the workflow goal");
    }
    validate_selected_issues(run, plan)?;
    let normalized_write_set = normalize_write_set(plan)?;
    validate_required_checks(plan)?;
    validate_changes(plan, &normalized_write_set)
}

fn validate_selected_issues(run: &RunArtifact, plan: &PlanArtifact) -> Result<()> {
    if plan.selected_issue_ids.is_empty() {
        bail!("plan must select at least one issue");
    }
    let available = run.initial_issue_ids.iter().collect::<BTreeSet<_>>();
    for issue in &plan.selected_issue_ids {
        if !available.contains(issue) {
            bail!("plan selects unknown issue {issue}");
        }
    }
    Ok(())
}

fn normalize_write_set(plan: &PlanArtifact) -> Result<Vec<String>> {
    if plan.write_set.is_empty() {
        bail!("plan write_set must not be empty");
    }
    plan.write_set
        .iter()
        .map(|path| normalize_write_path(path))
        .collect()
}

fn validate_required_checks(plan: &PlanArtifact) -> Result<()> {
    if !plan
        .required_checks
        .iter()
        .any(|check| check.kind == CheckKind::Test)
    {
        bail!("plan must require at least one test check");
    }
    Ok(())
}

fn validate_changes(plan: &PlanArtifact, write_set: &[String]) -> Result<()> {
    for change in &plan.changes {
        if change.issue_ids.is_empty() || change.paths.is_empty() {
            bail!("each planned change needs issue_ids and paths");
        }
        for path in &change.paths {
            let path = normalize_write_path(path)?;
            if !write_set.iter().any(|allowed| path_allowed(&path, allowed)) {
                bail!("planned change path {path} is outside write_set");
            }
        }
    }
    Ok(())
}

pub(crate) fn require_phase(run: &RunArtifact, expected: Phase) -> Result<()> {
    if run.phase != expected {
        bail!(
            "command requires {:?} phase; run is {:?}",
            expected,
            run.phase
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(phase: Phase) -> RunArtifact {
        RunArtifact {
            artifact_schema_version: crate::SCHEMA_VERSION,
            phase,
            goal: "goal".into(),
            workspace_root: "/tmp".into(),
            workspace_identity: "rw5-test".into(),
            source_revision: None,
            initial_reports: Vec::new(),
            initial_issue_ids: Vec::new(),
            initial_producers: Vec::new(),
        }
    }

    #[test]
    fn phase_guard_accepts_only_the_exact_requested_phase() {
        let phases = [
            Phase::Imported,
            Phase::Planned,
            Phase::Approved,
            Phase::Applied,
            Phase::Verified,
        ];
        for current in phases {
            for expected in phases {
                assert_eq!(
                    require_phase(&run(current), expected).is_ok(),
                    current == expected
                );
            }
        }
    }
}
