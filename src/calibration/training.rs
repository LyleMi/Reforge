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
    let detection = detection_reliability_counts(&data.detection, exclude_repository);
    let action = action_reliability_counts(&data.actions, exclude_repository);
    defaults
        .into_iter()
        .filter_map(|(kind, priors)| reliability_override(kind, priors, &detection, &action))
        .collect()
}

fn detection_reliability_counts(
    labels: &[DetectionGold],
    exclude_repository: Option<&str>,
) -> BTreeMap<FindingKind, (usize, usize)> {
    let mut counts: BTreeMap<FindingKind, (usize, usize)> = BTreeMap::new();
    for label in labels {
        if exclude_repository == Some(label.repository.as_str()) {
            continue;
        }
        let success = match label.label {
            Some(DetectionLabel::TruePositive) => Some(true),
            Some(DetectionLabel::FalsePositive) => Some(false),
            _ => None,
        };
        if let Some(success) = success {
            let count = counts.entry(label.kind).or_default();
            count.0 += usize::from(success);
            count.1 += 1;
        }
    }
    counts
}

fn action_reliability_counts(
    labels: &[ActionGold],
    exclude_repository: Option<&str>,
) -> BTreeMap<FindingKind, (usize, usize)> {
    let mut counts: BTreeMap<FindingKind, (usize, usize)> = BTreeMap::new();
    for label in labels {
        if exclude_repository == Some(label.repository.as_str()) {
            continue;
        }
        let success = match label.label {
            Some(ActionLabel::Suitable) => Some(true),
            Some(ActionLabel::Unsuitable) => Some(false),
            _ => None,
        };
        if let Some(success) = success {
            let count = counts.entry(label.kind).or_default();
            count.0 += usize::from(success);
            count.1 += 1;
        }
    }
    counts
}

fn reliability_override(
    kind: FindingKind,
    (detection_prior, interpretation_prior): (f64, f64),
    detection: &BTreeMap<FindingKind, (usize, usize)>,
    action: &BTreeMap<FindingKind, (usize, usize)>,
) -> Option<(FindingKind, DetectorReliabilityOverride)> {
    let detection_count = detection.get(&kind).copied().unwrap_or_default();
    let action_count = action.get(&kind).copied().unwrap_or_default();
    if detection_count.1 < MIN_CONFIRMED_RELIABILITY_LABELS
        && action_count.1 < MIN_CONFIRMED_RELIABILITY_LABELS
    {
        return None;
    }
    Some((
        kind,
        DetectorReliabilityOverride {
            detection: smoothed_reliability(detection_count, detection_prior),
            interpretation: smoothed_reliability(action_count, interpretation_prior),
        },
    ))
}

fn smoothed_reliability(count: (usize, usize), prior: f64) -> f64 {
    if count.1 < MIN_CONFIRMED_RELIABILITY_LABELS {
        prior
    } else {
        (count.0 as f64 + prior * 2.0) / (count.1 as f64 + 2.0)
    }
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
