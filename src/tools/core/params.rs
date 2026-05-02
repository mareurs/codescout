//! Parameter-parsing helpers for tool input JSON values.

use super::types::RecoverableError;

/// Convenience: extract a required parameter from a JSON `Value`, returning
/// `RecoverableError` (not a fatal error) if it is missing.
pub fn require_param<'a>(
    input: &'a serde_json::Value,
    name: &str,
) -> anyhow::Result<&'a serde_json::Value> {
    input.get(name).ok_or_else(|| {
        RecoverableError::with_hint(
            format!("missing '{}' parameter", name),
            format!("Add the required '{}' parameter to the tool call.", name),
        )
        .into()
    })
}

/// Like `require_param`, but also checks common LLM aliases for the parameter.
/// If the canonical name isn't found, tries each alias in order.
/// Returns the value from whichever name matched first.
pub fn require_param_or<'a>(
    input: &'a serde_json::Value,
    name: &str,
    aliases: &[&str],
) -> anyhow::Result<&'a serde_json::Value> {
    if let Some(v) = input.get(name) {
        return Ok(v);
    }
    for alias in aliases {
        if let Some(v) = input.get(*alias) {
            return Ok(v);
        }
    }
    Err(RecoverableError::with_hint(
        format!("missing '{}' parameter", name),
        format!("Add the required '{}' parameter to the tool call.", name),
    )
    .into())
}

/// Like `require_str_param`, but also checks common LLM aliases.
pub fn require_str_param_or<'a>(
    input: &'a serde_json::Value,
    name: &str,
    aliases: &[&str],
) -> anyhow::Result<&'a str> {
    require_param_or(input, name, aliases)?
        .as_str()
        .ok_or_else(|| {
            RecoverableError::with_hint(
                format!("'{}' must be a string", name),
                format!("Provide '{}' as a string value.", name),
            )
            .into()
        })
}

/// Convenience: extract a required string parameter from a JSON `Value`.
pub fn require_str_param<'a>(input: &'a serde_json::Value, name: &str) -> anyhow::Result<&'a str> {
    require_param(input, name)?.as_str().ok_or_else(|| {
        RecoverableError::with_hint(
            format!("'{}' must be a string", name),
            format!("Provide '{}' as a string value.", name),
        )
        .into()
    })
}

/// Convenience: extract a required u64 parameter from a JSON `Value`.
pub fn require_u64_param(input: &serde_json::Value, name: &str) -> anyhow::Result<u64> {
    let val = require_param(input, name)?;
    // Accept both JSON numbers and string-encoded integers (LLMs sometimes quote them).
    if let Some(n) = val.as_u64() {
        return Ok(n);
    }
    if let Some(s) = val.as_str() {
        if let Ok(n) = s.trim().parse::<u64>() {
            return Ok(n);
        }
    }
    Err(RecoverableError::with_hint(
        format!("'{}' must be a non-negative integer", name),
        format!("Provide '{}' as a non-negative integer.", name),
    )
    .into())
}

/// Parse a boolean parameter from a JSON `Value`.
///
/// MCP clients (including Claude Code) may serialize boolean parameters as
/// JSON strings (`"true"` / `"false"`) rather than native JSON booleans.
/// This helper accepts both representations, defaulting to `false`.
pub fn parse_bool_param(val: &serde_json::Value) -> bool {
    val.as_bool()
        .or_else(|| val.as_str().and_then(|s| s.parse::<bool>().ok()))
        .unwrap_or(false)
}

/// Extract an optional boolean parameter with lenient coercion.
///
/// Returns `Some(bool)` if the parameter is present and coercible (native JSON
/// boolean or `"true"`/`"false"` string), `None` if absent or null. This is
/// the `Option`-returning counterpart to [`parse_bool_param`] — use it when
/// the caller needs to distinguish "not provided" from "explicitly false".
pub fn optional_bool_param(input: &serde_json::Value, name: &str) -> Option<bool> {
    let val = input.get(name)?;
    if val.is_null() {
        return None;
    }
    val.as_bool()
        .or_else(|| val.as_str().and_then(|s| s.parse::<bool>().ok()))
}

/// Extract an optional u64 parameter with lenient coercion.
///
/// Accepts both native JSON numbers and string-encoded integers (`"42"` → 42).
/// Returns `None` if the parameter is absent, null, or not coercible.
pub fn optional_u64_param(input: &serde_json::Value, name: &str) -> Option<u64> {
    let val = input.get(name)?;
    if val.is_null() {
        return None;
    }
    val.as_u64()
        .or_else(|| val.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
}

/// Extract an optional i64 parameter with lenient coercion.
///
/// Accepts both native JSON numbers and string-encoded integers (`"-1"` → -1).
/// Returns `None` if the parameter is absent, null, or not coercible.
pub fn optional_i64_param(input: &serde_json::Value, name: &str) -> Option<i64> {
    let val = input.get(name)?;
    if val.is_null() {
        return None;
    }
    val.as_i64()
        .or_else(|| val.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
}

/// Extract an optional f64 parameter with lenient coercion.
///
/// Accepts both native JSON numbers and string-encoded floats (`"0.5"` → 0.5).
/// Returns `None` if the parameter is absent, null, or not coercible.
pub fn optional_f64_param(input: &serde_json::Value, name: &str) -> Option<f64> {
    let val = input.get(name)?;
    if val.is_null() {
        return None;
    }
    val.as_f64()
        .or_else(|| val.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
}

/// Extract an optional JSON array parameter with lenient coercion.
///
/// Some MCP clients serialize array-typed tool parameters as JSON strings
/// (e.g. `"[\"a\",\"b\"]"` instead of `["a","b"]`). This helper tries
/// `as_array()` first, then falls back to parsing the string as JSON.
/// Returns `None` if the parameter is absent, null, or not coercible.
pub fn optional_array_param(
    input: &serde_json::Value,
    name: &str,
) -> Option<Vec<serde_json::Value>> {
    let val = input.get(name)?;
    if val.is_null() {
        return None;
    }
    // Native JSON array — fast path
    if let Some(arr) = val.as_array() {
        return Some(arr.clone());
    }
    // String-encoded JSON array — fallback for MCP clients that stringify arrays
    if let Some(s) = val.as_str() {
        if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str(s) {
            return Some(arr);
        }
    }
    None
}
