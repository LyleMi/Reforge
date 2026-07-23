//! Independent producer for Reforge's experimental Unity analysis.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use reforge_schema::{
    ANALYSIS_UNITY, AnalysisCoverage, CoverageLimitation, CoverageObservation, CoverageStatus,
    Evidence, Issue, LanguageCoverage, Location, Measurement, Producer, Report, RuleExecution,
    Subject, SuppressionSummary, Target,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const ZERO_GUID: &str = "00000000000000000000000000000000";
const LIFECYCLE_METHODS: &[&str] = &[
    "Awake",
    "OnEnable",
    "Start",
    "FixedUpdate",
    "Update",
    "LateUpdate",
    "OnDisable",
    "OnDestroy",
    "OnApplicationPause",
    "OnApplicationQuit",
    "OnValidate",
    "Reset",
];

#[derive(Debug, Clone)]
pub struct Config {
    pub max_assembly_dependencies: usize,
    pub max_scene_objects: usize,
    pub max_prefab_objects: usize,
    pub max_serialized_fields: usize,
    pub max_lifecycle_methods: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_assembly_dependencies: 12,
            max_scene_objects: 5_000,
            max_prefab_objects: 500,
            max_serialized_fields: 20,
            max_lifecycle_methods: 8,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnalyzeOptions {
    pub root: PathBuf,
    pub config: Config,
    pub reproducible: bool,
}

#[derive(Clone, Copy)]
struct RuleDef {
    name: &'static str,
    family: &'static str,
    subject: SubjectKind,
    observation: ObservationKind,
    description: &'static str,
}

#[derive(Clone, Copy)]
enum SubjectKind {
    File,
    Symbol,
    Group,
}

#[derive(Clone, Copy)]
enum ObservationKind {
    Assets,
    Assemblies,
}

const RULES: &[RuleDef] = &[
    rule(
        "assembly_cycle",
        "dependency_topology",
        SubjectKind::Group,
        ObservationKind::Assemblies,
        "Reports cycles between Unity assembly definitions.",
    ),
    rule(
        "assembly_hub",
        "dependency_topology",
        SubjectKind::File,
        ObservationKind::Assemblies,
        "Reports Unity assemblies with many direct dependencies.",
    ),
    rule(
        "unresolved_assembly_reference",
        "reference_integrity",
        SubjectKind::File,
        ObservationKind::Assemblies,
        "Reports unresolved asmdef references.",
    ),
    rule(
        "runtime_editor_dependency",
        "runtime_editor_boundary",
        SubjectKind::File,
        ObservationKind::Assemblies,
        "Reports runtime assemblies that depend on Editor-only assemblies.",
    ),
    rule(
        "duplicate_guid",
        "reference_integrity",
        SubjectKind::File,
        ObservationKind::Assets,
        "Reports duplicate Unity meta GUIDs.",
    ),
    rule(
        "missing_meta",
        "reference_integrity",
        SubjectKind::File,
        ObservationKind::Assets,
        "Reports Unity assets without meta files.",
    ),
    rule(
        "orphan_meta",
        "reference_integrity",
        SubjectKind::File,
        ObservationKind::Assets,
        "Reports Unity meta files without assets.",
    ),
    rule(
        "broken_asset_reference",
        "reference_integrity",
        SubjectKind::File,
        ObservationKind::Assets,
        "Reports unresolved asset GUID references.",
    ),
    rule(
        "missing_script",
        "reference_integrity",
        SubjectKind::File,
        ObservationKind::Assets,
        "Reports missing MonoScript references.",
    ),
    rule(
        "non_text_serialization",
        "project_configuration",
        SubjectKind::File,
        ObservationKind::Assets,
        "Reports Unity projects that do not force text serialization.",
    ),
    rule(
        "scene_build_drift",
        "project_configuration",
        SubjectKind::File,
        ObservationKind::Assets,
        "Reports scenes absent from build settings.",
    ),
    rule(
        "large_scene",
        "responsibility_decomposition",
        SubjectKind::File,
        ObservationKind::Assets,
        "Reports scenes with many serialized objects.",
    ),
    rule(
        "large_prefab",
        "responsibility_decomposition",
        SubjectKind::File,
        ObservationKind::Assets,
        "Reports prefabs with many serialized objects.",
    ),
    rule(
        "serialized_field_bloat",
        "responsibility_decomposition",
        SubjectKind::Symbol,
        ObservationKind::Assets,
        "Reports Unity behaviours with many serialized fields.",
    ),
    rule(
        "lifecycle_overload",
        "lifecycle_correctness",
        SubjectKind::Symbol,
        ObservationKind::Assets,
        "Reports Unity behaviours with many lifecycle methods.",
    ),
    rule(
        "expensive_frame_call",
        "runtime_performance",
        SubjectKind::File,
        ObservationKind::Assets,
        "Reports expensive lookups reachable from frame-loop methods.",
    ),
    rule(
        "editor_api_in_runtime",
        "runtime_editor_boundary",
        SubjectKind::File,
        ObservationKind::Assets,
        "Reports UnityEditor API usage in runtime paths.",
    ),
    rule(
        "unbalanced_event_subscription",
        "lifecycle_correctness",
        SubjectKind::Symbol,
        ObservationKind::Assets,
        "Reports event subscriptions without matching unsubscriptions.",
    ),
];

const fn rule(
    name: &'static str,
    family: &'static str,
    subject: SubjectKind,
    observation: ObservationKind,
    description: &'static str,
) -> RuleDef {
    RuleDef {
        name,
        family,
        subject,
        observation,
        description,
    }
}

pub fn rules() -> Vec<serde_json::Value> {
    RULES
        .iter()
        .map(|rule| {
            let (source, unit) = match rule.observation {
                ObservationKind::Assets => ("unity_assets", "unity_asset"),
                ObservationKind::Assemblies => ("unity_assemblies", "unity_assembly"),
            };
            serde_json::json!({
                "rule": qualified_rule(rule.name),
                "analysis": ANALYSIS_UNITY,
                "family": qualified_family(rule.family),
                "subject": match rule.subject {
                    SubjectKind::File => "file",
                    SubjectKind::Symbol => "symbol",
                    SubjectKind::Group => "group",
                },
                "observation": { "source": source, "unit": unit },
                "description": rule.description,
                "guidance": guidance(rule.family),
                "languages": ["unity"],
                "measurements": ["group.size"],
            })
        })
        .collect()
}

#[derive(Debug)]
struct Detection {
    rule: &'static str,
    path: String,
    line: usize,
    message: String,
    value: usize,
    threshold: usize,
    related: Vec<(String, String)>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Asmdef {
    name: String,
    #[serde(default)]
    references: Vec<String>,
    #[serde(default)]
    include_platforms: Vec<String>,
}

#[derive(Debug)]
struct Assembly {
    name: String,
    path: String,
    references: Vec<String>,
    editor_only: bool,
    guid: Option<String>,
}

#[derive(Default)]
struct Scan {
    paths: Vec<PathBuf>,
    detections: Vec<Detection>,
    limitations: Vec<CoverageLimitation>,
    assets: usize,
    assemblies: usize,
}

pub fn analyze(options: &AnalyzeOptions) -> Result<Report> {
    let root = options
        .root
        .canonicalize()
        .with_context(|| format!("failed to resolve analysis root {}", options.root.display()))?;
    if !root.join("Assets").is_dir() || !root.join("ProjectSettings/ProjectVersion.txt").is_file() {
        bail!(
            "{} is not a Unity project root (expected Assets and ProjectSettings)",
            root.display()
        );
    }

    let scan = scan_project(&root, &options.config)?;
    let coverage = unity_coverage(&scan);
    let issues = project_issues(scan.detections);
    Ok(Report::new(
        Producer {
            name: "reforge.unity".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            revision: option_env!("REFORGE_BUILD_REVISION").map(str::to_owned),
        },
        Target {
            root: root.to_string_lossy().into_owned(),
            workspace_identity: workspace_identity(&root),
            source_revision: git(&root, &["rev-parse", "HEAD"]),
        },
        SuppressionSummary::default(),
        coverage,
        issues,
    ))
}

fn scan_project(root: &Path, config: &Config) -> Result<Scan> {
    let mut scan = Scan::default();
    collect_files(&root.join("Assets"), &mut scan.paths)?;
    collect_files(&root.join("Packages"), &mut scan.paths)?;
    scan.assets = scan
        .paths
        .iter()
        .filter(|path| path.extension().and_then(|value| value.to_str()) != Some("meta"))
        .count();
    scan_project_settings(root, &mut scan);
    scan_meta_and_assets(root, config, &mut scan)?;
    scan_assemblies(root, config, &mut scan)?;
    scan_csharp(root, config, &mut scan)?;
    scan_build_settings(root, &mut scan);
    Ok(scan)
}

fn unity_coverage(scan: &Scan) -> BTreeMap<String, AnalysisCoverage> {
    let status = if scan.limitations.is_empty() {
        CoverageStatus::Observed
    } else {
        CoverageStatus::Partial
    };
    BTreeMap::from([(
        ANALYSIS_UNITY.into(),
        AnalysisCoverage {
            status,
            scanned_files: scan.assets,
            languages: BTreeMap::from([(
                "unity".into(),
                LanguageCoverage {
                    status,
                    files: scan.assets,
                    functions: 0,
                    limitations: scan.limitations.clone(),
                },
            )]),
            rules: RULES
                .iter()
                .map(|rule| {
                    let (name, count, unit) = match rule.observation {
                        ObservationKind::Assets => {
                            ("unity_assets_scanned", scan.assets, "unity_asset")
                        }
                        ObservationKind::Assemblies => (
                            "unity_assemblies_scanned",
                            scan.assemblies,
                            "unity_assembly",
                        ),
                    };
                    (
                        qualified_rule(rule.name),
                        RuleExecution {
                            status,
                            observations: vec![CoverageObservation {
                                name: name.into(),
                                count,
                                unit: unit.into(),
                            }],
                            limitations: scan.limitations.clone(),
                        },
                    )
                })
                .collect(),
            limitations: scan.limitations.clone(),
        },
    )])
}

include!("projection.rs");
include!("scanning.rs");
