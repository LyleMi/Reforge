use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub(crate) const SCHEMA_VERSION: u16 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CheckKind {
    Test,
    Build,
    Lint,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Phase {
    Imported,
    Planned,
    Approved,
    Applied,
    Verified,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunArtifact {
    pub artifact_schema_version: u16,
    pub phase: Phase,
    pub goal: String,
    pub workspace_root: String,
    pub workspace_identity: String,
    pub source_revision: Option<String>,
    pub initial_reports: Vec<String>,
    pub initial_issue_ids: Vec<String>,
    pub initial_producers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanArtifact {
    pub artifact_schema_version: u16,
    pub goal: String,
    pub selected_issue_ids: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    pub changes: Vec<PlannedChange>,
    pub write_set: Vec<String>,
    pub required_checks: Vec<RequiredCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlannedChange {
    pub description: String,
    pub issue_ids: Vec<String>,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequiredCheck {
    pub kind: CheckKind,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApprovalArtifact {
    pub artifact_schema_version: u16,
    pub plan_hash: String,
    pub write_set: Vec<String>,
    pub workspace_snapshot: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApplicationArtifact {
    pub artifact_schema_version: u16,
    pub changed_paths: Vec<String>,
    pub workspace_snapshot: BTreeMap<String, String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChecksArtifact {
    pub artifact_schema_version: u16,
    pub checks: Vec<CheckResult>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckResult {
    pub kind: CheckKind,
    pub command: Vec<String>,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u128,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VerificationOutcome {
    Pass,
    Failed,
    NeedsInput,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VerificationArtifact {
    pub artifact_schema_version: u16,
    pub outcome: VerificationOutcome,
    pub reasons: Vec<String>,
    pub reports: Vec<String>,
    pub resolved_issue_ids: Vec<String>,
    pub remaining_issue_ids: Vec<String>,
    pub new_issue_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v5_plan_round_trips_notes_and_rejects_removed_fields() {
        let plan = PlanArtifact {
            artifact_schema_version: SCHEMA_VERSION,
            goal: "split boundary".into(),
            selected_issue_ids: vec!["ri6-example".into()],
            notes: vec!["keep the public API stable".into()],
            changes: vec![PlannedChange {
                description: "extract adapter".into(),
                issue_ids: vec!["ri6-example".into()],
                paths: vec!["src/adapter.rs".into()],
            }],
            write_set: vec!["src".into()],
            required_checks: vec![RequiredCheck {
                kind: CheckKind::Test,
                description: "workspace suite".into(),
            }],
        };
        let value = serde_json::to_value(&plan).unwrap();
        let parsed: PlanArtifact = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(parsed.notes, plan.notes);

        let mut old = value;
        old.as_object_mut()
            .unwrap()
            .insert("investigation".into(), serde_json::json!([]));
        assert!(serde_json::from_value::<PlanArtifact>(old).is_err());
    }
}
