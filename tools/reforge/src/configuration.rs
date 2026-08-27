use super::*;

use std::sync::LazyLock;

#[path = "validation.rs"]
mod validation;
pub(super) use validation::*;

pub(super) fn config(command: ConfigCommand) -> Result<()> {
    match command.command {
        ConfigSubcommand::Validate(source) => {
            let (path, value) = load_config(source.config.as_deref(), &source.path)?;
            validate_config(&value)?;
            println!(
                "Config valid: {}",
                path.map_or_else(
                    || "built-in defaults".into(),
                    |path| path.display().to_string()
                )
            );
            Ok(())
        }
        ConfigSubcommand::Show(show) => {
            let (path, mut value) = load_config(show.source.config.as_deref(), &show.source.path)?;
            let mut sources = effective_sources(&value, path.as_deref());
            for item in &show.overrides {
                let key = item.split_once('=').context("--set expects key=value")?.0;
                apply_override(&mut value, item)?;
                sources.insert(key.into(), "cli --set".into());
            }
            validate_config(&value)?;
            materialize_low_module_cohesion_thresholds(&mut value, &mut sources)?;
            render_config_view(
                &EffectiveConfigView {
                    config_file: path.map(|path| path.display().to_string()),
                    values: value,
                    sources,
                },
                show.output,
            )
        }
    }
}

pub(super) fn materialize_low_module_cohesion_thresholds(
    value: &mut toml::Value,
    sources: &mut BTreeMap<String, String>,
) -> Result<()> {
    let preset = value
        .get("codebase")
        .and_then(|value| value.get("preset"))
        .and_then(toml::Value::as_str)
        .unwrap_or("balanced");
    let (functions, percent) = match preset {
        "strict" => (16, 40),
        "relaxed" => (30, 60),
        _ => (20, 50),
    };
    let preset_source = sources
        .get("codebase.preset")
        .map(String::as_str)
        .unwrap_or("built-in default");
    let source = format!("{preset} preset ({preset_source})");
    for (key, threshold) in [
        ("codebase.min-module-functions", functions),
        ("codebase.min-clustered-function-percent", percent),
    ] {
        if value_at_path(value, key).is_none() {
            apply_override(value, &format!("{key}={threshold}"))?;
            sources.insert(key.into(), source.clone());
        }
    }
    Ok(())
}

fn value_at_path<'a>(root: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
    path.split('.').try_fold(root, |value, key| value.get(key))
}

const STARTER_RULES: &[&str] = &[
    "reforge.codebase.large_file",
    "reforge.codebase.long_function",
    "reforge.codebase.dependency_cycle",
    "reforge.codebase.similar_functions",
];

static CLI_DEFAULT_CONFIG: LazyLock<String> = LazyLock::new(|| {
    let mut value: toml::Value =
        toml::from_str(Config::DEFAULT_TOML).expect("engine defaults must be valid TOML");
    value["rules"]["enable"] = toml::Value::Array(
        STARTER_RULES
            .iter()
            .map(|rule| toml::Value::String((*rule).into()))
            .collect(),
    );
    toml::to_string_pretty(&value).expect("CLI defaults must serialize as TOML")
});

pub(super) fn default_config() -> &'static str {
    &CLI_DEFAULT_CONFIG
}

pub(super) fn load_config(
    explicit: Option<&Path>,
    target: &Path,
) -> Result<(Option<PathBuf>, toml::Value)> {
    let path = explicit
        .map(Path::to_owned)
        .or_else(|| discover_config(target));
    let value = match &path {
        Some(path) => {
            if path.file_name().and_then(|name| name.to_str()) != Some(CONFIG_NAME) {
                bail!(
                    "Reforge 0.2 only reads versioned {CONFIG_NAME}; regenerate legacy configuration"
                );
            }
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read configuration {}", path.display()))?;
            let configured: toml::Value = toml::from_str(&text)
                .with_context(|| format!("failed to parse configuration {}", path.display()))?;
            validate_config(&configured)?;
            let mut defaults: toml::Value = toml::from_str(default_config())?;
            merge_config(&mut defaults, configured);
            defaults
        }
        None => toml::from_str(default_config()).expect("built-in configuration must be valid"),
    };
    Ok((path, value))
}

pub(super) fn merge_config(target: &mut toml::Value, configured: toml::Value) {
    match (target, configured) {
        (toml::Value::Table(target), toml::Value::Table(configured)) => {
            for (key, value) in configured {
                if let Some(existing) = target.get_mut(&key) {
                    merge_config(existing, value);
                } else {
                    target.insert(key, value);
                }
            }
        }
        (target, configured) => *target = configured,
    }
}

pub(super) fn discover_config(target: &Path) -> Option<PathBuf> {
    let mut directory = if target.is_file() {
        target.parent()?.to_path_buf()
    } else {
        target.to_path_buf()
    };
    if !directory.is_absolute() {
        directory = std::env::current_dir().ok()?.join(directory);
    }
    loop {
        let candidate = directory.join(CONFIG_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !directory.pop() {
            return None;
        }
    }
}

pub(super) fn apply_override(root: &mut toml::Value, input: &str) -> Result<()> {
    let (key, raw_value) = input.split_once('=').context("--set expects key=value")?;
    if key.trim().is_empty() {
        bail!("--set key must not be empty");
    }
    let parsed = toml::from_str::<toml::Value>(&format!("value = {raw_value}"))
        .ok()
        .and_then(|document| document.get("value").cloned())
        .unwrap_or_else(|| toml::Value::String(raw_value.to_owned()));
    let mut current = root;
    let segments = key.split('.').collect::<Vec<_>>();
    for segment in &segments[..segments.len() - 1] {
        let table = current
            .as_table_mut()
            .with_context(|| format!("cannot set `{key}` because `{segment}` is not a table"))?;
        current = table
            .entry((*segment).to_owned())
            .or_insert_with(|| toml::Value::Table(Default::default()));
    }
    let table = current
        .as_table_mut()
        .with_context(|| format!("cannot set nested key `{key}`"))?;
    table.insert(segments[segments.len() - 1].to_owned(), parsed);
    Ok(())
}

#[cfg(test)]
pub(super) fn value_at<'a>(root: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
    value_at_path(root, path)
}

pub(super) fn effective_sources(
    config: &toml::Value,
    path: Option<&Path>,
) -> BTreeMap<String, String> {
    let configured_label = path
        .map(|path| format!("config {}", path.display()))
        .unwrap_or_else(|| "built-in default".into());
    let configured = configured_leaf_paths(path);
    let mut output = BTreeMap::new();
    record_sources(config, "", &configured, &configured_label, &mut output);
    output
}

fn configured_leaf_paths(path: Option<&Path>) -> BTreeSet<String> {
    let Some(path) =
        path.filter(|path| path.file_name().and_then(|name| name.to_str()) == Some(CONFIG_NAME))
    else {
        return BTreeSet::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return BTreeSet::new();
    };
    let Ok(raw) = toml::from_str::<toml::Value>(&text) else {
        return BTreeSet::new();
    };
    let mut configured = BTreeSet::new();
    collect_leaf_paths(&raw, "", &mut configured);
    configured
}

fn collect_leaf_paths(value: &toml::Value, prefix: &str, output: &mut BTreeSet<String>) {
    if let Some(table) = value.as_table() {
        for (key, value) in table {
            let path = nested_key(prefix, key);
            collect_leaf_paths(value, &path, output);
        }
    } else {
        output.insert(prefix.into());
    }
}

fn record_sources(
    value: &toml::Value,
    prefix: &str,
    configured: &BTreeSet<String>,
    configured_label: &str,
    output: &mut BTreeMap<String, String>,
) {
    if let Some(table) = value.as_table() {
        for (key, child) in table {
            let path = nested_key(prefix, key);
            record_sources(child, &path, configured, configured_label, output);
        }
    } else {
        let source = if configured.contains(prefix) {
            configured_label
        } else {
            "built-in default"
        };
        output.insert(prefix.into(), source.into());
    }
}

fn nested_key(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.into()
    } else {
        format!("{prefix}.{key}")
    }
}

pub(super) fn render_values(values: &[serde_json::Value], format: TextFormatArg) -> Result<()> {
    match format {
        TextFormatArg::Json => serde_json::to_writer_pretty(std::io::stdout().lock(), values)?,
        TextFormatArg::Yaml => serde_yaml::to_writer(std::io::stdout().lock(), values)?,
        TextFormatArg::Human => {
            for entry in values {
                let subjects = entry["subjects"]
                    .as_array()
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(|value| value.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let languages = entry["languages"]
                    .as_object()
                    .map(|values| {
                        values
                            .iter()
                            .map(|(language, capability)| {
                                let maturity = capability["maturity"].as_str().unwrap_or("unknown");
                                let capabilities = capability["capabilities"]
                                    .as_array()
                                    .map(|values| {
                                        values
                                            .iter()
                                            .filter_map(|value| value.as_str())
                                            .collect::<Vec<_>>()
                                            .join("+")
                                    })
                                    .unwrap_or_default();
                                format!("{language}={maturity}[{capabilities}]")
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                println!(
                    "{}\n  analysis: {}\n  family: {}\n  subjects: {}\n  maturity: {}\n  semantic version: {}\n  validation basis: {}\n  default enabled: {}\n  enforceable: {}\n  observation: {} ({})\n  description: {}\n  guidance: {}\n  languages: {}\n  measurements: {}\n",
                    entry["rule"].as_str().unwrap_or_default(),
                    entry["analysis"].as_str().unwrap_or_default(),
                    entry["family"].as_str().unwrap_or_default(),
                    subjects,
                    entry["maturity"].as_str().unwrap_or_default(),
                    entry["semantic_version"].as_str().unwrap_or_default(),
                    entry["validation_basis"].as_str().unwrap_or_default(),
                    entry["default_enabled"].as_bool().unwrap_or_default(),
                    entry["enforceable"].as_bool().unwrap_or_default(),
                    entry["observation"]["source"].as_str().unwrap_or_default(),
                    entry["observation"]["unit"].as_str().unwrap_or_default(),
                    entry["description"].as_str().unwrap_or_default(),
                    entry["guidance"].as_str().unwrap_or_default(),
                    languages,
                    entry["measurements"]
                        .as_array()
                        .map(|values| values
                            .iter()
                            .filter_map(|value| value.as_str())
                            .collect::<Vec<_>>()
                            .join(", "))
                        .unwrap_or_default(),
                );
            }
        }
    }
    Ok(())
}

pub(super) fn render_config_view(view: &EffectiveConfigView, format: TextFormatArg) -> Result<()> {
    match format {
        TextFormatArg::Json => serde_json::to_writer_pretty(std::io::stdout().lock(), view)?,
        TextFormatArg::Yaml => serde_yaml::to_writer(std::io::stdout().lock(), view)?,
        TextFormatArg::Human => {
            println!("{}", toml::to_string_pretty(&view.values)?);
            println!("Sources:");
            for (key, source) in &view.sources {
                println!("  {key}: {source}");
            }
        }
    }
    Ok(())
}

pub(super) fn config_destination(path: &Path) -> PathBuf {
    if path
        .extension()
        .is_some_and(|extension| extension == "toml")
    {
        path.to_path_buf()
    } else {
        path.join(CONFIG_NAME)
    }
}
