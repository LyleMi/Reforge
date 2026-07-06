mod human;

use std::io::Write;

use anyhow::Result;

pub use human::{
    print_human_report, print_human_report_colored, write_human_report, write_human_report_colored,
};

#[cfg(test)]
pub use human::{render_human_report, render_human_report_colored};

pub fn print_json_report(report: &ScanReport) -> Result<()> {
    write_json_report(std::io::stdout().lock(), report)
}

pub fn print_yaml_report(report: &ScanReport) -> Result<()> {
    write_yaml_report(std::io::stdout().lock(), report)
}

pub fn write_json_report(mut writer: impl Write, report: &ScanReport) -> Result<()> {
    writer.write_all(serde_json::to_string_pretty(report)?.as_bytes())?;
    writer.write_all(b"\n")?;
    Ok(())
}

pub fn write_yaml_report(mut writer: impl Write, report: &ScanReport) -> Result<()> {
    let output = serde_yaml::to_string(report)?;
    writer.write_all(output.as_bytes())?;
    if !output.ends_with('\n') {
        writer.write_all(b"\n")?;
    }
    Ok(())
}

use crate::model::ScanReport;

#[cfg(test)]
use std::collections::BTreeMap;

#[cfg(test)]
use crate::model::{Finding, FindingKind, Severity};

#[cfg(test)]
#[path = "../report_tests.rs"]
mod tests;
