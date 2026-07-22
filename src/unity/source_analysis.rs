struct UnityFindingInput {
    kind: FindingKind,
    path: String,
    line: usize,
    message: String,
    value: usize,
    threshold: usize,
    related: Vec<RelatedLocation>,
}
fn package_analysis_roots(packages: &Path) -> Result<Vec<PathBuf>> {
    let mut roots = embedded_package_roots(packages)?;
    roots.extend(local_package_roots(packages));
    Ok(roots)
}

fn embedded_package_roots(packages: &Path) -> Result<Vec<PathBuf>> {
    if !packages.is_dir() {
        return Ok(Vec::new());
    }
    fs::read_dir(packages)?
        .map(|entry| entry.map(|entry| entry.path()))
        .filter(|path| path.as_ref().is_ok_and(|path| path.is_dir()))
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn local_package_roots(packages: &Path) -> Vec<PathBuf> {
    let dependencies = fs::read_to_string(packages.join("manifest.json"))
        .ok()
        .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
        .and_then(|value| value.get("dependencies").and_then(|value| value.as_object()).cloned())
        .unwrap_or_default();
    dependencies
        .into_values()
        .filter_map(|value| value.as_str().and_then(|value| value.strip_prefix("file:")).map(str::to_string))
        .map(|relative| packages.join(relative))
        .map(|path| path.canonicalize().unwrap_or(path))
        .filter(|path| path.is_dir())
        .collect()
}

fn included_build_scenes(build_settings: &str) -> BTreeSet<String> {
    let mut included = BTreeSet::new();
    let mut enabled = None;
    for line in build_settings.lines() {
        let trimmed = line.trim().trim_start_matches("- ");
        match (
            trimmed.strip_prefix("enabled:"),
            trimmed.strip_prefix("path:"),
        ) {
            (Some(value), _) => enabled = Some(value.trim() == "1"),
            (_, Some(path)) => {
                if enabled.unwrap_or(true) {
                    included.insert(path.trim().to_string());
                }
                enabled = None;
            }
            _ => {}
        }
    }
    included
}

struct CSharpRecord {
    path: PathBuf,
    source: String,
    declared_types: Vec<String>,
}

fn load_csharp_records(
    files: &[PathBuf],
) -> (BTreeMap<String, String>, Vec<CSharpRecord>) {
    let mut base_by_type = BTreeMap::new();
    let records = files
        .iter()
        .map(|path| {
            let source = fs::read_to_string(path).unwrap_or_default();
            let declarations = class_declarations(&source);
            base_by_type.extend(declarations.iter().cloned());
            CSharpRecord {
                path: path.clone(),
                source,
                declared_types: declarations.into_iter().map(|(name, _)| name).collect(),
            }
        })
        .collect();
    (base_by_type, records)
}

fn is_editor_path(path: &str) -> bool {
    path.split('/')
        .any(|part| part.eq_ignore_ascii_case("editor"))
}

fn contains_test_attribute(source: &str) -> bool {
    source.contains("[UnityTest]") || source.contains("[Test]")
}

impl UnityFindingInput {
    fn new(
        kind: FindingKind,
        path: String,
        line: usize,
        message: String,
        metric: (usize, usize),
    ) -> Self {
        Self {
            kind,
            path,
            line,
            message,
            value: metric.0,
            threshold: metric.1,
            related: Vec::new(),
        }
    }
    fn with_related(mut self, related: Vec<RelatedLocation>) -> Self {
        self.related = related;
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
        .with_related_locations(input.related),
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

fn is_scannable_unity_asset(path: &Path) -> bool {
    !matches!(extension(path), Some("meta" | "asmdef" | "cs")) && path.is_file()
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

fn load_assembly_record(root: &Path, path: &Path) -> Option<AssemblyRecord> {
    let contents = fs::read_to_string(path).ok()?;
    let asmdef = serde_json::from_str::<AsmdefFile>(&contents).ok()?;
    if asmdef.name.is_empty() {
        return None;
    }
    let display = display_path(root, path);
    let editor_only = asmdef
        .include_platforms
        .iter()
        .any(|platform| platform.eq_ignore_ascii_case("editor"))
        || is_editor_path(&display);
    let test_assembly = asmdef
        .optional_unity_references
        .iter()
        .any(|reference| reference == "TestAssemblies")
        || display.contains("/Tests/");
    let guid = fs::read_to_string(format!("{}.meta", path.display()))
        .ok()
        .and_then(|contents| meta_guid(&contents));
    Some(AssemblyRecord {
        node: UnityAssemblyNode {
            name: asmdef.name,
            path: display,
            editor_only,
            test_assembly,
            predefined: false,
        },
        references: asmdef.references,
        guid,
    })
}

fn assembly_edges(assemblies: &[AssemblyRecord]) -> Vec<UnityAssemblyEdge> {
    let name_index = assemblies
        .iter()
        .map(|assembly| (assembly.node.name.clone(), assembly.node.name.clone()))
        .collect::<BTreeMap<_, _>>();
    let guid_index = assemblies
        .iter()
        .filter_map(|assembly| {
            assembly
                .guid
                .as_ref()
                .map(|guid| (guid.clone(), assembly.node.name.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    assemblies
        .iter()
        .flat_map(|assembly| {
            assembly.references.iter().map(|reference| {
                let target = reference
                    .strip_prefix("GUID:")
                    .and_then(|guid| guid_index.get(&guid.to_ascii_lowercase()))
                    .or_else(|| name_index.get(reference));
                UnityAssemblyEdge {
                    from: assembly.node.name.clone(),
                    to: target.cloned().unwrap_or_else(|| reference.clone()),
                    reference: reference.clone(),
                    resolved: target.is_some() || is_predefined_assembly(reference),
                }
            })
        })
        .collect()
}

fn local_assembly_names(assemblies: &[AssemblyRecord]) -> BTreeSet<String> {
    assemblies
        .iter()
        .filter(|assembly| !assembly.node.path.starts_with("Library/PackageCache/"))
        .map(|assembly| assembly.node.name.clone())
        .collect()
}

fn assembly_cycles(edges: &[UnityAssemblyEdge]) -> Vec<Vec<String>> {
    let adjacency = edges.iter().filter(|edge| edge.resolved).fold(
        BTreeMap::<String, Vec<String>>::new(),
        |mut map, edge| {
            map.entry(edge.from.clone())
                .or_default()
                .push(edge.to.clone());
            map
        },
    );
    let mut collector = AssemblyCycleCollector {
        adjacency: &adjacency,
        emitted: BTreeSet::new(),
        cycles: Vec::new(),
    };
    for start in adjacency.keys() {
        collector.visit(start, start, vec![start.clone()]);
    }
    collector.cycles
}

struct AssemblyCycleCollector<'a> {
    adjacency: &'a BTreeMap<String, Vec<String>>,
    emitted: BTreeSet<String>,
    cycles: Vec<Vec<String>>,
}

impl AssemblyCycleCollector<'_> {
    fn visit(&mut self, start: &str, node: &str, path: Vec<String>) {
        let next_nodes = self.adjacency.get(node).cloned().unwrap_or_default();
        for next in next_nodes {
            if next == start && path.len() > 1 {
                let mut members = path.clone();
                members.sort();
                members.dedup();
                if self.emitted.insert(members.join("|")) {
                    self.cycles.push(members);
                }
            } else if !path.contains(&next) && path.len() <= self.adjacency.len() {
                let mut extended = path.clone();
                extended.push(next.clone());
                self.visit(start, &next, extended);
            }
        }
    }
}

fn assembly_cycle_finding(members: &[String], assemblies: &[AssemblyRecord]) -> Finding {
    let assembly_path = |name: &str| {
        assemblies
            .iter()
            .find(|assembly| assembly.node.name == name)
            .map(|assembly| assembly.node.path.clone())
            .unwrap_or_else(|| name.to_string())
    };
    let related = members
        .iter()
        .map(|name| RelatedLocation {
            path: assembly_path(name),
            line: 1,
            name: Some(name.clone()),
        })
        .collect();
    unity_finding(
        UnityFindingInput::new(
            FindingKind::UnityAssemblyCycle,
            assembly_path(&members[0]),
            1,
            format!("Unity assembly cycle spans {} assemblies", members.len()), (
            members.len(),
            2))
        .with_related(related),
    )
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

include!("behaviour_analysis.rs");

fn scan_event_balance(source: &str, path: &str, findings: &mut Vec<Finding>) {
    let subscriptions = source.matches("+=").count();
    let unsubscriptions = source.matches("-=").count();
    if subscriptions > unsubscriptions {
        findings.push(unity_finding(
            UnityFindingInput::new(
                FindingKind::UnityUnbalancedEventSubscription,
                path.into(),
                1,
                format!("Unity type has {subscriptions} event subscriptions but only {unsubscriptions} unsubscriptions"), (
                subscriptions,
                unsubscriptions.max(1))),
        ));
    }
}
