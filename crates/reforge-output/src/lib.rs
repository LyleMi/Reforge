use std::io::Write;
use std::path::Path;

use anyhow::Result;
use reforge_schema::Report;

mod html;
mod loading;
mod sarif;

use html::write_html;
pub use loading::{ensure_schema_27, load_report};
use sarif::sarif;

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

include!("human.rs");

#[cfg(test)]
mod tests;
