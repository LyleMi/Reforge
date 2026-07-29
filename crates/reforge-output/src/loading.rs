use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};
use reforge_schema::{REPORT_SCHEMA_VERSION, Report};

pub fn load_report(path: &Path) -> Result<Report> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .with_context(|| format!("failed to open report {}", path.display()))?
        .read_to_end(&mut bytes)?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("json");
    let report: Report =
        if extension.eq_ignore_ascii_case("yaml") || extension.eq_ignore_ascii_case("yml") {
            serde_yaml::from_slice(&bytes)
                .map_err(|error| unsupported_schema_error(&bytes, error.into()))?
        } else {
            serde_json::from_slice(&bytes)
                .map_err(|error| unsupported_schema_error(&bytes, error.into()))?
        };
    report.validate()?;
    Ok(report)
}

fn unsupported_schema_error(bytes: &[u8], original: anyhow::Error) -> anyhow::Error {
    let version = serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .or_else(|| serde_yaml::from_slice::<serde_json::Value>(bytes).ok())
        .and_then(|value| {
            value
                .get("schema_version")
                .and_then(serde_json::Value::as_u64)
        });
    if version.is_some_and(|version| version < u64::from(REPORT_SCHEMA_VERSION)) {
        anyhow::anyhow!(
            "older Reforge report schema {} is unsupported; regenerate it with Reforge 0.2; see docs/upgrading-to-0.2.md",
            version.unwrap_or_default()
        )
    } else {
        original
    }
}

pub fn ensure_schema_27(value: &serde_json::Value) -> Result<()> {
    match value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
    {
        Some(27) => Ok(()),
        Some(version) => bail!(
            "report schema {version} is unsupported; expected schema 27; see docs/upgrading-to-0.2.md"
        ),
        None => bail!("report has no schema_version"),
    }
}
