//! `tree` tool — merged directory listing + glob file search.
//!
//! Polymorphic behavior: when the `glob` argument is set, behaves like a glob
//! file search (returns `files` + `total`). Otherwise lists directory contents
//! (returns `entries`, optionally recursive).

use anyhow::Result;
use serde_json::{json, Value};

use super::format::format_overflow;
use super::{
    optional_u64_param, parse_bool_param, OutputForm, RecoverableError, Tool, ToolContext,
};

// ── tree ────────────────────────────────────────────────────────────────────

pub struct Tree;

#[async_trait::async_trait]
impl Tool for Tree {
    fn name(&self) -> &str {
        "tree"
    }

    fn description(&self) -> &str {
        "Explore the filesystem. With `glob` set, returns matching files (e.g. '**/*.rs'). Without `glob`, lists directory entries (recursive optional). Respects .gitignore."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Subtree root (default: current dir)" },
                "glob": { "type": "string", "description": "When set, return matching paths; otherwise list directory" },
                "recursive": { "type": "boolean", "default": false, "description": "Descend into subdirectories (auto-capped at depth 3 in exploring mode). Ignored when `glob` is set." },
                "max_depth": { "type": "integer", "minimum": 1, "description": "Max depth (1=children only, default). Overrides recursive. Ignored when `glob` is set." },
                "detail_level": { "type": "string", "description": "'full' for all entries (default: compact)" },
                "offset": { "type": "integer", "description": "Pagination offset" },
                "limit": { "type": "integer", "description": "Max entries per page (default 50; for `glob`, max files default 100)" }
            },
            "description": "When `glob` is set → file search. Otherwise → directory listing (optionally recursive)."
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        if input.get("glob").and_then(|v| v.as_str()).is_some() {
            glob_impl(input, ctx).await
        } else {
            list_dir_impl(input, ctx).await
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        // Glob shape has `files`/`total`; list_dir shape has `entries`.
        if result.get("files").is_some() {
            Some(format_glob(result))
        } else {
            Some(format_list_dir(result))
        }
    }

    fn output_form(&self) -> OutputForm {
        OutputForm::Text
    }
}

// ── list_dir behavior ───────────────────────────────────────────────────────

async fn list_dir_impl(input: Value, ctx: &ToolContext) -> Result<Value> {
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

// ── glob behavior ───────────────────────────────────────────────────────────

async fn glob_impl(input: Value, ctx: &ToolContext) -> Result<Value> {
    let pattern = input["glob"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("glob arg required"))?;
    let raw_path = input["path"].as_str().unwrap_or(".");
    let project_root = ctx.agent.project_root().await;
    let security = ctx.agent.security_config().await;
    let search_path = crate::util::path_security::validate_read_path(
        raw_path,
        project_root.as_deref(),
        &security,
    )?;
    let max = optional_u64_param(&input, "limit").unwrap_or(100) as usize;

    let glob = globset::GlobBuilder::new(pattern)
        .literal_separator(false)
        .build()
        .map_err(|e| {
            RecoverableError::with_hint(
                format!("invalid glob pattern: {e}"),
                "Use glob syntax: * matches anything, ** crosses directories, ? matches one char",
            )
        })?
        .compile_matcher();

    let mut matches = vec![];
    let mut hit_cap = false;
    let walker = ignore::WalkBuilder::new(&search_path)
        .hidden(true)
        .git_ignore(true)
        .build();
    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(&search_path)
            .unwrap_or(entry.path());
        if glob.is_match(rel) {
            matches.push(entry.path().display().to_string());
            if matches.len() >= max {
                hit_cap = true;
                break;
            }
        }
    }

    let mut result = json!({ "files": matches, "total": matches.len() });
    if hit_cap {
        result["overflow"] = json!({
            "shown": matches.len(),
            "hint": format!(
                "Showing first {} files (cap hit). Narrow with a more specific glob or path=<dir>.",
                matches.len()
            )
        });
    }
    Ok(result)
}

// ── format helpers ──────────────────────────────────────────────────────────

pub(crate) fn format_list_dir(val: &Value) -> String {
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

pub(crate) fn common_path_prefix(paths: &[&str]) -> String {
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

pub(crate) fn format_glob(result: &Value) -> String {
    let total = result["total"].as_u64().unwrap_or(0);
    let overflow = result["overflow"].is_object();
    let cap_note = if overflow {
        " (cap hit — narrow glob)"
    } else {
        ""
    };
    let mut out = String::new();
    if let Some(files) = result["files"].as_array() {
        for f in files {
            if let Some(s) = f.as_str() {
                out.push_str(s);
                out.push('\n');
            }
        }
    }
    out.push_str(&format!("{total} files{cap_note}"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::lsp::LspManager;
    use tempfile::tempdir;

    async fn test_ctx() -> ToolContext {
        ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        }
    }

    #[tokio::test]
    async fn tree_lists_when_no_glob() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("foo.rs"), "").unwrap();
        std::fs::write(dir.path().join("bar.rs"), "").unwrap();

        let result = Tree
            .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        assert!(
            result.get("entries").is_some(),
            "expected list_dir shape with `entries`, got: {result}"
        );
        assert!(
            result.get("files").is_none(),
            "expected no glob `files` key in list_dir output"
        );
        let entries = result["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn tree_finds_when_glob_set() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("foo.rs"), "").unwrap();
        std::fs::write(dir.path().join("bar.rs"), "").unwrap();
        std::fs::write(dir.path().join("baz.txt"), "").unwrap();

        let result = Tree
            .call(
                json!({
                    "glob": "*.rs",
                    "path": dir.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(
            result.get("files").is_some(),
            "expected glob shape with `files`, got: {result}"
        );
        assert!(
            result.get("entries").is_none(),
            "expected no list_dir `entries` key in glob output"
        );
        let files = result["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.as_str().unwrap().ends_with(".rs")));
    }

    #[tokio::test]
    async fn tree_call_content_returns_text_not_json() {
        // Regression: small tree results used to serialize as pretty JSON via the
        // default Tool::call_content path. Now Tree declares OutputForm::Text, so
        // both list_dir and glob shapes come through as their compact text form.
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("foo.rs"), "").unwrap();
        std::fs::write(dir.path().join("bar.rs"), "").unwrap();

        let content = Tree
            .call_content(
                json!({ "glob": "*.rs", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(content.len(), 1, "expected exactly 1 content block");
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        assert!(
            !text.trim_start().starts_with('{'),
            "small tree output must NOT be JSON, got: {text}"
        );
        assert!(
            text.contains("foo.rs") && text.contains("bar.rs"),
            "text must list both matched files, got: {text}"
        );
    }
}
