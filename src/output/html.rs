use std::io::{self, Write};

use crate::model::ScanReport;

const REPORT_APP_CSS: &str = include_str!("../../assets/report-app.css");
const REPORT_APP_JS: &str = include_str!("../../assets/report-app.js");

pub fn print_html_report(report: &ScanReport) -> io::Result<()> {
    write_html_report(std::io::stdout().lock(), report)
}

pub fn write_html_report(mut writer: impl Write, report: &ScanReport) -> io::Result<()> {
    writer.write_all(render_html_report(report).as_bytes())
}

pub fn render_html_report(report: &ScanReport) -> String {
    let report_json =
        serde_json::to_string(report).expect("scan reports should serialize to JSON for HTML");
    let report_json = escape_script_json(&report_json);

    format!(
        concat!(
            "<!doctype html>\n",
            "<html lang=\"en\">\n",
            "<head>\n",
            "<meta charset=\"utf-8\">\n",
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n",
            "<title>Reforge scan report</title>\n",
            "<style>\n{css}\n</style>\n",
            "</head>\n",
            "<body>\n",
            "<div id=\"reforge-report-root\"></div>\n",
            "<script id=\"reforge-report-data\" type=\"application/json\">{report_json}</script>\n",
            "<script type=\"module\">\n{js}\n</script>\n",
            "</body>\n",
            "</html>\n"
        ),
        css = REPORT_APP_CSS,
        report_json = report_json,
        js = REPORT_APP_JS,
    )
}

fn escape_script_json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '<' => escaped.push_str("\\u003c"),
            '>' => escaped.push_str("\\u003e"),
            '&' => escaped.push_str("\\u0026"),
            '\u{2028}' => escaped.push_str("\\u2028"),
            '\u{2029}' => escaped.push_str("\\u2029"),
            _ => escaped.push(character),
        }
    }
    escaped
}
