use std::path::Path;

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
