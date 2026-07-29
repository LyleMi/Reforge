use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Observed,
    Partial,
    Unsupported,
    NotApplicable,
}

impl CoverageStatus {
    pub fn is_observable(self) -> bool {
        matches!(self, Self::Observed | Self::Partial)
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::Observed => 3,
            Self::Partial => 2,
            Self::NotApplicable => 1,
            Self::Unsupported => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageCoverage {
    pub status: CoverageStatus,
    pub files: usize,
    pub functions: usize,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub capabilities: BTreeMap<String, CapabilityReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<CoverageLimitation>,
}

impl Default for LanguageCoverage {
    fn default() -> Self {
        Self {
            status: CoverageStatus::Observed,
            files: 0,
            functions: 0,
            capabilities: BTreeMap::new(),
            limitations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityReceipt {
    pub status: CoverageStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<CoverageLimitation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageObservation {
    pub name: String,
    pub count: usize,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleExecution {
    pub status: CoverageStatus,
    pub maturity: String,
    pub enabled_source: RuleActivation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observations: Vec<CoverageObservation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<CoverageLimitation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleActivation {
    Default,
    Enable,
    Enforce,
    Disabled,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageLimitation {
    pub code: String,
    pub count: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisCoverage {
    pub status: CoverageStatus,
    pub scanned_files: usize,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub languages: BTreeMap<String, LanguageCoverage>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rules: BTreeMap<String, RuleExecution>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<CoverageLimitation>,
}
