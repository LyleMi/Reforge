use std::path::Path;

use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

/// Compiled once per traversal. Matching directories prunes their entire subtree.
pub(super) struct IgnorePaths(GlobSet);

impl IgnorePaths {
    pub(super) fn new(patterns: &[String]) -> Result<Self> {
        let mut builder = GlobSetBuilder::new();
        for (index, original) in patterns.iter().enumerate() {
            let normalized = original.replace('\\', "/");
            let pattern = normalized.trim_matches('/');
            let pattern = pattern.strip_prefix("./").unwrap_or(pattern);
            // Preserve the previous behavior for empty exclusions.
            if pattern.is_empty() {
                continue;
            }
            // `cases/**` also matches `cases` so the walker need not enumerate
            // a potentially huge directory just to reject all of its children.
            for candidate in std::iter::once(pattern).chain(pattern.strip_suffix("/**")) {
                let glob = GlobBuilder::new(candidate)
                    .literal_separator(true)
                    .backslash_escape(false)
                    .build()
                    .with_context(|| {
                        format!("scope.ignore-paths[{index}] contains invalid glob {original:?}")
                    })?;
                builder.add(glob);
            }
        }
        Ok(Self(
            builder.build().context("failed to compile ignore paths")?,
        ))
    }

    pub(super) fn matches(&self, path: &Path, root: &Path) -> bool {
        if self.0.is_empty() {
            return false;
        }
        let relative = path.strip_prefix(root).unwrap_or(path);
        // Checking ancestors preserves literal directory-prefix exclusions and
        // makes direct file scans obey the same semantics as directory traversal.
        relative
            .ancestors()
            .filter(|p| !p.as_os_str().is_empty())
            .any(|p| self.0.is_match(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_root_relative_globs_and_literal_subtrees() {
        let ignores = IgnorePaths::new(&[
            "vendor/".into(),
            "**/generated/**".into(),
            "*.min.js".into(),
            "src/test_?.[jt]s".into(),
            "{cases,audits}/**".into(),
            "windows\\cache".into(),
            "./local".into(),
        ])
        .unwrap();
        let root = Path::new("/repo");
        for path in [
            "vendor",
            "vendor/a/b.py",
            "src/generated/a.rs",
            "generated/a.rs",
            "app.min.js",
            "src/test_a.ts",
            "cases/a.py",
            "audits/a.py",
            "windows/cache/a.py",
            "local/a.py",
        ] {
            assert!(ignores.matches(&root.join(path), root), "{path}");
        }
        for path in [
            "vendorized/a.py",
            "src/vendor/a.py",
            "src/app.min.js",
            "src/test_ab.ts",
            "src/main.rs",
        ] {
            assert!(!ignores.matches(&root.join(path), root), "{path}");
        }
    }

    #[test]
    fn rejects_invalid_patterns_even_without_files() {
        assert!(
            IgnorePaths::new(&["[invalid".into()])
                .err()
                .unwrap()
                .to_string()
                .contains("scope.ignore-paths[0]")
        );
        assert!(
            !IgnorePaths::new(&["".into(), "/".into()])
                .unwrap()
                .matches(Path::new("/repo/main.rs"), Path::new("/repo"))
        );
    }

    #[test]
    fn prunes_excluded_directories_before_reading_sources() {
        let root =
            std::env::temp_dir().join(format!("reforge-glob-pruning-{}", std::process::id()));
        std::fs::create_dir_all(root.join("cases/deep")).unwrap();
        std::fs::write(root.join("cases/deep/broken.py"), [0xff]).unwrap();
        std::fs::write(root.join("main.py"), "pass\n").unwrap();
        let mut args = crate::execution::EffectiveConfig::defaults_for_path(root.clone());
        args.filters.ignore_paths = vec!["cases/**".into()];
        let plan = super::super::collect_source_scan_plan(&root, &args).unwrap();
        assert_eq!(plan.source_files, vec![root.join("main.py")]);
        assert_eq!(plan.directories_scanned, 1);
        args.filters.ignore_paths = vec!["*.py".into()];
        assert!(
            super::super::collect_source_scan_plan(&root.join("main.py"), &args)
                .unwrap()
                .source_files
                .is_empty()
        );
        assert!(
            super::super::comment_source_paths(&root.join("main.py"), &args)
                .unwrap()
                .is_empty()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
