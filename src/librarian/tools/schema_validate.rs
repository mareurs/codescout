//! Thin wrapper around `jsonschema` for params validation. Centralized so
//! every augmentation entry point (initial seed, merge, refresh-commit) uses
//! the same compile-and-validate path with consistent error messages.

use anyhow::{anyhow, Result};
use serde_json::Value;

/// Validate `params` against a JSON Schema. Returns Err with a single-line
/// reason on failure (concatenating the first 3 errors when multiple).
pub fn validate(schema: &Value, params: &Value) -> Result<()> {
    let validator =
        jsonschema::validator_for(schema).map_err(|e| anyhow!("invalid params_schema: {e}"))?;
    let errors: Vec<String> = validator
        .iter_errors(params)
        .take(3)
        .map(|e| format!("{}: {}", e.instance_path, e))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(errors.join("; ")))
    }
}

/// Parse a stored JSON-Schema string and validate. Convenience for the
/// common "row.params_schema is `Option<String>`" case.
pub fn validate_against_stored(schema_text: &str, params: &Value) -> Result<()> {
    let schema: Value = serde_json::from_str(schema_text)
        .map_err(|e| anyhow!("stored params_schema is not valid JSON: {e}"))?;
    validate(&schema, params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema_required_int() -> Value {
        json!({
            "type": "object",
            "required": ["count"],
            "properties": { "count": { "type": "integer", "minimum": 0 } }
        })
    }

    #[test]
    fn validates_conforming_params() {
        validate(&schema_required_int(), &json!({"count": 3})).unwrap();
    }

    #[test]
    fn rejects_missing_required_key() {
        let err = validate(&schema_required_int(), &json!({})).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("count"));
    }

    #[test]
    fn rejects_wrong_type() {
        let err = validate(&schema_required_int(), &json!({"count": "five"})).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("integer")
                || err.to_string().to_lowercase().contains("type")
        );
    }

    #[test]
    fn rejects_below_minimum() {
        let err = validate(&schema_required_int(), &json!({"count": -1})).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("minimum") || err.to_string().contains("-1")
        );
    }

    #[test]
    fn rejects_invalid_schema() {
        let bad_schema = json!({"type": "not-a-real-type"});
        let err = validate(&bad_schema, &json!({})).unwrap_err();
        assert!(err.to_string().contains("invalid params_schema"));
    }

    #[test]
    fn validate_against_stored_parses_text() {
        let text = serde_json::to_string(&schema_required_int()).unwrap();
        validate_against_stored(&text, &json!({"count": 0})).unwrap();
        let err = validate_against_stored(&text, &json!({})).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("count"));
    }

    #[test]
    fn validate_against_stored_rejects_garbage_json() {
        let err = validate_against_stored("{not json}", &json!({})).unwrap_err();
        assert!(err.to_string().contains("not valid JSON"));
    }
}
