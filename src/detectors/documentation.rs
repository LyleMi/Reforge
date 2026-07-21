use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};

use crate::cli::Cli;
use crate::evidence_analysis::FindingInput;
use crate::model::{Finding, FindingKind, FindingMetric, MetricId};

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
    "summary",
    "stats",
    "metrics_summary",
    "raw_metrics",
    "raw_metric_manifest",
    "dependency_graph",
    "agent_evidence",
    "unity_project",
    "suppression_summary",
    "coverage_manifest",
    "coverage_summary",
    "detector_execution",
    "raw_metric_coverage",
    "issues",
    "detector_manifest",
    "findings",
    "id",
    "kind",
    "path",
    "line",
    "metrics",
    "construct",
    "mechanism",
    "issue_id",
    "message",
    "recommendation",
    "related_locations",
    "action",
    "entity_scope",
    "issue_family",
    "evidence_role",
    "constituent_kinds",
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

pub(crate) fn scan_documentation(root: &Path) -> Result<Vec<Finding>> {
    if !should_scan_documentation(root) {
        return Ok(Vec::new());
    }

    DocumentationInventory::load(root)?.findings()
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

    fn findings(&self) -> Result<Vec<Finding>> {
        let mut findings = Vec::new();

        if let Some(finding) = self.missing_user_guide() {
            findings.push(finding);
        }
        if self.report_schema.is_none() {
            findings.push(self.missing_doc_finding(
                FindingKind::MissingReportSchemaDocs,
                "missing report schema docs; document JSON/YAML fields and compatibility expectations",
                90,
            ));
        }
        if self.metrics_model.is_none() {
            findings.push(self.missing_doc_finding(
                FindingKind::MissingMetricsModelDocs,
                "missing metrics model docs; document raw metrics, percentiles, findings, and coverage",
                75,
            ));
        }
        if self.architecture.is_none() {
            findings.push(self.missing_doc_finding(
                FindingKind::MissingArchitectureDocs,
                "missing architecture docs; document scan pipeline, detector boundaries, data flow, and extension points",
                75,
            ));
        }
        if let Some(finding) = self.stale_cli_documentation() {
            findings.push(finding);
        }
        if let Some(finding) = self.stale_schema_documentation() {
            findings.push(finding);
        }

        Ok(findings)
    }

    fn missing_user_guide(&self) -> Option<Finding> {
        let Some(path) = &self.user_guide else {
            return Some(Finding::from(FindingInput::new(
                FindingKind::MissingUserGuide,
                display_path(&self.root),
                None,
                "missing user guide; users need install, quick start, CLI, configuration, output, examples, and troubleshooting docs",
                vec![FindingMetric::threshold(
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

        Some(Finding::from(FindingInput::new(
            FindingKind::MissingUserGuide,
            display_path(path),
            Some(1),
            format!(
                "user guide has shallow coverage; missing topics: {}",
                missing_topics.join(", ")
            ),
            vec![FindingMetric::threshold(
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

    fn missing_doc_finding(&self, kind: FindingKind, message: &str, risk: usize) -> Finding {
        Finding::from(FindingInput::new(
            kind,
            display_path(&self.root),
            None,
            message,
            vec![FindingMetric::threshold(
                MetricId::DocumentationRisk,
                risk,
                35,
                "risk",
            )],
        ))
    }

    fn stale_cli_documentation(&self) -> Option<Finding> {
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

        Some(Finding::from(FindingInput::new(
            FindingKind::StaleCliDocumentation,
            self.user_guide
                .as_ref()
                .or(self.configuration.as_ref())
                .or(self.readme.as_ref())
                .map(|path| display_path(path))
                .unwrap_or_else(|| display_path(&self.root)),
            Some(1),
            format!("CLI documentation is stale: {}", problems.join("; ")),
            vec![FindingMetric::threshold(
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
            .filter(|command| Cli::try_parse_from(command).is_err())
            .map(|command| command.join(" "))
            .collect()
    }

    fn stale_schema_documentation(&self) -> Option<Finding> {
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

        Some(Finding::from(FindingInput::new(
            FindingKind::StaleSchemaDocumentation,
            display_path(path),
            Some(1),
            format!("report schema docs are stale: {}", problems.join("; ")),
            vec![FindingMetric::threshold(
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

fn declares_schema_field(text: &str, field: &str) -> bool {
    text.contains(&format!("`{field}`")) || text.contains(&format!("\"{field}\""))
}

fn executable_reforge_commands(contents: &str) -> Vec<Vec<String>> {
    let mut executable_fence = false;
    let mut commands = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if let Some(language) = trimmed.strip_prefix("```") {
            if executable_fence {
                executable_fence = false;
            } else {
                executable_fence = matches!(
                    language.trim().to_ascii_lowercase().as_str(),
                    "" | "bash" | "console" | "powershell" | "sh" | "shell" | "zsh"
                );
            }
            continue;
        }
        if !executable_fence {
            continue;
        }
        let command = trimmed.strip_prefix("$ ").unwrap_or(trimmed);
        if !command.starts_with("reforge ") {
            continue;
        }
        commands.push(command_tokens(command));
    }
    commands
}

fn command_tokens(command: &str) -> Vec<String> {
    let mut tokenizer = CommandTokenizer::default();
    for character in command.chars() {
        tokenizer.consume(character);
    }
    tokenizer.finish()
}

#[derive(Default)]
struct CommandTokenizer {
    tokens: Vec<String>,
    current: String,
    quote: Option<char>,
    escaped: bool,
}

impl CommandTokenizer {
    fn consume(&mut self, character: char) {
        if self.escaped {
            self.current.push(character);
            self.escaped = false;
            return;
        }
        if character == '\\' && self.quote != Some('\'') {
            self.escaped = true;
            return;
        }
        if matches!(character, '\'' | '"') {
            self.consume_quote(character);
        } else if character.is_whitespace() && self.quote.is_none() {
            self.flush();
        } else {
            self.current.push(character);
        }
    }

    fn consume_quote(&mut self, character: char) {
        match self.quote {
            Some(quote) if quote == character => self.quote = None,
            None => self.quote = Some(character),
            Some(_) => self.current.push(character),
        }
    }

    fn flush(&mut self) {
        if !self.current.is_empty() {
            self.tokens.push(std::mem::take(&mut self.current));
        }
    }

    fn finish(mut self) -> Vec<String> {
        if self.escaped {
            self.current.push('\\');
        }
        self.flush();
        self.tokens
    }
}

fn scan_cli_flags() -> Vec<String> {
    let command = Cli::command();
    let Some(scan) = command.find_subcommand("scan") else {
        return Vec::new();
    };

    scan.get_arguments()
        .filter_map(|argument| argument.get_long())
        .map(|long| format!("--{long}"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn first_existing(root: &Path, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
}

fn collect_markdown_files(root: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_markdown_files(&path, output)?;
        } else if path.extension().is_some_and(|extension| extension == "md") {
            output.push(path);
        }
    }
    Ok(())
}

fn display_path(path: &Path) -> String {
    let display = path.to_string_lossy().replace('\\', "/");
    display
        .strip_prefix("//?/")
        .unwrap_or(display.as_str())
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_flags_are_read_from_clap_metadata() {
        let flags = scan_cli_flags();

        assert!(flags.contains(&"--output".to_string()));
        assert!(flags.contains(&"--max-file-lines".to_string()));
        assert!(flags.contains(&"--churn-window-days".to_string()));
    }

    #[test]
    fn parses_only_executable_reforge_code_blocks() {
        let commands = executable_reforge_commands(
            "```bash\nreforge scan . --progress never\n```\n```text\nreforge scan [OPTIONS]\n```\n",
        );
        assert_eq!(
            commands,
            vec![vec!["reforge", "scan", ".", "--progress", "never"]]
        );
        assert!(Cli::try_parse_from(&commands[0]).is_ok());
    }

    #[test]
    fn preserves_quoted_command_arguments() {
        let commands = executable_reforge_commands(
            "```bash\nreforge workflow select run --issue ri3-example --goal \"desired outcome\"\n```\n",
        );

        assert_eq!(
            commands[0].last().map(String::as_str),
            Some("desired outcome")
        );
        assert!(Cli::try_parse_from(&commands[0]).is_ok());
    }
}
