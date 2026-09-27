use std::fs::{self, OpenOptions, Permissions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}

pub(super) struct Source {
    pub text: String,
    pub encoding: Encoding,
}

impl Source {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let (payload, encoding) = if let Some(payload) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
            (payload, Encoding::Utf8Bom)
        } else if let Some(payload) = bytes.strip_prefix(&[0xff, 0xfe]) {
            (payload, Encoding::Utf16Le)
        } else if let Some(payload) = bytes.strip_prefix(&[0xfe, 0xff]) {
            (payload, Encoding::Utf16Be)
        } else {
            (bytes, Encoding::Utf8)
        };
        let text = match encoding {
            Encoding::Utf8 | Encoding::Utf8Bom => String::from_utf8(payload.to_vec())
                .context("unsupported or invalid UTF-8 encoding")?,
            Encoding::Utf16Le | Encoding::Utf16Be => {
                if payload.len() % 2 != 0 {
                    bail!("invalid UTF-16 encoding");
                }
                let units = payload
                    .chunks_exact(2)
                    .map(|pair| {
                        let pair = [pair[0], pair[1]];
                        if encoding == Encoding::Utf16Le {
                            u16::from_le_bytes(pair)
                        } else {
                            u16::from_be_bytes(pair)
                        }
                    })
                    .collect::<Vec<_>>();
                String::from_utf16(&units).context("invalid UTF-16 encoding")?
            }
        };
        Ok(Self { text, encoding })
    }

    pub fn encode(&self, text: &str) -> Vec<u8> {
        match self.encoding {
            Encoding::Utf8 => text.as_bytes().to_vec(),
            Encoding::Utf8Bom => [b"\xef\xbb\xbf".as_slice(), text.as_bytes()].concat(),
            Encoding::Utf16Le => [
                vec![0xff, 0xfe],
                text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            ]
            .concat(),
            Encoding::Utf16Be => [
                vec![0xfe, 0xff],
                text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
            ]
            .concat(),
        }
    }
}

pub(super) fn safe_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if relative.is_empty()
        || relative.contains('\\')
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        bail!("invalid relative source path: {relative}");
    }
    let mut full = root.to_path_buf();
    for part in path.components() {
        full.push(part);
        if fs::symlink_metadata(&full)?.file_type().is_symlink() {
            bail!(
                "symbolic link is not an editable source: {}",
                full.display()
            );
        }
    }
    let metadata = fs::metadata(&full)?;
    if !metadata.is_file() {
        bail!("source is not a regular file: {relative}");
    }
    Ok(full)
}

pub(super) fn editable(path: &Path) -> Result<Permissions> {
    let metadata = fs::metadata(path)?;
    if metadata.permissions().readonly() {
        bail!("source is read-only: {}", path.display());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() > 1 {
            bail!("refusing to replace hard-linked source: {}", path.display());
        }
    }
    Ok(metadata.permissions())
}

pub(super) struct Staged {
    path: PathBuf,
}

impl Staged {
    pub fn new(destination: &Path, bytes: &[u8], permissions: Permissions) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = destination.parent().context("source has no parent")?;
        let (path, mut file) = loop {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(".reforge-comments-{}-{id}.tmp", std::process::id()));
            match OpenOptions::new().create_new(true).write(true).open(&path) {
                Ok(file) => break (path, file),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        };
        let staged = Self { path };
        file.write_all(bytes)?;
        file.set_permissions(permissions)?;
        file.sync_all()?;
        Ok(staged)
    }

    pub fn commit(&self, destination: &Path) -> Result<()> {
        fs::rename(&self.path, destination)
            .with_context(|| format!("failed to replace {}", destination.display()))
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
