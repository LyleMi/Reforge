use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

use crate::cli::{FindingControlArgs, ScanArgs};
#[cfg(test)]
use crate::model::Severity;
use crate::model::{Finding, FindingKind, serialized_finding_kind};

use super::config::ConfigSuppression;

const DIRECTIVE_PREFIX: &str = "reforge:";

pub(super) fn apply_finding_controls(
    findings: &mut Vec<Finding>,
    root: &Path,
    args: &ScanArgs,
    config_suppressions: &[ConfigSuppression],
) -> Result<()> {
    let filters = FindingFilters::from_args(&args.finding_controls)?;
    let suppressions = Suppressions::load(root, findings, config_suppressions)?;

    findings.retain(|finding| filters.matches(finding) && !suppressions.matches(finding));
    Ok(())
}

#[derive(Debug, Default)]
struct FindingFilters {
    only: Option<BTreeSet<FindingKind>>,
    exclude_detector: BTreeSet<FindingKind>,
    min_priority: Option<u8>,
    severity: Option<crate::cli::FindingSeverity>,
}

impl FindingFilters {
    fn from_args(args: &FindingControlArgs) -> Result<Self> {
        if let Some(priority) = args.min_priority
            && priority > 100
        {
            bail!("--min-priority must be between 0 and 100");
        }

        Ok(Self {
            only: args
                .only
                .as_deref()
                .map(parse_required_kind_list)
                .transpose()?,
            exclude_detector: args
                .exclude_detector
                .as_deref()
                .map(parse_required_kind_list)
                .transpose()?
                .unwrap_or_default(),
            min_priority: args.min_priority,
            severity: args.severity,
        })
    }

    fn matches(&self, finding: &Finding) -> bool {
        if let Some(only) = &self.only
            && !only.contains(&finding.kind)
        {
            return false;
        }
        if self.exclude_detector.contains(&finding.kind) {
            return false;
        }
        if let Some(min_priority) = self.min_priority
            && finding.priority < min_priority
        {
            return false;
        }
        if let Some(severity) = self.severity
            && !severity.matches(finding.severity)
        {
            return false;
        }
        true
    }
}

#[derive(Debug, Default)]
struct Suppressions {
    root: String,
    rules: Vec<SuppressionRule>,
}

impl Suppressions {
    fn load(
        root: &Path,
        findings: &[Finding],
        config_suppressions: &[ConfigSuppression],
    ) -> Result<Self> {
        let root = normalize_path(&display_path(root));
        let mut rules = Vec::new();

        for suppression in config_suppressions {
            let kinds = suppression
                .kind
                .as_deref()
                .map(parse_required_kind_list)
                .transpose()?;
            let reason = suppression.reason.trim();
            if reason.is_empty() {
                bail!(
                    "suppression for {} must include a non-empty reason",
                    suppression.path
                );
            }
            rules.push(SuppressionRule {
                path: normalize_path(&suppression.path),
                kinds,
                line: suppression.line,
                scope: SuppressionScope::Config,
            });
        }

        let mut inline_rules = InlineRuleLoader::new(root.clone());
        rules.extend(inline_rules.load(findings)?);

        Ok(Self { root, rules })
    }

    fn matches(&self, finding: &Finding) -> bool {
        self.rules
            .iter()
            .any(|rule| rule.matches(finding, self.root.as_str()))
    }
}

#[derive(Debug)]
struct InlineRuleLoader {
    root: String,
    by_path: BTreeMap<String, Vec<SuppressionRule>>,
}

impl InlineRuleLoader {
    fn new(root: String) -> Self {
        Self {
            root,
            by_path: BTreeMap::new(),
        }
    }

    fn load(&mut self, findings: &[Finding]) -> Result<Vec<SuppressionRule>> {
        for finding in findings {
            let path = normalize_path(&finding.path);
            if self.by_path.contains_key(&path) {
                continue;
            }

            let source_path = source_path_for_finding(&self.root, &path);
            if !source_path.is_file() {
                self.by_path.insert(path, Vec::new());
                continue;
            }

            let source = fs::read_to_string(&source_path).with_context(|| {
                format!(
                    "failed to read source file {} while loading suppressions",
                    source_path.display()
                )
            })?;
            let path_rules = parse_inline_suppressions(&path, &source)?;
            self.by_path.insert(path, path_rules);
        }

        Ok(std::mem::take(&mut self.by_path)
            .into_values()
            .flatten()
            .collect())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuppressionScope {
    SameLine,
    NextLine,
    File,
    Config,
}

#[derive(Debug)]
struct SuppressionRule {
    path: String,
    kinds: Option<BTreeSet<FindingKind>>,
    line: Option<usize>,
    scope: SuppressionScope,
}

impl SuppressionRule {
    fn matches(&self, finding: &Finding, root: &str) -> bool {
        if let Some(kinds) = &self.kinds
            && !kinds.contains(&finding.kind)
        {
            return false;
        }
        if !path_matches(self.path.as_str(), finding.path.as_str(), root) {
            return false;
        }

        match self.scope {
            SuppressionScope::File => true,
            SuppressionScope::Config => self.line.is_none() || self.line == finding.line,
            SuppressionScope::SameLine => self.line == finding.line,
            SuppressionScope::NextLine => self
                .line
                .and_then(|line| line.checked_add(1))
                .is_some_and(|line| Some(line) == finding.line),
        }
    }
}

fn parse_inline_suppressions(path: &str, source: &str) -> Result<Vec<SuppressionRule>> {
    let mut rules = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let Some(directive) = parse_directive(line)? else {
            continue;
        };
        let line_number = index + 1;
        rules.push(SuppressionRule {
            path: path.to_string(),
            kinds: directive.kinds,
            line: Some(line_number),
            scope: directive.scope,
        });
    }
    Ok(rules)
}

#[derive(Debug)]
struct InlineDirective {
    scope: SuppressionScope,
    kinds: Option<BTreeSet<FindingKind>>,
}

fn parse_directive(line: &str) -> Result<Option<InlineDirective>> {
    let Some(start) = line.find(DIRECTIVE_PREFIX) else {
        return Ok(None);
    };
    let after_prefix = &line[start + DIRECTIVE_PREFIX.len()..];

    for (name, scope) in [
        ("ignore-next-line", SuppressionScope::NextLine),
        ("ignore-file", SuppressionScope::File),
        ("ignore", SuppressionScope::SameLine),
    ] {
        if let Some(body) = directive_body(after_prefix, name) {
            return Ok(Some(InlineDirective {
                scope,
                kinds: parse_optional_kind_list(body)?,
            }));
        }
    }

    Ok(None)
}

fn directive_body<'a>(text: &'a str, directive: &str) -> Option<&'a str> {
    let body = text.strip_prefix(directive)?;
    if body.is_empty() || body.starts_with(char::is_whitespace) {
        Some(body.trim())
    } else {
        None
    }
}

fn parse_optional_kind_list(text: &str) -> Result<Option<BTreeSet<FindingKind>>> {
    let Some(first_token) = text.split_whitespace().next() else {
        return Ok(None);
    };

    match parse_kind_list(first_token) {
        Ok(kinds) => Ok(Some(kinds)),
        Err(error) if looks_like_kind_list(first_token) => Err(error),
        Err(_) => Ok(None),
    }
}

fn parse_required_kind_list(text: &str) -> Result<BTreeSet<FindingKind>> {
    parse_kind_list(text.trim())
}

fn parse_kind_list(text: &str) -> Result<BTreeSet<FindingKind>> {
    let mut kinds = BTreeSet::new();
    for raw_kind in text.split(',') {
        let kind = raw_kind.trim();
        if kind.is_empty() {
            bail!("empty finding kind in list '{text}'");
        }
        kinds.insert(parse_finding_kind(kind)?);
    }
    if kinds.is_empty() {
        bail!("finding kind list cannot be empty");
    }
    Ok(kinds)
}

fn parse_finding_kind(kind: &str) -> Result<FindingKind> {
    serde_json::from_value::<FindingKind>(serde_json::Value::String(kind.to_string())).map_err(
        |_| {
            anyhow!(
                "unknown finding kind '{kind}'; expected one of: {}",
                known_finding_kinds().join(", ")
            )
        },
    )
}

fn looks_like_kind_list(text: &str) -> bool {
    text.contains(',') || text.contains('_')
}

fn path_matches(rule_path: &str, finding_path: &str, root: &str) -> bool {
    let rule_path = normalize_path(rule_path);
    let finding_path = normalize_path(finding_path);
    let finding_relative = relative_to_root(finding_path.as_str(), root);

    rule_path == finding_path || rule_path == finding_relative
}

fn source_path_for_finding(root: &str, finding_path: &str) -> PathBuf {
    let relative = relative_to_root(finding_path, root);
    if relative == finding_path {
        PathBuf::from(finding_path)
    } else {
        Path::new(root).join(relative)
    }
}

fn relative_to_root<'a>(path: &'a str, root: &str) -> &'a str {
    path.strip_prefix(root)
        .and_then(|suffix| suffix.strip_prefix('/'))
        .unwrap_or(path)
}

fn normalize_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .strip_prefix("//?/")
        .unwrap_or(normalized.as_str())
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_string()
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn known_finding_kinds() -> Vec<String> {
    [
        FindingKind::LargeFile,
        FindingKind::LargeDirectory,
        FindingKind::DebtMarker,
        FindingKind::SimilarFunctions,
        FindingKind::LongFunction,
        FindingKind::ComplexFunction,
        FindingKind::DeepNesting,
        FindingKind::ManyParameters,
        FindingKind::ReadabilityRisk,
        FindingKind::LargeType,
        FindingKind::LargePublicSurface,
        FindingKind::ImportHeavyFile,
        FindingKind::FunctionProliferation,
        FindingKind::UnusedFunction,
        FindingKind::RepeatedLiteral,
        FindingKind::RepeatedErrorPattern,
        FindingKind::TestDuplication,
        FindingKind::HappyPathOnlyTests,
        FindingKind::FileNamingDrift,
        FindingKind::DirectoryDrift,
        FindingKind::DataClump,
        FindingKind::ParallelImplementation,
        FindingKind::ShadowedAbstraction,
        FindingKind::DuplicateTypeShape,
        FindingKind::ConfigKeyDrift,
        FindingKind::FixtureFactoryDrift,
        FindingKind::GenericBucketDrift,
        FindingKind::AdapterBoundaryBypass,
        FindingKind::StaleCompatibilityPath,
        FindingKind::MissingDocumentationSet,
        FindingKind::MissingUserGuide,
        FindingKind::MissingReportSchemaDocs,
        FindingKind::MissingMetricsModelDocs,
        FindingKind::MissingArchitectureDocs,
        FindingKind::StaleCliDocumentation,
        FindingKind::StaleSchemaDocumentation,
        FindingKind::DependencyCycle,
        FindingKind::DependencyHub,
    ]
    .into_iter()
    .map(serialized_finding_kind)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::cli::{ChurnMode, ScanArgs};
    use crate::scan::scan_path;
    use crate::scoring::{FindingInput, finding};

    fn test_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("reforge-finding-control-{name}-{suffix}"))
    }

    fn scan_args(path: PathBuf) -> ScanArgs {
        let mut args = ScanArgs::defaults_for_path(path);
        args.churn = Some(ChurnMode::Off);
        args.output = Some(crate::cli::OutputFormat::Human);
        args
    }

    #[test]
    fn inline_suppressions_remove_matching_findings() -> Result<()> {
        let root = test_root("inline-suppressions");
        fs::create_dir_all(root.join("src"))?;
        fs::write(
            root.join("src/main.rs"),
            "\
// TODO: ignored same line reforge:ignore debt_marker accepted debt
// reforge:ignore-next-line debt_marker accepted generated marker
// TODO: ignored next line
// TODO: reported
",
        )?;
        fs::write(
            root.join("src/generated.rs"),
            "\
// reforge:ignore-file debt_marker generated fixture
// TODO: ignored for whole file
",
        )?;

        let findings = scan_path(&scan_args(root.clone()))?;

        fs::remove_dir_all(root)?;

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, FindingKind::DebtMarker);
        assert_eq!(findings[0].line, Some(4));
        assert!(findings[0].path.ends_with("src/main.rs"));
        Ok(())
    }

    #[test]
    fn config_suppressions_remove_matching_findings() -> Result<()> {
        let root = test_root("config-suppressions");
        fs::create_dir_all(root.join("src"))?;
        fs::write(
            root.join("reforge.toml"),
            "\
[[suppressions]]
kind = \"debt_marker\"
path = \"src/generated.rs\"
line = 1
reason = \"generated TODO marker\"
",
        )?;
        fs::write(
            root.join("src/generated.rs"),
            "// TODO: ignored\n// TODO: reported\n",
        )?;

        let findings = scan_path(&scan_args(root.clone()))?;

        fs::remove_dir_all(root)?;

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, Some(2));
        assert!(findings[0].path.ends_with("src/generated.rs"));
        Ok(())
    }

    #[test]
    fn config_suppression_without_kind_or_line_suppresses_whole_path() -> Result<()> {
        let root = test_root("config-path-suppression");
        fs::create_dir_all(root.join("src"))?;
        fs::write(
            root.join("reforge.toml"),
            "\
[[suppressions]]
path = \"src/generated.rs\"
reason = \"generated fixture\"
",
        )?;
        fs::write(
            root.join("src/generated.rs"),
            "// TODO: ignored\n// TODO: ignored\n",
        )?;
        fs::write(root.join("src/main.rs"), "// TODO: reported\n")?;

        let findings = scan_path(&scan_args(root.clone()))?;

        fs::remove_dir_all(root)?;

        assert_eq!(findings.len(), 1);
        assert!(findings[0].path.ends_with("src/main.rs"));
        Ok(())
    }

    #[test]
    fn unknown_config_suppression_kind_errors() -> Result<()> {
        let root = test_root("unknown-config-suppression-kind");
        fs::create_dir_all(root.join("src"))?;
        fs::write(
            root.join("reforge.toml"),
            "\
[[suppressions]]
kind = \"not_a_detector\"
path = \"src/main.rs\"
reason = \"typo\"
",
        )?;
        fs::write(root.join("src/main.rs"), "// TODO: reported\n")?;

        let result = scan_path(&scan_args(root.clone()));

        fs::remove_dir_all(root)?;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unknown finding kind 'not_a_detector'")
        );
        Ok(())
    }

    #[test]
    fn only_filter_keeps_selected_finding_kinds() -> Result<()> {
        let root = test_root("only-filter");
        fs::create_dir_all(root.join("src"))?;
        fs::write(
            root.join("src/main.rs"),
            "// TODO: reported\nlet value = 1;\n",
        )?;

        let mut args = scan_args(root.clone());
        args.max_file_lines = 1;
        args.finding_controls.only = Some("debt_marker".to_string());
        let findings = scan_path(&args)?;

        fs::remove_dir_all(root)?;

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, FindingKind::DebtMarker);
        Ok(())
    }

    #[test]
    fn exclude_detector_filter_removes_selected_finding_kinds() -> Result<()> {
        let root = test_root("exclude-detector-filter");
        fs::create_dir_all(root.join("src"))?;
        fs::write(
            root.join("src/main.rs"),
            "// TODO: reported\nlet value = 1;\n",
        )?;

        let mut args = scan_args(root.clone());
        args.max_file_lines = 1;
        args.finding_controls.exclude_detector = Some("debt_marker".to_string());
        let findings = scan_path(&args)?;

        fs::remove_dir_all(root)?;

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, FindingKind::LargeFile);
        Ok(())
    }

    #[test]
    fn severity_filter_keeps_threshold_and_above() -> Result<()> {
        let mut findings = vec![
            test_finding(FindingKind::DebtMarker, Severity::Info, 10),
            test_finding(FindingKind::LargeFile, Severity::Warning, 40),
            test_finding(FindingKind::ComplexFunction, Severity::Critical, 80),
        ];
        let args = FindingControlArgs {
            only: None,
            exclude_detector: None,
            min_priority: None,
            severity: Some(crate::cli::FindingSeverity::Warning),
        };
        let filters = FindingFilters::from_args(&args)?;

        findings.retain(|finding| filters.matches(finding));

        assert_eq!(findings.len(), 2);
        assert!(
            findings
                .iter()
                .all(|finding| finding.severity >= Severity::Warning)
        );
        Ok(())
    }

    #[test]
    fn min_priority_filter_keeps_threshold_and_above() -> Result<()> {
        let mut findings = vec![
            test_finding(FindingKind::DebtMarker, Severity::Info, 10),
            test_finding(FindingKind::LargeFile, Severity::Warning, 40),
        ];
        let args = FindingControlArgs {
            only: None,
            exclude_detector: None,
            min_priority: Some(35),
            severity: None,
        };
        let filters = FindingFilters::from_args(&args)?;

        findings.retain(|finding| filters.matches(finding));

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, FindingKind::LargeFile);
        Ok(())
    }

    fn test_finding(kind: FindingKind, severity: Severity, priority: u8) -> Finding {
        let mut finding = finding(FindingInput::new(
            kind,
            "src/main.rs",
            Some(1),
            "test finding",
            Vec::new(),
        ));
        finding.severity = severity;
        finding.priority = priority;
        finding
    }
}
