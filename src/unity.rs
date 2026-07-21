use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::cli::{ScanArgs, UnityMode};
use crate::evidence_analysis::FindingInput;
use crate::model::{
    Finding, FindingKind, FindingMetric, MetricId, RelatedLocation, UnityAssemblyEdge,
    UnityAssemblyNode, UnityCoverageEntry, UnityMetricManifestEntry, UnityProjectReport,
    UnityProjectStatus, UnityRawMetric, UnityReferenceProblem,
};

const ZERO_GUID: &str = "00000000000000000000000000000000";
const LIFECYCLE_METHODS: &[&str] = &[
    "Awake",
    "OnEnable",
    "Start",
    "FixedUpdate",
    "Update",
    "LateUpdate",
    "OnDisable",
    "OnDestroy",
    "OnApplicationPause",
    "OnApplicationQuit",
    "OnValidate",
    "Reset",
];

#[derive(Debug)]
pub(crate) struct UnityScan {
    pub report: UnityProjectReport,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AsmdefFile {
    name: String,
    #[serde(default)]
    references: Vec<String>,
    #[serde(default)]
    include_platforms: Vec<String>,
    #[serde(default)]
    optional_unity_references: Vec<String>,
}

#[derive(Debug)]
struct AssemblyRecord {
    node: UnityAssemblyNode,
    references: Vec<String>,
    guid: Option<String>,
}

pub(crate) fn scan_unity(root: &Path, args: &ScanArgs) -> Result<UnityScan> {
    let detected = root.join("ProjectSettings/ProjectVersion.txt").is_file();
    if args.unity == UnityMode::On && !detected {
        bail!(
            "Unity analysis was requested, but {} is not a Unity project root (missing ProjectSettings/ProjectVersion.txt)",
            root.display()
        );
    }
    if args.unity == UnityMode::Off {
        return Ok(UnityScan {
            report: UnityProjectReport {
                status: if detected {
                    UnityProjectStatus::Disabled
                } else {
                    UnityProjectStatus::NotDetected
                },
                ..Default::default()
            },
            findings: Vec::new(),
        });
    }
    if !detected {
        return Ok(UnityScan {
            report: UnityProjectReport::default(),
            findings: Vec::new(),
        });
    }

    let mut context = UnityContext::new(root, args);
    context.scan()?;
    Ok(UnityScan {
        report: context.report,
        findings: context.findings,
    })
}

struct UnityContext<'a> {
    root: &'a Path,
    args: &'a ScanArgs,
    report: UnityProjectReport,
    findings: Vec<Finding>,
    paths: Vec<PathBuf>,
    external_paths: Vec<PathBuf>,
    guid_paths: BTreeMap<String, Vec<String>>,
    assemblies: Vec<AssemblyRecord>,
    package_cache_present: bool,
}

include!("unity/context.rs");
include!("unity/source_analysis.rs");
include!("unity/tests.rs");
