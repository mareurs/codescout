//! `format_compact` helpers for symbol-tool results.
//!
//! Each tool's `format_compact()` impl delegates to one of these functions.
//! Pure functions over `serde_json::Value` — no state, no I/O.

use serde_json::Value;

use crate::tools::format::{format_line_range, format_overflow};

pub(super) fn format_goto_definition(val: &Value) -> String {
    let defs = match val["definitions"].as_array() {
        Some(arr) => arr,
        None => return String::new(),
    };

    if defs.is_empty() {
        // Empty defs is a successful "no definition" result. Surface the hint
        // attached by the tool (if any) instead of swallowing it silently.
        return val
            .get("hint")
            .and_then(Value::as_str)
            .map(|h| h.to_string())
            .unwrap_or_default();
    }

    if defs.len() == 1 {
        let d = &defs[0];
        let file = d["file"].as_str().unwrap_or("?");
        let line = d["line"].as_u64().unwrap_or(0);
        let context = d["context"].as_str().unwrap_or("");
        let source = d["source"].as_str().unwrap_or("project");

        let mut out = if source != "project" {
            format!("{}:{} ({})", file, line, source)
        } else {
            format!("{}:{}", file, line)
        };

        if !context.is_empty() {
            out.push_str("\n\n  ");
            out.push_str(context);
        }
        return out;
    }

    let mut out = format!("{} definitions\n", defs.len());
    for d in defs {
        let file = d["file"].as_str().unwrap_or("?");
        let line = d["line"].as_u64().unwrap_or(0);
        let context = d["context"].as_str().unwrap_or("");
        let source = d["source"].as_str().unwrap_or("project");

        out.push_str("\n  ");
        out.push_str(&format!("{}:{}", file, line));
        if source != "project" {
            out.push_str(&format!(" ({})", source));
        }
        if !context.is_empty() {
            out.push_str(&format!("   {}", context));
        }
    }
    out
}

pub(super) fn format_hover(val: &Value) -> String {
    let content = match val["content"].as_str() {
        Some(s) => s,
        None => {
            // No hover content. Surface a hint+location if the tool attached one
            // (the empty-result path now returns Ok with hint instead of erroring).
            let location = val["location"].as_str().unwrap_or("");
            let hint = val["hint"].as_str().unwrap_or("");
            return match (location.is_empty(), hint.is_empty()) {
                (true, true) => String::new(),
                (false, true) => location.to_string(),
                (true, false) => hint.to_string(),
                (false, false) => format!("{}\n  ({})", location, hint),
            };
        }
    };
    let location = val["location"].as_str().unwrap_or("");

    let mut out = String::new();
    if !location.is_empty() {
        out.push_str(location);
        out.push_str("\n\n");
    }

    let mut in_code_block = false;
    let mut first_content_line = true;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if !first_content_line {
            out.push('\n');
        }
        out.push_str("  ");
        out.push_str(line);
        first_content_line = false;
    }
    out
}

pub fn format_search_symbols(val: &Value) -> String {
    use crate::tools::file_group::{group_by_file, render_grouped};

    let symbols = match val["symbols"].as_array() {
        Some(arr) => arr,
        None => return String::new(),
    };

    let total = val["total"].as_u64().unwrap_or(symbols.len() as u64) as usize;

    if symbols.is_empty() {
        return "0 matches".to_string();
    }

    // The symbols search JSON may hoist `file` to the top level (Fix C
    // single-file path). Restore it per-symbol before grouping so the
    // renderer has uniform shape.
    let top_file = val.get("file").and_then(|v| v.as_str());
    let normalized: Vec<Value> = symbols
        .iter()
        .map(|s| {
            let mut s = s.clone();
            if s.get("file").is_none() {
                if let Some(f) = top_file {
                    if let Some(obj) = s.as_object_mut() {
                        obj.insert("file".to_string(), Value::String(f.to_string()));
                    }
                }
            }
            s
        })
        .collect();

    let groups = group_by_file(&normalized);
    let files = groups.len();
    let noun = if total == 1 { "match" } else { "matches" };

    let render_item = |item: &Value| -> String {
        let kind = item["kind"].as_str().unwrap_or("?");
        let start = item["start_line"].as_u64().unwrap_or(0);
        let end = item["end_line"].as_u64().unwrap_or(0);
        let range = if end > start {
            format!("{start}-{end}")
        } else {
            format!("{start}")
        };
        let name_path = item["symbol"]
            .as_str()
            .or_else(|| item["name"].as_str())
            .unwrap_or("?");
        let mut row = format!("  {kind}  {range}  {name_path}");
        if let Some(body) = item["body"].as_str() {
            const INLINE_BODY_LIMIT: usize = 500;
            if body.len() <= INLINE_BODY_LIMIT {
                for line in body.lines() {
                    row.push_str("\n      ");
                    row.push_str(line);
                }
            } else {
                let line_count = body.lines().count();
                row.push_str(&format!(
                    "\n      ({line_count}-line body — use json_path=\"$.symbols[0].body\" to extract)"
                ));
            }
        }
        row
    };

    let mut out = render_grouped(&groups, total, files, noun, render_item);

    if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
        out.push('\n');
        out.push_str(&format_overflow(overflow));
    }

    out
}

pub(super) fn format_overview_symbols(val: &Value) -> String {
    // File mode
    if let Some(file) = val["file"].as_str() {
        let symbols = match val["symbols"].as_array() {
            Some(arr) => arr,
            None => return String::new(),
        };
        let count = symbols.len();
        let sym_word = if count == 1 { "symbol" } else { "symbols" };
        let mut out = format!("{file} — {count} {sym_word}\n");
        format_symbol_tree(&mut out, symbols, 2);

        if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
            out.push('\n');
            out.push_str(&format_overflow(overflow));
        }
        return out;
    }

    // class_overview / directory_map mode
    if let Some(mode) = val["mode"].as_str() {
        let dir = val["directory"].as_str().unwrap_or(".");
        let total = val["total_files"].as_u64().unwrap_or(0);
        let empty: Vec<Value> = vec![];
        let subdirs = val["subdirectories"].as_array().unwrap_or(&empty);

        let mut out = format!("{dir} — {total} files\n");

        for subdir in subdirs {
            let path = subdir["path"].as_str().unwrap_or("?");
            let count = subdir["file_count"].as_u64().unwrap_or(0);
            out.push_str(&format!("\n  {path} ({count} files)"));
            if mode == "class_overview" {
                let empty_arr: Vec<Value> = vec![];
                let classes = subdir["classes"].as_array().unwrap_or(&empty_arr);
                if !classes.is_empty() {
                    let names: Vec<&str> = classes.iter().filter_map(|v| v.as_str()).collect();
                    out.push_str(&format!("\n    {}", names.join(", ")));
                }
            }
        }

        if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
            out.push('\n');
            out.push_str(&format_overflow(overflow));
        }

        if let Some(hint) = val["hint"].as_str() {
            out.push_str(&format!("\n\n{hint}"));
        }

        return out;
    }

    // Directory or pattern mode
    let dir = val["directory"]
        .as_str()
        .or_else(|| val["pattern"].as_str())
        .unwrap_or(".");
    let files = match val["files"].as_array() {
        Some(arr) => arr,
        None => return String::new(),
    };

    if files.is_empty() {
        return format!("{dir} — 0 symbols");
    }

    let mut out = format!("{dir}\n");

    for file_entry in files {
        let file = file_entry["file"].as_str().unwrap_or("?");
        let symbols = match file_entry["symbols"].as_array() {
            Some(arr) => arr,
            None => continue,
        };
        let count = symbols.len();
        let sym_word = if count == 1 { "symbol" } else { "symbols" };
        out.push_str(&format!("\n  {file} — {count} {sym_word}\n"));
        format_symbol_tree(&mut out, symbols, 4);
    }

    if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
        out.push('\n');
        out.push_str(&format_overflow(overflow));
    }

    out
}

fn format_symbol_tree(out: &mut String, symbols: &[Value], indent: usize) {
    let max_kind_len = symbols
        .iter()
        .map(|s| s["kind"].as_str().unwrap_or("").len())
        .max()
        .unwrap_or(0);
    let max_name_len = symbols
        .iter()
        .map(|s| {
            s["symbol"]
                .as_str()
                .or_else(|| s["name"].as_str())
                .unwrap_or("")
                .len()
        })
        .max()
        .unwrap_or(0);

    let pad = " ".repeat(indent);

    for sym in symbols {
        let kind = sym["kind"].as_str().unwrap_or("?");
        let name = sym["symbol"]
            .as_str()
            .or_else(|| sym["name"].as_str())
            .unwrap_or("?");
        let start = sym["start_line"].as_u64().unwrap_or(0);
        let end = sym["end_line"].as_u64().unwrap_or(0);
        let line_range = format_line_range(start, end);

        let kind_pad = max_kind_len - kind.len();
        let name_pad = max_name_len.saturating_sub(name.len());
        out.push('\n');
        out.push_str(&pad);
        out.push_str(kind);
        for _ in 0..kind_pad {
            out.push(' ');
        }
        out.push_str("   ");
        out.push_str(name);
        for _ in 0..name_pad {
            out.push(' ');
        }
        out.push_str("  ");
        out.push_str(&line_range);

        if let Some(children) = sym["children"].as_array() {
            let child_indent = indent + 5;
            let child_pad = " ".repeat(child_indent);
            let max_child_name = children
                .iter()
                .map(|c| c["name"].as_str().unwrap_or("").len())
                .max()
                .unwrap_or(0);

            for child in children {
                let child_kind = child["kind"].as_str().unwrap_or("?");
                let child_name = child["name"].as_str().unwrap_or("?");
                let cs = child["start_line"].as_u64().unwrap_or(0);
                let ce = child["end_line"].as_u64().unwrap_or(0);
                let child_lr = format_line_range(cs, ce);
                let child_name_pad = max_child_name.saturating_sub(child_name.len());

                out.push('\n');
                out.push_str(&child_pad);

                if child_kind == "EnumMember" || child_kind == "Field" {
                    out.push_str(child_name);
                    for _ in 0..child_name_pad {
                        out.push(' ');
                    }
                } else {
                    out.push_str(child_kind);
                    out.push_str("  ");
                    out.push_str(child_name);
                    for _ in 0..child_name_pad {
                        out.push(' ');
                    }
                }
                out.push_str("  ");
                out.push_str(&child_lr);
            }
        }
    }
}

pub(super) fn format_find_references(result: &Value) -> String {
    let total = result["total"].as_u64().unwrap_or_else(|| {
        result["references"]
            .as_array()
            .map(|a| a.len() as u64)
            .unwrap_or(0)
    });

    if total == 0 {
        return "No references found.".to_string();
    }

    let refs = match result["references"].as_array() {
        Some(r) => r,
        None => return format!("{total} refs"),
    };

    const MAX_SHOW: usize = 5;
    let mut out = format!("{total} refs");
    for r in refs.iter().take(MAX_SHOW) {
        let file = r["file"].as_str().unwrap_or("?");
        let line = r["line"].as_u64().unwrap_or(0);
        out.push_str(&format!("\n  {file}:{line}"));
    }
    let shown = refs.len().min(MAX_SHOW);
    let hidden = (total as usize).saturating_sub(shown);
    if hidden > 0 {
        out.push_str(&format!("\n  … +{hidden} more"));
    }
    out
}

pub(super) fn format_replace_symbol(result: &Value) -> String {
    let lines = result["replaced_lines"].as_str().unwrap_or("?");
    format!("replaced · L{lines}")
}

pub(super) fn format_remove_symbol(result: &Value) -> String {
    let lines = result["removed_lines"].as_str().unwrap_or("?");
    let count = result["line_count"].as_u64().unwrap_or(0);
    format!("removed · L{lines} ({count} lines)")
}

pub(super) fn format_insert_code(result: &Value) -> String {
    let line = result["inserted_at_line"].as_u64().unwrap_or(0);
    let pos = result["position"].as_str().unwrap_or("after");
    format!("inserted {pos} L{line}")
}

pub(super) fn format_rename_symbol(result: &Value) -> String {
    let total_edits = result["total_edits"].as_u64().unwrap_or(0);
    let textual = result["textual_match_count"].as_u64().unwrap_or(0);
    let total = total_edits + textual;
    let new_name = result["new_name"].as_str().unwrap_or("?");
    let files = result["files_changed"].as_u64().unwrap_or(0);
    if files <= 1 {
        format!("→ {new_name} · {total} sites")
    } else {
        format!("→ {new_name} · {total} sites · {files} files")
    }
}

#[cfg(test)]
mod tests {
    use super::format_overview_symbols;

    #[test]
    fn format_overview_symbols_class_overview_mode() {
        let val = serde_json::json!({
            "directory": "src/main/kotlin",
            "mode": "class_overview",
            "subdirectories": [
                { "path": "src/main/kotlin/api",    "file_count": 12, "classes": ["CourseController", "PlannerApi"] },
                { "path": "src/main/kotlin/domain", "file_count": 8,  "classes": ["Course", "Student"] }
            ],
            "total_files": 45,
            "hint": "Found 45 files — drill down with symbols('<subdir>')."
        });
        let result = format_overview_symbols(&val);
        assert!(result.contains("src/main/kotlin"));
        assert!(result.contains("45 files"));
        assert!(result.contains("api"));
        assert!(result.contains("12"));
        assert!(result.contains("CourseController"));
        assert!(result.contains("domain"));
        assert!(result.contains("Course"));
        assert!(result.contains("drill down"), "hint shown");
    }

    #[test]
    fn format_overview_symbols_directory_map_mode() {
        let val = serde_json::json!({
            "directory": "ktor-server/src",
            "mode": "directory_map",
            "subdirectories": [
                { "path": "ktor-server/src/main", "file_count": 80 },
                { "path": "ktor-server/src/test", "file_count": 40 }
            ],
            "total_files": 120,
            "hint": "Found 120 files — too large for symbol overview."
        });
        let result = format_overview_symbols(&val);
        assert!(result.contains("ktor-server/src"));
        assert!(result.contains("120 files"));
        assert!(result.contains("src/main"));
        assert!(result.contains("80"));
        assert!(result.contains("too large"));
    }

    #[test]
    fn format_overview_symbols_directory_map_with_overflow() {
        let subdirs: Vec<serde_json::Value> = (0..15)
            .map(|i| serde_json::json!({ "path": format!("sub/{i}"), "file_count": 10 }))
            .collect();
        let val = serde_json::json!({
            "directory": "big",
            "mode": "directory_map",
            "subdirectories": subdirs,
            "total_files": 300,
            "overflow": { "shown": 15, "total": 23, "hint": "Showing 15 of 23 directories (largest first)." },
            "hint": "Found 300 files."
        });
        let result = format_overview_symbols(&val);
        assert!(result.contains("Showing 15 of 23"));
    }
}
