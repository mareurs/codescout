//! Test instrument: FOLLOW an overflow envelope's own recovery hint.
//!
//! A test that checks the hint's TEXT passes whatever the route does. The defect this module
//! exists for was a hint that read fine and could not work: `json_path="$.field"` attached to
//! a payload with no `field`, `$.included_ids[*]` attached to a result whose content sat at
//! `$.markdown`. Every assertion about the hint's wording was green while the call it named
//! failed or returned the wrong thing. So the check that matters is the one an agent performs:
//! take the path the envelope names, run `read_file` on the envelope's handle with it, and see
//! whether the data comes back.
//!
//! `hinted_path` reads the path out of the envelope's own `hint` string, so a tool that
//! changes how it words the hint without changing the route still passes, and a tool that
//! keeps the wording and breaks the route does not.

use crate::tools::{Tool, ToolContext};
use serde_json::{json, Value};

/// The text of a `call_content` result's first block, which is where the primary payload sits.
pub(crate) fn primary_text(content: &[rmcp::model::Content]) -> String {
    content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .unwrap_or_default()
}

/// Parse the primary block as the overflow envelope it must be, naming the text on failure.
///
/// Every caller's precondition is that its response SPILLED behind `@tool_*`. When that no
/// longer holds (a later change bounds the payload so it stays inline), the failure says so
/// instead of surfacing a JSON parse error from deep inside the instrument.
pub(crate) fn envelope_of(content: &[rmcp::model::Content]) -> Value {
    envelope_in(&primary_text(content)).unwrap_or_else(|why| panic!("{why}"))
}

/// The body of [`envelope_of`], returning the failure message so it can be pinned by a test.
fn envelope_in(text: &str) -> Result<Value, String> {
    const PRECONDITION: &str = "the response was expected to spill behind @tool_* and did not";
    let envelope: Value = serde_json::from_str(text).map_err(|e| {
        format!("{PRECONDITION}: the primary block is not a JSON envelope ({e}): {text:.300}")
    })?;
    match envelope["output_id"].as_str() {
        Some(id) if id.starts_with("@tool_") => Ok(envelope),
        other => Err(format!(
            "{PRECONDITION}: output_id is {other:?}, not a @tool_* handle: {text:.300}"
        )),
    }
}

/// The `json_path` the envelope's `hint` tells the caller to use.
///
/// Panics, with the whole hint, when it names none: an envelope that overflowed is
/// contractually owed a route, and "no path named" is a finding, not a skip.
pub(crate) fn hinted_path(envelope: &Value) -> String {
    let hint = envelope["hint"].as_str().unwrap_or_default();
    let start = hint
        .find("json_path=\"")
        .map(|i| i + "json_path=\"".len())
        .unwrap_or_else(|| panic!("the envelope's hint names no json_path: {hint:?}"));
    let end = hint[start..]
        .find('"')
        .unwrap_or_else(|| panic!("the envelope's json_path is not terminated: {hint:?}"));
    hint[start..start + end].to_string()
}

/// Follow the hint: `read_file(<envelope handle>, json_path=<hinted path>)`.
///
/// Returns the path that was followed with the call's result, so a failing test can print
/// both. The result is the tool's own `Value`, exactly what an agent would receive.
pub(crate) async fn follow_hint(
    envelope: &Value,
    ctx: &ToolContext,
) -> (String, anyhow::Result<Value>) {
    let handle = envelope["output_id"]
        .as_str()
        .unwrap_or_else(|| panic!("the envelope carries no output_id: {envelope}"))
        .to_string();
    let jp = hinted_path(envelope);
    let result = crate::tools::read_file::ReadFile
        .call(json!({ "path": handle, "json_path": jp }), ctx)
        .await;
    (jp, result)
}

/// Store `payload` as a real `@tool_*` buffer and run `read_file(<handle>, json_path=<jp>)` on
/// it: the call an agent makes after reading a hint, without needing a tool that produces
/// `payload`. For a hint decision tested against a payload SHAPE rather than a live result.
pub(crate) async fn follow_path_on(
    payload: &Value,
    jp: &str,
    ctx: &ToolContext,
) -> anyhow::Result<Value> {
    let handle = ctx
        .output_buffer
        .store_tool("hint_probe", payload.to_string());
    crate::tools::read_file::ReadFile
        .call(json!({ "path": handle, "json_path": jp }), ctx)
        .await
}

/// Every `json_path="..."` route a hint's text offers, with the `<field>` template filled by
/// `field`. The template is the one placeholder a hint is allowed to carry; a literal `$.field`
/// is NOT filled, so a hint that offers it fails when followed.
pub(crate) fn json_paths_in(text: &str, field: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("json_path=") {
        rest = &rest[i + "json_path=".len()..];
        let quoted = rest.trim_start_matches('\\').trim_start_matches('"');
        if let Some(end) = quoted.find(['"', '\\']) {
            out.push(quoted[..end].replace("<field>", field));
        }
    }
    out
}

/// Every `run_command("...")` route a hint's text offers, with `PATTERN` replaced by `pattern`.
pub(crate) fn commands_in(text: &str, pattern: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("run_command(\"") {
        rest = &rest[i + "run_command(\"".len()..];
        if let Some(end) = rest.find("\")") {
            out.push(rest[..end].replace("PATTERN", pattern));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A response that stayed inline is the case `envelope_of` exists to name: the primary block
    /// is text, not an envelope. The failure must state the PRECONDITION and carry the text, not
    /// surface as a bare JSON parse error.
    #[test]
    fn an_inline_response_fails_by_naming_the_spill_precondition() {
        let err = envelope_in("450 lines\n  … showing 0 of 450 — Outline only").unwrap_err();
        assert!(
            err.starts_with("the response was expected to spill behind @tool_* and did not: "),
            "{err}"
        );
        assert!(err.contains("not a JSON envelope"), "{err}");
        assert!(
            err.contains("Outline only"),
            "the text must be shown: {err}"
        );
    }

    /// JSON that parses is still not the envelope when it carries no `@tool_*` handle: an
    /// inline JSON result, or a `@cmd_*` / `@file_*` handle, would otherwise fail later in
    /// `hinted_path` with a message about the hint rather than about the missing spill.
    #[test]
    fn a_json_block_without_a_tool_handle_fails_by_the_same_name() {
        for text in [
            r#"{"content":"x"}"#,
            r#"{"output_id":"@file_abc","summary":"s"}"#,
            r#"{"output_id":"@cmd_abc"}"#,
        ] {
            let err = envelope_in(text).unwrap_err();
            assert!(
                err.starts_with("the response was expected to spill behind @tool_* and did not: "),
                "{err}"
            );
            assert!(err.contains("not a @tool_* handle"), "{err}");
        }
    }

    #[test]
    fn a_tool_envelope_is_returned_as_parsed() {
        let text = r#"{"output_id":"@tool_abc","summary":"s","hint":"h"}"#;
        assert_eq!(envelope_in(text).unwrap()["summary"], "s");
    }
}
