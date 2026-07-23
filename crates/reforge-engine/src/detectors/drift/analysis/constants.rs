pub(super) const WORD_CONFIG: &str = "config";
pub(super) const WORD_FACTORY: &str = "factory";

pub(super) const HTTP_BYPASS_PATTERNS: &[&str] = &[
    "fetch(",
    "axios.",
    "axios(",
    "requests.get(",
    "requests.post(",
    "reqwest::",
    "hyper::",
    "http.client",
];

pub(super) const CONFIG_BYPASS_PATTERNS: &[&str] = &[
    "process.env",
    "std::env::var",
    "env::var(",
    "os.environ",
    "os.getenv(",
    "getenv(",
];

pub(super) const FILESYSTEM_BYPASS_PATTERNS: &[&str] = &[
    "fs.readfile",
    "fs.writfile",
    "fs.writefile",
    "std::fs::read",
    "std::fs::write",
    "file::open",
    "read_to_string",
];

pub(super) const LOGGING_BYPASS_PATTERNS: &[&str] = &[
    "console.log(",
    "println!(",
    "dbg!(",
    "print(",
    "log.printf(",
    "log.println(",
];

pub(super) const BYPASS_RULES: &[BypassRule] = &[
    BypassRule {
        kind: BypassKind::Http,
        patterns: HTTP_BYPASS_PATTERNS,
        occurrence_name: "direct HTTP call",
    },
    BypassRule {
        kind: BypassKind::Config,
        patterns: CONFIG_BYPASS_PATTERNS,
        occurrence_name: "direct config read",
    },
    BypassRule {
        kind: BypassKind::Filesystem,
        patterns: FILESYSTEM_BYPASS_PATTERNS,
        occurrence_name: "direct filesystem call",
    },
    BypassRule {
        kind: BypassKind::Logging,
        patterns: LOGGING_BYPASS_PATTERNS,
        occurrence_name: "direct log call",
    },
];

pub(super) const PARALLEL_CAPABILITY_WORDS: &[&str] = &[
    "adapt",
    "adapter",
    "build",
    "cache",
    "client",
    WORD_CONFIG,
    "load",
    "logger",
    "map",
    "normalize",
    "parse",
    "retry",
    "validate",
];

pub(super) const PARALLEL_STOP_WORDS: &[&str] = &[
    "a", "and", "do", "for", "from", "get", "has", "is", "make", "new", "of", "set", "the", "to",
    "with",
];

pub(super) const SHADOW_HELPER_WORDS: &[&str] = &[
    "common",
    "helper",
    "helpers",
    "shared",
    "util",
    "utils",
    "adapter",
    "normalizer",
    "validator",
    "mapper",
    WORD_FACTORY,
];

pub(super) const SHADOW_STOP_WORDS: &[&str] = &[
    "common",
    "helper",
    "helpers",
    "shared",
    "util",
    "utils",
    WORD_FACTORY,
    "make",
    "create",
    "build",
    "get",
    "set",
];

pub(super) const FIXTURE_WORDS: &[&str] = &[
    "builder",
    "dummy",
    WORD_FACTORY,
    "fake",
    "fixture",
    "mock",
    "sample",
    "setup",
    "test",
];

pub(super) const GENERIC_BUCKET_WORDS: &[&str] = &[
    "common", "helper", "helpers", "lib", "misc", "shared", "util", "utils",
];

pub(super) const STOP_WORDS: &[&str] = &[
    "api", "app", "cmd", "for", "from", "get", "has", "impl", "index", "main", "mod", "new", "old",
    "src", "test", "tests", "the", "this", "type", "use", "with",
];

pub(super) const CONFIG_KEY_WORDS: &[&str] = &[
    "api",
    "auth",
    "base",
    "client",
    "code",
    WORD_CONFIG,
    "database",
    "db",
    "endpoint",
    "env",
    "error",
    "host",
    "key",
    "path",
    "port",
    "route",
    "secret",
    "service",
    "token",
    "url",
];

pub(super) const HTTP_BOUNDARY_WORDS: &[&str] = &[
    "adapter",
    "adapters",
    "api",
    "client",
    "clients",
    "gateway",
    "http",
    "request",
    "transport",
];

pub(super) const CONFIG_BOUNDARY_WORDS: &[&str] =
    &[WORD_CONFIG, "configuration", "env", "setting", "settings"];

pub(super) const FS_BOUNDARY_WORDS: &[&str] = &[
    "dao",
    "file",
    "filesystem",
    "persistence",
    "repository",
    "storage",
    "store",
];

pub(super) const LOG_BOUNDARY_WORDS: &[&str] =
    &["log", "logger", "logging", "telemetry", "trace", "tracing"];
