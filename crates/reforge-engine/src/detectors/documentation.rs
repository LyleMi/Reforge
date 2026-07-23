use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::evidence_analysis::DetectedEvidenceInput;
use crate::model::{DetectedEvidence, DetectedMeasurement, MetricId, Rule};
use anyhow::{Context, Result};

const PROJECT_MARKERS: &[&str] = &[
    "README.md",
    "readme.md",
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
    ".git",
];

const USER_GUIDE_TOPICS: &[(&str, &[&str])] = &[
    ("installation", &["install", "installation"]),
    ("quick_start", &["quick start", "quickstart"]),
    ("cli", &["cli", "command", "scan", "--"]),
    (
        "configuration",
        &["configuration", "config", "reforge.toml"],
    ),
    ("output", &["output", "json", "yaml", "report"]),
    (
        "troubleshooting",
        &["troubleshooting", "troubleshoot", "debug"],
    ),
];

const REQUIRED_SCHEMA_FIELDS: &[&str] = &[
    "schema_version",
    "producer",
    "target",
    "summary",
    "suppression",
    "coverage",
    "issues",
    "id",
    "family",
    "subject",
    "title",
    "guidance",
    "evidence",
    "rule",
    "message",
    "measurements",
    "locations",
];

const REMOVED_SCHEMA_FIELDS: &[&str] = &[
    "hotspots",
    "scoring_policy",
    "severity",
    "priority",
    "detection_reliability",
    "interpretation_reliability",
    "priority_factors",
    "rank_explanation",
];

#[derive(Debug)]
struct DocumentationInventory {
    root: PathBuf,
    readme: Option<PathBuf>,
    docs_index: Option<PathBuf>,
    user_guide: Option<PathBuf>,
    configuration: Option<PathBuf>,
    report_schema: Option<PathBuf>,
    metrics_model: Option<PathBuf>,
    detectors: Option<PathBuf>,
    architecture: Option<PathBuf>,
    contributing: Option<PathBuf>,
    release: Option<PathBuf>,
    contents: BTreeMap<PathBuf, String>,
}

pub(crate) fn scan_documentation(root: &Path) -> Result<Vec<DetectedEvidence>> {
    if !should_scan_documentation(root) {
        return Ok(Vec::new());
    }

    DocumentationInventory::load(root)?.detections()
}

fn should_scan_documentation(root: &Path) -> bool {
    root.is_dir()
        && PROJECT_MARKERS
            .iter()
            .any(|marker| root.join(marker).exists())
        && is_reforge_project(root)
}

fn is_reforge_project(root: &Path) -> bool {
    manifest_project_name(&root.join("Cargo.toml"), "package")
        .or_else(|| manifest_project_name(&root.join("package.json"), ""))
        .is_some_and(|name| name == "reforge")
}

fn manifest_project_name(path: &Path, table: &str) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
        return serde_json::from_str::<serde_json::Value>(&contents)
            .ok()?
            .get("name")?
            .as_str()
            .map(ToString::to_string);
    }

    let value = toml::from_str::<toml::Value>(&contents).ok()?;
    if table.is_empty() {
        value.get("name")?.as_str().map(ToString::to_string)
    } else {
        value
            .get(table)?
            .get("name")?
            .as_str()
            .map(ToString::to_string)
    }
}

impl DocumentationInventory {
    fn load(root: &Path) -> Result<Self> {
        let docs_dir = root.join("docs");
        let readme = first_existing(root, &["README.md", "readme.md"]);
        let docs_index = first_existing(&docs_dir, &["README.md", "index.md"]);
        let user_guide = first_existing(&docs_dir, &["user-guide.md", "usage.md", "cli.md"]);
        let configuration = first_existing(&docs_dir, &["configuration.md", "config.md"]);
        let report_schema = first_existing(&docs_dir, &["report-schema.md", "schema.md"]);
        let metrics_model = first_existing(&docs_dir, &["metrics-model.md", "scoring.md"]);
        let detectors = first_existing(&docs_dir, &["detectors.md"]);
        let architecture = first_existing(&docs_dir, &["architecture.md", "design.md"]);
        let contributing = first_existing(&docs_dir, &["contributing.md"])
            .or_else(|| first_existing(root, &["CONTRIBUTING.md", "contributing.md"]));
        let release = first_existing(&docs_dir, &["release.md", "releasing.md"]);
        let mut inventory = Self {
            root: root.to_path_buf(),
            readme,
            docs_index,
            user_guide,
            configuration,
            report_schema,
            metrics_model,
            detectors,
            architecture,
            contributing,
            release,
            contents: BTreeMap::new(),
        };
        inventory.read_known_docs()?;
        Ok(inventory)
    }

    fn read_known_docs(&mut self) -> Result<()> {
        let mut paths = self.known_doc_paths();
        collect_markdown_files(&self.root.join("docs"), &mut paths)?;
        collect_markdown_files(&self.root.join("skills"), &mut paths)?;
        paths.sort();
        paths.dedup();
        for path in paths {
            if self.contents.contains_key(&path) {
                continue;
            }
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("failed to read documentation file {}", path.display()))?;
            self.contents.insert(path, contents);
        }
        Ok(())
    }

    fn known_doc_paths(&self) -> Vec<PathBuf> {
        [
            self.readme.as_ref(),
            self.docs_index.as_ref(),
            self.user_guide.as_ref(),
            self.configuration.as_ref(),
            self.report_schema.as_ref(),
            self.metrics_model.as_ref(),
            self.detectors.as_ref(),
            self.architecture.as_ref(),
            self.contributing.as_ref(),
            self.release.as_ref(),
        ]
        .into_iter()
        .flatten()
        .cloned()
        .collect()
    }

    fn detections(&self) -> Result<Vec<DetectedEvidence>> {
        let mut detections = Vec::new();

        if let Some(detection) = self.missing_user_guide() {
            detections.push(detection);
        }
        if self.report_schema.is_none() {
            detections.push(self.missing_doc_detection(
                Rule::MissingReportSchemaDocs,
                "missing report schema docs; document JSON/YAML fields and compatibility expectations",
                90,
            ));
        }
        if self.metrics_model.is_none() {
            detections.push(self.missing_doc_detection(
                Rule::MissingMetricsModelDocs,
                "missing metrics model docs; document measurements, evidence, issues, and coverage",
                75,
            ));
        }
        if self.architecture.is_none() {
            detections.push(self.missing_doc_detection(
                Rule::MissingArchitectureDocs,
                "missing architecture docs; document scan pipeline, detector boundaries, data flow, and extension points",
                75,
            ));
        }
        if let Some(detection) = self.stale_cli_documentation() {
            detections.push(detection);
        }
        if let Some(detection) = self.stale_schema_documentation() {
            detections.push(detection);
        }

        Ok(detections)
    }

    fn missing_user_guide(&self) -> Option<DetectedEvidence> {
        let Some(path) = &self.user_guide else {
            return Some(DetectedEvidence::from(DetectedEvidenceInput::new(
                Rule::MissingUserGuide,
                display_path(&self.root),
                None,
                "missing user guide; users need install, quick start, CLI, configuration, output, examples, and troubleshooting docs",
                vec![DetectedMeasurement::threshold(
                    MetricId::DocumentationMissingUserTopics,
                    USER_GUIDE_TOPICS.len(),
                    1,
                    "topics",
                )],
            )));
        };

        let missing_topics = self.missing_user_guide_topics(path);
        if missing_topics.is_empty() {
            return None;
        }

        Some(DetectedEvidence::from(DetectedEvidenceInput::new(
            Rule::MissingUserGuide,
            display_path(path),
            Some(1),
            format!(
                "user guide has shallow coverage; missing topics: {}",
                missing_topics.join(", ")
            ),
            vec![DetectedMeasurement::threshold(
                MetricId::DocumentationMissingUserTopics,
                missing_topics.len(),
                1,
                "topics",
            )],
        )))
    }

    fn missing_user_guide_topics(&self, path: &Path) -> Vec<&'static str> {
        let text = self
            .contents
            .get(path)
            .map(|contents| contents.to_ascii_lowercase())
            .unwrap_or_default();
        USER_GUIDE_TOPICS
            .iter()
            .filter_map(|(topic, terms)| {
                (!terms.iter().any(|term| text.contains(term))).then_some(*topic)
            })
            .collect()
    }

    fn missing_doc_detection(&self, kind: Rule, message: &str, risk: usize) -> DetectedEvidence {
        DetectedEvidence::from(DetectedEvidenceInput::new(
            kind,
            display_path(&self.root),
            None,
            message,
            vec![DetectedMeasurement::threshold(
                MetricId::DocumentationRisk,
                risk,
                35,
                "risk",
            )],
        ))
    }

    fn stale_cli_documentation(&self) -> Option<DetectedEvidence> {
        let documented_text = self
            .texts_for([
                self.user_guide.as_ref(),
                self.configuration.as_ref(),
                self.readme.as_ref(),
            ])
            .to_ascii_lowercase();
        let documented_any_flag = documented_text.contains("--");
        let invalid_commands = self.invalid_documented_commands();
        if !documented_any_flag && invalid_commands.is_empty() {
            return None;
        }

        let missing_flags = scan_cli_flags()
            .into_iter()
            .filter(|flag| !documented_text.contains(&flag.to_ascii_lowercase()))
            .collect::<Vec<_>>();
        if missing_flags.is_empty() && invalid_commands.is_empty() {
            return None;
        }

        let mut problems = Vec::new();
        if !missing_flags.is_empty() {
            problems.push(format!(
                "missing current flags: {}",
                missing_flags.join(", ")
            ));
        }
        if !invalid_commands.is_empty() {
            problems.push(format!(
                "contains commands rejected by the parser: {}",
                invalid_commands.join(" | ")
            ));
        }

        Some(DetectedEvidence::from(DetectedEvidenceInput::new(
            Rule::StaleCliDocumentation,
            self.user_guide
                .as_ref()
                .or(self.configuration.as_ref())
                .or(self.readme.as_ref())
                .map(|path| display_path(path))
                .unwrap_or_else(|| display_path(&self.root)),
            Some(1),
            format!("CLI documentation is stale: {}", problems.join("; ")),
            vec![DetectedMeasurement::threshold(
                MetricId::DocumentationMissingCliFlags,
                missing_flags.len() + invalid_commands.len(),
                1,
                "flags",
            )],
        )))
    }

    fn invalid_documented_commands(&self) -> Vec<String> {
        self.contents
            .values()
            .flat_map(|contents| executable_reforge_commands(contents))
            .filter(|command| !is_current_reforge_command(command))
            .map(|command| command.join(" "))
            .collect()
    }

    fn stale_schema_documentation(&self) -> Option<DetectedEvidence> {
        let path = self.report_schema.as_ref()?;
        let text = self
            .contents
            .get(path)
            .map(|contents| contents.to_ascii_lowercase())
            .unwrap_or_default();
        let current_contract = text
            .split("## compatibility notes")
            .next()
            .unwrap_or(text.as_str());
        let missing_fields = REQUIRED_SCHEMA_FIELDS
            .iter()
            .filter(|field| !declares_schema_field(current_contract, field))
            .copied()
            .collect::<Vec<_>>();
        let removed_fields = REMOVED_SCHEMA_FIELDS
            .iter()
            .filter(|field| declares_schema_field(current_contract, field))
            .copied()
            .collect::<Vec<_>>();
        if missing_fields.is_empty() && removed_fields.is_empty() {
            return None;
        }

        let mut problems = Vec::new();
        if !missing_fields.is_empty() {
            problems.push(format!(
                "missing current fields: {}",
                missing_fields.join(", ")
            ));
        }
        if !removed_fields.is_empty() {
            problems.push(format!(
                "declares removed schema 20 fields as current: {}",
                removed_fields.join(", ")
            ));
        }

        Some(DetectedEvidence::from(DetectedEvidenceInput::new(
            Rule::StaleSchemaDocumentation,
            display_path(path),
            Some(1),
            format!("report schema docs are stale: {}", problems.join("; ")),
            vec![DetectedMeasurement::threshold(
                MetricId::DocumentationMissingSchemaFields,
                missing_fields.len() + removed_fields.len(),
                1,
                "fields",
            )],
        )))
    }

    fn texts_for<const N: usize>(&self, paths: [Option<&PathBuf>; N]) -> String {
        paths
            .into_iter()
            .flatten()
            .filter_map(|path| self.contents.get(path))
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }
}

include!("documentation_parsing.rs");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_contract_lists_current_analyze_flags() {
        let flags = scan_cli_flags();

        assert!(flags.contains(&"--output".to_string()));
        assert!(flags.contains(&"--analysis".to_string()));
        assert!(flags.contains(&"--set".to_string()));
    }

    #[test]
    fn parses_only_executable_reforge_code_blocks() {
        let commands = executable_reforge_commands(
            "```bash\nreforge config validate .\n```\n```text\nreforge config validate [PATH]\n```\n",
        );
        assert_eq!(commands, vec![vec!["reforge", "config", "validate", "."]]);
        assert!(is_current_reforge_command(&commands[0]));
    }

    #[test]
    fn preserves_quoted_command_arguments() {
        let commands = executable_reforge_commands(
            "```bash\nreforge analyze . --set codebase.max-file-lines=500\n```\n",
        );

        assert_eq!(
            commands[0].last().map(String::as_str),
            Some("codebase.max-file-lines=500")
        );
        assert!(is_current_reforge_command(&commands[0]));
    }
}
