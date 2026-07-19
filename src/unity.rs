use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::cli::{ScanArgs, UnityMode};
use crate::model::{
    Finding, FindingKind, FindingMetric, MetricId, RelatedLocation, UnityAssemblyEdge,
    UnityAssemblyNode, UnityCoverageEntry, UnityMetricManifestEntry, UnityProjectReport,
    UnityProjectStatus, UnityRawMetric, UnityReferenceProblem,
};
use crate::scoring::FindingInput;

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

#[derive(Debug)]
pub(crate) struct UnityScan {
    pub report: UnityProjectReport,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AsmdefFile {
    name: String,
    #[serde(default)]
    references: Vec<String>,
    #[serde(default)]
    include_platforms: Vec<String>,
    #[serde(default)]
    optional_unity_references: Vec<String>,
}

#[derive(Debug)]
struct AssemblyRecord {
    node: UnityAssemblyNode,
    references: Vec<String>,
    guid: Option<String>,
}

pub(crate) fn scan_unity(root: &Path, args: &ScanArgs) -> Result<UnityScan> {
    let detected = root.join("ProjectSettings/ProjectVersion.txt").is_file();
    if args.unity == UnityMode::On && !detected {
        bail!(
            "Unity analysis was requested, but {} is not a Unity project root (missing ProjectSettings/ProjectVersion.txt)",
            root.display()
        );
    }
    if args.unity == UnityMode::Off {
        return Ok(UnityScan {
            report: UnityProjectReport {
                status: if detected {
                    UnityProjectStatus::Disabled
                } else {
                    UnityProjectStatus::NotDetected
                },
                ..Default::default()
            },
            findings: Vec::new(),
        });
    }
    if !detected {
        return Ok(UnityScan {
            report: UnityProjectReport::default(),
            findings: Vec::new(),
        });
    }

    let mut context = UnityContext::new(root, args);
    context.scan()?;
    Ok(UnityScan {
        report: context.report,
        findings: context.findings,
    })
}

struct UnityContext<'a> {
    root: &'a Path,
    args: &'a ScanArgs,
    report: UnityProjectReport,
    findings: Vec<Finding>,
    paths: Vec<PathBuf>,
    external_paths: Vec<PathBuf>,
    guid_paths: BTreeMap<String, Vec<String>>,
    assemblies: Vec<AssemblyRecord>,
    package_cache_present: bool,
}

impl<'a> UnityContext<'a> {
    fn new(root: &'a Path, args: &'a ScanArgs) -> Self {
        Self {
            root,
            args,
            report: UnityProjectReport {
                status: UnityProjectStatus::Observed,
                metric_manifest: unity_metric_manifest(),
                ..Default::default()
            },
            findings: Vec::new(),
            paths: Vec::new(),
            external_paths: Vec::new(),
            guid_paths: BTreeMap::new(),
            assemblies: Vec::new(),
            package_cache_present: false,
        }
    }

    fn scan(&mut self) -> Result<()> {
        self.read_project_settings()?;
        let roots = self.analysis_roots()?;
        for root in &roots {
            collect_files(root, &mut self.paths)?;
            self.report
                .analysis_roots
                .push(display_path(self.root, root));
        }
        let package_cache = self.root.join("Library/PackageCache");
        self.package_cache_present = package_cache.is_dir();
        if self.package_cache_present {
            collect_files(&package_cache, &mut self.external_paths)?;
        } else {
            self.degrade("Library/PackageCache is unavailable; external package GUID and assembly references were not verified");
        }
        self.index_meta(false)?;
        self.index_meta(true)?;
        self.scan_meta_integrity();
        self.scan_assets()?;
        self.scan_assemblies()?;
        self.scan_csharp()?;
        self.scan_build_settings()?;
        self.report.stats.guids = self.guid_paths.len();
        self.report.stats.assemblies = self.report.assemblies.len();
        self.report.coverage = self.coverage();
        Ok(())
    }

    fn read_project_settings(&mut self) -> Result<()> {
        let version = fs::read_to_string(self.root.join("ProjectSettings/ProjectVersion.txt"))
            .context("failed to read Unity ProjectVersion.txt")?;
        self.report.editor_version = version.lines().find_map(|line| {
            line.trim()
                .strip_prefix("m_EditorVersion:")
                .map(str::trim)
                .map(str::to_string)
        });
        let editor_settings =
            fs::read_to_string(self.root.join("ProjectSettings/EditorSettings.asset"))
                .unwrap_or_default();
        let mode = editor_settings.lines().find_map(|line| {
            line.trim()
                .strip_prefix("m_SerializationMode:")
                .map(str::trim)
        });
        self.report.serialization_mode = Some(
            match mode {
                Some("2") => "force_text",
                Some("1") => "force_binary",
                Some("0") => "mixed",
                _ => "unknown",
            }
            .to_string(),
        );
        if mode != Some("2") {
            self.degrade("Unity asset serialization is not Force Text; binary assets cannot be reference-checked");
            self.push_finding(
                FindingKind::UnityNonTextSerialization,
                "ProjectSettings/EditorSettings.asset",
                1,
                "Unity project is not configured for Force Text serialization",
                1,
                1,
            );
        }
        Ok(())
    }

    fn analysis_roots(&self) -> Result<Vec<PathBuf>> {
        let mut roots = Vec::new();
        let assets = self.root.join("Assets");
        if assets.is_dir() {
            roots.push(assets);
        }
        let packages = self.root.join("Packages");
        if packages.is_dir() {
            for entry in fs::read_dir(&packages)? {
                let path = entry?.path();
                if path.is_dir() {
                    roots.push(path);
                }
            }
        }
        let manifest = packages.join("manifest.json");
        if let Ok(contents) = fs::read_to_string(manifest)
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents)
            && let Some(dependencies) = value
                .get("dependencies")
                .and_then(|value| value.as_object())
        {
            for value in dependencies.values().filter_map(|value| value.as_str()) {
                if let Some(relative) = value.strip_prefix("file:") {
                    let path = packages.join(relative);
                    let path = path.canonicalize().unwrap_or(path);
                    if path.is_dir() && !roots.contains(&path) {
                        roots.push(path);
                    }
                }
            }
        }
        Ok(roots)
    }

    fn index_meta(&mut self, external: bool) -> Result<()> {
        let paths = if external {
            &self.external_paths
        } else {
            &self.paths
        };
        for path in paths.iter().filter(|path| extension(path) == Some("meta")) {
            if !external {
                self.report.stats.meta_files += 1;
            }
            let Ok(contents) = fs::read_to_string(path) else {
                continue;
            };
            let Some(guid) = meta_guid(&contents) else {
                continue;
            };
            let display = display_path(self.root, path);
            self.guid_paths.entry(guid).or_default().push(display);
        }
        Ok(())
    }

    fn scan_meta_integrity(&mut self) {
        let duplicates = self
            .guid_paths
            .values()
            .filter(|paths| paths.len() > 1)
            .cloned()
            .collect::<Vec<_>>();
        for paths in duplicates {
            let local = paths
                .iter()
                .filter(|path| !path.starts_with("Library/PackageCache/"))
                .cloned()
                .collect::<Vec<_>>();
            if local.len() > 1 {
                let related = local
                    .iter()
                    .map(|path| RelatedLocation {
                        path: path.clone(),
                        line: 2,
                        name: Some("duplicate GUID".into()),
                    })
                    .collect();
                self.findings.push(unity_finding(
                    UnityFindingInput::new(
                        FindingKind::UnityDuplicateGuid,
                        local[0].clone(),
                        2,
                        format!("Unity GUID is declared by {} local meta files", local.len()),
                        local.len(),
                        1,
                    )
                    .with_related(related),
                ));
            }
        }
        let local_paths = self.paths.iter().cloned().collect::<BTreeSet<_>>();
        for path in self.paths.clone() {
            if extension(&path) == Some("meta") {
                let target = PathBuf::from(path.to_string_lossy().trim_end_matches(".meta"));
                if !target.exists() {
                    self.push_finding(
                        FindingKind::UnityOrphanMeta,
                        &display_path(self.root, &path),
                        1,
                        "Unity meta file has no matching asset",
                        1,
                        1,
                    );
                }
                continue;
            }
            if path == self.root.join("Assets")
                || path
                    .file_name()
                    .is_some_and(|name| name == "manifest.json" || name == "packages-lock.json")
            {
                continue;
            }
            let meta = PathBuf::from(format!("{}.meta", path.display()));
            if !meta.exists() && local_paths.contains(&path) {
                self.push_finding(
                    FindingKind::UnityMissingMeta,
                    &display_path(self.root, &path),
                    1,
                    "Unity asset has no matching meta file",
                    1,
                    1,
                );
            }
        }
    }

    fn scan_assets(&mut self) -> Result<()> {
        let local_guids = self.guid_paths.keys().cloned().collect::<BTreeSet<_>>();
        for path in self.paths.clone() {
            if extension(&path) == Some("meta")
                || extension(&path) == Some("asmdef")
                || extension(&path) == Some("cs")
                || !path.is_file()
            {
                continue;
            }
            self.report.stats.assets += 1;
            let bytes = fs::read(&path)?;
            let Ok(text) = std::str::from_utf8(&bytes) else {
                self.report.stats.binary_assets += 1;
                self.degrade("one or more Unity assets are binary or not UTF-8 and were counted without reference analysis");
                continue;
            };
            if !text.starts_with("%YAML") && !text.contains("--- !u!") {
                continue;
            }
            self.report.stats.yaml_assets += 1;
            let objects = text
                .lines()
                .filter(|line| line.starts_with("--- !u!"))
                .count();
            match extension(&path) {
                Some("unity") => {
                    self.report.stats.scenes += 1;
                    if objects > self.args.max_unity_scene_objects {
                        self.push_finding(
                            FindingKind::UnityLargeScene,
                            &display_path(self.root, &path),
                            1,
                            &format!("Unity scene contains {objects} serialized objects"),
                            objects,
                            self.args.max_unity_scene_objects,
                        );
                    }
                }
                Some("prefab") => {
                    self.report.stats.prefabs += 1;
                    if objects > self.args.max_unity_prefab_objects {
                        self.push_finding(
                            FindingKind::UnityLargePrefab,
                            &display_path(self.root, &path),
                            1,
                            &format!("Unity prefab contains {objects} serialized objects"),
                            objects,
                            self.args.max_unity_prefab_objects,
                        );
                    }
                }
                _ => {}
            }
            if matches!(extension(&path), Some("unity" | "prefab")) {
                self.report.raw_metrics.push(UnityRawMetric {
                    name: "unity.asset.objects".into(),
                    path: display_path(self.root, &path),
                    value: objects,
                    unit: "objects".into(),
                });
            }
            for (line_index, line) in text.lines().enumerate() {
                for guid in guids_in_line(line) {
                    self.report.stats.asset_references += 1;
                    if guid == ZERO_GUID || local_guids.contains(&guid) {
                        continue;
                    }
                    if !self.package_cache_present {
                        continue;
                    }
                    let is_script = line.contains("m_Script:");
                    let problem = UnityReferenceProblem {
                        source_path: display_path(self.root, &path),
                        line: line_index + 1,
                        guid: guid.clone(),
                        file_id: file_id_in_line(line),
                        category: if is_script { "script" } else { "asset" }.into(),
                        resolved_target: None,
                    };
                    self.report.problem_references.push(problem);
                    let kind = if is_script {
                        FindingKind::UnityMissingScript
                    } else {
                        FindingKind::UnityBrokenAssetReference
                    };
                    self.push_finding(
                        kind,
                        &display_path(self.root, &path),
                        line_index + 1,
                        if is_script {
                            "Unity asset references a missing MonoScript"
                        } else {
                            "Unity asset contains an unresolved GUID reference"
                        },
                        1,
                        1,
                    );
                }
            }
        }
        Ok(())
    }

    fn scan_assemblies(&mut self) -> Result<()> {
        for path in self
            .paths
            .iter()
            .chain(self.external_paths.iter())
            .filter(|path| extension(path) == Some("asmdef"))
        {
            let Ok(contents) = fs::read_to_string(path) else {
                continue;
            };
            let Ok(asmdef) = serde_json::from_str::<AsmdefFile>(&contents) else {
                continue;
            };
            if asmdef.name.is_empty() {
                continue;
            }
            let display = display_path(self.root, path);
            let editor_only = asmdef
                .include_platforms
                .iter()
                .any(|platform| platform.eq_ignore_ascii_case("editor"))
                || display
                    .split('/')
                    .any(|part| part.eq_ignore_ascii_case("editor"));
            let test_assembly = asmdef
                .optional_unity_references
                .iter()
                .any(|reference| reference == "TestAssemblies")
                || display.contains("/Tests/");
            if test_assembly {
                self.report.stats.tests += 1;
            }
            let meta = PathBuf::from(format!("{}.meta", path.display()));
            let guid = fs::read_to_string(meta)
                .ok()
                .and_then(|contents| meta_guid(&contents));
            self.assemblies.push(AssemblyRecord {
                node: UnityAssemblyNode {
                    name: asmdef.name,
                    path: display,
                    editor_only,
                    test_assembly,
                    predefined: false,
                },
                references: asmdef.references,
                guid,
            });
        }
        let name_index = self
            .assemblies
            .iter()
            .map(|assembly| (assembly.node.name.clone(), assembly.node.name.clone()))
            .collect::<BTreeMap<_, _>>();
        let guid_index = self
            .assemblies
            .iter()
            .filter_map(|assembly| {
                assembly
                    .guid
                    .as_ref()
                    .map(|guid| (guid.clone(), assembly.node.name.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        let mut edges = Vec::new();
        for assembly in &self.assemblies {
            for reference in &assembly.references {
                let target = reference
                    .strip_prefix("GUID:")
                    .and_then(|guid| guid_index.get(&guid.to_ascii_lowercase()))
                    .or_else(|| name_index.get(reference));
                let predefined = is_predefined_assembly(reference);
                edges.push(UnityAssemblyEdge {
                    from: assembly.node.name.clone(),
                    to: target.cloned().unwrap_or_else(|| reference.clone()),
                    reference: reference.clone(),
                    resolved: target.is_some() || predefined,
                });
            }
        }
        let local_assembly_names = self
            .assemblies
            .iter()
            .filter(|assembly| !assembly.node.path.starts_with("Library/PackageCache/"))
            .map(|assembly| assembly.node.name.clone())
            .collect::<BTreeSet<_>>();
        self.report.assembly_edges = edges
            .iter()
            .filter(|edge| local_assembly_names.contains(&edge.from))
            .cloned()
            .collect();
        self.report.assemblies = self
            .assemblies
            .iter()
            .filter(|assembly| !assembly.node.path.starts_with("Library/PackageCache/"))
            .map(|assembly| assembly.node.clone())
            .collect();
        let assembly_nodes = self
            .assemblies
            .iter()
            .map(|assembly| assembly.node.clone())
            .collect::<Vec<_>>();
        for assembly in &assembly_nodes {
            if assembly.path.starts_with("Library/PackageCache/") {
                continue;
            }
            let outgoing = edges
                .iter()
                .filter(|edge| edge.from == assembly.name)
                .collect::<Vec<_>>();
            if outgoing.len() > self.args.max_unity_assembly_dependencies {
                self.push_finding(
                    FindingKind::UnityAssemblyHub,
                    &assembly.path,
                    1,
                    &format!(
                        "Unity assembly {} has {} direct dependencies",
                        assembly.name,
                        outgoing.len()
                    ),
                    outgoing.len(),
                    self.args.max_unity_assembly_dependencies,
                );
            }
            self.report.raw_metrics.push(UnityRawMetric {
                name: "unity.assembly.dependencies".into(),
                path: assembly.path.clone(),
                value: outgoing.len(),
                unit: "assemblies".into(),
            });
            for edge in outgoing {
                if !edge.resolved && self.package_cache_present {
                    self.push_finding(
                        FindingKind::UnityUnresolvedAssemblyReference,
                        &assembly.path,
                        1,
                        &format!(
                            "Unity assembly reference '{}' could not be resolved",
                            edge.reference
                        ),
                        1,
                        1,
                    );
                }
                let editor_target = edge.to.starts_with("UnityEditor")
                    || assembly_nodes
                        .iter()
                        .find(|candidate| candidate.name == edge.to)
                        .is_some_and(|target| target.editor_only);
                if !assembly.editor_only && editor_target {
                    self.push_finding(
                        FindingKind::UnityRuntimeEditorDependency,
                        &assembly.path,
                        1,
                        &format!(
                            "runtime Unity assembly {} depends on Editor-only assembly {}",
                            assembly.name, edge.to
                        ),
                        1,
                        1,
                    );
                }
            }
        }
        let report_edges = self.report.assembly_edges.clone();
        self.find_assembly_cycles(&report_edges);
        Ok(())
    }

    fn find_assembly_cycles(&mut self, edges: &[UnityAssemblyEdge]) {
        let adjacency = edges.iter().filter(|edge| edge.resolved).fold(
            BTreeMap::<String, Vec<String>>::new(),
            |mut map, edge| {
                map.entry(edge.from.clone())
                    .or_default()
                    .push(edge.to.clone());
                map
            },
        );
        let mut emitted = BTreeSet::new();
        for start in adjacency.keys() {
            let mut stack = vec![(start.clone(), vec![start.clone()])];
            while let Some((node, path)) = stack.pop() {
                for next in adjacency.get(&node).into_iter().flatten() {
                    if next == start && path.len() > 1 {
                        let mut members = path.clone();
                        members.sort();
                        members.dedup();
                        let key = members.join("|");
                        if emitted.insert(key) {
                            let related = members
                                .iter()
                                .map(|name| RelatedLocation {
                                    path: self
                                        .assemblies
                                        .iter()
                                        .find(|assembly| &assembly.node.name == name)
                                        .map(|assembly| assembly.node.path.clone())
                                        .unwrap_or_else(|| name.clone()),
                                    line: 1,
                                    name: Some(name.clone()),
                                })
                                .collect();
                            let primary = self
                                .assemblies
                                .iter()
                                .find(|assembly| assembly.node.name == members[0])
                                .map(|assembly| assembly.node.path.clone())
                                .unwrap_or_else(|| members[0].clone());
                            self.findings.push(unity_finding(
                                UnityFindingInput::new(
                                    FindingKind::UnityAssemblyCycle,
                                    primary,
                                    1,
                                    format!(
                                        "Unity assembly cycle spans {} assemblies",
                                        members.len()
                                    ),
                                    members.len(),
                                    2,
                                )
                                .with_related(related),
                            ));
                        }
                    } else if !path.contains(next) && path.len() <= adjacency.len() {
                        let mut extended = path.clone();
                        extended.push(next.clone());
                        stack.push((next.clone(), extended));
                    }
                }
            }
        }
    }

    fn scan_csharp(&mut self) -> Result<()> {
        let files = self
            .paths
            .iter()
            .filter(|path| extension(path) == Some("cs"))
            .cloned()
            .collect::<Vec<_>>();
        let mut base_by_type = BTreeMap::new();
        let mut records = Vec::new();
        for path in &files {
            let source = fs::read_to_string(path).unwrap_or_default();
            let declarations = class_declarations(&source);
            for (name, base) in &declarations {
                base_by_type.insert(name.clone(), base.clone());
            }
            records.push((
                path.clone(),
                source,
                declarations
                    .into_iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>(),
            ));
        }
        let unity_types = base_by_type
            .keys()
            .filter(|name| inherits_unity(name, &base_by_type, &mut BTreeSet::new()))
            .cloned()
            .collect::<BTreeSet<_>>();
        for (path, source, declared_types) in records {
            let display = display_path(self.root, &path);
            if !display
                .split('/')
                .any(|part| part.eq_ignore_ascii_case("editor"))
            {
                scan_editor_api(&source, &display, &mut self.findings);
            }
            if source.contains("[UnityTest]") || source.contains("[Test]") {
                self.report.stats.tests += 1;
            }
            let unity_type_names = declared_types
                .into_iter()
                .filter(|name| unity_types.contains(name))
                .collect::<Vec<_>>();
            if unity_type_names.is_empty() {
                continue;
            }
            let type_label = unity_type_names.join(", ");
            let serialized_fields = serialized_field_count(&source);
            let lifecycle = lifecycle_method_count(&source);
            self.report.raw_metrics.push(UnityRawMetric {
                name: "unity.type.serialized_fields".into(),
                path: display.clone(),
                value: serialized_fields,
                unit: "fields".into(),
            });
            self.report.raw_metrics.push(UnityRawMetric {
                name: "unity.type.lifecycle_methods".into(),
                path: display.clone(),
                value: lifecycle,
                unit: "methods".into(),
            });
            if serialized_fields > self.args.max_unity_serialized_fields {
                self.push_finding(
                    FindingKind::UnitySerializedFieldBloat,
                    &display,
                    1,
                    &format!(
                        "Unity file containing {type_label} has {serialized_fields} serialized fields"
                    ),
                    serialized_fields,
                    self.args.max_unity_serialized_fields,
                );
            }
            if lifecycle > self.args.max_unity_lifecycle_methods {
                self.push_finding(
                    FindingKind::UnityLifecycleOverload,
                    &display,
                    1,
                    &format!(
                        "Unity file containing {type_label} implements {lifecycle} lifecycle methods"
                    ),
                    lifecycle,
                    self.args.max_unity_lifecycle_methods,
                );
            }
            scan_frame_calls(&source, &display, &mut self.findings);
            scan_event_balance(&source, &display, &mut self.findings);
        }
        Ok(())
    }

    fn scan_build_settings(&mut self) -> Result<()> {
        let build = fs::read_to_string(self.root.join("ProjectSettings/EditorBuildSettings.asset"))
            .unwrap_or_default();
        let mut included = BTreeSet::new();
        let mut enabled = None;
        for line in build.lines() {
            let trimmed = line.trim().trim_start_matches("- ");
            if let Some(value) = trimmed.strip_prefix("enabled:") {
                enabled = Some(value.trim() == "1");
            } else if let Some(path) = trimmed.strip_prefix("path:") {
                if enabled.unwrap_or(true) {
                    included.insert(path.trim().to_string());
                }
                enabled = None;
            }
        }
        let scenes = self
            .paths
            .iter()
            .filter(|path| extension(path) == Some("unity"))
            .map(|path| display_path(self.root, path))
            .filter(|path| path.starts_with("Assets/"))
            .collect::<Vec<_>>();
        for scene in scenes {
            if !included.contains(&scene) {
                self.push_finding(
                    FindingKind::UnitySceneBuildDrift,
                    &scene,
                    1,
                    "Unity scene is not listed in EditorBuildSettings",
                    1,
                    1,
                );
            }
        }
        Ok(())
    }

    fn coverage(&self) -> Vec<UnityCoverageEntry> {
        vec![
            UnityCoverageEntry {
                area: "assemblies".into(),
                status: if self.package_cache_present {
                    UnityProjectStatus::Observed
                } else {
                    UnityProjectStatus::PartiallyObserved
                },
                reason: (!self.package_cache_present)
                    .then(|| "external package assemblies unavailable".into()),
            },
            UnityCoverageEntry {
                area: "asset_references".into(),
                status: self.report.status,
                reason: self.report.degraded_reasons.first().cloned(),
            },
            UnityCoverageEntry {
                area: "csharp_semantics".into(),
                status: UnityProjectStatus::Observed,
                reason: None,
            },
        ]
    }

    fn degrade(&mut self, reason: &str) {
        self.report.status = UnityProjectStatus::PartiallyObserved;
        if !self
            .report
            .degraded_reasons
            .iter()
            .any(|value| value == reason)
        {
            self.report.degraded_reasons.push(reason.into());
        }
    }

    fn push_finding(
        &mut self,
        kind: FindingKind,
        path: &str,
        line: usize,
        message: &str,
        value: usize,
        threshold: usize,
    ) {
        self.findings.push(unity_finding(UnityFindingInput::new(
            kind,
            path.to_string(),
            line,
            message.to_string(),
            value,
            threshold,
        )));
    }
}

struct UnityFindingInput {
    kind: FindingKind,
    path: String,
    line: usize,
    message: String,
    value: usize,
    threshold: usize,
    related: Vec<RelatedLocation>,
    reliability: f64,
}

impl UnityFindingInput {
    fn new(
        kind: FindingKind,
        path: String,
        line: usize,
        message: String,
        value: usize,
        threshold: usize,
    ) -> Self {
        Self {
            kind,
            path,
            line,
            message,
            value,
            threshold,
            related: Vec::new(),
            reliability: 1.0,
        }
    }
    fn with_related(mut self, related: Vec<RelatedLocation>) -> Self {
        self.related = related;
        self
    }
    fn with_reliability(mut self, reliability: f64) -> Self {
        self.reliability = reliability;
        self
    }
}

fn unity_finding(input: UnityFindingInput) -> Finding {
    Finding::from(
        FindingInput::new(
            input.kind,
            input.path,
            Some(input.line),
            input.message,
            vec![FindingMetric::threshold(
                MetricId::GroupSize,
                input.value,
                input.threshold,
                "items",
            )],
        )
        .with_related_locations(input.related)
        .with_detection_reliability(input.reliability),
    )
}

fn collect_files(root: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory)
            .with_context(|| format!("failed to read Unity directory {}", directory.display()))?
        {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                output.push(path);
            }
        }
    }
    Ok(())
}

fn unity_metric_manifest() -> Vec<UnityMetricManifestEntry> {
    vec![
        UnityMetricManifestEntry {
            name: "unity.assembly.dependencies".into(),
            entity: "assembly".into(),
            unit: "assemblies".into(),
            description: "Resolved and declared direct asmdef dependencies.".into(),
        },
        UnityMetricManifestEntry {
            name: "unity.asset.objects".into(),
            entity: "scene_or_prefab".into(),
            unit: "objects".into(),
            description: "Serialized Unity YAML object records.".into(),
        },
        UnityMetricManifestEntry {
            name: "unity.type.serialized_fields".into(),
            entity: "type".into(),
            unit: "fields".into(),
            description: "Unity-serializable fields on MonoBehaviour and ScriptableObject types."
                .into(),
        },
        UnityMetricManifestEntry {
            name: "unity.type.lifecycle_methods".into(),
            entity: "type".into(),
            unit: "methods".into(),
            description: "Implemented Unity lifecycle methods.".into(),
        },
    ]
}

fn extension(path: &Path) -> Option<&str> {
    path.extension().and_then(|value| value.to_str())
}
fn is_predefined_assembly(reference: &str) -> bool {
    reference.starts_with("UnityEngine")
        || reference.starts_with("UnityEditor")
        || reference.starts_with("Unity.")
        || reference.starts_with("System")
        || reference.starts_with("Microsoft")
        || reference.starts_with("nunit")
        || matches!(reference, "Assembly-CSharp" | "Assembly-CSharp-Editor")
}
fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
fn meta_guid(contents: &str) -> Option<String> {
    contents
        .lines()
        .find_map(|line| line.trim().strip_prefix("guid:").map(str::trim))
        .filter(|guid| guid.len() == 32 && guid.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
}
fn guids_in_line(line: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut rest = line;
    while let Some(index) = rest.find("guid:") {
        rest = &rest[index + 5..];
        let guid = rest
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect::<String>();
        if guid.len() == 32 {
            output.push(guid.to_ascii_lowercase());
        }
        rest = rest.get(guid.len()..).unwrap_or_default();
    }
    output
}
fn file_id_in_line(line: &str) -> Option<String> {
    let rest = line.split_once("fileID:")?.1.trim_start();
    let value = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect::<String>();
    (!value.is_empty()).then_some(value)
}

fn class_declarations(source: &str) -> Vec<(String, String)> {
    source
        .lines()
        .filter_map(|line| {
            let tokens = line
                .replace(['{', ','], " ")
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>();
            let index = tokens.iter().position(|token| token == "class")?;
            let name = tokens.get(index + 1)?.trim_matches(':').to_string();
            let base = tokens
                .iter()
                .position(|token| token == ":")
                .and_then(|colon| tokens.get(colon + 1))
                .cloned()
                .or_else(|| {
                    line.split_once(':').and_then(|(_, rest)| {
                        rest.split(|c: char| c == ',' || c == '{' || c.is_whitespace())
                            .find(|value| !value.is_empty())
                            .map(str::to_string)
                    })
                })
                .unwrap_or_default();
            Some((name, base.trim().to_string()))
        })
        .collect()
}

fn inherits_unity(
    name: &str,
    bases: &BTreeMap<String, String>,
    visiting: &mut BTreeSet<String>,
) -> bool {
    if name == "MonoBehaviour" || name == "ScriptableObject" {
        return true;
    }
    if !visiting.insert(name.to_string()) {
        return false;
    }
    let Some(base) = bases.get(name) else {
        return false;
    };
    base.ends_with("MonoBehaviour")
        || base.ends_with("ScriptableObject")
        || inherits_unity(base, bases, visiting)
}

fn serialized_field_count(source: &str) -> usize {
    let mut serialized_attribute = false;
    let mut count = 0;
    for line in source.lines() {
        let line = line.trim();
        if line.contains("[SerializeField]") || line.contains("[SerializeReference]") {
            serialized_attribute = true;
        }
        if line.contains("[NonSerialized]") {
            serialized_attribute = false;
            continue;
        }
        if !line.ends_with(';') || line.contains('(') {
            continue;
        }
        let excluded = line.contains(" const ")
            || line.starts_with("const ")
            || line.contains(" static ")
            || line.starts_with("static ");
        if !excluded && (line.starts_with("public ") || serialized_attribute) {
            count += 1;
        }
        serialized_attribute = false;
    }
    count
}

fn lifecycle_method_count(source: &str) -> usize {
    LIFECYCLE_METHODS
        .iter()
        .filter(|name| {
            source.lines().any(|line| {
                line.contains(&format!(" {name}("))
                    || line.trim_start().starts_with(&format!("{name}("))
            })
        })
        .count()
}

fn scan_frame_calls(source: &str, path: &str, findings: &mut Vec<Finding>) {
    let methods = csharp_methods(source);
    let mut reachable = ["Update", "FixedUpdate", "LateUpdate"]
        .into_iter()
        .filter(|name| methods.contains_key(*name))
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    loop {
        let before = reachable.len();
        for name in reachable.clone() {
            let Some((_, body)) = methods.get(&name) else {
                continue;
            };
            for candidate in methods.keys() {
                if body.contains(&format!("{candidate}(")) {
                    reachable.insert(candidate.clone());
                }
            }
        }
        if reachable.len() == before {
            break;
        }
    }
    for name in reachable {
        let Some((start_line, body)) = methods.get(&name) else {
            continue;
        };
        for (offset, text) in body.lines().enumerate() {
            let expensive = text.contains("GameObject.Find")
                || text.contains("FindObjectOfType")
                || text.contains("FindFirstObjectByType")
                || text.contains("Resources.Load");
            let component = text.contains("GetComponent") || text.contains("TryGetComponent");
            if expensive || component {
                findings.push(unity_finding(
                    UnityFindingInput::new(
                        FindingKind::UnityExpensiveFrameCall,
                        path.into(),
                        start_line + offset,
                        format!("Unity frame-loop call path through {name} performs a repeated object or resource lookup"),
                        1,
                        1,
                    ).with_reliability(if expensive { 0.9 } else { 0.7 }),
                ));
            }
        }
    }
}

fn csharp_methods(source: &str) -> BTreeMap<String, (usize, String)> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut methods = BTreeMap::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let Some(paren) = line.find('(') else {
            index += 1;
            continue;
        };
        if line.trim_start().starts_with(['i', 'f'])
            && (line.trim_start().starts_with("if") || line.trim_start().starts_with("for"))
        {
            index += 1;
            continue;
        }
        let name = line[..paren]
            .split_whitespace()
            .last()
            .unwrap_or_default()
            .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_');
        if name.is_empty() || matches!(name, "if" | "for" | "while" | "switch" | "catch") {
            index += 1;
            continue;
        }
        let mut body = String::new();
        let mut depth = 0isize;
        let mut opened = false;
        let start = index + 1;
        while index < lines.len() {
            let current = lines[index];
            depth += current.matches('{').count() as isize;
            if current.contains('{') {
                opened = true;
            }
            depth -= current.matches('}').count() as isize;
            body.push_str(current);
            body.push('\n');
            index += 1;
            if opened && depth <= 0 {
                break;
            }
        }
        if opened {
            methods.insert(name.to_string(), (start, body));
        } else {
            index += 1;
        }
    }
    methods
}

fn scan_editor_api(source: &str, path: &str, findings: &mut Vec<Finding>) {
    let mut editor_only_branches = Vec::new();
    for (line, text) in source.lines().enumerate() {
        let trimmed = text.trim();
        if let Some(condition) = trimmed.strip_prefix("#if") {
            editor_only_branches.push((editor_only_condition(condition), false));
            continue;
        }
        if let Some(condition) = trimmed.strip_prefix("#elif") {
            if let Some((editor_only, has_elif)) = editor_only_branches.last_mut() {
                *editor_only = editor_only_condition(condition);
                *has_elif = true;
            }
            continue;
        }
        if trimmed.starts_with("#else") {
            if let Some((editor_only, has_elif)) = editor_only_branches.last_mut() {
                *editor_only = !*has_elif && !*editor_only;
            }
            continue;
        }
        if trimmed.starts_with("#endif") {
            editor_only_branches.pop();
            continue;
        }
        if !editor_only_branches
            .iter()
            .any(|(editor_only, _)| *editor_only)
            && (trimmed.starts_with("using UnityEditor") || trimmed.contains("UnityEditor."))
        {
            findings.push(unity_finding(
                UnityFindingInput::new(
                    FindingKind::UnityEditorApiInRuntime,
                    path.into(),
                    line + 1,
                    "UnityEditor API is reachable from runtime code without a UNITY_EDITOR guard"
                        .into(),
                    1,
                    1,
                )
                .with_reliability(0.95),
            ));
        }
    }
}

fn editor_only_condition(condition: &str) -> bool {
    condition
        .split("||")
        .all(|branch| branch.contains("UNITY_EDITOR") && !branch.contains("!UNITY_EDITOR"))
}

fn scan_event_balance(source: &str, path: &str, findings: &mut Vec<Finding>) {
    let subscriptions = source.matches("+=").count();
    let unsubscriptions = source.matches("-=").count();
    if subscriptions > unsubscriptions {
        findings.push(unity_finding(
            UnityFindingInput::new(
                FindingKind::UnityUnbalancedEventSubscription,
                path.into(),
                1,
                format!("Unity type has {subscriptions} event subscriptions but only {unsubscriptions} unsubscriptions"),
                subscriptions,
                unsubscriptions.max(1),
            ).with_reliability(0.6),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("reforge-unity-{name}-{suffix}"))
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn meta(guid: &str) -> String {
        format!("fileFormatVersion: 2\nguid: {guid}\n")
    }

    fn unity_project(name: &str) -> PathBuf {
        let root = test_root(name);
        write(
            &root.join("ProjectSettings/ProjectVersion.txt"),
            "m_EditorVersion: 2022.3.62f1\n",
        );
        write(
            &root.join("ProjectSettings/EditorSettings.asset"),
            "%YAML 1.1\nm_SerializationMode: 2\n",
        );
        write(
            &root.join("ProjectSettings/EditorBuildSettings.asset"),
            "EditorBuildSettings:\n  m_Scenes:\n  - enabled: 1\n    path: Assets/Main.unity\n",
        );
        root
    }

    #[test]
    fn parses_meta_guids_case_insensitively() {
        assert_eq!(
            meta_guid("fileFormatVersion: 2\nguid: AABBCCDDEEFF00112233445566778899\n").as_deref(),
            Some("aabbccddeeff00112233445566778899")
        );
    }

    #[test]
    fn detects_indirect_unity_inheritance() {
        let bases = BTreeMap::from([
            ("Base".into(), "MonoBehaviour".into()),
            ("Game".into(), "Base".into()),
        ]);
        assert!(inherits_unity("Game", &bases, &mut BTreeSet::new()));
    }

    #[test]
    fn auto_scans_text_assets_asmdefs_and_unity_csharp() -> Result<()> {
        let root = unity_project("complete");
        write(
            &root.join("Assets/Core.asmdef"),
            r#"{"name":"Game.Core","references":[]}"#,
        );
        write(
            &root.join("Assets/Core.asmdef.meta"),
            &meta("11111111111111111111111111111111"),
        );
        write(
            &root.join("Assets/Game.cs"),
            "using UnityEngine;\npublic class Game : MonoBehaviour {\n[SerializeField] private int score;\nvoid Update() { Tick(); }\nvoid Tick() { Resources.Load(\"card\"); }\n}\n",
        );
        write(
            &root.join("Assets/Game.cs.meta"),
            &meta("22222222222222222222222222222222"),
        );
        write(
            &root.join("Assets/Main.unity"),
            "%YAML 1.1\n--- !u!1 &1\nGameObject:\n--- !u!114 &2\nMonoBehaviour:\n  m_Script: {fileID: 11500000, guid: 22222222222222222222222222222222, type: 3}\n",
        );
        write(
            &root.join("Assets/Main.unity.meta"),
            &meta("33333333333333333333333333333333"),
        );
        let args = ScanArgs::defaults_for_path(root.clone());

        let scan = scan_unity(&root, &args)?;

        fs::remove_dir_all(&root)?;
        assert_eq!(scan.report.editor_version.as_deref(), Some("2022.3.62f1"));
        assert_eq!(
            scan.report.serialization_mode.as_deref(),
            Some("force_text")
        );
        assert_eq!(scan.report.stats.scenes, 1);
        assert!(
            scan.report
                .assemblies
                .iter()
                .any(|assembly| assembly.name == "Game.Core")
        );
        assert!(
            scan.findings
                .iter()
                .any(|finding| finding.kind == FindingKind::UnityExpensiveFrameCall)
        );
        assert!(!scan.findings.iter().any(|finding| matches!(
            finding.kind,
            FindingKind::UnityBrokenAssetReference | FindingKind::UnityMissingScript
        )));
        assert_eq!(scan.report.status, UnityProjectStatus::PartiallyObserved);
        Ok(())
    }

    #[test]
    fn package_cache_enables_definitive_broken_reference_findings() -> Result<()> {
        let root = unity_project("broken");
        fs::create_dir_all(root.join("Library/PackageCache"))?;
        write(
            &root.join("Assets/Broken.prefab"),
            "%YAML 1.1\n--- !u!114 &1\nMonoBehaviour:\n  m_Script: {fileID: 11500000, guid: ffffffffffffffffffffffffffffffff, type: 3}\n",
        );
        write(
            &root.join("Assets/Broken.prefab.meta"),
            &meta("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        );
        let args = ScanArgs::defaults_for_path(root.clone());

        let scan = scan_unity(&root, &args)?;

        fs::remove_dir_all(&root)?;
        assert_eq!(scan.report.status, UnityProjectStatus::Observed);
        assert!(
            scan.findings
                .iter()
                .any(|finding| finding.kind == FindingKind::UnityMissingScript)
        );
        assert_eq!(scan.report.problem_references.len(), 1);
        Ok(())
    }

    #[test]
    fn scans_each_csharp_file_once_and_checks_non_behaviour_editor_api() -> Result<()> {
        let root = unity_project("csharp-file-scope");
        write(
            &root.join("Assets/Behaviours.cs"),
            "using UnityEngine;\npublic class First : MonoBehaviour { void Update() { Resources.Load(\"card\"); } }\n[Test]\npublic class Second : MonoBehaviour {}\n",
        );
        write(
            &root.join("Assets/RuntimeHelper.cs"),
            "public class RuntimeHelper { UnityEditor.Editor editor; }\n",
        );
        let args = ScanArgs::defaults_for_path(root.clone());

        let scan = scan_unity(&root, &args)?;

        fs::remove_dir_all(&root)?;
        assert_eq!(
            scan.findings
                .iter()
                .filter(|finding| finding.kind == FindingKind::UnityExpensiveFrameCall)
                .count(),
            1
        );
        assert_eq!(
            scan.findings
                .iter()
                .filter(|finding| finding.kind == FindingKind::UnityEditorApiInRuntime)
                .count(),
            1
        );
        assert_eq!(scan.report.stats.tests, 1);
        assert_eq!(scan.report.raw_metrics.len(), 2);
        Ok(())
    }

    #[test]
    fn editor_api_guards_handle_else_and_nested_directives() {
        let source = "#if UNITY_EDITOR\nusing UnityEditor;\n#if DEBUG\nUnityEditor.Editor editor;\n#endif\n#else\nUnityEditor.Editor runtimeEditor;\n#endif\n";
        let mut findings = Vec::new();

        scan_editor_api(source, "Assets/Runtime.cs", &mut findings);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, Some(7));
    }

    #[test]
    fn disabled_build_settings_scenes_are_reported_as_drift() -> Result<()> {
        let root = unity_project("disabled-scene");
        write(
            &root.join("ProjectSettings/EditorBuildSettings.asset"),
            "EditorBuildSettings:\n  m_Scenes:\n  - enabled: 0\n    path: Assets/Main.unity\n",
        );
        write(
            &root.join("Assets/Main.unity"),
            "%YAML 1.1\n--- !u!1 &1\nGameObject:\n",
        );
        let args = ScanArgs::defaults_for_path(root.clone());

        let scan = scan_unity(&root, &args)?;

        fs::remove_dir_all(&root)?;
        assert!(
            scan.findings
                .iter()
                .any(|finding| finding.kind == FindingKind::UnitySceneBuildDrift)
        );
        Ok(())
    }

    #[test]
    fn unity_on_requires_a_project_root_and_off_records_disabled() -> Result<()> {
        let root = test_root("modes");
        fs::create_dir_all(&root)?;
        let mut args = ScanArgs::defaults_for_path(root.clone());
        args.unity = UnityMode::On;
        assert!(
            scan_unity(&root, &args)
                .unwrap_err()
                .to_string()
                .contains("not a Unity project root")
        );
        let project = unity_project("disabled");
        args = ScanArgs::defaults_for_path(project.clone());
        args.unity = UnityMode::Off;
        assert_eq!(
            scan_unity(&project, &args)?.report.status,
            UnityProjectStatus::Disabled
        );
        fs::remove_dir_all(root)?;
        fs::remove_dir_all(project)?;
        Ok(())
    }
}
