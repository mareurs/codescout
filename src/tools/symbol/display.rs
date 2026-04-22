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
        return String::new();
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
        None => return String::new(),
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

pub(super) fn format_find_symbol(val: &Value) -> String {
    let symbols = match val["symbols"].as_array() {
        Some(arr) => arr,
        None => return String::new(),
    };

    let total = val["total"].as_u64().unwrap_or(symbols.len() as u64);

    if symbols.is_empty() {
        return "0 matches".to_string();
    }

    struct SymRow {
        kind: String,
        location: String,
        name_path: String,
        body: Option<String>,
    }

    let rows: Vec<SymRow> = symbols
        .iter()
        .map(|s| {
            let kind = s["kind"].as_str().unwrap_or("?").to_string();
            let file = s["file"].as_str().unwrap_or("?");
            let start = s["start_line"].as_u64().unwrap_or(0);
            let end = s["end_line"].as_u64().unwrap_or(0);
            let location = if end > start {
                format!("{file}:{start}-{end}")
            } else {
                format!("{file}:{start}")
            };
            let name_path = s["symbol"]
                .as_str()
                .or_else(|| s["name"].as_str())
                .unwrap_or("?")
                .to_string();
            let body = s["body"].as_str().map(|b| b.to_string());
            SymRow {
                kind,
                location,
                name_path,
                body,
            }
        })
        .collect();

    let max_kind_len = rows.iter().map(|r| r.kind.len()).max().unwrap_or(0);
    let max_loc_len = rows.iter().map(|r| r.location.len()).max().unwrap_or(0);

    let match_word = if total == 1 { "match" } else { "matches" };
    let header = if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
        let shown = overflow["shown"].as_u64().unwrap_or(symbols.len() as u64);
        format!("{shown} {match_word} ({total} total)")
    } else {
        format!("{total} {match_word}")
    };
    let mut out = format!("{header}\n");

    for row in &rows {
        let kind_pad = max_kind_len - row.kind.len();
        let loc_pad = max_loc_len - row.location.len();
        out.push_str("\n  ");
        out.push_str(&row.kind);
        for _ in 0..kind_pad {
            out.push(' ');
        }
        out.push_str("  ");
        out.push_str(&row.location);
        for _ in 0..loc_pad {
            out.push(' ');
        }
        out.push_str("   ");
        out.push_str(&row.name_path);

        if let Some(body) = &row.body {
            // Short bodies are shown inline. Long bodies are replaced with a
            // navigation hint — embedding a 300-line function in the compact
            // summary only causes truncation mid-body, which misleads agents
            // into thinking the body is incomplete rather than available via
            // json_path. The threshold is intentionally well below the
            // COMPACT_SUMMARY_MAX_BYTES (2000) so even a single long function
            // leaves room for the rest of the summary.
            const INLINE_BODY_LIMIT: usize = 500;
            if body.len() <= INLINE_BODY_LIMIT {
                out.push('\n');
                for line in body.lines() {
                    out.push_str("\n      ");
                    out.push_str(line);
                }
            } else {
                let line_count = body.lines().count();
                out.push_str(&format!(
                    "\n      ({line_count}-line body — use json_path=\"$.symbols[0].body\" to extract)"
                ));
            }
        }
    }

    if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
        out.push('\n');
        out.push_str(&format_overflow(overflow));
    }

    out
}

pub(super) fn format_list_symbols(val: &Value) -> String {
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
