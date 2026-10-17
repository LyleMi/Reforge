use super::*;
use sha2::{Digest, Sha256};

#[derive(Default)]
pub(super) struct IgnorePolicy {
    files: BTreeMap<PathBuf, Vec<u8>>,
}

impl IgnorePolicy {
    pub(super) fn for_root(root: &Path, git_ignore: bool) -> Result<Self> {
        let mut policy = Self::default();
        let directory = if root.is_file() {
            root.parent().unwrap_or(root)
        } else {
            root
        };
        for parent in directory.ancestors() {
            policy.observe_directory(parent, git_ignore)?;
        }
        if git_ignore {
            policy.observe_git_excludes(directory)?;
        }
        Ok(policy)
    }

    pub(super) fn observe_directory(&mut self, directory: &Path, git_ignore: bool) -> Result<()> {
        self.observe_file(directory.join(".ignore"))?;
        if git_ignore {
            self.observe_file(directory.join(".gitignore"))?;
        }
        Ok(())
    }

    fn observe_git_excludes(&mut self, directory: &Path) -> Result<()> {
        if let Some(path) = git_path(directory, &["rev-parse", "--git-path", "info/exclude"]) {
            self.observe_file(path)?;
        }
        if let Some(path) = git_path(
            directory,
            &["config", "--path", "--get", "core.excludesFile"],
        ) {
            self.observe_file(path)?;
        } else if let Some(config) = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        {
            self.observe_file(config.join("git/ignore"))?;
        }
        Ok(())
    }

    fn observe_file(&mut self, path: PathBuf) -> Result<()> {
        if self.files.contains_key(&path) {
            return Ok(());
        }
        match fs::read(&path) {
            Ok(contents) => {
                self.files.insert(path, Sha256::digest(contents).to_vec());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to fingerprint ignore policy {}", path.display())
                });
            }
        }
        Ok(())
    }

    pub(super) fn digest(&self, root: &Path) -> String {
        let mut hash = Sha256::new();
        let mut entries = self
            .files
            .iter()
            .map(|(path, contents)| (stable_policy_path(root, path), contents))
            .collect::<Vec<_>>();
        entries.sort();
        for (path, contents) in entries {
            hash.update(path.as_bytes());
            hash.update([0]);
            hash.update(contents);
        }
        hex::encode(hash.finalize())
    }
}

fn stable_policy_path(root: &Path, path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(root) {
        return format!("root/{}", relative.to_string_lossy());
    }
    if let Some(parent) = path.parent()
        && let Ok(relative_root) = root.strip_prefix(parent)
    {
        return format!(
            "ancestor/{}/{}",
            relative_root.components().count(),
            path.file_name().unwrap_or_default().to_string_lossy()
        );
    }
    format!(
        "external/{}",
        path.file_name().unwrap_or_default().to_string_lossy()
    )
}

fn git_path(directory: &Path, args: &[&str]) -> Option<PathBuf> {
    let output = std::process::Command::new("git")
        .current_dir(directory)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let path = PathBuf::from(text.trim());
    if path.as_os_str().is_empty() {
        return None;
    }
    Some(if path.is_absolute() {
        path
    } else {
        directory.join(path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_does_not_depend_on_checkout_or_external_config_paths() {
        let contents = Sha256::digest(b"target/\n").to_vec();
        let policy = |base: &str| IgnorePolicy {
            files: BTreeMap::from([
                (
                    PathBuf::from(format!("/{base}/project/.gitignore")),
                    contents.clone(),
                ),
                (
                    PathBuf::from(format!("/{base}/global/ignore")),
                    contents.clone(),
                ),
            ]),
        };
        assert_eq!(
            policy("first").digest(Path::new("/first/project")),
            policy("second").digest(Path::new("/second/project"))
        );
    }
}
