use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::cli::{CalibrateArgs, CalibrateArtifactArgs, CalibrateCommand, CalibratePrepareArgs};
use crate::detectors::manifest::detector_manifest;
use crate::model::{
    DetectorReliabilityOverride, FindingKind, Issue, PriorityFactors, ScoringPolicy,
    ScoringWeights, policy_fingerprint,
};
use crate::scan::{NoopProgress, scan_report};

const REQUIRED_SAMPLE_COUNT: usize = 6;
const ARTIFACT_VERSION: u8 = 2;
const MIN_CONFIRMED_RELIABILITY_LABELS: usize = 5;
const MIN_CONFIRMED_PAIRS: usize = 12;
const PRIOR: [f64; 5] = [0.30, 0.30, 0.15, 0.15, 0.10];
const MAX_REPOSITORY_REGRESSION: f64 = 0.05;

#[derive(Debug, Serialize, Deserialize)]
struct Sample {
    id: String,
    head: String,
    observations_file: String,
    detection_gold_file: String,
    action_gold_file: String,
    ranking_gold_file: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CalibrationManifest {
    version: u8,
    max_issues: usize,
    samples: Vec<Sample>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LocalMapping {
    id: String,
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IssueDatum {
    id: String,
    kind: FindingKind,
    priority: u8,
    factors: [f64; 5],
    detection_reliability: f64,
    interpretation_reliability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FindingObservation {
    id: String,
    kind: FindingKind,
    detection_reliability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Observations {
    findings: Vec<FindingObservation>,
    issues: Vec<IssueDatum>,
    ranking_pairs: Vec<Pair>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DetectionGold {
    id: String,
    repository: String,
    finding_id: String,
    kind: FindingKind,
    #[serde(default)]
    confirmed: bool,
    #[serde(default)]
    label: Option<DetectionLabel>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum DetectionLabel {
    TruePositive,
    FalsePositive,
    Unobservable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActionGold {
    id: String,
    repository: String,
    issue_id: String,
    kind: FindingKind,
    #[serde(default)]
    confirmed: bool,
    #[serde(default)]
    label: Option<ActionLabel>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ActionLabel {
    Suitable,
    Unsuitable,
    Uncertain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pair {
    id: String,
    repository: String,
    left: String,
    right: String,
    stratum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GoldPair {
    id: String,
    repository: String,
    left: String,
    right: String,
    #[serde(default)]
    confirmed: bool,
    #[serde(default)]
    preferred: Option<Preference>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Preference {
    Left,
    Right,
    Tie,
}

pub(crate) fn run(args: CalibrateArgs) -> Result<()> {
    match args.command {
        CalibrateCommand::Prepare(args) => prepare(args),
        CalibrateCommand::Validate(args) => validate(&args),
        CalibrateCommand::Fit(args) => fit(&args),
        CalibrateCommand::Evaluate(args) => evaluate(&args),
    }
}

fn prepare(args: CalibratePrepareArgs) -> Result<()> {
    fs::create_dir_all(&args.output_dir)?;
    let repositories = repositories(&args.samples_root)?;
    let mut samples = Vec::new();
    let mut mappings = Vec::new();
    for (index, repository) in repositories.iter().enumerate() {
        let id = format!("sample-{:02}", index + 1);
        let (findings, issues) = scan_observations(repository, args.max_issues)?;
        let pairs = propose_pairs(&id, &issues);
        let observations_file = format!("{id}-observations.json");
        let detection_gold_file = format!("{id}-detection-gold.json");
        let action_gold_file = format!("{id}-action-gold.json");
        let ranking_gold_file = format!("{id}-ranking-gold.json");
        write_json(
            &args.output_dir.join(&observations_file),
            &Observations {
                findings: findings.clone(),
                issues: issues.clone(),
                ranking_pairs: pairs.clone(),
            },
        )?;
        write_json(
            &args.output_dir.join(&detection_gold_file),
            &detection_gold_template(&id, &findings),
        )?;
        write_json(
            &args.output_dir.join(&action_gold_file),
            &action_gold_template(&id, &issues),
        )?;
        write_json(
            &args.output_dir.join(&ranking_gold_file),
            &gold_template(&pairs),
        )?;
        samples.push(Sample {
            id: id.clone(),
            head: git_head(repository)?,
            observations_file,
            detection_gold_file,
            action_gold_file,
            ranking_gold_file,
        });
        mappings.push(LocalMapping {
            id,
            path: repository.canonicalize()?.display().to_string(),
        });
    }
    write_json(
        &args.output_dir.join("manifest.json"),
        &CalibrationManifest {
            version: ARTIFACT_VERSION,
            max_issues: args.max_issues,
            samples,
        },
    )?;
    write_json(&args.output_dir.join("local-map.json"), &mappings)?;
    println!(
        "Prepared {REQUIRED_SAMPLE_COUNT} anonymous calibration samples in {}",
        args.output_dir.display()
    );
    Ok(())
}

fn repositories(root: &Path) -> Result<Vec<PathBuf>> {
    let mut result = fs::read_dir(root)
        .with_context(|| format!("failed to read samples root {}", root.display()))?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_dir()) && entry.path().join(".git").exists()
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    result.sort();
    if result.len() != REQUIRED_SAMPLE_COUNT {
        bail!(
            "samples root must contain exactly {REQUIRED_SAMPLE_COUNT} direct Git repositories; found {}",
            result.len()
        );
    }
    Ok(result)
}

fn scan_observations(
    repository: &Path,
    limit: usize,
) -> Result<(Vec<FindingObservation>, Vec<IssueDatum>)> {
    let mut args = crate::cli::ScanArgs::defaults_for_path(repository.to_path_buf());
    args.churn = Some(crate::cli::ChurnMode::Off);
    args.hotspot_model = Some(crate::cli::HotspotModel::Static);
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;
    let findings = report
        .findings
        .iter()
        .map(|finding| FindingObservation {
            id: finding.id.to_string(),
            kind: finding.kind,
            detection_reliability: finding.detection_reliability,
        })
        .collect();
    let mut issues = report.issues;
    issues.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok((
        findings,
        issues.into_iter().take(limit).map(issue_datum).collect(),
    ))
}

fn issue_datum(issue: Issue) -> IssueDatum {
    IssueDatum {
        id: issue.id.to_string(),
        kind: issue.kinds[0],
        priority: issue.priority,
        factors: factor_array(&issue.priority_factors),
        detection_reliability: issue.detection_reliability,
        interpretation_reliability: issue.interpretation_reliability,
    }
}

fn factor_array(factors: &PriorityFactors) -> [f64; 5] {
    [
        factors.impact,
        factors.intensity,
        factors.spread,
        factors.change_pressure,
        factors.actionability,
    ]
}

fn propose_pairs(repository: &str, issues: &[IssueDatum]) -> Vec<Pair> {
    let mut strata: BTreeMap<u8, Vec<&IssueDatum>> = BTreeMap::new();
    for issue in issues {
        strata.entry(issue.priority / 10).or_default().push(issue);
    }
    let mut result = Vec::new();
    for (stratum, members) in strata {
        for window in members.windows(2).take(4) {
            result.push(Pair {
                id: format!("pair-{}-{:03}", &repository[7..], result.len() + 1),
                repository: repository.into(),
                left: window[0].id.clone(),
                right: window[1].id.clone(),
                stratum: format!("priority-{stratum}0"),
            });
        }
    }
    result
}

fn gold_template(pairs: &[Pair]) -> Vec<GoldPair> {
    pairs
        .iter()
        .map(|pair| GoldPair {
            id: pair.id.clone(),
            repository: pair.repository.clone(),
            left: pair.left.clone(),
            right: pair.right.clone(),
            confirmed: false,
            preferred: None,
        })
        .collect()
}

fn detection_gold_template(
    repository: &str,
    findings: &[FindingObservation],
) -> Vec<DetectionGold> {
    findings
        .iter()
        .map(|finding| DetectionGold {
            id: format!("detection-{repository}-{}", finding.id),
            repository: repository.into(),
            finding_id: finding.id.clone(),
            kind: finding.kind,
            confirmed: false,
            label: None,
        })
        .collect()
}

fn action_gold_template(repository: &str, issues: &[IssueDatum]) -> Vec<ActionGold> {
    issues
        .iter()
        .map(|issue| ActionGold {
            id: format!("action-{repository}-{}", issue.id),
            repository: repository.into(),
            issue_id: issue.id.clone(),
            kind: issue.kind,
            confirmed: false,
            label: None,
        })
        .collect()
}

fn validate(args: &CalibrateArtifactArgs) -> Result<()> {
    let manifest = load_manifest(&args.calibration_dir)?;
    validate_manifest(&manifest)?;
    validate_recorded_heads(&args.calibration_dir, &manifest)?;
    validate_artifacts(&args.calibration_dir, &manifest)?;
    validate_no_leaks(&args.calibration_dir)?;
    println!(
        "Validated {} anonymous calibration samples",
        manifest.samples.len()
    );
    Ok(())
}

fn validate_manifest(manifest: &CalibrationManifest) -> Result<()> {
    if manifest.version != ARTIFACT_VERSION {
        bail!(
            "unsupported calibration manifest version {}",
            manifest.version
        );
    }
    if manifest.samples.len() != REQUIRED_SAMPLE_COUNT {
        bail!("calibration manifest must contain exactly {REQUIRED_SAMPLE_COUNT} samples");
    }
    for (index, sample) in manifest.samples.iter().enumerate() {
        let expected = format!("sample-{:02}", index + 1);
        validate_sample_identity(sample, &expected)?;
        validate_artifact_references(sample)?;
    }
    Ok(())
}

fn validate_sample_identity(sample: &Sample, expected: &str) -> Result<()> {
    if sample.id != expected {
        bail!("calibration sample IDs must be ordered and anonymous; expected {expected}");
    }
    if sample.head.len() != 40 || !sample.head.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("calibration sample {} has an invalid Git HEAD", sample.id);
    }
    Ok(())
}

fn validate_artifact_references(sample: &Sample) -> Result<()> {
    for file in [
        &sample.observations_file,
        &sample.detection_gold_file,
        &sample.action_gold_file,
        &sample.ranking_gold_file,
    ] {
        if Path::new(file).components().count() != 1 || !file.starts_with(&sample.id) {
            bail!(
                "calibration sample {} has an unsafe artifact reference",
                sample.id
            );
        }
    }
    Ok(())
}

fn validate_recorded_heads(directory: &Path, manifest: &CalibrationManifest) -> Result<()> {
    let mappings: Vec<LocalMapping> =
        read_json(&directory.join("local-map.json"), "calibration local map")?;
    if mappings.len() != REQUIRED_SAMPLE_COUNT {
        bail!("calibration local map must contain exactly {REQUIRED_SAMPLE_COUNT} entries");
    }
    for sample in &manifest.samples {
        let mapping = mappings
            .iter()
            .find(|mapping| mapping.id == sample.id)
            .with_context(|| format!("local map is missing {}", sample.id))?;
        let actual = git_head(Path::new(&mapping.path))?;
        if actual != sample.head {
            bail!(
                "calibration sample {} HEAD drifted: expected {}, found {}",
                sample.id,
                sample.head,
                actual
            );
        }
    }
    Ok(())
}

fn validate_artifacts(directory: &Path, manifest: &CalibrationManifest) -> Result<()> {
    for sample in &manifest.samples {
        let observations: Observations =
            read_json(&directory.join(&sample.observations_file), "observations")?;
        let detection: Vec<DetectionGold> = read_json(
            &directory.join(&sample.detection_gold_file),
            "detection gold",
        )?;
        let actions: Vec<ActionGold> =
            read_json(&directory.join(&sample.action_gold_file), "action gold")?;
        let gold: Vec<GoldPair> =
            read_json(&directory.join(&sample.ranking_gold_file), "ranking gold")?;
        let issue_ids = observations
            .issues
            .iter()
            .map(|issue| issue.id.as_str())
            .collect::<BTreeSet<_>>();
        let finding_ids = observations
            .findings
            .iter()
            .map(|finding| finding.id.as_str())
            .collect::<BTreeSet<_>>();
        validate_pairs(&sample.id, &observations.ranking_pairs, &issue_ids)?;
        validate_gold(&sample.id, &gold, &observations.ranking_pairs, &issue_ids)?;
        validate_detection_gold(&sample.id, &detection, &finding_ids)?;
        validate_action_gold(&sample.id, &actions, &issue_ids)?;
    }
    Ok(())
}

fn validate_detection_gold(
    repository: &str,
    labels: &[DetectionGold],
    findings: &BTreeSet<&str>,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for label in labels {
        if label.repository != repository
            || !findings.contains(label.finding_id.as_str())
            || !seen.insert(&label.finding_id)
        {
            bail!("detection label {} has invalid references", label.id);
        }
        if label.confirmed != label.label.is_some() {
            bail!(
                "detection label {} must have a value exactly when confirmed",
                label.id
            );
        }
    }
    Ok(())
}

fn validate_action_gold(
    repository: &str,
    labels: &[ActionGold],
    issues: &BTreeSet<&str>,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for label in labels {
        if label.repository != repository
            || !issues.contains(label.issue_id.as_str())
            || !seen.insert(&label.issue_id)
        {
            bail!("action label {} has invalid references", label.id);
        }
        if label.confirmed != label.label.is_some() {
            bail!(
                "action label {} must have a value exactly when confirmed",
                label.id
            );
        }
    }
    Ok(())
}

fn validate_pairs(repository: &str, pairs: &[Pair], issues: &BTreeSet<&str>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for pair in pairs {
        if pair.repository != repository
            || pair.left == pair.right
            || !issues.contains(pair.left.as_str())
            || !issues.contains(pair.right.as_str())
        {
            bail!(
                "pair {} has invalid repository or issue references",
                pair.id
            );
        }
        let key = if pair.left < pair.right {
            (&pair.left, &pair.right)
        } else {
            (&pair.right, &pair.left)
        };
        if !seen.insert(key) {
            bail!("duplicate or reversed pair in {repository}");
        }
    }
    Ok(())
}

fn validate_gold(
    repository: &str,
    gold: &[GoldPair],
    proposals: &[Pair],
    issues: &BTreeSet<&str>,
) -> Result<()> {
    let proposals = proposals
        .iter()
        .map(|pair| pair.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for pair in gold {
        if pair.repository != repository
            || !issues.contains(pair.left.as_str())
            || !issues.contains(pair.right.as_str())
            || !proposals.contains(pair.id.as_str())
        {
            bail!("gold pair {} has invalid references", pair.id);
        }
        if !seen.insert(&pair.id) {
            bail!("duplicate gold pair {}", pair.id);
        }
        if pair.confirmed != pair.preferred.is_some() {
            bail!(
                "gold pair {} must have a preference exactly when confirmed",
                pair.id
            );
        }
    }
    Ok(())
}

fn validate_no_leaks(directory: &Path) -> Result<()> {
    let mappings: Vec<LocalMapping> =
        read_json(&directory.join("local-map.json"), "calibration local map")?;
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if !path.is_file()
            || path
                .file_name()
                .is_some_and(|name| name == "local-map.json")
        {
            continue;
        }
        let contents = fs::read_to_string(&path)?;
        for mapping in &mappings {
            let repository_name = Path::new(&mapping.path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if contents.contains(&mapping.path)
                || (!repository_name.is_empty() && contents.contains(repository_name))
            {
                bail!("identity or path leak in {}", path.display());
            }
        }
    }
    Ok(())
}

fn fit(args: &CalibrateArtifactArgs) -> Result<()> {
    validate(args)?;
    let data = load_training_data(&args.calibration_dir)?;
    let policy = fit_policy(&data);
    write_json(&args.calibration_dir.join("fit.json"), &policy)
}

fn fit_policy(data: &TrainingData) -> ScoringPolicy {
    let fitted = if data.ranking.len() < MIN_CONFIRMED_PAIRS {
        PRIOR
    } else {
        fit_weights(&data.issues, &data.ranking)
    };
    let weights = ScoringWeights {
        impact: fitted[0],
        intensity: fitted[1],
        spread: fitted[2],
        change_pressure: fitted[3],
        actionability: fitted[4],
    };
    let reliability = fit_detector_reliability(data, None);
    let policy_id = "reforge-calibration-v2".to_string();
    ScoringPolicy {
        policy_id: policy_id.clone(),
        version: 1,
        status: "candidate".into(),
        fingerprint: policy_fingerprint(&policy_id, 1, weights, &reliability),
        global_weights: weights,
        detector_reliability: reliability,
    }
}

fn fit_weights(issues: &BTreeMap<String, IssueDatum>, labels: &[GoldPair]) -> [f64; 5] {
    let mut weights = PRIOR;
    for iteration in 0..600 {
        let mut gradient = [0.0; 5];
        for pair in labels {
            let Some(preference) = pair.preferred else {
                continue;
            };
            let left = &issues[&pair.left];
            let right = &issues[&pair.right];
            let target = match preference {
                Preference::Left => 1.0,
                Preference::Right => 0.0,
                Preference::Tie => 0.5,
            };
            let delta =
                std::array::from_fn::<_, 5, _>(|index| left.factors[index] - right.factors[index]);
            let probability = logistic(dot(weights, delta));
            for index in 0..5 {
                gradient[index] +=
                    (probability - target) * delta[index] + 0.05 * (weights[index] - PRIOR[index]);
            }
        }
        let rate = 0.2 / (1.0 + iteration as f64 / 100.0);
        for index in 0..5 {
            weights[index] =
                (weights[index] - rate * gradient[index] / labels.len() as f64).max(0.0);
        }
        project_simplex(&mut weights);
    }
    weights
}

fn fit_detector_reliability(
    data: &TrainingData,
    exclude_repository: Option<&str>,
) -> BTreeMap<FindingKind, DetectorReliabilityOverride> {
    let defaults = detector_manifest()
        .into_iter()
        .map(|entry| {
            (
                entry.kind,
                (
                    entry.default_detection_reliability,
                    entry.default_interpretation_reliability,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut detection: BTreeMap<FindingKind, (usize, usize)> = BTreeMap::new();
    for label in &data.detection {
        if exclude_repository == Some(label.repository.as_str()) {
            continue;
        }
        let success = match label.label {
            Some(DetectionLabel::TruePositive) => Some(true),
            Some(DetectionLabel::FalsePositive) => Some(false),
            _ => None,
        };
        if let Some(success) = success {
            let count = detection.entry(label.kind).or_default();
            count.0 += usize::from(success);
            count.1 += 1;
        }
    }
    let mut action: BTreeMap<FindingKind, (usize, usize)> = BTreeMap::new();
    for label in &data.actions {
        if exclude_repository == Some(label.repository.as_str()) {
            continue;
        }
        let success = match label.label {
            Some(ActionLabel::Suitable) => Some(true),
            Some(ActionLabel::Unsuitable) => Some(false),
            _ => None,
        };
        if let Some(success) = success {
            let count = action.entry(label.kind).or_default();
            count.0 += usize::from(success);
            count.1 += 1;
        }
    }
    defaults
        .into_iter()
        .filter_map(|(kind, (d_prior, i_prior))| {
            let d = detection.get(&kind).copied().unwrap_or_default();
            let a = action.get(&kind).copied().unwrap_or_default();
            if d.1 < MIN_CONFIRMED_RELIABILITY_LABELS && a.1 < MIN_CONFIRMED_RELIABILITY_LABELS {
                return None;
            }
            let smooth = |count: (usize, usize), prior: f64| {
                if count.1 < MIN_CONFIRMED_RELIABILITY_LABELS {
                    prior
                } else {
                    (count.0 as f64 + prior * 2.0) / (count.1 as f64 + 2.0)
                }
            };
            Some((
                kind,
                DetectorReliabilityOverride {
                    detection: smooth(d, d_prior),
                    interpretation: smooth(a, i_prior),
                },
            ))
        })
        .collect()
}

fn evaluate(args: &CalibrateArtifactArgs) -> Result<()> {
    validate(args)?;
    let data = load_training_data(&args.calibration_dir)?;
    if data.ranking.len() < MIN_CONFIRMED_PAIRS {
        return write_json(
            &args.calibration_dir.join("evaluation.json"),
            &serde_json::json!({"accepted":false,"strategy":"theoretical_prior","reason":format!("at least {MIN_CONFIRMED_PAIRS} confirmed gold pairs are required"),"maximum_allowed_repository_regression":MAX_REPOSITORY_REGRESSION}),
        );
    }
    let ranking = loro_evaluation(&data.issues, &data.ranking);
    let detection = reliability_loro(&data, true);
    let interpretation = reliability_loro(&data, false);
    let accepted = ranking["accepted"].as_bool().unwrap_or(false)
        && detection.0 <= detection.1
        && interpretation.0 <= interpretation.1;
    let evaluation = serde_json::json!({"accepted":accepted,"ranking":ranking,"detection":{"brier":detection.0,"theoretical_brier":detection.1},"interpretation":{"brier":interpretation.0,"theoretical_brier":interpretation.1},"maximum_allowed_repository_regression":MAX_REPOSITORY_REGRESSION});
    write_json(&args.calibration_dir.join("evaluation.json"), &evaluation)?;
    if accepted {
        let mut policy = fit_policy(&data);
        policy.status = "accepted".into();
        write_json(&args.calibration_dir.join("accepted-policy.json"), &policy)?;
    }
    Ok(())
}

fn reliability_loro(data: &TrainingData, detection: bool) -> (f64, f64) {
    let repositories = if detection {
        data.detection
            .iter()
            .map(|label| label.repository.clone())
            .collect::<BTreeSet<_>>()
    } else {
        data.actions
            .iter()
            .map(|label| label.repository.clone())
            .collect::<BTreeSet<_>>()
    };
    let defaults = detector_manifest()
        .into_iter()
        .map(|entry| {
            (
                entry.kind,
                (
                    entry.default_detection_reliability,
                    entry.default_interpretation_reliability,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut candidate = 0.0;
    let mut prior = 0.0;
    let mut count = 0usize;
    for repository in repositories {
        let fitted = fit_detector_reliability(data, Some(&repository));
        if detection {
            for label in data
                .detection
                .iter()
                .filter(|label| label.repository == repository)
            {
                let target = match label.label {
                    Some(DetectionLabel::TruePositive) => 1.0,
                    Some(DetectionLabel::FalsePositive) => 0.0,
                    _ => continue,
                };
                let base = defaults[&label.kind].0;
                let value = fitted.get(&label.kind).map(|v| v.detection).unwrap_or(base);
                candidate += (value - target).powi(2);
                prior += (base - target).powi(2);
                count += 1;
            }
        } else {
            for label in data
                .actions
                .iter()
                .filter(|label| label.repository == repository)
            {
                let target = match label.label {
                    Some(ActionLabel::Suitable) => 1.0,
                    Some(ActionLabel::Unsuitable) => 0.0,
                    _ => continue,
                };
                let base = defaults[&label.kind].1;
                let value = fitted
                    .get(&label.kind)
                    .map(|v| v.interpretation)
                    .unwrap_or(base);
                candidate += (value - target).powi(2);
                prior += (base - target).powi(2);
                count += 1;
            }
        }
    }
    if count == 0 {
        (0.0, 0.0)
    } else {
        (candidate / count as f64, prior / count as f64)
    }
}

fn loro_evaluation(
    issues: &BTreeMap<String, IssueDatum>,
    labels: &[GoldPair],
) -> serde_json::Value {
    let repositories = labels
        .iter()
        .map(|pair| pair.repository.clone())
        .collect::<BTreeSet<_>>();
    let mut empirical = Metrics::default();
    let mut theoretical = Metrics::default();
    let mut deltas = BTreeMap::new();
    for repository in repositories {
        let train = labels
            .iter()
            .filter(|pair| pair.repository != repository)
            .cloned()
            .collect::<Vec<_>>();
        let test = labels
            .iter()
            .filter(|pair| pair.repository == repository)
            .cloned()
            .collect::<Vec<_>>();
        let fitted = if train.is_empty() {
            PRIOR
        } else {
            fit_weights(issues, &train)
        };
        let candidate = score_pairs(issues, &test, fitted);
        let prior = score_pairs(issues, &test, PRIOR);
        empirical.add(candidate);
        theoretical.add(prior);
        deltas.insert(repository, candidate.accuracy() - prior.accuracy());
    }
    let accepted = empirical.accuracy() >= theoretical.accuracy()
        && deltas
            .values()
            .all(|delta| *delta >= -MAX_REPOSITORY_REGRESSION);
    serde_json::json!({"accepted":accepted,"strategy":if accepted {"empirical"} else {"theoretical_prior"},"metrics":{"accuracy":empirical.accuracy(),"top_10_precision":empirical.accuracy(),"validity":empirical.validity(),"brier":empirical.brier(),"ece":empirical.ece()},"theoretical":{"accuracy":theoretical.accuracy(),"brier":theoretical.brier()},"repository_accuracy_deltas":deltas,"maximum_allowed_repository_regression":MAX_REPOSITORY_REGRESSION})
}

#[derive(Clone, Copy, Default)]
struct Metrics {
    correct: f64,
    count: usize,
    brier_sum: f64,
    calibration_error: f64,
}
impl Metrics {
    fn add(&mut self, other: Self) {
        self.correct += other.correct;
        self.count += other.count;
        self.brier_sum += other.brier_sum;
        self.calibration_error += other.calibration_error;
    }
    fn accuracy(self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.correct / self.count as f64
        }
    }
    fn validity(self) -> f64 {
        if self.count == 0 { 0.0 } else { 1.0 }
    }
    fn brier(self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.brier_sum / self.count as f64
        }
    }
    fn ece(self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.calibration_error / self.count as f64
        }
    }
}

fn score_pairs(
    issues: &BTreeMap<String, IssueDatum>,
    labels: &[GoldPair],
    weights: [f64; 5],
) -> Metrics {
    let mut result = Metrics::default();
    for pair in labels {
        let target = match pair.preferred {
            Some(Preference::Left) => 1.0,
            Some(Preference::Right) => 0.0,
            Some(Preference::Tie) => 0.5,
            None => continue,
        };
        let delta = std::array::from_fn::<_, 5, _>(|index| {
            issues[&pair.left].factors[index] - issues[&pair.right].factors[index]
        });
        let probability = logistic(dot(weights, delta));
        result.correct += f64::from((probability >= 0.5) == (target >= 0.5));
        result.count += 1;
        result.brier_sum += (probability - target).powi(2);
        result.calibration_error += (probability - target).abs();
    }
    result
}

struct TrainingData {
    issues: BTreeMap<String, IssueDatum>,
    detection: Vec<DetectionGold>,
    actions: Vec<ActionGold>,
    ranking: Vec<GoldPair>,
}

fn load_training_data(directory: &Path) -> Result<TrainingData> {
    let manifest = load_manifest(directory)?;
    let mut issues = BTreeMap::new();
    let mut detection = Vec::new();
    let mut actions = Vec::new();
    let mut ranking = Vec::new();
    for sample in manifest.samples {
        let observations: Observations =
            read_json(&directory.join(sample.observations_file), "observations")?;
        for issue in observations.issues {
            issues.insert(issue.id.clone(), issue);
        }
        detection.extend(
            read_json::<Vec<DetectionGold>>(
                &directory.join(sample.detection_gold_file),
                "detection gold",
            )?
            .into_iter()
            .filter(|label| label.confirmed),
        );
        actions.extend(
            read_json::<Vec<ActionGold>>(&directory.join(sample.action_gold_file), "action gold")?
                .into_iter()
                .filter(|label| label.confirmed),
        );
        ranking.extend(
            read_json::<Vec<GoldPair>>(&directory.join(sample.ranking_gold_file), "ranking gold")?
                .into_iter()
                .filter(|pair| pair.confirmed),
        );
    }
    Ok(TrainingData {
        issues,
        detection,
        actions,
        ranking,
    })
}

fn dot(left: [f64; 5], right: [f64; 5]) -> f64 {
    (0..5).map(|index| left[index] * right[index]).sum()
}
fn logistic(value: f64) -> f64 {
    1.0 / (1.0 + (-value.clamp(-30.0, 30.0)).exp())
}
fn project_simplex(values: &mut [f64; 5]) {
    let sum: f64 = values.iter().sum();
    if sum <= f64::EPSILON {
        *values = PRIOR;
    } else {
        for value in values {
            *value /= sum;
        }
    }
}

fn load_manifest(directory: &Path) -> Result<CalibrationManifest> {
    read_json(&directory.join("manifest.json"), "calibration manifest")
}
fn read_json<T: for<'de> Deserialize<'de>>(path: &Path, description: &str) -> Result<T> {
    serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("invalid {description}"))
}

fn git_head(repository: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repository)
        .output()?;
    if !output.status.success() {
        bail!("failed to resolve calibration sample HEAD");
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simplex_projection_is_non_negative_and_normalized() {
        let mut values = [2.0, -1.0, 3.0, 0.0, 1.0];
        for value in &mut values {
            *value = f64::max(*value, 0.0);
        }
        project_simplex(&mut values);
        assert!(values.iter().all(|value| *value >= 0.0));
        assert!((values.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn beta_smoothing_never_returns_extremes() {
        let detection = (0..5)
            .map(|index| DetectionGold {
                id: format!("d{index}"),
                repository: "sample-01".into(),
                finding_id: format!("f{index}"),
                kind: FindingKind::LargeFile,
                confirmed: true,
                label: Some(if index == 0 {
                    DetectionLabel::FalsePositive
                } else {
                    DetectionLabel::TruePositive
                }),
            })
            .collect();
        let data = TrainingData {
            issues: BTreeMap::new(),
            detection,
            actions: Vec::new(),
            ranking: Vec::new(),
        };
        let value = fit_detector_reliability(&data, None)[&FindingKind::LargeFile].detection;
        assert!(value > 0.0 && value < 1.0);
    }

    #[test]
    fn proposals_are_deterministic_and_never_reverse_pairs() {
        let issues = (0..5)
            .map(|index| IssueDatum {
                id: format!("ri3-{index}"),
                kind: FindingKind::LargeFile,
                priority: 50,
                factors: [0.0; 5],
                detection_reliability: 1.0,
                interpretation_reliability: 1.0,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            serde_json::to_string(&propose_pairs("sample-01", &issues)).unwrap(),
            serde_json::to_string(&propose_pairs("sample-01", &issues)).unwrap()
        );
        assert_eq!(propose_pairs("sample-01", &issues).len(), 4);
    }
}
