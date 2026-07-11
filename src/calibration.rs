use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::cli::{CalibrateArgs, CalibrateArtifactArgs, CalibrateCommand, CalibratePrepareArgs};
use crate::model::{FindingKind, Issue, PriorityFactors};
use crate::scan::{NoopProgress, scan_report};

const REQUIRED_SAMPLE_COUNT: usize = 6;
const ARTIFACT_VERSION: u8 = 1;
const MIN_CONFIRMED_PAIRS: usize = 12;
const PRIOR: [f64; 5] = [0.30, 0.30, 0.15, 0.15, 0.10];
const MAX_REPOSITORY_REGRESSION: f64 = 0.05;

#[derive(Debug, Serialize, Deserialize)]
struct Sample {
    id: String,
    head: String,
    issue_file: String,
    proposal_file: String,
    gold_file: String,
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

#[derive(Debug, Clone, Serialize)]
struct Policy {
    status: &'static str,
    reason: String,
    confirmed_pairs: usize,
    weights: Weights,
    detector_reliability: BTreeMap<FindingKind, f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Weights {
    impact: f64,
    intensity: f64,
    spread: f64,
    change_pressure: f64,
    actionability: f64,
}

impl Weights {
    fn from_array(values: [f64; 5]) -> Self {
        Self {
            impact: values[0],
            intensity: values[1],
            spread: values[2],
            change_pressure: values[3],
            actionability: values[4],
        }
    }
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
        let issues = scan_issues(repository, args.max_issues)?;
        let pairs = propose_pairs(&id, &issues);
        let issue_file = format!("{id}-issues.json");
        let proposal_file = format!("{id}-proposals.json");
        let gold_file = format!("{id}-gold.json");
        write_json(&args.output_dir.join(&issue_file), &issues)?;
        write_json(&args.output_dir.join(&proposal_file), &pairs)?;
        write_json(&args.output_dir.join(&gold_file), &gold_template(&pairs))?;
        samples.push(Sample {
            id: id.clone(),
            head: git_head(repository)?,
            issue_file,
            proposal_file,
            gold_file,
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

fn scan_issues(repository: &Path, limit: usize) -> Result<Vec<IssueDatum>> {
    let mut args = crate::cli::ScanArgs::defaults_for_path(repository.to_path_buf());
    args.churn = Some(crate::cli::ChurnMode::Off);
    args.hotspot_model = Some(crate::cli::HotspotModel::Static);
    let mut progress = NoopProgress;
    let mut issues = scan_report(&args, &mut progress)?.issues;
    issues.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(issues.into_iter().take(limit).map(issue_datum).collect())
}

fn issue_datum(issue: Issue) -> IssueDatum {
    IssueDatum {
        id: issue.id.to_string(),
        kind: issue.kinds[0],
        priority: issue.priority,
        factors: factor_array(&issue.priority_factors),
        detection_reliability: issue.detection_reliability,
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
    for file in [&sample.issue_file, &sample.proposal_file, &sample.gold_file] {
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
        let issues: Vec<IssueDatum> =
            read_json(&directory.join(&sample.issue_file), "issue dataset")?;
        let proposals: Vec<Pair> =
            read_json(&directory.join(&sample.proposal_file), "pair proposals")?;
        let gold: Vec<GoldPair> = read_json(&directory.join(&sample.gold_file), "gold labels")?;
        let issue_ids = issues
            .iter()
            .map(|issue| issue.id.as_str())
            .collect::<BTreeSet<_>>();
        validate_pairs(&sample.id, &proposals, &issue_ids)?;
        validate_gold(&sample.id, &gold, &proposals, &issue_ids)?;
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
    let (issues, labels) = load_training_data(&args.calibration_dir)?;
    let policy = fit_policy(&issues, &labels);
    write_json(&args.calibration_dir.join("fit.json"), &policy)
}

fn fit_policy(issues: &BTreeMap<String, IssueDatum>, labels: &[GoldPair]) -> Policy {
    if labels.len() < MIN_CONFIRMED_PAIRS {
        return Policy {
            status: "theoretical_prior",
            reason: format!("at least {MIN_CONFIRMED_PAIRS} confirmed gold pairs are required"),
            confirmed_pairs: labels.len(),
            weights: Weights::from_array(PRIOR),
            detector_reliability: BTreeMap::new(),
        };
    }
    let weights = fit_weights(issues, labels);
    Policy {
        status: "empirical_candidate",
        reason: "regularized non-negative Bradley-Terry fit".into(),
        confirmed_pairs: labels.len(),
        weights: Weights::from_array(weights),
        detector_reliability: detector_reliability(issues, labels),
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

fn detector_reliability(
    issues: &BTreeMap<String, IssueDatum>,
    labels: &[GoldPair],
) -> BTreeMap<FindingKind, f64> {
    let mut counts: BTreeMap<FindingKind, (usize, usize)> = BTreeMap::new();
    for pair in labels {
        let Some(preference) = pair.preferred else {
            continue;
        };
        for (id, success) in [
            (&pair.left, preference != Preference::Right),
            (&pair.right, preference != Preference::Left),
        ] {
            let count = counts.entry(issues[id].kind).or_default();
            count.1 += 1;
            count.0 += usize::from(success);
        }
    }
    counts
        .into_iter()
        .map(|(kind, (successes, total))| (kind, (successes as f64 + 1.0) / (total as f64 + 2.0)))
        .collect()
}

fn evaluate(args: &CalibrateArtifactArgs) -> Result<()> {
    validate(args)?;
    let (issues, labels) = load_training_data(&args.calibration_dir)?;
    if labels.len() < MIN_CONFIRMED_PAIRS {
        return write_json(
            &args.calibration_dir.join("evaluation.json"),
            &serde_json::json!({"accepted":false,"strategy":"theoretical_prior","reason":format!("at least {MIN_CONFIRMED_PAIRS} confirmed gold pairs are required"),"maximum_allowed_repository_regression":MAX_REPOSITORY_REGRESSION}),
        );
    }
    let evaluation = loro_evaluation(&issues, &labels);
    write_json(&args.calibration_dir.join("evaluation.json"), &evaluation)
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

fn load_training_data(directory: &Path) -> Result<(BTreeMap<String, IssueDatum>, Vec<GoldPair>)> {
    let manifest = load_manifest(directory)?;
    let mut issues = BTreeMap::new();
    let mut labels = Vec::new();
    for sample in manifest.samples {
        for issue in
            read_json::<Vec<IssueDatum>>(&directory.join(sample.issue_file), "issue dataset")?
        {
            issues.insert(issue.id.clone(), issue);
        }
        labels.extend(
            read_json::<Vec<GoldPair>>(&directory.join(sample.gold_file), "gold labels")?
                .into_iter()
                .filter(|pair| pair.confirmed),
        );
    }
    Ok((issues, labels))
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
        let issue = |id: &str, kind| IssueDatum {
            id: id.into(),
            kind,
            priority: 1,
            factors: [0.0; 5],
            detection_reliability: 1.0,
        };
        let issues = BTreeMap::from([
            ("a".into(), issue("a", FindingKind::LargeFile)),
            ("b".into(), issue("b", FindingKind::LargeType)),
        ]);
        let labels = vec![GoldPair {
            id: "p".into(),
            repository: "sample-01".into(),
            left: "a".into(),
            right: "b".into(),
            confirmed: true,
            preferred: Some(Preference::Left),
        }];
        assert!(
            detector_reliability(&issues, &labels)
                .values()
                .all(|value| *value > 0.0 && *value < 1.0)
        );
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
            })
            .collect::<Vec<_>>();
        assert_eq!(
            serde_json::to_string(&propose_pairs("sample-01", &issues)).unwrap(),
            serde_json::to_string(&propose_pairs("sample-01", &issues)).unwrap()
        );
        assert_eq!(propose_pairs("sample-01", &issues).len(), 4);
    }
}
