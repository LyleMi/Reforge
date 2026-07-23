use std::io::{Read, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use reforge_schema::{REPORT_SCHEMA_VERSION, Report};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Html,
    Json,
    Yaml,
    Sarif,
}

impl OutputFormat {
    pub fn infer(explicit: Option<Self>, path: Option<&Path>) -> Self {
        explicit.unwrap_or_else(|| {
            match path
                .and_then(Path::extension)
                .and_then(|value| value.to_str())
            {
                Some(value) if value.eq_ignore_ascii_case("json") => Self::Json,
                Some(value)
                    if value.eq_ignore_ascii_case("yaml") || value.eq_ignore_ascii_case("yml") =>
                {
                    Self::Yaml
                }
                Some(value)
                    if value.eq_ignore_ascii_case("html") || value.eq_ignore_ascii_case("htm") =>
                {
                    Self::Html
                }
                Some(value) if value.eq_ignore_ascii_case("sarif") => Self::Sarif,
                _ => Self::Human,
            }
        })
    }
}

pub fn write_report(mut writer: impl Write, report: &Report, format: OutputFormat) -> Result<()> {
    report.validate()?;
    match format {
        OutputFormat::Json => serde_json::to_writer_pretty(&mut writer, report)?,
        OutputFormat::Yaml => serde_yaml::to_writer(&mut writer, report)?,
        OutputFormat::Human => write_human(&mut writer, report)?,
        OutputFormat::Sarif => serde_json::to_writer_pretty(&mut writer, &sarif(report))?,
        OutputFormat::Html => write_html(&mut writer, report)?,
    }
    if matches!(
        format,
        OutputFormat::Json | OutputFormat::Human | OutputFormat::Sarif
    ) {
        writeln!(writer)?;
    }
    Ok(())
}

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

// Schema 26 is a hard boundary: older inputs fail with regeneration guidance.
fn unsupported_schema_error(bytes: &[u8], original: anyhow::Error) -> anyhow::Error {
    let version = serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
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

include!("human.rs");

fn write_html(mut writer: impl Write, report: &Report) -> Result<()> {
    let payload = serde_json::to_string(report)?.replace('<', "\\u003c");
    writer.write_all(br#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Reforge report</title><style>"#)?;
    writer.write_all(include_str!("../../../assets/report-app.css").as_bytes())?;
    writer.write_all(br#"</style></head><body><div id="reforge-report-root"></div><script id="reforge-report-data" type="application/json">"#)?;
    writer.write_all(payload.as_bytes())?;
    writer.write_all(br#"</script><script>"#)?;
    writer.write_all(include_str!("../../../assets/report-app.js").as_bytes())?;
    writer.write_all(b"</script></body></html>")?;
    Ok(())
}

fn sarif(report: &Report) -> impl Serialize + '_ {
    serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": { "name": report.producer.name, "version": report.producer.version } },
            "results": report.issues.iter().map(|issue| {
                let location = issue.evidence.iter().flat_map(|evidence| &evidence.locations).next();
                serde_json::json!({
                    "ruleId": issue.family,
                    "message": { "text": issue.title },
                    "partialFingerprints": { "reforgeIssueId": issue.id },
                    "locations": location.map(|location| vec![serde_json::json!({
                        "physicalLocation": {
                            "artifactLocation": { "uri": location.path },
                            "region": { "startLine": location.line.unwrap_or(1) }
                        }
                    })]).unwrap_or_default()
                })
            }).collect::<Vec<_>>()
        }]
    })
}

pub fn ensure_schema_26(value: &serde_json::Value) -> Result<()> {
    match value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
    {
        Some(26) => Ok(()),
        Some(version) => bail!("report schema {version} is unsupported; expected schema 26"),
        None => bail!("report has no schema_version"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_extensions() {
        assert_eq!(
            OutputFormat::infer(None, Some(Path::new("report.sarif"))),
            OutputFormat::Sarif
        );
        assert_eq!(
            OutputFormat::infer(None, Some(Path::new("report.yml"))),
            OutputFormat::Yaml
        );
    }

    #[test]
    fn schema_26_is_a_hard_input_boundary() {
        ensure_schema_26(&serde_json::json!({ "schema_version": 26 })).unwrap();
        let error = ensure_schema_26(&serde_json::json!({ "schema_version": 25 }))
            .unwrap_err()
            .to_string();
        assert!(error.contains("expected schema 26"));
    }
}
