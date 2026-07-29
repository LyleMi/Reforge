use std::io::Write;

use anyhow::Result;
use reforge_schema::Report;

pub(super) fn write_html(mut writer: impl Write, report: &Report) -> Result<()> {
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
