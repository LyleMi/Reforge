use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::CommandFactory;

use crate::cli::Cli;
use crate::model::{Finding, FindingKind, FindingMetric};
use crate::scoring::{FindingInput, finding};

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
    "hotspots",
    "findings",
    "kind",
    "severity",
    "path",
    "line",
    "metrics",
    "priority",
    "confidence",
    "priority_factors",
    "rank_explanation",
    "related_locations",
];

#[derive(Debug)]
struct DocumentationInventory {
    root: PathBuf,
    readme: Option<PathBuf>,
    docs_dir: PathBuf,
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
            docs_dir,
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
        for path in self.known_doc_paths() {
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

        if let Some(finding) = self.missing_documentation_set() {
            findings.push(finding);
        }
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
                "missing metrics model docs; document raw metrics, findings, hotspots, priority, and confidence",
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

    fn missing_documentation_set(&self) -> Option<Finding> {
        let missing_count = [
            self.docs_index.as_ref(),
            self.user_guide.as_ref(),
            self.configuration.as_ref(),
            self.report_schema.as_ref(),
            self.metrics_model.as_ref(),
            self.detectors.as_ref(),
            self.architecture.as_ref(),
            self.contributing.as_ref(),
        ]
        .into_iter()
        .filter(|path| path.is_none())
        .count();

        if self.docs_dir.is_dir() && missing_count == 0 {
            return None;
        }

        Some(finding(FindingInput::new(
            FindingKind::MissingDocumentationSet,
            display_path(&self.root),
            None,
            "missing independent docs set; README should link to stable docs for users and maintainers",
            vec![FindingMetric::threshold(
                "missing_required_docs",
                missing_count.max(1),
                1,
                "documents",
            )],
        )))
    }

    fn missing_user_guide(&self) -> Option<Finding> {
        let Some(path) = &self.user_guide else {
            return Some(finding(FindingInput::new(
                FindingKind::MissingUserGuide,
                display_path(&self.root),
                None,
                "missing user guide; users need install, quick start, CLI, configuration, output, examples, and troubleshooting docs",
                vec![FindingMetric::threshold(
                    "missing_user_topics",
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

        Some(finding(FindingInput::new(
            FindingKind::MissingUserGuide,
            display_path(path),
            Some(1),
            format!(
                "user guide has shallow coverage; missing topics: {}",
                missing_topics.join(", ")
            ),
            vec![FindingMetric::threshold(
                "missing_user_topics",
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
        finding(FindingInput::new(
            kind,
            display_path(&self.root),
            None,
            message,
            vec![FindingMetric::threshold(
                "documentation_risk",
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
        if !documented_any_flag {
            return None;
        }

        let missing_flags = scan_cli_flags()
            .into_iter()
            .filter(|flag| !documented_text.contains(&flag.to_ascii_lowercase()))
            .collect::<Vec<_>>();
        if missing_flags.is_empty() {
            return None;
        }

        Some(finding(FindingInput::new(
            FindingKind::StaleCliDocumentation,
            self.user_guide
                .as_ref()
                .or(self.configuration.as_ref())
                .or(self.readme.as_ref())
                .map(|path| display_path(path))
                .unwrap_or_else(|| display_path(&self.root)),
            Some(1),
            format!(
                "CLI documentation is missing current flags: {}",
                missing_flags.join(", ")
            ),
            vec![FindingMetric::threshold(
                "missing_cli_flags",
                missing_flags.len(),
                1,
                "flags",
            )],
        )))
    }

    fn stale_schema_documentation(&self) -> Option<Finding> {
        let path = self.report_schema.as_ref()?;
        let text = self
            .contents
            .get(path)
            .map(|contents| contents.to_ascii_lowercase())
            .unwrap_or_default();
        let missing_fields = REQUIRED_SCHEMA_FIELDS
            .iter()
            .filter(|field| !text.contains(*field))
            .copied()
            .collect::<Vec<_>>();
        if missing_fields.is_empty() {
            return None;
        }

        Some(finding(FindingInput::new(
            FindingKind::StaleSchemaDocumentation,
            display_path(path),
            Some(1),
            format!(
                "report schema docs are missing current fields: {}",
                missing_fields.join(", ")
            ),
            vec![FindingMetric::threshold(
                "missing_schema_fields",
                missing_fields.len(),
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
}
