use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use ignore::WalkBuilder;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::cli::{
    Cli, Command, ScanArgs, WorkflowArgs, WorkflowCheckArgs, WorkflowCheckKind, WorkflowCommand,
    WorkflowConfirmLineageArgs, WorkflowRunArgs, WorkflowSelectArgs, WorkflowStartArgs,
};
use crate::model::{
    DetectorExecutionStatus, EvidenceId, FindingKind, IssueKey, SCAN_REPORT_SCHEMA_VERSION,
    ScanReport,
};
use crate::scan::{self, NoopProgress};

const ARTIFACT_SCHEMA_VERSION: u8 = 2;
const OUTPUT_SUMMARY_LIMIT: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkflowPhase {
    Scanned,
    Selected,
    Investigated,
    Planned,
    Approved,
    Applied,
    Verified,
    Failed,
    NeedsInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunArtifact {
    artifact_schema_version: u8,
    reforge_version: String,
    report_schema_version: u8,
    target_root: String,
    phase: WorkflowPhase,
    scan_command: Vec<String>,
    effective_config: Value,
    report_fingerprint: String,
    config_fingerprint: String,
    source_fingerprint: String,
    created_at_epoch_ms: u128,
    updated_at_epoch_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionArtifact {
    artifact_schema_version: u8,
    report_fingerprint: String,
    issue_ids: Vec<IssueKey>,
    goal: String,
    selected_at_epoch_ms: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InvestigationStatus {
    Complete,
    NeedsInput,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InvestigationFact {
    pub path: String,
    pub line: Option<usize>,
    pub statement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RejectedAlternative {
    pub alternative: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommandSpec {
    pub kind: WorkflowCheckKind,
    pub program: String,
    pub args: Vec<String>,
    pub expected_observation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InvestigationArtifact {
    pub artifact_schema_version: u8,
    pub issue_id: IssueKey,
    pub finding_ids: Vec<EvidenceId>,
    pub report_fingerprint: String,
    pub status: InvestigationStatus,
    pub facts: Vec<InvestigationFact>,
    pub analysis: Vec<String>,
    pub unknowns: Vec<String>,
    pub rejected_alternatives: Vec<RejectedAlternative>,
    pub inspected_files: Vec<String>,
    pub read_set: Vec<String>,
    pub write_set: Vec<String>,
    pub coverage_limitations: Vec<String>,
    pub checks: Vec<CommandSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConflictEdge {
    pub left_issue_id: IssueKey,
    pub right_issue_id: IssueKey,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanBatch {
    pub issue_ids: Vec<IssueKey>,
    pub write_set: Vec<String>,
    pub outcome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlannedCheck {
    pub kind: WorkflowCheckKind,
    pub program: String,
    pub args: Vec<String>,
    pub required: bool,
    pub expected_observation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanArtifact {
    pub artifact_schema_version: u8,
    pub report_fingerprint: String,
    pub goal: String,
    pub outcome: String,
    pub selected_issue_ids: Vec<IssueKey>,
    pub batches: Vec<PlanBatch>,
    pub write_set: Vec<String>,
    pub behavior_assumptions: Vec<String>,
    pub checks: Vec<PlannedCheck>,
    pub unresolved_risks: Vec<String>,
    pub conflicts: Vec<ConflictEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApprovalArtifact {
    artifact_schema_version: u8,
    report_fingerprint: String,
    plan_fingerprint: String,
    write_set: Vec<String>,
    workspace_snapshot_fingerprint: String,
    workspace_snapshot: BTreeMap<String, String>,
    approved_at_epoch_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileChange {
    path: String,
    before_sha256: Option<String>,
    after_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationArtifact {
    artifact_schema_version: u8,
    plan_fingerprint: String,
    changed_files: Vec<FileChange>,
    workspace_snapshot_fingerprint: String,
    applied_at_epoch_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckRecord {
    kind: WorkflowCheckKind,
    program: String,
    args: Vec<String>,
    declared: bool,
    command_found: bool,
    success: bool,
    timed_out: bool,
    exit_code: Option<i32>,
    duration_ms: u128,
    output_summary: String,
    recorded_at_epoch_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerificationArtifact {
    artifact_schema_version: u8,
    checks: Vec<CheckRecord>,
    result: Option<WorkflowPhase>,
    reasons: Vec<String>,
    finished_at_epoch_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RescanArtifact {
    artifact_schema_version: u8,
    original_report_fingerprint: String,
    rescan_report_fingerprint: String,
    selected_evidence_removed: Vec<EvidenceId>,
    selected_evidence_still_present: Vec<EvidenceId>,
    new_evidence: Vec<EvidenceId>,
    unobservable: Vec<EvidenceId>,
    coverage_limitations: Vec<String>,
    selected_issues_removed: Vec<IssueKey>,
    selected_issues_unobservable: Vec<IssueKey>,
    lineage_candidates: Vec<crate::model::LineageCandidate>,
    rescanned_at_epoch_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LineageRecordKind {
    Supersedes,
    Remediated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LineageRecord {
    kind: LineageRecordKind,
    previous_issue_id: IssueKey,
    successor_issue_id: Option<IssueKey>,
    candidate_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LineageArtifact {
    artifact_schema_version: u8,
    original_report_fingerprint: String,
    rescan_report_fingerprint: String,
    records: Vec<LineageRecord>,
    confirmed_at_epoch_ms: u128,
}

#[derive(Debug)]
struct RunContext {
    dir: PathBuf,
    run: RunArtifact,
    root: PathBuf,
}

pub(crate) fn run(args: WorkflowArgs) -> Result<()> {
    match args.command {
        WorkflowCommand::Start(args) => start(*args),
        WorkflowCommand::Select(args) => select(args),
        WorkflowCommand::Status(args) => status(args),
        WorkflowCommand::Validate(args) => validate_command(args),
        WorkflowCommand::Advance(args) => advance(args),
        WorkflowCommand::Approve(args) => approve(args),
        WorkflowCommand::MarkApplied(args) => mark_applied(args),
        WorkflowCommand::Check(args) => check(args),
        WorkflowCommand::Rescan(args) => rescan(args),
        WorkflowCommand::ConfirmLineage(args) => confirm_lineage(args),
        WorkflowCommand::Finish(args) => finish(args),
    }
}

include!("workflow/commands.rs");
include!("workflow/validation.rs");
include!("workflow/conflicts.rs");
include!("workflow/support.rs");
include!("workflow/tests.rs");
