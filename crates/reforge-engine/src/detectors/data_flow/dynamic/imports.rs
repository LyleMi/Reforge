use super::*;

pub(super) fn static_imports(
    root: &Path,
    file: &ParsedSourceFile,
) -> BTreeMap<String, ImportTarget> {
    let module = stable_path(root, &file.file.path);
    let parent = Path::new(&module).parent().unwrap_or_else(|| Path::new(""));
    let mut imports = BTreeMap::new();
    for line in file.file.source.lines().map(str::trim) {
        if let Some((names, hint)) =
            javascript_import(parent, line).or_else(|| python_import(parent, line))
        {
            extend_imports(&mut imports, names, hint);
        }
    }
    imports
}

fn javascript_import<'a>(parent: &Path, line: &'a str) -> Option<(&'a str, String)> {
    let (names, module_name) = line.strip_prefix("import {")?.split_once("} from ")?;
    Some((names, normalize_module_hint(parent, module_name)))
}

fn python_import<'a>(parent: &Path, line: &'a str) -> Option<(&'a str, String)> {
    let (module_name, names) = line.strip_prefix("from ")?.split_once(" import ")?;
    Some((
        names,
        normalize_python_module_hint(parent, module_name.trim()),
    ))
}

fn extend_imports(imports: &mut BTreeMap<String, ImportTarget>, names: &str, module_hint: String) {
    for name in names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        let (exported, local) = name
            .split_once(" as ")
            .map_or((name, name), |(exported, local)| {
                (exported.trim(), local.trim())
            });
        imports.insert(
            local.into(),
            ImportTarget {
                exported_name: exported.into(),
                module_hint: module_hint.clone(),
            },
        );
    }
}

fn normalize_module_hint(parent: &Path, raw: &str) -> String {
    let raw = raw
        .trim()
        .trim_end_matches(';')
        .trim_matches(|character| character == '\'' || character == '"');
    let raw = raw.strip_prefix("./").unwrap_or(raw);
    parent.join(raw).to_string_lossy().replace('\\', "/")
}

fn normalize_python_module_hint(parent: &Path, raw: &str) -> String {
    if let Some(relative) = raw.strip_prefix('.') {
        parent.join(relative.replace('.', "/"))
    } else {
        PathBuf::from(raw.replace('.', "/"))
    }
    .to_string_lossy()
    .replace('\\', "/")
}

pub(super) fn module_matches(module: &str, hint: &str) -> bool {
    let module = module
        .strip_suffix(".js")
        .or_else(|| module.strip_suffix(".ts"))
        .or_else(|| module.strip_suffix(".tsx"))
        .or_else(|| module.strip_suffix(".py"))
        .unwrap_or(module);
    module == hint || module.ends_with(&format!("/{hint}"))
}
