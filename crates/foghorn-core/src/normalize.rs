use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Strip volatile fields before canonicalization.
/// Removes `extensions` (subgraph metadata, timing) and `headers`.
pub fn strip_volatile(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            map.remove("extensions");
            map.remove("headers");
            Value::Object(map)
        }
        other => other,
    }
}

/// JCS-style canonicalization (RFC 8785 subset).
/// Recursively sorts object keys lexicographically and serializes deterministically.
pub fn jcs_serialize(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                assert!(!f.is_nan() && !f.is_infinite(), "JCS: NaN/Infinity not allowed");
            }
            n.to_string()
        }
        Value::String(s) => serde_json::to_string(s).expect("string serialize"),
        Value::Array(arr) => {
            let parts: Vec<String> = arr.iter().map(jcs_serialize).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Object(map) => {
            let sorted: BTreeMap<&str, &Value> =
                map.iter().map(|(k, v)| (k.as_str(), v)).collect();
            let parts: Vec<String> = sorted
                .into_iter()
                .map(|(k, v)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).expect("key serialize"),
                        jcs_serialize(v)
                    )
                })
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

/// Compute SHA-256 of the canonicalized, stripped response.
/// Returns lowercase hex string.
pub fn hash_response(value: &Value) -> String {
    let canonical = jcs_serialize(value);
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}

/// Full normalize + hash pipeline for a raw GraphQL response body.
pub fn normalize_and_hash(raw: &str) -> anyhow::Result<String> {
    let value: Value = serde_json::from_str(raw)?;
    let stripped = strip_volatile(value);
    Ok(hash_response(&stripped))
}

/// Also return the stripped value alongside the hash (for diff storage).
pub fn normalize_and_hash_with_value(raw: &str) -> anyhow::Result<(String, Value)> {
    let value: Value = serde_json::from_str(raw)?;
    let stripped = strip_volatile(value);
    let hash = hash_response(&stripped);
    Ok((hash, stripped))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_order_deterministic() {
        let a = r#"{"data":{"pool":{"id":"0xabc","tvl":"1000"}}}"#;
        let b = r#"{"data":{"pool":{"tvl":"1000","id":"0xabc"}}}"#;
        assert_eq!(
            normalize_and_hash(a).unwrap(),
            normalize_and_hash(b).unwrap()
        );
    }

    #[test]
    fn test_extensions_stripped() {
        let a = r#"{"data":{"pool":{"id":"0xabc"}},"extensions":{"timing":1}}"#;
        let b = r#"{"data":{"pool":{"id":"0xabc"}}}"#;
        assert_eq!(
            normalize_and_hash(a).unwrap(),
            normalize_and_hash(b).unwrap()
        );
    }

    #[test]
    fn test_different_data_different_hash() {
        let a = r#"{"data":{"pool":{"tvl":"1000"}}}"#;
        let b = r#"{"data":{"pool":{"tvl":"2000"}}}"#;
        assert_ne!(
            normalize_and_hash(a).unwrap(),
            normalize_and_hash(b).unwrap()
        );
    }
}
