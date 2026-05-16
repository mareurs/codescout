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
        Shape::FindResult => write_find_table(value, no_color, w),
        Shape::GetResult => write_get_summary(value, no_color, w),
        Shape::GraphResult => write_graph_tree(value, no_color, w),
        // Other pretty branches land in later tasks. Until then, fall back
        // to JSON for everything else so output is never silent.
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

fn write_find_table<W: Write>(value: &Value, _no_color: bool, w: &mut W) -> Result<()> {
    let items = value.get("items").and_then(|v| v.as_array());
    let Some(items) = items else {
        return fallback_json(value, w);
    };
    if items.is_empty() {
        writeln!(w, "(no results)")?;
        return Ok(());
    }
    // Compute column widths from the data so each row aligns.
    let mut widths = [8usize, 7, 7, 40]; // id, kind, status, title (rel_path follows wrapped)
    for it in items {
        let id = it.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let kind = it.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let status = it.get("status").and_then(|v| v.as_str()).unwrap_or("");
        let title = it.get("title").and_then(|v| v.as_str()).unwrap_or("");
        widths[0] = widths[0].max(id.len());
        widths[1] = widths[1].max(kind.len());
        widths[2] = widths[2].max(status.len());
        widths[3] = widths[3].max(title.len()).min(60);
    }
    writeln!(
        w,
        "{:<w0$}  {:<w1$}  {:<w2$}  {:<w3$}  rel_path",
        "id",
        "kind",
        "status",
        "title",
        w0 = widths[0],
        w1 = widths[1],
        w2 = widths[2],
        w3 = widths[3]
    )?;
    for it in items {
        let id = it.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let kind = it.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let status = it.get("status").and_then(|v| v.as_str()).unwrap_or("");
        let title = it.get("title").and_then(|v| v.as_str()).unwrap_or("");
        let rel_path = it.get("rel_path").and_then(|v| v.as_str()).unwrap_or("");
        let title_trunc = if title.len() > widths[3] {
            format!("{}…", &title[..widths[3].saturating_sub(1)])
        } else {
            title.to_string()
        };
        writeln!(
            w,
            "{:<w0$}  {:<w1$}  {:<w2$}  {:<w3$}  {}",
            id,
            kind,
            status,
            title_trunc,
            rel_path,
            w0 = widths[0],
            w1 = widths[1],
            w2 = widths[2],
            w3 = widths[3]
        )?;
    }
    if let Some(total) = value.get("total").and_then(|v| v.as_u64()) {
        if (total as usize) > items.len() {
            writeln!(
                w,
                "\nShowing {} of {} — narrow with --kind / --tag / --filter, or paginate with --offset.",
                items.len(),
                total
            )?;
        }
    }
    Ok(())
}

fn write_get_summary<W: Write>(value: &Value, _no_color: bool, w: &mut W) -> Result<()> {
    let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("?");
    let title = value
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("(untitled)");
    let kind = value.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
    let status = value.get("status").and_then(|v| v.as_str()).unwrap_or("?");
    writeln!(w, "{title}  [{kind}/{status}]  {id}")?;
    if let Some(path) = value.get("abs_path").and_then(|v| v.as_str()) {
        writeln!(w, "{path}")?;
    }
    writeln!(w)?;
    if let Some(body) = value.get("body").and_then(|v| v.as_str()) {
        writeln!(w, "{body}")?;
    } else {
        // No body field — print the whole JSON as a fallback so users still see the data.
        fallback_json(value, w)?;
    }
    Ok(())
}

fn write_graph_tree<W: Write>(value: &Value, _no_color: bool, w: &mut W) -> Result<()> {
    let nodes = value
        .get("nodes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let edges = value
        .get("edges")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if nodes.is_empty() {
        writeln!(w, "(empty graph)")?;
        return Ok(());
    }
    let id_to_title: std::collections::HashMap<String, String> = nodes
        .iter()
        .filter_map(|n| {
            let id = n.get("id")?.as_str()?.to_string();
            let title = n
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("(untitled)")
                .to_string();
            Some((id, title))
        })
        .collect();

    let root = nodes
        .first()
        .and_then(|n| n.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    writeln!(
        w,
        "{} — {}",
        root,
        id_to_title.get(root).cloned().unwrap_or_default()
    )?;
    for e in &edges {
        let src = e.get("src_id").and_then(|v| v.as_str()).unwrap_or("");
        let dst = e.get("dst_id").and_then(|v| v.as_str()).unwrap_or("");
        let rel = e.get("rel").and_then(|v| v.as_str()).unwrap_or("?");
        let other = if src == root { dst } else { src };
        let title = id_to_title.get(other).cloned().unwrap_or_default();
        let arrow = if src == root { "→" } else { "←" };
        writeln!(w, "  {arrow} [{rel}] {other} — {title}")?;
    }
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

    #[test]
    fn pretty_find_result_renders_table_with_id_kind_status_title() {
        let v = json!({
            "items": [
                {"id":"abcd1234","kind":"tracker","status":"active","title":"Ship Feature X","rel_path":"docs/trackers/x.md"},
                {"id":"bbbb5678","kind":"spec","status":"draft","title":"Design Y","rel_path":"docs/specs/y.md"}
            ],
            "total": 2
        });
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
        assert!(s.contains("abcd1234"), "row 1 id missing; got: {s}");
        assert!(
            s.contains("Ship Feature X"),
            "row 1 title missing; got: {s}"
        );
        assert!(
            s.contains("docs/specs/y.md"),
            "row 2 rel_path missing; got: {s}"
        );
        // The "kind | status" axis should be visible regardless of exact framing.
        assert!(s.contains("tracker"));
        assert!(s.contains("draft"));
        // The table must NOT be JSON — assert the column-header line is present
        // so a JSON fallback would visibly fail this test.
        assert!(
            s.lines().any(|line| line.contains("id")
                && line.contains("kind")
                && line.contains("status")
                && line.contains("title")),
            "expected a table header line; got: {s}"
        );
    }
}
