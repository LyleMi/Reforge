fn scan_project_settings(root: &Path, scan: &mut Scan) {
    let settings =
        fs::read_to_string(root.join("ProjectSettings/EditorSettings.asset")).unwrap_or_default();
    let force_text = settings
        .lines()
        .any(|line| line.trim() == "m_SerializationMode: 2");
    if !force_text {
        scan.limitations.push(CoverageLimitation {
            code: "non_text_serialization".into(),
            count: 1,
            message: "binary assets cannot be reference-checked".into(),
        });
        push!(
            scan,
            "non_text_serialization",
            "ProjectSettings/EditorSettings.asset",
            1,
            "Unity project is not configured for Force Text serialization",
            1,
            1,
        );
    }
}

include!("asset_scanning.rs");
include!("assembly_scanning.rs");
include!("csharp_scanning.rs");

fn scan_build_settings(root: &Path, scan: &mut Scan) {
    let settings = fs::read_to_string(root.join("ProjectSettings/EditorBuildSettings.asset"))
        .unwrap_or_default();
    let included = settings
        .lines()
        .filter_map(|line| line.trim().strip_prefix("path:").map(str::trim))
        .collect::<BTreeSet<_>>();
    let scenes = scan
        .paths
        .iter()
        .filter(|path| extension(path) == Some("unity"))
        .map(|path| display(root, path))
        .collect::<Vec<_>>();
    for scene in scenes {
        if scene.starts_with("Assets/") && !included.contains(scene.as_str()) {
            push!(
                scan,
                "scene_build_drift",
                &scene,
                1,
                "Unity scene is not listed in EditorBuildSettings",
                1,
                1,
            );
        }
    }
}


fn collect_files(root: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(&path, output)?;
        } else {
            output.push(path);
        }
    }
    Ok(())
}

fn extension(path: &Path) -> Option<&str> {
    path.extension().and_then(|value| value.to_str())
}
fn display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
fn meta_guid(contents: &str) -> Option<String> {
    contents
        .lines()
        .find_map(|line| line.trim().strip_prefix("guid:").map(str::trim))
        .filter(|value| {
            value.len() == 32 && value.chars().all(|character| character.is_ascii_hexdigit())
        })
        .map(str::to_ascii_lowercase)
}
fn guids(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut remaining = line;
    while let Some((_, rest)) = remaining.split_once("guid:") {
        let value = rest
            .trim_start()
            .chars()
            .take_while(|character| character.is_ascii_hexdigit())
            .collect::<String>();
        if value.len() == 32 {
            values.push(value.to_ascii_lowercase());
        }
        remaining = rest.get(value.len()..).unwrap_or_default();
    }
    values
}
fn class_name(source: &str) -> Option<&str> {
    source.lines().find_map(|line| {
        let tokens = line
            .split(|character: char| {
                character.is_whitespace() || character == ':' || character == '{'
            })
            .collect::<Vec<_>>();
        tokens
            .iter()
            .position(|token| *token == "class")
            .and_then(|index| tokens.get(index + 1).copied())
    })
}
fn serialized_field_count(source: &str) -> usize {
    let mut attribute = false;
    let mut count = 0;
    for line in source.lines().map(str::trim) {
        attribute |= line.contains("[SerializeField]") || line.contains("[SerializeReference]");
        let declaration = line.ends_with(';') && !line.contains('(');
        if declaration
            && !line.contains(" static ")
            && !line.starts_with("static ")
            && (line.starts_with("public ") || attribute)
        {
            count += 1;
        }
        if declaration {
            attribute = false;
        }
    }
    count
}

fn graph_cycles(adjacency: &BTreeMap<String, Vec<String>>) -> Vec<Vec<String>> {
    fn visit(
        start: &str,
        node: &str,
        adjacency: &BTreeMap<String, Vec<String>>,
        path: &mut Vec<String>,
        output: &mut BTreeSet<Vec<String>>,
    ) {
        for next in adjacency.get(node).into_iter().flatten() {
            if next == start && path.len() > 1 {
                let mut members = path.clone();
                members.sort();
                members.dedup();
                output.insert(members);
            } else if !path.contains(next) {
                path.push(next.clone());
                visit(start, next, adjacency, path, output);
                path.pop();
            }
        }
    }
    let mut output = BTreeSet::new();
    for start in adjacency.keys() {
        visit(
            start,
            start,
            adjacency,
            &mut vec![start.clone()],
            &mut output,
        );
    }
    output.into_iter().collect()
}
