use std::path::Path;
use std::process::Command;

/// Return the source revision used by the public report target.
///
/// Configuration, policy, and dirty-state provenance are intentionally not
/// part of the analyzer model.
pub(crate) fn git_revision(root: &Path) -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
