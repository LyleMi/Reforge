use std::io::Write;

use terminal_size::{Width, terminal_size};

const PERCENT_SCALE: usize = 100;
const DEFAULT_DYNAMIC_WIDTH: usize = 120;
const MIN_DYNAMIC_WIDTH: usize = 20;
const DYNAMIC_WIDTH_MARGIN: usize = 1;
const TRUNCATION_MARKER: &str = "...";

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProgressEvent<'a> {
    pub(super) completed: usize,
    pub(super) total: usize,
    pub(super) detail: &'a str,
}

impl ProgressEvent<'_> {
    fn percent(self) -> Option<usize> {
        if self.total == 0 {
            return None;
        }

        Some(self.completed.saturating_mul(PERCENT_SCALE) / self.total)
    }

    fn detail_suffix(self) -> String {
        if self.detail.is_empty() {
            String::new()
        } else {
            format!(" {}", self.detail)
        }
    }

    fn is_complete(self) -> bool {
        self.completed == self.total
    }
}

pub(crate) trait ProgressSink {
    fn report(&mut self, message: &str);

    fn report_scan_progress(&mut self, event: ProgressEvent<'_>) {
        let Some(percent) = event.percent() else {
            return;
        };
        self.report(&format!(
            "[{percent:>3}%] Scanning source files ({}/{}) {}",
            event.completed, event.total, event.detail
        ));
    }

    fn report_analysis_progress(&mut self, event: ProgressEvent<'_>, phase: &str) {
        let Some(percent) = event.percent() else {
            return;
        };
        let detail = event.detail_suffix();
        self.report(&format!(
            "[{percent:>3}%] Analyzing similar functions: {phase} ({}/{}){detail}",
            event.completed, event.total
        ));
    }

    fn wants_detailed_progress(&self) -> bool {
        false
    }

    fn finish(&mut self) {}
}

pub struct NoopProgress;

impl ProgressSink for NoopProgress {
    fn report(&mut self, _message: &str) {}
}

pub struct StderrProgress {
    dynamic: bool,
    last_dynamic_len: usize,
    last_bucket: Option<usize>,
}

impl StderrProgress {
    pub fn new(dynamic: bool) -> Self {
        Self {
            dynamic,
            last_dynamic_len: 0,
            last_bucket: None,
        }
    }

    fn finish_dynamic_line(&mut self) {
        if self.last_dynamic_len > 0 {
            let _ = writeln!(std::io::stderr());
            self.last_dynamic_len = 0;
        }
    }

    fn report_percent_progress(&mut self, message: &str, event: ProgressEvent<'_>) {
        if self.dynamic {
            let message = fit_dynamic_message(message, dynamic_line_width());
            let padding = self.last_dynamic_len.saturating_sub(message.len());
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "\r{message}{}", " ".repeat(padding));
            let _ = stderr.flush();
            self.last_dynamic_len = message.len();
        } else {
            let Some(percent) = event.percent() else {
                return;
            };
            let bucket = percent / 10;
            if self.last_bucket != Some(bucket) || event.is_complete() {
                self.report(message);
                self.last_bucket = Some(bucket);
            }
        }
    }
}

impl ProgressSink for StderrProgress {
    fn report(&mut self, message: &str) {
        self.finish_dynamic_line();
        let _ = writeln!(std::io::stderr(), "{message}");
    }

    fn report_scan_progress(&mut self, event: ProgressEvent<'_>) {
        let Some(percent) = event.percent() else {
            return;
        };
        let message = format!(
            "[{percent:>3}%] Scanning source files ({}/{}) {}",
            event.completed, event.total, event.detail
        );

        self.report_percent_progress(&message, event);
    }

    fn report_analysis_progress(&mut self, event: ProgressEvent<'_>, phase: &str) {
        let Some(percent) = event.percent() else {
            return;
        };
        let detail = event.detail_suffix();
        let message = format!(
            "[{percent:>3}%] Analyzing similar functions: {phase} ({}/{}){detail}",
            event.completed, event.total
        );

        self.report_percent_progress(&message, event);
    }

    fn wants_detailed_progress(&self) -> bool {
        true
    }

    fn finish(&mut self) {
        self.finish_dynamic_line();
    }
}

fn dynamic_line_width() -> usize {
    terminal_size()
        .map(|(Width(width), _)| usize::from(width))
        .filter(|width| *width >= MIN_DYNAMIC_WIDTH)
        .unwrap_or(DEFAULT_DYNAMIC_WIDTH)
}

fn fit_dynamic_message(message: &str, terminal_width: usize) -> String {
    let max_width = terminal_width
        .saturating_sub(DYNAMIC_WIDTH_MARGIN)
        .max(TRUNCATION_MARKER.len());
    let char_count = message.chars().count();
    if char_count <= max_width {
        return message.to_string();
    }

    let prefix_width = max_width.saturating_sub(TRUNCATION_MARKER.len());
    let mut fitted = message.chars().take(prefix_width).collect::<String>();
    fitted.push_str(TRUNCATION_MARKER);
    fitted
}

#[cfg(test)]
pub struct WriterProgress<W: Write> {
    writer: W,
}

#[cfg(test)]
impl<W: Write> WriterProgress<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

#[cfg(test)]
impl<W: Write> ProgressSink for WriterProgress<W> {
    fn report(&mut self, message: &str) {
        let _ = writeln!(self.writer, "{message}");
    }

    fn wants_detailed_progress(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_dynamic_message_within_terminal_width() {
        let message = "[ 50%] Scanning source files (1/2) src/generated/very/long/path/example.ts";

        let fitted = fit_dynamic_message(message, 40);

        assert_eq!(fitted.chars().count(), 39);
        assert!(fitted.ends_with(TRUNCATION_MARKER));
    }

    #[test]
    fn leaves_short_dynamic_message_unchanged() {
        let message = "[100%] Scanning source files (2/2)";

        assert_eq!(fit_dynamic_message(message, 80), message);
    }
}
