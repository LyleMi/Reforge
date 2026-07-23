use super::*;

pub(super) fn validate_public_keys(value: &toml::Value) -> Result<()> {
    validate_table_keys(
        value,
        "",
        &[
            "version",
            "analysis",
            "scope",
            ANALYSIS_CODEBASE,
            ANALYSIS_DATAFLOW,
            "suppressions",
        ],
    )?;
    validate_table_keys(value, "analysis", &["enabled", "lenses"])?;
    validate_table_keys(
        value,
        "scope",
        &[
            "include-hidden",
            "include-generated",
            "no-gitignore",
            "exclude-tests",
            "ignore-paths",
        ],
    )?;
    validate_table_keys(
        value,
        ANALYSIS_CODEBASE,
        &[
            "preset",
            "max-file-lines",
            "max-dir-files",
            "min-similar-functions",
            "min-function-tokens",
            "function-similarity",
            "max-function-lines",
            "max-function-complexity",
            "max-nesting-depth",
            "max-function-parameters",
            "max-type-lines",
            "max-type-members",
            "max-imports",
            "max-public-items",
            "max-functions-per-file",
            "max-functions-per-100-lines",
            "max-small-function-ratio",
            "min-repeated-literal-occurrences",
            "min-data-clump-occurrences",
            "churn",
            "churn-window-days",
            "churn-max-commit-lines",
        ],
    )?;
    validate_table_keys(
        value,
        ANALYSIS_DATAFLOW,
        &["search", "relay", "fan-out", "policies"],
    )?;
    validate_table_keys(
        value,
        "dataflow.search",
        &[
            "max-function-hops",
            "max-path-steps",
            "max-module-hops",
            "max-paths-per-source",
            "max-sinks-per-source",
            "work-budget",
        ],
    )?;
    validate_table_keys(
        value,
        "dataflow.relay",
        &["min-function-hops", "min-module-hops", "min-relay-percent"],
    )?;
    validate_table_keys(value, "dataflow.fan-out", &["min-sinks", "min-modules"])
}

fn validate_table_keys(root: &toml::Value, path: &str, known: &[&str]) -> Result<()> {
    let value = if path.is_empty() {
        root
    } else if let Some(value) = value_at(root, path) {
        value
    } else {
        return Ok(());
    };
    let table = value.as_table().with_context(|| {
        format!(
            "{} must be a table",
            if path.is_empty() { "root" } else { path }
        )
    })?;
    for key in table.keys() {
        if !known.contains(&key.as_str()) {
            let full = if path.is_empty() {
                key.to_owned()
            } else {
                format!("{path}.{key}")
            };
            bail!("unknown configuration key `{full}`");
        }
    }
    Ok(())
}

pub(super) fn parse_enabled(value: Option<&toml::Value>) -> Result<BTreeSet<Analysis>> {
    let Some(value) = value else {
        return Ok(BTreeSet::from([Analysis::Codebase]));
    };
    let values = value
        .as_array()
        .context("analysis.enabled must be an array")?;
    let enabled = values
        .iter()
        .enumerate()
        .map(|(index, value)| match value.as_str() {
            Some(ANALYSIS_CODEBASE) => Ok(Analysis::Codebase),
            Some(ANALYSIS_DATAFLOW) => Ok(Analysis::Dataflow),
            _ => bail!("analysis.enabled[{index}] must be codebase or dataflow"),
        })
        .collect::<Result<BTreeSet<_>>>()?;
    if enabled.is_empty() {
        bail!("analysis.enabled must select at least one analysis");
    }
    Ok(enabled)
}

pub(super) fn parse_scope(value: &toml::Value) -> Result<ScopeConfig> {
    let boolean = |key: &str| -> Result<bool> {
        value_at(value, key)
            .map(|value| {
                value
                    .as_bool()
                    .with_context(|| format!("{key} must be a boolean"))
            })
            .transpose()
            .map(Option::unwrap_or_default)
    };
    let ignore_paths = value_at(value, "scope.ignore-paths")
        .map(|value| {
            value
                .as_array()
                .context("scope.ignore-paths must be an array")?
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .with_context(|| format!("scope.ignore-paths[{index}] must be a string"))
                })
                .collect()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(ScopeConfig {
        include_hidden: boolean("scope.include-hidden")?,
        include_generated: boolean("scope.include-generated")?,
        no_gitignore: boolean("scope.no-gitignore")?,
        exclude_tests: boolean("scope.exclude-tests")?,
        ignore_paths,
    })
}

pub(super) fn validate_suppressions(value: &toml::Value) -> Result<()> {
    let Some(items) = value.get("suppressions") else {
        return Ok(());
    };
    let items = items.as_array().context("suppressions must be an array")?;
    let registered = rules(&BTreeSet::from([Analysis::Codebase, Analysis::Dataflow]))
        .into_iter()
        .filter_map(|entry| entry["rule"].as_str().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    for (index, item) in items.iter().enumerate() {
        let table = item
            .as_table()
            .with_context(|| format!("suppressions[{index}] must be a table"))?;
        for key in table.keys() {
            if !["rule", "path", "line", "reason"].contains(&key.as_str()) {
                bail!("unknown configuration key `suppressions[{index}].{key}`");
            }
        }
        let rule = table
            .get("rule")
            .and_then(toml::Value::as_str)
            .with_context(|| format!("suppressions[{index}].rule must be a string"))?;
        if !registered.contains(rule) {
            bail!("unknown suppression rule `{rule}` at suppressions[{index}].rule");
        }
        table
            .get("path")
            .and_then(toml::Value::as_str)
            .with_context(|| format!("suppressions[{index}].path must be a string"))?;
        table
            .get("reason")
            .and_then(toml::Value::as_str)
            .filter(|reason| !reason.trim().is_empty())
            .with_context(|| format!("suppressions[{index}].reason must be a non-empty string"))?;
        if let Some(line) = table.get("line")
            && line.as_integer().is_none_or(|line| line <= 0)
        {
            bail!("suppressions[{index}].line must be a positive integer");
        }
    }
    Ok(())
}
