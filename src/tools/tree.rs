//! `tree` tool — merged directory listing + glob file search.
//!
//! Polymorphic behavior: when the `glob` argument is set, behaves like a glob
//! file search (returns `files` + `total`). Otherwise lists directory contents
//! (returns `entries`, optionally recursive).

use anyhow::Result;
use serde_json::{json, Value};

use super::format::{insert_below_header, overflow_head};
use super::{
    optional_u64_param, parse_bool_param, OutputForm, RecoverableError, Tool, ToolContext,
};
use crate::util::fs::to_forward_slash;

// ── tree ────────────────────────────────────────────────────────────────────

pub struct Tree;

#[async_trait::async_trait]
impl Tool for Tree {
    fn name(&self) -> &str {
        "tree"
    }

    fn annotations(&self) -> Option<rmcp::model::ToolAnnotations> {
        crate::tools::annot::read_only_closed()
    }

    fn description(&self) -> &str {
        "Explore the filesystem. With `glob` set, returns matching files (e.g. '**/*.rs'). Without `glob`, lists directory entries (recursive optional). Respects .gitignore."
    }

    fn relevant_guide_topic(&self, _result: &Value) -> Option<&str> {
        Some("progressive-disclosure")
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Subtree root (default: current dir)" },
                "glob": { "type": "string", "description": "When set, return matching paths; otherwise list directory" },
                "recursive": { "type": "boolean", "default": false, "description": "Descend into subdirectories (auto-capped at depth 3 in exploring mode). Ignored when `glob` is set." },
                "max_depth": { "type": "integer", "minimum": 1, "description": "Max depth (1=children only, default). Overrides recursive. Ignored when `glob` is set." },
                "include_hidden": { "type": "boolean", "default": false, "description": "Also include hidden files/dirs (dotfiles, .github/)" },
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
    let project_root = ctx
        .agent
        .project_root_for(ctx.workspace_override.as_deref())
        .await;
    let security = ctx
        .agent
        .security_config_for(ctx.workspace_override.as_deref())
        .await;
    let path = crate::util::path_security::validate_read_path(
        raw_path,
        project_root.as_deref(),
        &security,
    )?;
    let recursive = parse_bool_param(&input["recursive"]);
    let include_hidden = input
        .get("include_hidden")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
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

    // Hidden entries are pruned AT THE BOUNDARY and counted there, rather than admitted
    // and classified as in `glob_impl`. The two branches ask different questions and so
    // want different counts.
    //
    // A listing asks "what is in this directory?", and what a caller loses to the filter
    // is `.github/` — one entry — not the nine files inside it. Classifying after the
    // walk answers with the subtree instead, which is both less useful and *depth-
    // dependent*. Measured on this repo: **3877** entries live under the root's hidden
    // directories (2548 of them session artefacts in `.buddy/`, 1061 in `.worktrees/`),
    // so the subtree-counting form ran into the thousands where pruning reports 16 at
    // depth 1 and 51 unlimited — each exactly matching a `find` that excludes anything
    // inside a hidden directory. Reported by `codescout-fe`.
    //
    // The count is NOT depth-invariant and should not be: a deeper walk really would have
    // listed more hidden entries. What pruning excludes is the CONTENTS of a pruned entry.
    //
    // A glob asks "which files match?", where the useful count IS the matching files
    // inside a hidden directory, so that branch must descend. See `glob_impl`.
    //
    // This walk sets `git_ignore(false)` / `ignore(false)`, so hidden is its ONLY
    // exclusion — a listing that shows gitignored `target/` while silently omitting
    // `.github/` reads as complete precisely because it is showing you what other tools
    // hide. `.git` / `.codescout` are pruned WITHOUT counting: they exist in every
    // project, so counting them would put a nonzero note on every listing.
    let withheld = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = withheld.clone();
    let walker = ignore::WalkBuilder::new(&path)
        .max_depth(walker_depth)
        .hidden(false)
        .git_ignore(false)
        .git_exclude(false)
        .git_global(false)
        .ignore(false)
        .filter_entry(move |e| {
            let name = e.file_name().to_string_lossy();
            if matches!(name.as_ref(), ".git" | ".codescout") {
                return false;
            }
            if e.depth() > 0 && name.starts_with('.') && !include_hidden {
                counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return false;
            }
            true
        })
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
        // Normalize like the sibling glob branch (:228) — a raw `.display()` here
        // emits backslashes on Windows, which defeats post_process's
        // forward-slash root-stripping and degrades the `/`-based
        // common_path_prefix + tree indentation (omnibus 49ee6a03, F7).
        entries.push(format!("{}{}", to_forward_slash(entry.path()), suffix));
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
    let hidden_withheld = withheld.load(std::sync::atomic::Ordering::Relaxed);
    if hidden_withheld > 0 {
        // When the entry cap stopped the walk the tally is a LOWER BOUND, and naming the
        // cause is not decoration. Measured live: the default listing reports 16 withheld
        // while `recursive=true` reports 10, because the collect loop broke before the
        // walk finished. So a caller who WIDENS the search watches the omission count
        // FALL, and the natural reading — "less is hidden now" — is the exact inverse of
        // the truth. A bare "at least N" does not block that reading; the comparison
        // between two runs is what has to be refused. Found on the live check by me and
        // independently by `codescout-fe`.
        let lead = "this listing describes what was walked, not the directory";
        let body = format!(
            "{hidden_withheld} hidden entr{} not listed",
            if hidden_withheld == 1 { "y" } else { "ies" }
        );
        result["completeness_warning"] = json!(if hit_early_cap {
            withheld_note_truncated(lead, &body, "listed")
        } else {
            withheld_note(lead, &body, "listed")
        });
    }
    Ok(result)
}

// ── glob behavior ───────────────────────────────────────────────────────────

/// The completeness note both branches emit, as a pure function so the *wording* can be
/// tested without staging a walk that both hits the entry cap and prunes a hidden entry —
/// an ordering `ignore::Walk` does not guarantee, so a test built on it would pass or fail
/// by luck.
///
/// `truncated` is the load-bearing argument. When the entry cap stopped the walk the tally
/// is a LOWER BOUND, and the danger is not vagueness but sign: measured live, a default
/// listing reports 16 withheld while `recursive=true` reports 10, because the collect loop
/// broke before the walk finished. A caller who WIDENS the search watches the omission
/// count FALL, and reads it as "less is hidden now" — the exact inverse of the truth. A
/// bare "at least N" does not block that reading, so the note refuses the comparison
/// outright.
fn withheld_note(lead: &str, body: &str, never: &str) -> String {
    format!(
        "{lead}: {body}. Pass include_hidden=true to list them. `.git` and `.codescout` \
         are never {never}."
    )
}

/// [`withheld_note`] for a tally the entry cap cut short.
fn withheld_note_truncated(lead: &str, body: &str, never: &str) -> String {
    format!(
        "{lead}: at least {body} — this result was itself cut short, so the true total is \
         higher and this count is NOT comparable with one from a wider search. Pass \
         include_hidden=true to list them. `.git` and `.codescout` are never {never}."
    )
}

/// Glob search under `path`.
///
/// **Hidden entries are admitted into the walk and classified afterwards, rather than
/// pruned by the walker.** Pruning is what made a withheld match indistinguishable from
/// no match: `tree(glob=".github/workflows/*.yml")` returned a bare `0 files` while both
/// files existed and were tracked, and `**/*.yml` returned 1 of 3 — a *plausible
/// non-zero*, so not even a suspicious-zero heuristic could fire.
/// `docs/issues/archive/2026-08-30-tree-glob-silently-omits-dotfiles.md`.
///
/// Counting rather than gating on a zero is deliberate, and differs from `grep`'s
/// `completeness_warning`, which fires only when the result is empty. That rule is a
/// proxy: it cannot see the 1-of-3 case above. Tree's walk opens no files, so it can
/// afford the exact measure — and the exact measure is one rule instead of two, silent
/// precisely when nothing was withheld.
///
/// `.git` and `.codescout` stay pruned. They exist by construction in every project
/// codescout touches, so counting matches inside them would fire this warning on
/// essentially every call and train the reader to skip it — the same reasoning as
/// `grep`'s `uninformative` list.
async fn glob_impl(input: Value, ctx: &ToolContext) -> Result<Value> {
    let pattern = input["glob"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("glob arg required"))?;
    let raw_path = input["path"].as_str().unwrap_or(".");
    let project_root = ctx
        .agent
        .project_root_for(ctx.workspace_override.as_deref())
        .await;
    let security = ctx
        .agent
        .security_config_for(ctx.workspace_override.as_deref())
        .await;
    let search_path = crate::util::path_security::validate_read_path(
        raw_path,
        project_root.as_deref(),
        &security,
    )?;
    let max = optional_u64_param(&input, "limit").unwrap_or(100) as usize;
    let include_hidden = input
        .get("include_hidden")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

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
    let mut hidden_withheld = 0usize;
    let mut hit_cap = false;
    let walker = ignore::WalkBuilder::new(&search_path)
        .hidden(false)
        .git_ignore(true)
        .filter_entry(|e| {
            !matches!(
                e.file_name().to_string_lossy().as_ref(),
                ".git" | ".codescout"
            )
        })
        .build();
    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(&search_path)
            .unwrap_or(entry.path());
        if !glob.is_match(rel) {
            continue;
        }
        // Any dot-prefixed component makes the path hidden, not just the file name:
        // `.github/workflows/ci.yml` is hidden by its directory, and that is the case
        // the bug was reported for.
        let hidden = rel.components().any(|c| {
            let s = c.as_os_str().to_string_lossy();
            s.starts_with('.') && s.as_ref() != "." && s.as_ref() != ".."
        });
        if hidden && !include_hidden {
            hidden_withheld += 1;
            continue;
        }
        matches.push(to_forward_slash(entry.path()));
        if matches.len() >= max {
            hit_cap = true;
            break;
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
    if hidden_withheld > 0 {
        let lead = "this count describes what was searched, not the pattern";
        let body = if hidden_withheld == 1 {
            format!("{hidden_withheld} matching file under hidden paths was withheld")
        } else {
            format!("{hidden_withheld} matching files under hidden paths were withheld")
        };
        result["completeness_warning"] = json!(if hit_cap {
            withheld_note_truncated(lead, &body, "counted")
        } else {
            withheld_note(lead, &body, "counted")
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

    // Both cap signals go immediately BELOW the header rather than after the entries.
    // The compact summary is cut from its tail (`truncate_compact`), so a note appended
    // under a long listing is dropped on exactly the listings big enough to need it —
    // see `format::overflow_head`. This surface is the one `get_guide("progressive-
    // disclosure")` cites as the tool that *does* announce its cap, and tail placement
    // made that reputation hold only below the compaction threshold.
    let mut head_extra = String::new();
    if let Some(depth) = val.get("depth_capped").and_then(|v| v.as_u64()) {
        head_extra.push_str(&format!(
            "[depth capped at {depth} — use max_depth=N or detail_level='full' for deeper]\n"
        ));
    }
    // Same placement rule as the cap signals above, and for the same reason: appended
    // under a long listing it is cut by `truncate_compact` on exactly the listings that
    // hide the most.
    if let Some(w) = val["completeness_warning"].as_str() {
        head_extra.push_str(&format!("[{w}]\n"));
    }
    head_extra.push_str(&overflow_head(val));

    insert_below_header(out, &head_extra)
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
    // Rendered here rather than left in the JSON, because this is the caller's only
    // surface: `tree` returns text, so a key nothing formats is a key nobody reads.
    //
    // FIRST, not appended. `format_compact` output passes through `truncate_compact`
    // (soft 2 KB), and a 100-file listing runs several times that — so a note after the
    // list is cut on exactly the results big enough to need it. The sibling
    // `format_list_dir` states the same rule and solves it with `insert_below_header`;
    // that helper puts the note after the first line, which works there because the
    // count is the header. Here the count prints LAST, so leading is the equivalent
    // placement. An earlier revision of this function appended it and was wrong.
    match result["completeness_warning"].as_str() {
        Some(w) => format!("{w}\n\n{out}"),
        None => out,
    }
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
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(
                crate::tools::guide_ledger::GuideLedger::mid_session(),
            )),
            workspace_override: None,
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

    /// Fixture for the hidden-path cases: two files under a dot-prefixed directory and
    /// one beside it, all matching the same glob. Mirrors the reported shape — this
    /// repo's `.github/workflows/{ci,manual}.yml` against `docker-compose.yml`.
    fn seed_hidden_and_visible(dir: &std::path::Path) {
        std::fs::create_dir_all(dir.join(".github/workflows")).unwrap();
        std::fs::write(dir.join(".github/workflows/ci.yml"), "").unwrap();
        std::fs::write(dir.join(".github/workflows/manual.yml"), "").unwrap();
        std::fs::write(dir.join("docker-compose.yml"), "").unwrap();
    }

    /// The reported defect: a glob naming a hidden directory returned a bare `0 files`
    /// while both files existed. The zero was correct about the walk and wrong about the
    /// question, and nothing in the response said which.
    ///
    /// Asserts on the FORMATTED output, not the JSON. `tree` returns text, so a
    /// `completeness_warning` key that `format_glob` does not render is invisible to the
    /// caller — a JSON-only assertion passes while the defect stands, which is the same
    /// shape as the bug itself.
    #[tokio::test]
    async fn glob_naming_a_hidden_dir_reports_what_it_withheld() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        seed_hidden_and_visible(dir.path());

        let result = Tree
            .call(
                json!({ "glob": ".github/workflows/*.yml", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["total"], 0, "default still withholds: {result}");
        // The exact phrase, not `contains('2')`: the rendered output carries tempdir
        // paths, and a temp path with a `2` in it would satisfy a bare digit check
        // whatever the warning said — or whether one was rendered at all.
        let rendered = format_glob(&result);
        assert!(
            rendered.contains("2 matching files"),
            "the caller must be told HOW MANY were withheld — a bare zero is what made \
             this indistinguishable from no match: {rendered}"
        );
        assert!(
            rendered.contains("include_hidden"),
            "naming the remedy is the actionable half; without it the reader knows only \
             that the answer is wrong: {rendered}"
        );
    }

    /// The under-reporting case, and the one no zero-gated warning can reach: the result
    /// is a plausible NON-zero, so nothing about it invites suspicion. `grep`'s
    /// `completeness_warning` fires only on an empty result and would stay silent here.
    #[tokio::test]
    async fn a_glob_matching_both_reports_the_withheld_count_beside_a_nonzero_result() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        seed_hidden_and_visible(dir.path());

        let result = Tree
            .call(
                json!({ "glob": "**/*.yml", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(
            result["total"], 1,
            "one visible match — the shape that reads as a complete answer: {result}"
        );
        let rendered = format_glob(&result);
        assert!(
            rendered.contains("2 matching files"),
            "1 of 3 must not render as a complete answer: {rendered}"
        );
    }

    /// The escape hatch the tool had no way to offer: before this, hidden paths were
    /// unreachable by any argument, so the warning above would have named a remedy that
    /// did not exist.
    #[tokio::test]
    async fn include_hidden_lists_the_files_the_default_withholds() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        seed_hidden_and_visible(dir.path());

        let result = Tree
            .call(
                json!({
                    "glob": "**/*.yml",
                    "path": dir.path().to_str().unwrap(),
                    "include_hidden": true
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["total"], 3, "all three must be listed: {result}");
        assert!(
            result["completeness_warning"].is_null(),
            "nothing was withheld, so there is nothing to warn about: {result}"
        );
    }

    /// The silent direction, and the load-bearing one. A warning attached to every glob
    /// would be noise, and `symbols.rs`'s `completeness_warning` states why: it trains
    /// the reader to skip it in the case that matters. Here a hidden directory EXISTS
    /// and simply holds nothing the glob matches — presence of dotfiles is not the
    /// trigger; a withheld MATCH is.
    #[tokio::test]
    async fn no_warning_when_the_hidden_paths_held_nothing_matching() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        seed_hidden_and_visible(dir.path());
        std::fs::write(dir.path().join("main.rs"), "").unwrap();

        let result = Tree
            .call(
                json!({ "glob": "*.rs", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["total"], 1);
        assert!(
            result["completeness_warning"].is_null(),
            "a dot-prefixed directory that holds no match must not trigger the warning, \
             or it fires on nearly every call: {result}"
        );
        assert!(
            !format_glob(&result).contains("include_hidden"),
            "and it must not reach the rendered output either: {result}"
        );
    }

    /// List mode had the same `.hidden(true)` and never read `include_hidden` — the parse
    /// sat inside `glob_impl` alone. So the parameter was accepted by the schema and
    /// silently ignored, which is a worse contract than an absent one: an absent
    /// parameter sends you elsewhere, an inert one tells you the answer you got is the
    /// answer you asked for. Reported by `codescout-fe`.
    ///
    /// List mode is the sharper case. It sets `git_ignore(false)` / `ignore(false)`, so
    /// hidden is its ONLY exclusion — the listing shows gitignored paths while omitting
    /// `.github/`, and reads as complete *because* it is showing you what other tools
    /// hide.
    #[tokio::test]
    async fn list_mode_reports_the_hidden_entries_it_withheld() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        seed_hidden_and_visible(dir.path());

        let result = Tree
            .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        let entries = result["entries"].as_array().unwrap();
        assert_eq!(
            entries.len(),
            1,
            "default still withholds — the listing itself is unchanged: {result}"
        );
        let rendered = format_list_dir(&result);
        assert!(
            rendered.contains("1 hidden entry"),
            "the caller must be told the listing is partial: {rendered}"
        );
        // Placement, not just presence. `truncate_compact` cuts from the tail, so a note
        // under a long listing is dropped on exactly the listings that hide the most —
        // the rule `format_list_dir` already states for its cap signals.
        let first_two: Vec<&str> = rendered.lines().take(2).collect();
        assert!(
            first_two.iter().any(|l| l.contains("hidden entry")),
            "must sit in the header region, not below the entries: {rendered}"
        );
    }

    /// The parameter now does something in list mode, which is the whole point of the
    /// report: before this it was accepted and inert.
    #[tokio::test]
    async fn list_mode_include_hidden_lists_them() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        seed_hidden_and_visible(dir.path());

        let result = Tree
            .call(
                json!({ "path": dir.path().to_str().unwrap(), "include_hidden": true }),
                &ctx,
            )
            .await
            .unwrap();

        let entries = result["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2, "`.github/` must now appear: {result}");
        assert!(
            entries
                .iter()
                .any(|e| e.as_str().unwrap().contains(".github")),
            "and it must be the hidden one that appeared: {result}"
        );
        assert!(
            result["completeness_warning"].is_null(),
            "nothing withheld, nothing to warn about: {result}"
        );
    }

    /// Silence where there is nothing to disclose. Measured on this repo: the warning
    /// fires at the project root (16 informative dotfiles) and in none of `src`,
    /// `src/tools`, `docs`, `crates` — so it marks the listings that are genuinely
    /// partial rather than annotating every call.
    #[tokio::test]
    async fn list_mode_is_silent_with_no_hidden_entries() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "").unwrap();
        std::fs::write(dir.path().join("b.rs"), "").unwrap();

        let result = Tree
            .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        assert_eq!(result["entries"].as_array().unwrap().len(), 2);
        assert!(
            result["completeness_warning"].is_null(),
            "a directory with no dotfiles must produce no note: {result}"
        );
        assert!(
            !format_list_dir(&result).contains("include_hidden"),
            "and nothing in the rendered output either: {result}"
        );
    }

    /// When the entry cap cut the walk, the tally is a lower bound — and the danger is
    /// not vagueness but SIGN. Measured live before this: the default listing reports 16
    /// withheld and `recursive=true` reports 10, because the collect loop broke before
    /// the walk finished. A caller who widens the search watches the omission count FALL
    /// and reads "less is hidden now", the exact inverse of the truth.
    ///
    /// So "at least" is necessary and not sufficient: 10 is still visibly less than 16,
    /// and a reader comparing two runs is not stopped by a hedge on one of them. The note
    /// has to refuse the comparison outright, which is what the second assertion pins.
    ///
    /// Tested on the pure function rather than through a walk: reproducing this needs a
    /// run that BOTH hits the entry cap AND prunes a hidden entry before breaking, and
    /// `ignore::Walk` does not guarantee that ordering — such a test would pass or fail by
    /// luck, which is worse than none.
    #[test]
    fn a_capped_tally_says_at_least_and_refuses_the_comparison() {
        let note = withheld_note_truncated("lead", "10 hidden entries not listed", "listed");

        assert!(
            note.contains("at least 10"),
            "a tally the cap cut short is a lower bound and must say so: {note}"
        );
        assert!(
            note.contains("NOT comparable"),
            "the hedge alone leaves 10-vs-16 readable as 'less is hidden now'; the note \
             must refuse the cross-run comparison: {note}"
        );
    }

    /// The other side of the pair, and the one that keeps the qualifier meaningful: an
    /// uncapped tally is exact and must NOT hedge, or "at least" appears on every note and
    /// stops distinguishing the case it exists for.
    #[test]
    fn an_uncapped_tally_states_the_count_without_hedging() {
        let note = withheld_note("lead", "16 hidden entries not listed", "listed");

        assert!(note.contains("16 hidden entries"), "{note}");
        assert!(
            !note.contains("at least"),
            "an exact count must not hedge, or the hedge means nothing: {note}"
        );
        assert!(
            !note.contains("NOT comparable"),
            "and an uncapped count IS comparable across runs: {note}"
        );
    }

    /// A pruned hidden directory counts as ONE withheld entry, never as its contents.
    /// `.github/` is one thing the caller lost; the files inside it were never going to
    /// be listed as siblings of `docker-compose.yml`.
    ///
    /// The first version of this fix classified after the walk and so counted the
    /// SUBTREE. Measured on this repo: **3877** entries live under the root's hidden
    /// directories, 2548 of them session artefacts in `.buddy/`, so that form ran into
    /// the thousands. Found by `codescout-fe`.
    ///
    /// **The invariant is subtree-exclusion, NOT depth-invariance**, and this test's name
    /// claims more than it verifies. The count legitimately grows with the walk, because
    /// a deeper walk really would have listed more hidden entries — measured live: **16**
    /// at depth 1 and **51** unlimited, each exactly matching a `find` that excludes
    /// anything inside a hidden directory. Equality across the two depths holds *here*
    /// only because this fixture has no hidden entry below depth 1; that is the mechanism
    /// this test uses, not the property it asserts.
    ///
    /// Mutation: count by rel-path component instead of pruning, and this reads 1
    /// against 4. A first mutation that only stopped the pruning SURVIVED — the fixture's
    /// sole dot-named entry is `.github` itself, so a name-keyed tally stays 1 at every
    /// depth. The mutation had not expressed the defect.
    #[tokio::test]
    async fn a_pruned_hidden_directory_counts_as_one_entry_not_as_its_contents() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        seed_hidden_and_visible(dir.path());

        let shallow = Tree
            .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        let deep = Tree
            .call(
                json!({ "path": dir.path().to_str().unwrap(), "recursive": true }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(
            shallow["completeness_warning"], deep["completeness_warning"],
            "this fixture has exactly one hidden entry and it sits at depth 1, so a walk \
             that prunes at the boundary must report the same thing at either depth. A \
             larger number in the deep run means the contents of `.github/` were counted.\n\
             shallow: {shallow}\ndeep: {deep}"
        );
        assert!(
            shallow["completeness_warning"]
                .as_str()
                .unwrap()
                .contains("1 hidden entry"),
            "and the unit is the pruned entry, not its contents: {shallow}"
        );
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
