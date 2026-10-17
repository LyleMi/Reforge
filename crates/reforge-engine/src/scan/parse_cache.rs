use super::*;
use sha2::{Digest, Sha256};

/// Reuse immutable source buffers and syntax trees for exact source copies.
/// Paths remain per-file so reports and dependency resolution retain ownership.
#[derive(Default)]
pub(super) struct ParseCache {
    entries: BTreeMap<(String, [u8; 32]), CachedSource>,
}

struct CachedSource {
    source: Arc<str>,
    parsed: Option<ParsedSourceFile>,
}

impl ParseCache {
    pub(super) fn parse(
        &mut self,
        mut file: SourceFile,
    ) -> Result<(SourceFile, Option<ParsedSourceFile>)> {
        let key = (
            file.path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            Sha256::digest(file.source.as_bytes()).into(),
        );
        if let Some(entry) = self
            .entries
            .get(&key)
            .filter(|entry| entry.source == file.source)
        {
            file.source = Arc::clone(&entry.source);
            let parsed = entry.parsed.clone().map(|mut parsed| {
                parsed.file.path = file.path.clone();
                parsed.file.display_path = file.display_path.clone();
                parsed
            });
            return Ok((file, parsed));
        }
        let parsed = parse_source_file(file.clone())?;
        self.entries.insert(
            key,
            CachedSource {
                source: Arc::clone(&file.source),
                parsed: parsed.clone(),
            },
        );
        Ok((file, parsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(path: &str, text: &str) -> SourceFile {
        SourceFile {
            path: path.into(),
            display_path: path.into(),
            source: text.into(),
        }
    }

    #[test]
    fn copies_share_source_but_keep_paths_and_language_parsers_separate() {
        let mut cache = ParseCache::default();
        let (first, parsed_first) = cache
            .parse(source("a/check.py", "def check(): pass\n"))
            .unwrap();
        let (second, parsed_second) = cache
            .parse(source("b/check.py", "def check(): pass\n"))
            .unwrap();
        assert!(Arc::ptr_eq(&first.source, &second.source));
        assert_eq!(parsed_first.unwrap().file.display_path, "a/check.py");
        assert_eq!(parsed_second.unwrap().file.display_path, "b/check.py");
        assert_eq!(cache.entries.len(), 1);
        cache.parse(source("a.js", "const x: number = 1;")).unwrap();
        let (_, typed) = cache.parse(source("a.ts", "const x: number = 1;")).unwrap();
        assert!(typed.is_some());
        assert_eq!(cache.entries.len(), 3);
    }

    #[test]
    fn identical_parse_failures_keep_individual_source_paths() {
        let mut cache = ParseCache::default();
        for path in ["a.py", "b.py"] {
            let (file, parsed) = cache.parse(source(path, "def broken(:\n")).unwrap();
            assert_eq!(file.display_path, path);
            assert!(parsed.is_none());
        }
        assert_eq!(cache.entries.len(), 1);
    }
}
