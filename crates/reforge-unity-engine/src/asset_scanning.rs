fn scan_meta_and_assets(root: &Path, config: &Config, scan: &mut Scan) -> Result<()> {
    let guid_paths = collect_meta_guids(root, &scan.paths);
    record_duplicate_guids(scan, &guid_paths);
    let known_guids = guid_paths.keys().cloned().collect::<BTreeSet<_>>();
    let package_cache = root.join("Library/PackageCache").is_dir();
    if !package_cache {
        scan.limitations.push(CoverageLimitation {
            code: "package_cache_unavailable".into(),
            count: 1,
            message: "external package references were not verified".into(),
        });
    }
    let context = AssetScanContext {
        root,
        config,
        known_guids: &known_guids,
        package_cache,
    };
    for path in scan.paths.clone() {
        scan_asset(&context, scan, &path);
    }
    Ok(())
}

fn collect_meta_guids(root: &Path, paths: &[PathBuf]) -> BTreeMap<String, Vec<String>> {
    let mut guid_paths = BTreeMap::<String, Vec<String>>::new();
    for path in paths.iter().filter(|path| extension(path) == Some("meta")) {
        if let Ok(contents) = fs::read_to_string(path)
            && let Some(guid) = meta_guid(&contents)
        {
            guid_paths
                .entry(guid)
                .or_default()
                .push(display(root, path));
        }
    }
    guid_paths
}

fn record_duplicate_guids(scan: &mut Scan, guid_paths: &BTreeMap<String, Vec<String>>) {
    for paths in guid_paths.values().filter(|paths| paths.len() > 1) {
        let mut detection = detection!(
            "duplicate_guid",
            &paths[0],
            2,
            format!("Unity GUID is declared by {} local meta files", paths.len()),
            paths.len(),
            1,
        );
        detection.related = paths
            .iter()
            .map(|path| (path.clone(), "duplicate GUID".into()))
            .collect();
        scan.detections.push(detection);
    }
}

struct AssetScanContext<'a> {
    root: &'a Path,
    config: &'a Config,
    known_guids: &'a BTreeSet<String>,
    package_cache: bool,
}

fn scan_asset(context: &AssetScanContext<'_>, scan: &mut Scan, path: &Path) {
    if extension(path) == Some("meta") {
        let target = PathBuf::from(path.to_string_lossy().trim_end_matches(".meta"));
        if !target.exists() {
            push!(
                scan,
                "orphan_meta",
                &display(context.root, path),
                1,
                "Unity meta file has no matching asset",
                1,
                1,
            );
        }
        return;
    }
    if path
        .file_name()
        .is_some_and(|name| name == "manifest.json" || name == "packages-lock.json")
    {
        return;
    }
    if !PathBuf::from(format!("{}.meta", path.display())).exists() {
        push!(
            scan,
            "missing_meta",
            &display(context.root, path),
            1,
            "Unity asset has no matching meta file",
            1,
            1,
        );
    }
    let Ok(bytes) = fs::read(path) else { return };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        scan.limitations.push(CoverageLimitation {
            code: "binary_asset".into(),
            count: 1,
            message: "a Unity asset could not be reference-checked".into(),
        });
        return;
    };
    if text.starts_with("%YAML") || text.contains("--- !u!") {
        scan_yaml_asset(context, scan, path, text);
    }
}

fn scan_yaml_asset(
    context: &AssetScanContext<'_>,
    scan: &mut Scan,
    path: &Path,
    text: &str,
) {
    let objects = text
        .lines()
        .filter(|line| line.starts_with("--- !u!"))
        .count();
    match extension(path) {
        Some("unity") if objects > context.config.max_scene_objects => push!(
            scan,
            "large_scene",
            &display(context.root, path),
            1,
            &format!("Unity scene contains {objects} serialized objects"),
            objects,
            context.config.max_scene_objects,
        ),
        Some("prefab") if objects > context.config.max_prefab_objects => push!(
            scan,
            "large_prefab",
            &display(context.root, path),
            1,
            &format!("Unity prefab contains {objects} serialized objects"),
            objects,
            context.config.max_prefab_objects,
        ),
        _ => {}
    }
    if context.package_cache {
        record_broken_guids(context.root, scan, path, text, context.known_guids);
    }
}

fn record_broken_guids(
    root: &Path,
    scan: &mut Scan,
    path: &Path,
    text: &str,
    known_guids: &BTreeSet<String>,
) {
    for (index, line) in text.lines().enumerate() {
        for guid in guids(line) {
            if guid == ZERO_GUID || known_guids.contains(&guid) {
                continue;
            }
            let (rule, message) = if line.contains("m_Script:") {
                ("missing_script", "Unity asset references a missing MonoScript")
            } else {
                (
                    "broken_asset_reference",
                    "Unity asset contains an unresolved GUID reference",
                )
            };
            push!(
                scan,
                rule,
                &display(root, path),
                index + 1,
                message,
                1,
                1,
            );
        }
    }
}
