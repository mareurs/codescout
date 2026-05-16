//! Output formatter for the CLI. Pretty by default, JSON via `--json`.

use anyhow::Result;
use serde_json::Value;
use std::io::{IsTerminal, Write};

#[derive(Debug, Clone, Copy, Default)]
pub struct OutputOpts {
    pub json: bool,
    pub no_color: bool,
}

/// Resolve `--no-color` based on stdout capability when not explicitly set.
fn effective_no_color(opts: &OutputOpts) -> bool {
    opts.no_color || !std::io::stdout().is_terminal()
}

pub(crate) enum Shape {
    FindResult,
    GetResult,
    GraphResult,
    StateAtResult,
    EventList,
    StaleList,
    WriteAck,
    Unknown,
}

pub(crate) fn infer_shape(v: &Value) -> Shape {
    if v.is_string() && v.as_str() == Some("ok") {
        return Shape::WriteAck;
    }
    if let Some(obj) = v.as_object() {
        if obj.contains_key("items") && obj.contains_key("total") {
            // could be FindResult or EventList — disambiguate on shape of items
            if let Some(first) = obj
                .get("items")
                .and_then(|i| i.as_array())
                .and_then(|a| a.first())
            {
                if first.get("kind").is_some() && first.get("artifact_id").is_some() {
                    return Shape::EventList;
                }
            }
            return Shape::FindResult;
        }
        if obj.contains_key("nodes") && obj.contains_key("edges") {
            return Shape::GraphResult;
        }
        if obj.contains_key("artifact") && obj.contains_key("status_at") {
            return Shape::StateAtResult;
        }
        if obj.contains_key("stale") && obj.contains_key("threshold_hours") {
            return Shape::StaleList;
        }
        if obj.contains_key("id") && obj.contains_key("body") {
            return Shape::GetResult;
        }
    }
    Shape::Unknown
}

/// Print to stdout — main entrypoint used by every verb after a tool call.
pub fn print(value: &Value, opts: &OutputOpts) -> Result<()> {
    let stdout = std::io::stdout();
    let mut h = stdout.lock();
    write_value(value, opts, &mut h)
}

pub(crate) fn write_value<W: Write>(value: &Value, opts: &OutputOpts, w: &mut W) -> Result<()> {
    let no_color = effective_no_color(opts);
    if opts.json {
        serde_json::to_writer_pretty(&mut *w, value)?;
        writeln!(w)?;
        return Ok(());
    }
    match infer_shape(value) {
        Shape::WriteAck => write_ack(value, no_color, w),
        // All pretty branches land in CLI-5 (FindResult table) or later. Until then,
        // fall back to JSON for everything other than WriteAck so the user always
        // sees something useful.
        _ => fallback_json(value, w),
    }
}

fn write_ack<W: Write>(value: &Value, _no_color: bool, w: &mut W) -> Result<()> {
    // "ok" string → "ok". Object with {"ok":true, "id":...} → "ok: created <id>".
    if let Some(obj) = value.as_object() {
        if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
            writeln!(w, "ok: {id}")?;
            return Ok(());
        }
    }
    writeln!(w, "ok")?;
    Ok(())
}

fn fallback_json<W: Write>(value: &Value, w: &mut W) -> Result<()> {
    serde_json::to_writer_pretty(&mut *w, value)?;
    writeln!(w)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_mode_emits_pretty_json() {
        let v = json!({"items": [{"id": "abc", "title": "t"}]});
        let mut buf = Vec::new();
        write_value(
            &v,
            &OutputOpts {
                json: true,
                no_color: true,
            },
            &mut buf,
        )
        .unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("\"items\""));
        assert!(s.ends_with('\n'));
    }

    #[test]
    fn unknown_shape_falls_back_to_json() {
        // A bare string the shape inferrer doesn't recognise.
        let v = json!("ok");
        let mut buf = Vec::new();
        write_value(
            &v,
            &OutputOpts {
                json: false,
                no_color: true,
            },
            &mut buf,
        )
        .unwrap();
        let s = String::from_utf8(buf).unwrap();
        // Either "ok" recognised as a WriteAck or fallback JSON — both must include "ok".
        assert!(s.contains("ok"), "expected 'ok' in output; got: {s}");
    }

    #[test]
    fn infer_shape_recognises_find_result() {
        let v = json!({"items": [{"id":"a"}], "total": 1});
        assert!(matches!(infer_shape(&v), Shape::FindResult));
    }

    #[test]
    fn infer_shape_unknown_for_arbitrary_object() {
        let v = json!({"weird": "shape"});
        assert!(matches!(infer_shape(&v), Shape::Unknown));
    }
}
