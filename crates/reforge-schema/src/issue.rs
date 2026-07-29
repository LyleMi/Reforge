use serde::{Deserialize, Serialize};

use crate::{canonical_path, content_fingerprint, evidence_id, issue_id};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityRef {
    pub key: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

impl EntityRef {
    pub fn new(key: impl Into<String>, path: impl Into<String>, symbol: Option<String>) -> Self {
        Self {
            key: key.into(),
            path: canonical_path(&path.into()),
            symbol,
        }
    }

    pub(crate) fn canonicalized(mut self) -> Self {
        self.path = canonical_path(&self.path);
        self
    }

    pub(crate) fn identity(&self) -> String {
        let entity = self.clone().canonicalized();
        format!(
            "{}:{}:{}",
            entity.key,
            entity.path,
            entity.symbol.as_deref().unwrap_or_default()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Subject {
    Repository,
    Directory { entity: EntityRef },
    File { entity: EntityRef },
    Symbol { entity: EntityRef },
    Group { members: Vec<EntityRef> },
}

impl Subject {
    pub fn canonicalized(mut self) -> Self {
        match &mut self {
            Self::Repository => {}
            Self::Directory { entity } | Self::File { entity } | Self::Symbol { entity } => {
                *entity = entity.clone().canonicalized();
            }
            Self::Group { members } => {
                for member in &mut *members {
                    *member = member.clone().canonicalized();
                }
                members.sort();
                members.dedup();
            }
        }
        self
    }

    pub fn identity(&self) -> String {
        match self.clone().canonicalized() {
            Self::Repository => "repository".into(),
            Self::Directory { entity } => format!("directory:{}", entity.identity()),
            Self::File { entity } => format!("file:{}", entity.identity()),
            Self::Symbol { entity } => format!("symbol:{}", entity.identity()),
            Self::Group { members } => format!(
                "group:{}",
                members
                    .iter()
                    .map(EntityRef::identity)
                    .collect::<Vec<_>>()
                    .join("|")
            ),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::Repository => "repository".into(),
            Self::Directory { entity } | Self::File { entity } => canonical_path(&entity.path),
            Self::Symbol { entity } => format!(
                "{} in {}",
                entity.symbol.as_deref().unwrap_or(&entity.key),
                canonical_path(&entity.path)
            ),
            Self::Group { members } => {
                let count = members.len();
                format!("{count} related items")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    pub name: String,
    pub value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    pub unit: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowResolution {
    Exact,
    Modeled,
    Unresolved,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowEndpoint {
    pub path: String,
    pub symbol: String,
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowStep {
    pub path: String,
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    pub operation: String,
    pub resolution: FlowResolution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowWitness {
    pub source: FlowEndpoint,
    pub sink: FlowEndpoint,
    pub ordered_steps: Vec<FlowStep>,
    pub function_hops: usize,
    pub module_hops: usize,
    pub resolution: FlowResolution,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub id: String,
    pub rule: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub measurements: Vec<Measurement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<Location>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub witness: Option<FlowWitness>,
}

impl Evidence {
    pub fn new(rule: impl Into<String>, semantic_anchor: &str, message: impl Into<String>) -> Self {
        let rule = rule.into();
        Self {
            id: evidence_id(&rule, semantic_anchor),
            rule,
            message: message.into(),
            measurements: Vec::new(),
            locations: Vec::new(),
            witness: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Issue {
    pub id: String,
    pub kind: IssueKind,
    pub content_fingerprint: String,
    pub analysis: String,
    pub family: String,
    pub subject: Subject,
    pub title: String,
    pub guidance: String,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    Advisory,
    Policy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IssueInput {
    pub kind: IssueKind,
    pub analysis: String,
    pub family: String,
    pub subject: Subject,
    pub title: String,
    pub guidance: String,
    pub evidence: Vec<Evidence>,
}

impl Issue {
    pub fn new(input: IssueInput) -> Self {
        let IssueInput {
            kind,
            analysis,
            family,
            subject,
            title,
            guidance,
            mut evidence,
        } = input;
        let subject = subject.canonicalized();
        evidence.sort_by(|left, right| left.id.cmp(&right.id));
        let mut issue = Self {
            id: issue_id(&family, &subject),
            kind,
            content_fingerprint: String::new(),
            analysis,
            family,
            subject,
            title,
            guidance,
            evidence,
        };
        issue.content_fingerprint = content_fingerprint(&issue);
        issue
    }
}
