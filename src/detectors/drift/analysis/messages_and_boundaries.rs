use super::*;

pub(super) fn parallel_implementation_message(key: &str, count: usize) -> String {
    format!("capability `{key}` has {count} parallel-looking implementations")
}

pub(super) fn shadowed_abstraction_message(key: &str, count: usize) -> String {
    format!("helper abstraction `{key}` is shadowed in {count} locations")
}

pub(super) fn config_key_drift_message(key: &str, count: usize) -> String {
    format!("config or route key {key:?} appears in {count} locations")
}

pub(super) fn fixture_factory_drift_message(key: &str, count: usize) -> String {
    format!("test fixture factory concept `{key}` appears in {count} locations")
}

pub(super) fn boundary_inventory(files: &[SourceFile]) -> BoundaryInventory {
    let mut inventory = BoundaryInventory::default();

    for file in files.iter().filter(|file| !is_test_source(&file.path)) {
        let words = path_words(&file.path);
        if words
            .iter()
            .any(|word| HTTP_BOUNDARY_WORDS.contains(&word.as_str()))
        {
            inventory.http = true;
        }
        if words
            .iter()
            .any(|word| CONFIG_BOUNDARY_WORDS.contains(&word.as_str()))
        {
            inventory.config = true;
        }
        if words
            .iter()
            .any(|word| FS_BOUNDARY_WORDS.contains(&word.as_str()))
        {
            inventory.filesystem = true;
        }
        if words
            .iter()
            .any(|word| LOG_BOUNDARY_WORDS.contains(&word.as_str()))
        {
            inventory.logging = true;
        }
    }

    inventory
}

pub(super) fn is_boundary_file(path: &Path, kind: BypassKind) -> bool {
    let words = path_words(path);
    match kind {
        BypassKind::Http => words
            .iter()
            .any(|word| HTTP_BOUNDARY_WORDS.contains(&word.as_str())),
        BypassKind::Config => words
            .iter()
            .any(|word| CONFIG_BOUNDARY_WORDS.contains(&word.as_str())),
        BypassKind::Filesystem => words
            .iter()
            .any(|word| FS_BOUNDARY_WORDS.contains(&word.as_str())),
        BypassKind::Logging => words
            .iter()
            .any(|word| LOG_BOUNDARY_WORDS.contains(&word.as_str())),
    }
}

impl BoundaryInventory {
    pub(super) fn has(self, kind: BypassKind) -> bool {
        match kind {
            BypassKind::Http => self.http,
            BypassKind::Config => self.config,
            BypassKind::Filesystem => self.filesystem,
            BypassKind::Logging => self.logging,
        }
    }
}

impl BypassKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            BypassKind::Http => "HTTP",
            BypassKind::Config => WORD_CONFIG,
            BypassKind::Filesystem => "filesystem",
            BypassKind::Logging => "logging",
        }
    }
}
