struct AssemblyIndex {
    names: BTreeSet<String>,
    guid_names: BTreeMap<String, String>,
    editor_names: BTreeSet<String>,
    package_cache: bool,
}

fn scan_assemblies(root: &Path, config: &Config, scan: &mut Scan) -> Result<()> {
    let assemblies = load_assemblies(root, &scan.paths);
    scan.assemblies = assemblies.len();
    let index = AssemblyIndex {
        names: assemblies.iter().map(|item| item.name.clone()).collect(),
        guid_names: assemblies
            .iter()
            .filter_map(|item| {
                item.guid
                    .as_ref()
                    .map(|guid| (guid.clone(), item.name.clone()))
            })
            .collect(),
        editor_names: assemblies
            .iter()
            .filter(|item| item.editor_only)
            .map(|item| item.name.clone())
            .collect(),
        package_cache: root.join("Library/PackageCache").is_dir(),
    };
    let mut adjacency = BTreeMap::<String, Vec<String>>::new();
    for assembly in &assemblies {
        analyze_assembly(scan, config, assembly, &index, &mut adjacency);
    }
    record_assembly_cycles(scan, &assemblies, &adjacency);
    Ok(())
}

fn load_assemblies(root: &Path, paths: &[PathBuf]) -> Vec<Assembly> {
    paths
        .iter()
        .filter(|path| extension(path) == Some("asmdef"))
        .filter_map(|path| load_assembly(root, path))
        .collect()
}

fn load_assembly(root: &Path, path: &Path) -> Option<Assembly> {
    let value = serde_json::from_str::<Asmdef>(&fs::read_to_string(path).ok()?).ok()?;
    if value.name.is_empty() {
        return None;
    }
    let display_path = display(root, path);
    Some(Assembly {
        name: value.name,
        path: display_path.clone(),
        references: value.references,
        editor_only: value
            .include_platforms
            .iter()
            .any(|value| value.eq_ignore_ascii_case("editor"))
            || display_path
                .split('/')
                .any(|part| part.eq_ignore_ascii_case("editor")),
        guid: fs::read_to_string(format!("{}.meta", path.display()))
            .ok()
            .and_then(|value| meta_guid(&value)),
    })
}

fn analyze_assembly(
    scan: &mut Scan,
    config: &Config,
    assembly: &Assembly,
    index: &AssemblyIndex,
    adjacency: &mut BTreeMap<String, Vec<String>>,
) {
    if assembly.references.len() > config.max_assembly_dependencies {
        push!(
            scan,
            "assembly_hub",
            &assembly.path,
            1,
            &format!(
                "Unity assembly {} has {} direct dependencies",
                assembly.name,
                assembly.references.len()
            ),
            assembly.references.len(),
            config.max_assembly_dependencies,
        );
    }
    for reference in &assembly.references {
        let target = resolve_assembly_reference(reference, &index.guid_names);
        record_unresolved_assembly(scan, assembly, reference, &target, index);
        record_runtime_editor_dependency(scan, assembly, &target, index);
        if index.names.contains(&target) {
            adjacency
                .entry(assembly.name.clone())
                .or_default()
                .push(target);
        }
    }
}

fn resolve_assembly_reference(reference: &str, guid_names: &BTreeMap<String, String>) -> String {
    reference
        .strip_prefix("GUID:")
        .and_then(|guid| guid_names.get(&guid.to_ascii_lowercase()))
        .cloned()
        .unwrap_or_else(|| reference.into())
}

fn record_unresolved_assembly(
    scan: &mut Scan,
    assembly: &Assembly,
    reference: &str,
    target: &str,
    index: &AssemblyIndex,
) {
    let predefined = target.starts_with("Unity")
        || target.starts_with("System")
        || target.starts_with("Microsoft")
        || target.starts_with("nunit");
    if !index.names.contains(target) && !predefined && index.package_cache {
        push!(
            scan,
            "unresolved_assembly_reference",
            &assembly.path,
            1,
            &format!("Unity assembly reference '{reference}' could not be resolved"),
            1,
            1,
        );
    }
}

fn record_runtime_editor_dependency(
    scan: &mut Scan,
    assembly: &Assembly,
    target: &str,
    index: &AssemblyIndex,
) {
    if !assembly.editor_only
        && (target.starts_with("UnityEditor") || index.editor_names.contains(target))
    {
        push!(
            scan,
            "runtime_editor_dependency",
            &assembly.path,
            1,
            &format!(
                "runtime Unity assembly {} depends on Editor-only assembly {target}",
                assembly.name
            ),
            1,
            1,
        );
    }
}

fn record_assembly_cycles(
    scan: &mut Scan,
    assemblies: &[Assembly],
    adjacency: &BTreeMap<String, Vec<String>>,
) {
    for members in graph_cycles(adjacency) {
        let path_for = |name: &str| {
            assemblies
                .iter()
                .find(|item| item.name == name)
                .map(|item| item.path.clone())
                .unwrap_or_else(|| name.into())
        };
        let mut value = detection!(
            "assembly_cycle",
            &path_for(&members[0]),
            1,
            format!("Unity assembly cycle spans {} assemblies", members.len()),
            members.len(),
            2,
        );
        value.related = members
            .iter()
            .map(|name| (path_for(name), name.clone()))
            .collect();
        scan.detections.push(value);
    }
}
