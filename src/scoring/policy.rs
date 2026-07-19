use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::model::{
    EffectiveScoringPolicy, ScoringPolicy, ScoringPolicySource, policy_fingerprint,
};

pub(crate) fn load_scoring_policy(path: Option<&Path>) -> Result<EffectiveScoringPolicy> {
    let Some(path) = path else {
        return Ok(EffectiveScoringPolicy::builtin());
    };
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read scoring policy {}", path.display()))?;
    let policy: ScoringPolicy = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid scoring policy {}", path.display()))?;
    validate(&policy)?;
    Ok(EffectiveScoringPolicy {
        source: ScoringPolicySource::File,
        path: Some(path.to_string_lossy().replace('\\', "/")),
        policy_id: policy.policy_id,
        version: policy.version,
        fingerprint: policy.fingerprint,
        global_weights: policy.global_weights,
        detector_reliability: policy.detector_reliability,
    })
}

fn validate(policy: &ScoringPolicy) -> Result<()> {
    validate_identity(policy)?;
    validate_weights(policy.global_weights)?;
    validate_reliability(policy)?;
    validate_policy_fingerprint(policy)
}

fn validate_identity(policy: &ScoringPolicy) -> Result<()> {
    if policy.version != 1 {
        bail!(
            "unsupported scoring policy version {}; expected 1",
            policy.version
        );
    }
    if policy.status != "accepted" {
        bail!("scoring policy must have accepted status");
    }
    if policy.policy_id.trim().is_empty() {
        bail!("scoring policy ID must not be empty");
    }
    Ok(())
}

fn validate_weights(weights: crate::model::ScoringWeights) -> Result<()> {
    let values = [
        weights.impact,
        weights.intensity,
        weights.spread,
        weights.change_pressure,
        weights.actionability,
    ];
    if values
        .iter()
        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        bail!("scoring policy weights must be finite values in 0..=1");
    }
    if (weights.sum() - 1.0).abs() > 1e-9 {
        bail!("scoring policy weights must sum to 1");
    }
    Ok(())
}

fn validate_reliability(policy: &ScoringPolicy) -> Result<()> {
    for (kind, value) in &policy.detector_reliability {
        if !value.detection.is_finite()
            || !value.interpretation.is_finite()
            || !(0.0..=1.0).contains(&value.detection)
            || !(0.0..=1.0).contains(&value.interpretation)
        {
            bail!("scoring policy reliability for {kind:?} must be in 0..=1");
        }
    }
    Ok(())
}

fn validate_policy_fingerprint(policy: &ScoringPolicy) -> Result<()> {
    let weights = policy.global_weights;
    let expected = policy_fingerprint(
        &policy.policy_id,
        policy.version,
        weights,
        &policy.detector_reliability,
    );
    if policy.fingerprint != expected {
        bail!("scoring policy fingerprint mismatch: expected {expected}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ScoringWeights, policy_fingerprint};
    use std::collections::BTreeMap;

    fn policy(status: &str) -> ScoringPolicy {
        let weights = ScoringWeights::default();
        let overrides = BTreeMap::new();
        ScoringPolicy {
            policy_id: "test".into(),
            version: 1,
            status: status.into(),
            fingerprint: policy_fingerprint("test", 1, weights, &overrides),
            global_weights: weights,
            detector_reliability: overrides,
        }
    }

    #[test]
    fn runtime_accepts_only_accepted_policy_v1_with_matching_fingerprint() {
        assert!(validate(&policy("accepted")).is_ok());
        assert!(validate(&policy("candidate")).is_err());
        let mut wrong = policy("accepted");
        wrong.version = 2;
        assert!(validate(&wrong).is_err());
        let mut wrong = policy("accepted");
        wrong.fingerprint = "wrong".into();
        assert!(validate(&wrong).is_err());
    }

    #[test]
    fn policy_weights_must_be_a_simplex() {
        let mut value = policy("accepted");
        value.global_weights.impact = 0.31;
        value.fingerprint = policy_fingerprint(
            &value.policy_id,
            value.version,
            value.global_weights,
            &value.detector_reliability,
        );
        assert!(validate(&value).is_err());
    }
}
