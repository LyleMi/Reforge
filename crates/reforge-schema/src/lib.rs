//! Stable, producer-neutral contracts shared by every Reforge tool.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

mod baseline;
mod coverage;
mod issue;
mod report;

pub use baseline::{
    AnalysisProvenance, BaselineComparison, BaselineEntry, BaselineState, ReportProvenance,
    RuleProvenance,
};
pub use coverage::{
    AnalysisCoverage, CapabilityReceipt, CoverageLimitation, CoverageObservation, CoverageStatus,
    LanguageCoverage, RuleActivation, RuleExecution,
};
pub use issue::{
    EntityRef, Evidence, FlowEndpoint, FlowResolution, FlowStep, FlowWitness, Issue, IssueInput,
    IssueKind, Location, Measurement, Subject,
};
pub use report::{
    Producer, Report, ReportInput, ReportSummary, SuppressionSummary, Target, default_provenance,
};

pub const REPORT_SCHEMA_VERSION: u16 = 27;
pub const IDENTITY_SCHEME: &str = "reforge-identity-v7";
pub const ANALYSIS_CODEBASE: &str = "codebase";
pub const ANALYSIS_DATAFLOW: &str = "dataflow";
pub const ANALYSIS_UNITY: &str = "unity";

include!("validation.rs");

#[cfg(test)]
mod tests;
