//! `list_dir` tool and related format helpers.

use anyhow::Result;
use serde_json::{json, Value};

use super::format::format_overflow;
use super::{optional_u64_param, parse_bool_param, Tool, ToolContext};

// ── list_dir ────────────────────────────────────────────────────────────────

pub struct ListDir;

#[async_trait::async_trait]
impl Tool for ListDir {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn description(&self) -> &str {
        "List files and directories. Pass recursive=true for a full tree."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string" },
                "recursive": { "type": "boolean", "default": false, "description": "Descend into subdirectories (auto-capped at depth 3 in exploring mode)." },
                "max_depth": { "type": "integer", "minimum": 1, "description": "Max depth (1=children only, default). Overrides recursive." },
                "detail_level": { "type": "string", "description": "'full' for all entries (default: compact)" },
                "offset": { "type": "integer", "description": "Pagination offset" },
                "limit": { "type": "integer", "description": "Max entries per page (default 50)" }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        use super::output::{OutputGuard, OutputMode, OverflowInfo};

        let raw_path = input["path"].as_str().unwrap_or(".");
        let project_root = ctx.agent.project_root().await;
        let security = ctx.agent.security_config().await;
        let path = crate::util::path_security::validate_read_path(
            raw_path,
            project_root.as_deref(),
            &security,
        )?;
        let recursive = parse_bool_param(&input["recursive"]);
        let explicit_max_depth = optional_u64_param(&input, "max_depth").map(|d| d as usize);
        let guard = OutputGuard::from_input(&input);

        // Determine requested depth:
        //   explicit max_depth > recursive=true (unlimited) > default (1 level)
        let requested_depth: Option<usize> = match explicit_max_depth {
            Some(d) => Some(d),
            None if recursive => None, // unlimited
            None => Some(1),
        };

        // In exploring mode, cap unlimited recursive walks at depth 3 to
        // avoid returning hundreds of deeply-nested paths as an unstructured flat list.
        let depth_auto_capped = guard.mode == OutputMode::Exploring && requested_depth.is_none();
        let walker_depth: Option<usize> = if depth_auto_capped {
            Some(3)
        } else {
            requested_depth
        };

        let walker = ignore::WalkBuilder::new(&path)
            .max_depth(walker_depth)
            .hidden(true)
            .git_ignore(false)
            .git_exclude(false)
            .git_global(false)
            .ignore(false)
            .build()
            .flatten()
            .filter(|e| e.depth() > 0);

        // In exploring mode, stop collecting once we exceed max_results.
        // We collect max_results+1 to detect overflow without walking the
        // entire tree (we lose the exact total, which is fine for exploring).
        let cap = match guard.mode {
            OutputMode::Exploring => Some(guard.max_results + 1),
            OutputMode::Focused => None,
        };

        let mut entries = Vec::new();
        for entry in walker {
            let suffix = if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                "/"
            } else {
                ""
            };
            entries.push(format!("{}{}", entry.path().display(), suffix));
            if let Some(c) = cap {
                if entries.len() >= c {
                    break;
                }
            }
        }

        let hit_early_cap = cap.is_some() && entries.len() > guard.max_results;

        let overflow_hint = if depth_auto_capped {
            "Depth auto-capped at 3 in exploring mode. Use max_depth=N for a specific depth, or detail_level='full' for unlimited depth.".to_string()
        } else {
            "Use a more specific path or set recursive=false".to_string()
        };

        let (entries, overflow) = if hit_early_cap {
            // We collected max_results+1, truncate and report overflow
            entries.truncate(guard.max_results);
            let overflow = OverflowInfo {
                shown: guard.max_results,
                total: guard.max_results + 1, // at least this many
                hint: overflow_hint,
                next_offset: None,
                by_file: None,
                by_file_overflow: 0,
            };
            (entries, Some(overflow))
        } else {
            guard.cap_items(entries, &overflow_hint)
        };

        let mut result = json!({ "entries": entries });
        if let Some(ov) = overflow {
            result["overflow"] = OutputGuard::overflow_json(&ov);
        }
        if depth_auto_capped {
            result["depth_capped"] = json!(3);
        }
        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_list_dir(result))
    }
}

// ── format helpers ──────────────────────────────────────────────────────

pub(super) fn format_list_dir(val: &Value) -> String {
    let entries = match val["entries"].as_array() {
        Some(arr) => arr,
        None => return String::new(),
    };

    if entries.is_empty() {
        return "(empty directory)".to_string();
    }

    let names: Vec<&str> = entries.iter().filter_map(|e| e.as_str()).collect();
    if names.is_empty() {
        return "(empty directory)".to_string();
    }

    let prefix = common_path_prefix(&names);

    let short_names: Vec<&str> = names
        .iter()
        .map(|n| {
            let stripped = &n[prefix.len()..];
            if stripped.is_empty() {
                *n
            } else {
                stripped
            }
        })
        .collect();

    let dir_display = if prefix.is_empty() {
        ".".to_string()
    } else {
        prefix.trim_end_matches('/').to_string()
    };
    let mut out = format!("{} — {} entries\n", dir_display, names.len());

    // Tree mode: any entry spans multiple path levels after prefix stripping.
    // Detected by a '/' in the stripped name (excluding a lone trailing slash on dirs).
    let is_tree = short_names
        .iter()
        .any(|n| n.trim_end_matches('/').contains('/'));

    out.push('\n');
    if is_tree {
        format_list_dir_tree_body(&short_names, &mut out);
    } else {
        let max_name_len = short_names.iter().map(|n| n.len()).max().unwrap_or(0);
        let col_width = max_name_len + 2;
        let num_cols = (78 / col_width).max(1);

        for (i, name) in short_names.iter().enumerate() {
            if i % num_cols == 0 {
                out.push_str("  ");
            }
            out.push_str(name);
            if (i + 1) % num_cols != 0 && i + 1 < short_names.len() {
                let padding = col_width - name.len();
                for _ in 0..padding {
                    out.push(' ');
                }
            }
            if (i + 1) % num_cols == 0 && i + 1 < short_names.len() {
                out.push('\n');
            }
        }
    }

    if let Some(depth) = val.get("depth_capped").and_then(|v| v.as_u64()) {
        out.push_str(&format!(
            "\n[depth capped at {} — use max_depth=N or detail_level='full' for deeper]\n",
            depth
        ));
    }

    if let Some(overflow) = val.get("overflow") {
        if overflow.is_object() {
            out.push('\n');
            out.push_str(&format_overflow(overflow));
        }
    }

    out
}

/// Renders entries with indentation based on path depth when the listing
/// spans multiple directory levels. Each level adds two spaces of indent.
fn format_list_dir_tree_body(short_names: &[&str], out: &mut String) {
    for name in short_names {
        let path_part = name.trim_end_matches('/');
        let depth = path_part.matches('/').count();
        let indent = "  ".repeat(depth + 1);
        let base = path_part.rsplit('/').next().unwrap_or(path_part);
        if name.ends_with('/') {
            out.push_str(&format!("{}{}/\n", indent, base));
        } else {
            out.push_str(&format!("{}{}\n", indent, base));
        }
    }
}

pub(super) fn common_path_prefix(paths: &[&str]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    if paths.len() == 1 {
        if let Some(pos) = paths[0].rfind('/') {
            return paths[0][..=pos].to_string();
        }
        return String::new();
    }

    let first = paths[0];
    let mut prefix_len = 0;
    let mut last_slash = 0;

    for (i, ch) in first.char_indices() {
        if paths[1..]
            .iter()
            .any(|p| p.len() <= i || p.as_bytes()[i] != ch as u8)
        {
            break;
        }
        prefix_len = i + ch.len_utf8();
        if ch == '/' {
            last_slash = prefix_len;
        }
    }

    if last_slash > 0 {
        first[..last_slash].to_string()
    } else {
        let candidate = &first[..prefix_len];
        if candidate.ends_with('/') {
            candidate.to_string()
        } else {
            String::new()
        }
    }
}
