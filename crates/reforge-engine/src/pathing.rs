use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

pub(crate) fn normalize_path_text(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .strip_prefix("//?/UNC/")
        .map(|path| format!("//{path}"))
        .or_else(|| normalized.strip_prefix("//?/").map(ToString::to_string))
        .unwrap_or(normalized)
}

pub(crate) fn display_path(path: &Path) -> String {
    normalize_path_text(&path.to_string_lossy())
}

/// Identifies the logical repository independently of its checkout directory.
pub(crate) fn workspace_identity(root: &Path) -> String {
    let identity = git(root, &["config", "--get", "remote.origin.url"])
        .or_else(|| {
            git(root, &["rev-parse", "--show-toplevel"]).and_then(|top| {
                Path::new(&top)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
        })
        .unwrap_or_else(|| {
            root.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
    let mut digest = Sha256::new();
    digest.update(
        identity
            .trim_end_matches(".git")
            .replace('\\', "/")
            .as_bytes(),
    );
    let hash = format!("{:x}", digest.finalize());
    format!("rw5-{}", &hash[..20])
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_windows_drive_and_unc_verbatim_paths() {
        assert_eq!(normalize_path_text(r"\\?\C:\project\src"), "C:/project/src");
        assert_eq!(
            normalize_path_text(r"\\?\UNC\server\share\src"),
            "//server/share/src"
        );
    }
}
