//! Parameter-parsing helpers for tool input JSON values.

use super::types::RecoverableError;

/// A concrete call shape for parameters whose meaning is identical at every call
/// site, replacing the generic *"Add the required 'X' parameter to the tool call"*
/// — a sentence that restates the error rather than teaching the call.
///
/// BL-3 Class B measured ~13 of 34 live schema errors shipping that template. The
/// principle was already settled in
/// `docs/issues/archive/2026-06-04-edit-file-old-string-miss-no-closest-match.md`:
/// a bare "not found" is a defect *because the tool holds the content needed to
/// help*. The server knows the parameter's name, type and purpose when it rejects.
///
/// **Deliberately absent: `path` and `action`.** Both name different things in
/// different tools — `path` is a directory to approve (`approve_write`), a project
/// root (`workspace(activate)`) and a library root (`library(register)`); `action`
/// indexes a different enum per tool. A shared entry would be confidently wrong at
/// two sites out of three and would read as authoritative, which is worse than
/// saying little. Those call sites pass their own hint through
/// [`require_str_param_or_hint`].
fn param_hint(name: &str) -> Option<&'static str> {
    Some(match name {
        "symbol" => {
            "Name the symbol, e.g. symbol=\"MyStruct/my_method\" for a method or \
             symbol=\"my_fn\" for a free function. Run symbols(name=\"...\") first if you \
             need the exact name-path."
        }
        "old_string" => {
            "Pass the exact text to find, e.g. old_string=\"let x = 1;\". It is \
             whitespace-sensitive and must match exactly once unless replace_all=true."
        }
        "command" => {
            "Pass the shell command as one string, e.g. command=\"cargo test --lib\". Run it \
             bare and query the @cmd_* buffer rather than piping to a log-trimmer."
        }
        "pattern" => {
            "Pass the regex to search for, e.g. pattern=\"fn my_func\" — 'query' and 'regex' \
             are also accepted. Add glob=\"*.rs\" to narrow by file type."
        }
        "query" => {
            "Pass the natural-language query, e.g. query=\"how are LSP clients restarted\". \
             This is meaning-based search, so a phrase beats a bare keyword."
        }
        "content" => {
            "Pass the text to write as a string, e.g. content=\"# Title\\n\\nBody\". The value \
             is written verbatim."
        }
        "topic" => {
            "Pass the memory topic key, e.g. topic=\"architecture\". Path-like keys such as \
             topic=\"conventions/testing\" are valid, and memory(action=\"list\") shows what \
             exists."
        }
        _ => return None,
    })
}

/// The hint for a missing `name`, falling back to the generic template for
/// parameters [`param_hint`] does not cover.
fn missing_param_hint(name: &str) -> String {
    param_hint(name)
        .map(str::to_string)
        .unwrap_or_else(|| format!("Add the required '{name}' parameter to the tool call."))
}

/// The hint for a `name` that is present but not a string. Same table — knowing
/// the shape helps just as much when the value was sent as a number or an object.
fn wrong_type_hint(name: &str) -> String {
    param_hint(name)
        .map(str::to_string)
        .unwrap_or_else(|| format!("Provide '{name}' as a string value."))
}

/// Convenience: extract a required parameter from a JSON `Value`, returning
/// `RecoverableError` (not a fatal error) if it is missing.
pub fn require_param<'a>(
    input: &'a serde_json::Value,
    name: &str,
) -> anyhow::Result<&'a serde_json::Value> {
    input.get(name).ok_or_else(|| {
        RecoverableError::with_hint(
            format!("missing '{name}' parameter"),
            missing_param_hint(name),
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
        format!("missing '{name}' parameter"),
        missing_param_hint(name),
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
            RecoverableError::with_hint(format!("'{name}' must be a string"), wrong_type_hint(name))
                .into()
        })
}

/// Like `require_str_param_or`, but emits a caller-supplied usage hint on
/// failure (missing, or present-but-not-a-string) instead of the generic
/// "add the required parameter" text.
///
/// Use on tools where showing the *correct call shape* materially helps the
/// agent recover — the path-bearing file/markdown tools especially. usage.db
/// shows agents habitually either omit `path` entirely (emulating a stateful
/// editor with an implicit "current file" — codescout has none) or send a
/// reasonable-but-wrong alias. A hint that echoes a concrete correct call
/// closes that loop far better than "add the required 'path' parameter".
pub fn require_str_param_or_hint<'a>(
    input: &'a serde_json::Value,
    name: &str,
    aliases: &[&str],
    hint: &str,
) -> anyhow::Result<&'a str> {
    let value = input
        .get(name)
        .or_else(|| aliases.iter().find_map(|a| input.get(*a)));
    match value {
        Some(v) => v.as_str().ok_or_else(|| {
            RecoverableError::with_hint(format!("'{}' must be a string", name), hint.to_string())
                .into()
        }),
        None => Err(RecoverableError::with_hint(
            format!("missing '{}' parameter", name),
            hint.to_string(),
        )
        .into()),
    }
}

/// Convenience: extract a required string parameter from a JSON `Value`.
pub fn require_str_param<'a>(input: &'a serde_json::Value, name: &str) -> anyhow::Result<&'a str> {
    require_param(input, name)?.as_str().ok_or_else(|| {
        RecoverableError::with_hint(format!("'{name}' must be a string"), wrong_type_hint(name))
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
