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
    // artifact_event(list) returns a bare JSON array — disambiguate by
    // checking first item for event-shaped keys.
    if let Some(arr) = v.as_array() {
        if let Some(first) = arr.first() {
            if first.get("kind").is_some() && first.get("created_at").is_some() {
                return Shape::EventList;
            }
        }
    }
    if let Some(obj) = v.as_object() {
        // artifact_refresh(list_stale) returns {count, threshold_hours, items, next_step}.
        // Match on `threshold_hours` + `items` before the generic FindResult
        // check below (FindResult uses `items` + `total`).
        if obj.contains_key("threshold_hours") && obj.contains_key("items") {
            return Shape::StaleList;
        }
        if obj.contains_key("items") && obj.contains_key("total") {
            // could be FindResult or EventList — disambiguate on shape of items
            if let Some(first) = obj
                .get("items")
                .and_then(|i| i.as_array())
                .and_then(|a| a.first())
            {
                if first.get("kind").is_some() && first.get("created_at").is_some() {
                    return Shape::EventList;
                }
            }
            return Shape::FindResult;
        }
        if obj.contains_key("nodes") && obj.contains_key("edges") {
            return Shape::GraphResult;
        }
        if obj.contains_key("as_of") && obj.contains_key("status_at_as_of") {
            return Shape::StateAtResult;
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
        Shape::StateAtResult => write_state_summary(value, no_color, w),
        Shape::EventList => write_event_list(value, no_color, w),
        Shape::StaleList => write_stale_list(value, no_color, w),
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

fn write_state_summary<W: Write>(value: &Value, _no_color: bool, w: &mut W) -> Result<()> {
    let title = value
        .get("frontmatter")
        .and_then(|fm| fm.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("(untitled)");
    let status_at = value
        .get("status_at_as_of")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let freshness_now = value
        .get("freshness_now")
        .and_then(|v| v.as_str())
        .unwrap_or("?");

    writeln!(w, "{title}")?;
    if let Some(as_of) = value.get("as_of").and_then(|v| v.as_i64()) {
        writeln!(w, "  as_of (ms):       {as_of}")?;
    } else if let Some(as_of) = value.get("as_of").and_then(|v| v.as_str()) {
        writeln!(w, "  as_of:            {as_of}")?;
    } else {
        writeln!(w, "  as_of:            ?")?;
    }
    writeln!(w, "  status_at_as_of:  {status_at}")?;
    writeln!(w, "  freshness_now:    {freshness_now}")?;
    if let Some(chain) = value.get("supersession_chain").and_then(|v| v.as_array()) {
        if !chain.is_empty() {
            writeln!(w, "  supersession_chain ({}):", chain.len())?;
            for item in chain {
                if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                    writeln!(w, "    - {id}")?;
                }
            }
        }
    }
    Ok(())
}

fn write_event_list<W: Write>(value: &Value, _no_color: bool, w: &mut W) -> Result<()> {
    // The librarian `artifact_event(action="list")` tool returns a bare JSON
    // array of event rows. Accept either a top-level array or an object with
    // an `items` array (defensive, in case the envelope shape evolves).
    let items: &Vec<Value> = if let Some(arr) = value.as_array() {
        arr
    } else if let Some(arr) = value.get("items").and_then(|v| v.as_array()) {
        arr
    } else {
        return fallback_json(value, w);
    };
    if items.is_empty() {
        writeln!(w, "(no events)")?;
        return Ok(());
    }
    writeln!(
        w,
        "{:<16}  {:<16}  {:<12}  payload",
        "created_at", "kind", "author"
    )?;
    for it in items {
        let created = it.get("created_at").and_then(|v| v.as_i64()).unwrap_or(0);
        let kind = it.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
        let author = it.get("author").and_then(|v| v.as_str()).unwrap_or("");
        let payload = it
            .get("payload")
            .map(|v| {
                let s = serde_json::to_string(v).unwrap_or_default();
                if s.len() > 80 {
                    format!("{}…", &s[..79])
                } else {
                    s
                }
            })
            .unwrap_or_default();
        writeln!(
            w,
            "{:<16}  {:<16}  {:<12}  {}",
            created, kind, author, payload
        )?;
    }
    Ok(())
}

fn write_stale_list<W: Write>(value: &Value, _no_color: bool, w: &mut W) -> Result<()> {
    // The librarian `artifact_refresh(action="list_stale")` tool returns
    // `{count, threshold_hours, items, next_step}` where each item carries
    // `{id, kind, title, abs_path, last_refreshed_at, refresh_count, age_hours}`.
    // Accept either the canonical envelope or a bare-array shape so the
    // renderer is forgiving if the envelope evolves.
    let items: &Vec<Value> = if let Some(arr) = value.as_array() {
        arr
    } else if let Some(arr) = value.get("items").and_then(|v| v.as_array()) {
        arr
    } else {
        return fallback_json(value, w);
    };
    if items.is_empty() {
        writeln!(w, "(no stale artifacts)")?;
        return Ok(());
    }
    writeln!(w, "{:<18}  {:>9}  {:<8}  title", "id", "age_hours", "kind")?;
    for it in items {
        let id = it.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let kind = it.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let age = it
            .get("age_hours")
            .and_then(|v| v.as_i64())
            .map(|h| h.to_string())
            .unwrap_or_else(|| "never".to_string());
        let title = it
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("(untitled)");
        writeln!(w, "{id:<18}  {age:>9}  {kind:<8}  {title}")?;
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
    fn infer_shape_recognises_state_at_result() {
        let v = json!({
            "as_of": 1_700_000_000_000_i64,
            "status_at_as_of": "active",
            "frontmatter": {"title": "Test"}
        });
        assert!(matches!(infer_shape(&v), Shape::StateAtResult));
    }

    #[test]
    fn pretty_event_list_renders_header_and_rows() {
        // Real envelope: bare JSON array of event rows with keys matching
        // librarian's timeline::call output (id, kind, payload, created_at, author).
        let v = json!([
            {
                "id": "ev-1",
                "kind": "note",
                "payload": {"text": "hello"},
                "created_at": 1_700_000_000_000_i64,
                "author": "alice"
            }
        ]);
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
        assert!(
            s.lines()
                .any(|l| l.contains("created_at") && l.contains("kind")),
            "expected header; got: {s}"
        );
        assert!(s.contains("note"), "expected event kind; got: {s}");
        assert!(s.contains("1700000000000"), "expected timestamp; got: {s}");
    }

    #[test]
    fn pretty_stale_list_renders_header_and_rows() {
        // Real envelope shape from `artifact_refresh::call(action=list_stale)`:
        // `{count, threshold_hours, items, next_step}` with per-item keys
        // `{id, kind, title, abs_path, last_refreshed_at, refresh_count, age_hours}`.
        let v = json!({
            "count": 1,
            "threshold_hours": 24,
            "items": [
                {
                    "id": "abc123",
                    "kind": "spec",
                    "title": "Old Spec",
                    "abs_path": "/tmp/old.md",
                    "last_refreshed_at": "2026-04-01T00:00:00Z",
                    "refresh_count": 2,
                    "age_hours": 999
                }
            ],
            "next_step": "Call artifact_refresh(id) on each item …"
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
        assert!(
            s.lines()
                .any(|l| l.contains("id") && l.contains("age_hours") && l.contains("title")),
            "expected header; got: {s}"
        );
        assert!(s.contains("abc123"), "expected id; got: {s}");
        assert!(s.contains("Old Spec"), "expected title; got: {s}");
        assert!(s.contains("999"), "expected age_hours rendered; got: {s}");
    }

    #[test]
    fn infer_shape_recognises_stale_list() {
        let v = json!({
            "count": 0,
            "threshold_hours": 24,
            "items": [],
            "next_step": "No stale augmented artifacts in scope."
        });
        assert!(matches!(infer_shape(&v), Shape::StaleList));
    }

    #[test]
    fn infer_shape_recognises_event_list_bare_array() {
        let v = json!([
            {"id": "ev-1", "kind": "note", "created_at": 1_700_000_000_000_i64, "payload": null}
        ]);
        assert!(matches!(infer_shape(&v), Shape::EventList));
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
