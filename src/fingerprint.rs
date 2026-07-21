use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(crate) mod provenance {
    include!("provenance.rs");
}

pub(crate) fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_json(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        _ => value.clone(),
    }
}

pub(crate) fn fingerprint_json(value: &Value) -> String {
    let bytes = serde_json::to_vec(&canonical_json(value)).expect("JSON value should serialize");
    hash_bytes(&bytes)
}

pub(crate) fn fingerprint<T: Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(fingerprint_json(&serde_json::to_value(value)?))
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256-{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_fingerprint_ignores_object_key_order() {
        let left = serde_json::json!({"b": 2, "a": {"d": 4, "c": 3}});
        let right = serde_json::json!({"a": {"c": 3, "d": 4}, "b": 2});
        assert_eq!(fingerprint_json(&left), fingerprint_json(&right));
        assert!(fingerprint_json(&left).starts_with("sha256-"));
    }
}
