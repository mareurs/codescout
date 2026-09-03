//! Markdown editing implementation — heading-addressed section editing, reached
//! through `edit_file`'s markdown grammar (see `edit_file::call`).

use std::ops::Range;

use anyhow::Result;
use serde_json::{json, Value};

use super::super::{parse_bool_param, RecoverableError, ToolContext};
use super::frontmatter;

// ── edit_markdown ────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Helper functions (moved from section_edit.rs)
// ---------------------------------------------------------------------------

/// Scan `text` for surface-marker HTML comments — lines that exactly match
/// `<!-- @surface NAME -->` or `<!-- @end -->`. Returns the markers in
/// document order; duplicates preserved. Used by the F-7 marker-preservation
/// gate in `perform_section_edit_ext`'s replace arm: a replace whose new
/// content omits markers present in the OLD body would silently drop them.
///
/// Pattern is strict (line-anchored, exact whitespace) to avoid false
/// positives from prose that quotes the marker shape (e.g. F-5 in
/// `docs/trackers/prompt-guide-refactor-session-log.md` documents the
/// dual problem in `extract_surface`).
fn extract_surface_markers(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let is_marker = trimmed == "<!-- @end -->"
                || (trimmed.starts_with("<!-- @surface ")
                    && trimmed.ends_with(" -->")
                    && !trimmed[14..trimmed.len() - 4].contains("<!--"));
            is_marker.then(|| trimmed.to_string())
        })
        .collect()
}

/// Returns surface markers that appear in `old_body` but not in `new_content`.
/// Used by the F-7 gate — see [`extract_surface_markers`].
fn find_lost_surface_markers(old_body: &str, new_content: &str) -> Vec<String> {
    let old = extract_surface_markers(old_body);
    let new = extract_surface_markers(new_content);
    old.into_iter().filter(|m| !new.contains(m)).collect()
}

/// Pure string transformation: apply `action` to the section identified by `heading_query`.
///
/// Test-only thin wrapper that delegates to `perform_section_edit_ext` with
/// `at=None` and `force=false`, preserving the historical 4-arg signature
/// for the test suite. Production code (`edit_file::call`, via `markdown::edit`) calls
/// `perform_section_edit_ext` directly so the `at` parameter and the F-7
/// surface-marker-preservation gate's `force` override thread through.
///
/// Returns the full modified file content (always ends with a single newline).
#[cfg(test)]
pub fn perform_section_edit(
    content: &str,
    heading_query: &str,
    action: &str,
    new_content: Option<&str>,
) -> Result<String> {
    perform_section_edit_ext(content, heading_query, action, new_content, None, false)
}

/// Plan the byte-span edit(s) that `action` on `heading_query` would produce,
/// without applying them. Behavior-identical to the historical
/// `perform_section_edit_ext` (see that function's doc comment for the full
/// contract on `at`/`force`) -- `perform_section_edit_ext` is now a thin
/// wrapper delegating here + `apply_planned_edits`, and batch-mode callers
/// use this directly to collect edits from multiple `edits[]` entries before
/// detecting overlaps and applying them together.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_section_edit<'q, Q: Into<crate::tools::file_summary::HeadingQuery<'q>>>(
    content: &str,
    off: &LineOffsets,
    heading_query: Q,
    action: &str,
    new_content: Option<&str>,
    at: Option<&str>,
    force: bool,
    edit_index: usize,
) -> Result<Vec<PlannedEdit>> {
    use crate::tools::core::types::RecoverableError;
    use crate::tools::file_summary::{heading_level, resolve_section_range};

    // Destructure once: `heading_query` stays a `&str` for every downstream error
    // message, while the occurrence selector rides along to the resolver.
    let query = heading_query.into();
    let heading_query = query.text;

    let range = resolve_section_range(content, query).map_err(|e| anyhow::anyhow!("{}", e))?;

    let lines: Vec<&str> = content.split('\n').collect();
    let heading_idx = range.heading_line - 1;
    let end_idx = compute_section_end(&lines, heading_idx + 1, range.level);

    match action {
        "replace" => {
            let new = new_content
                .ok_or_else(|| anyhow::anyhow!("content is required for the 'replace' action (it overwrites the whole section body); for a scoped text swap pass action='edit' with old_string + new_string"))?;

            // F-7: surface-marker-preservation gate.
            // The section's body may contain `<!-- @surface NAME -->` or
            // `<!-- @end -->` HTML-comment markers that demarcate prompt
            // surfaces in source.md (and similar). A replace whose new
            // content omits them silently drops the markers, breaking the
            // build.rs slice extractor (F-5's sibling). Refuse the replace
            // unless the caller passes `force=true`.
            if !force {
                let body_start = heading_idx + 1;
                let body_end = end_idx.min(lines.len());
                if body_start < body_end {
                    let old_body = lines[body_start..body_end].join("\n");
                    let lost = find_lost_surface_markers(&old_body, new);
                    if !lost.is_empty() {
                        let listed = lost.join(", ");
                        return Err(RecoverableError::with_hint(
                            format!(
                                "replace on section {heading_query:?} would drop {} surface marker(s) from the body: {listed}",
                                lost.len()
                            ),
                            "Include those markers verbatim in the new content (they must be alone on their own lines), OR pass force=true if the structural change is intentional. See F-7 in docs/trackers/prompt-guide-refactor-session-log.md.",
                        )
                        .into());
                    }
                }
            }

            // Only treat the new content's first line as a replacement for the
            // TARGET heading itself when it's a heading at the SAME level.
            // A deeper-level heading (e.g. replacing an H2 with content whose
            // first line is an H3) is a subsection under H2, not a replacement
            // for H2 -- checking "is this any heading" instead of "is this the
            // same level as the target" silently deleted the target heading.
            // See docs/issues/archive/2026-07-02-edit-markdown-replace-drops-target-heading-on-heading-shaped-content.md.
            let replace_heading = new
                .lines()
                .next()
                .and_then(|l| heading_level(l.trim_end()))
                .map(|lvl| lvl == range.level)
                .unwrap_or(false);

            // F-3: a trailing horizontal-rule separator (`---`, `***`, `___`)
            // immediately before the next sibling heading is structurally a
            // between-sections separator, not the current section's content.
            // Wholesale-body replace silently destroys it. Shrink the replace
            // range to exclude that trailing HR so it survives the edit.
            // Only applies when the HR is preceded by at least one line of
            // real body content (a section whose entire body is just an HR
            // legitimately has that HR replaced).
            let body_start = heading_idx + 1;
            let replace_end_idx = {
                let mut walk = end_idx;
                while walk > body_start && lines[walk - 1].trim().is_empty() {
                    walk -= 1;
                }
                if walk <= body_start {
                    end_idx
                } else {
                    let hr_idx = walk - 1;
                    let line = lines[hr_idx];
                    let is_hr = !line.starts_with("    ") && {
                        let trimmed = line.trim();
                        match trimmed.chars().next() {
                            Some(marker @ ('-' | '*' | '_')) => {
                                let mut count = 0usize;
                                let mut ok = true;
                                for c in trimmed.chars() {
                                    if c == marker {
                                        count += 1;
                                    } else if c != ' ' {
                                        ok = false;
                                        break;
                                    }
                                }
                                ok && count >= 3
                            }
                            _ => false,
                        }
                    };
                    if !is_hr {
                        end_idx
                    } else {
                        let mut before_hr = hr_idx;
                        while before_hr > body_start && lines[before_hr - 1].trim().is_empty() {
                            before_hr -= 1;
                        }
                        if before_hr <= body_start {
                            end_idx
                        } else {
                            hr_idx
                        }
                    }
                }
            };

            let span = off.line_start(heading_idx)..off.line_start(replace_end_idx);
            let replacement = if replace_heading {
                ensure_trailing_newline(new)
            } else {
                let heading_line_str = lines[heading_idx];
                let separator = if new.starts_with('\n') { "\n" } else { "\n\n" };
                format!(
                    "{}{}{}",
                    heading_line_str,
                    separator,
                    ensure_trailing_newline(new)
                )
            };
            Ok(vec![PlannedEdit {
                span,
                replacement,
                edit_index,
                order: edit_index,
            }])
        }

        "insert_before" => {
            let new = new_content.ok_or_else(|| {
                anyhow::anyhow!("content is required for the insert_before action")
            })?;
            let span = off.line_start(heading_idx)..off.line_start(heading_idx);
            Ok(vec![PlannedEdit {
                span,
                replacement: ensure_trailing_newline(new),
                edit_index,
                order: edit_index,
            }])
        }

        "insert_after" => {
            let new = new_content.ok_or_else(|| {
                anyhow::anyhow!("content is required for the insert_after action")
            })?;
            let insert_idx = match at.unwrap_or("end-of-section") {
                "end-of-section" => end_idx,
                "after-heading-line" => heading_idx + 1,
                other => {
                    return Err(anyhow::anyhow!(
                        "invalid at={:?}; expected 'end-of-section' (default) or 'after-heading-line'",
                        other
                    ));
                }
            };
            let span = off.line_start(insert_idx)..off.line_start(insert_idx);
            // EOF-append edge: `compute_section_end`'s fallback returns
            // `lines.len()` for the last section, and legacy's
            // `join_lines(&lines[..insert_idx])` unconditionally appends a
            // newline even for the full slice -- i.e. it equals `content +
            // "\n"`, not `content`. Reproduce that blank line here; this is
            // the one arm where `off.line_start(i)` does not equal the
            // legacy before-boundary when `i == lines.len()`.
            let prefix = if insert_idx == lines.len() { "\n" } else { "" };
            let replacement = format!("{prefix}{}", ensure_trailing_newline(new));
            Ok(vec![PlannedEdit {
                span,
                replacement,
                edit_index,
                order: edit_index,
            }])
        }

        "remove" => {
            let mut remove_end = end_idx;
            if remove_end < lines.len() && lines[remove_end].trim().is_empty() {
                remove_end += 1;
            }
            let span = off.line_start(heading_idx)..off.line_start(remove_end);
            Ok(vec![PlannedEdit {
                span,
                replacement: String::new(),
                edit_index,
                order: edit_index,
            }])
        }

        other => Err(anyhow::anyhow!(
            "invalid action {:?}; expected replace, insert_before, insert_after, or remove",
            other
        )),
    }
}
/// Extended form: `at` controls where `insert_after` lands. Pass
/// `Some("end-of-section")` (or `None`) for the historical behavior
/// of inserting at the end of the heading's section, or
/// `Some("after-heading-line")` to insert content immediately after
/// the heading line itself. Ignored by other actions.
///
/// `force` bypasses the F-7 surface-marker-preservation gate: when `false`
/// (default for the user-facing tool path), a `replace` whose new content
/// would drop `<!-- @surface NAME -->` or `<!-- @end -->` lines present in
/// the OLD body returns `RecoverableError` naming the lost markers. Pass
/// `true` to override (e.g. intentional structural change). See F-7 in
/// `docs/trackers/prompt-guide-refactor-session-log.md` for the bug-class
/// rationale.
///
/// Thin wrapper over `plan_section_edit` + `apply_planned_edits`: plans a
/// single edit at `edit_index=0` and applies it immediately, preserving the
/// historical single-edit string-in-string-out contract for callers that
/// don't need batch-mode overlap detection.
pub fn perform_section_edit_ext<'q, Q: Into<crate::tools::file_summary::HeadingQuery<'q>>>(
    content: &str,
    heading_query: Q,
    action: &str,
    new_content: Option<&str>,
    at: Option<&str>,
    force: bool,
) -> Result<String> {
    let off = LineOffsets::new(content);
    let edits = plan_section_edit(
        content,
        &off,
        heading_query,
        action,
        new_content,
        at,
        force,
        0,
    )?;
    Ok(apply_planned_edits(content, edits))
}

/// Compute the exclusive-end index (into `split('\n')` lines) for a section
/// that starts at `start_idx` (0-based) and has heading level `level`.
/// Skips headings inside fenced code blocks (``` ... ```).
fn compute_section_end(lines: &[&str], start_idx: usize, level: usize) -> usize {
    // Mirror parse_all_headings: if the fences in the slice are unbalanced,
    // treat them as plain text so an unclosed fence in the section's body
    // doesn't swallow the next sibling heading.
    let fences_balanced =
        crate::util::markdown_fence::fences_balanced(lines[start_idx..].iter().copied());

    let mut fence = crate::util::markdown_fence::FenceState::new();
    for (i, &line) in lines[start_idx..].iter().enumerate() {
        if fences_balanced && fence.feed(line) {
            continue;
        }
        if fence.in_fence() {
            continue;
        }
        if let Some(lvl) = crate::tools::file_summary::heading_level(line) {
            if lvl <= level {
                return start_idx + i;
            }
        }
    }
    lines.len()
}

/// List the sub-heading texts that a `replace` on `heading_query` would wipe.
///
/// BUG-043: when a section has nested sub-headings (deeper heading levels than
/// the target), `replace` silently consumes them. For plan/spec files whose
/// `##` sections contain dozens of `###` tasks, this causes catastrophic data
/// loss. Callers check this before `replace` and refuse unless the user opts
/// in via `include_subsections: true`.
///
/// Returns the headings with their `#` prefix intact so the error message can
/// echo them verbatim. Empty vec means the section has no children and `replace`
/// is safe.
pub fn find_consumed_subsections<'q, Q: Into<crate::tools::file_summary::HeadingQuery<'q>>>(
    content: &str,
    heading_query: Q,
) -> Result<Vec<String>> {
    use crate::tools::file_summary::{heading_level, resolve_section_range};

    let range =
        resolve_section_range(content, heading_query).map_err(|e| anyhow::anyhow!("{}", e))?;

    let lines: Vec<&str> = content.split('\n').collect();
    let heading_idx = range.heading_line - 1;
    let end_idx = compute_section_end(&lines, heading_idx + 1, range.level);

    let mut fence = crate::util::markdown_fence::FenceState::new();
    let mut out = Vec::new();
    for &line in &lines[heading_idx + 1..end_idx] {
        if fence.feed(line) {
            continue;
        }
        if fence.in_fence() {
            continue;
        }
        if heading_level(line).is_some() {
            out.push(line.trim_end().to_string());
        }
    }
    Ok(out)
}

/// Format the BUG-043 guard error. The message itself names `include_subsections`
/// so the opt-in is visible to any caller that only inspects the error text.
fn subsection_guard_error(
    batch_idx: Option<usize>,
    heading: &str,
    victims: &[String],
) -> RecoverableError {
    let prefix = match batch_idx {
        Some(i) => format!("edits[{i}]: "),
        None => String::new(),
    };
    RecoverableError::with_hint(
        format!(
            "{prefix}replace on '{heading}' would wipe {n} nested heading(s): {list}. \
             Pass include_subsections: true to opt into consuming children.",
            n = victims.len(),
            list = victims.join(", "),
        ),
        "Prefer action=\"edit\" with old_string/new_string to target text \
         inside the section without touching its subsections.",
    )
}

/// Resolve `action="edit"`'s replacement text, requiring the key to be **present**.
///
/// This used to be `edit["new_string"].as_str().unwrap_or("")` at three independent
/// sites, so omitting the key deleted every match of `old_string` and reported
/// success. The natural way to omit it is to pass `content` — the correct key for
/// `replace` / `insert_*`, declared in the same schema, and simply unread by this
/// action — which turned a one-word slip into silent data loss. The mirror mistake
/// (`replace` without `content`) was already refused *with a pointer to this
/// action*; only this direction fell through, and it fell into the destructive
/// branch.
///
/// Deleting via `edit` stays supported — pass `new_string: ""`. That explicit empty
/// string is the difference between asking for a deletion and forgetting the
/// replacement, which is precisely what the old default could not distinguish.
///
/// `prefix` locates the entry for batch callers (`"edits[3]: "`,
/// `"body_edits[0]: "`) and is empty for the single-edit path.
///
/// See `docs/issues/archive/2026-08-17-edit-markdown-edit-action-deletes-when-new-string-is-omitted.md`.
pub(crate) fn require_new_string<'a>(edit: &'a Value, prefix: &str) -> Result<&'a str> {
    let has_content = edit.get("content").is_some();

    if let Some(s) = edit.get("new_string").and_then(|v| v.as_str()) {
        // Both keys present: the caller is describing two different actions at
        // once. Ignoring one silently is how the original defect stayed invisible.
        if has_content {
            return Err(RecoverableError::with_hint(
                format!(
                    "{prefix}action=\"edit\" was given both new_string and content, \
                     and content is not read by this action"
                ),
                "content belongs to 'replace' / 'insert_before' / 'insert_after'. \
                 Drop content to keep the scoped swap, or change the action.",
            )
            .into());
        }
        return Ok(s);
    }

    let hint = if has_content {
        "Rename content to new_string — 'edit' performs a scoped old_string -> \
         new_string swap and never reads content (that key belongs to 'replace' / \
         'insert_before' / 'insert_after'). To DELETE the matched text, pass \
         new_string=\"\" explicitly."
    } else {
        "Pass the replacement for old_string, e.g. new_string=\"let x = 2;\". \
         To DELETE the matched text, pass new_string=\"\" explicitly."
    };
    Err(RecoverableError::with_hint(
        format!("{prefix}new_string is required for action=\"edit\""),
        hint,
    )
    .into())
}

// No longer called from production code as of Task 4 (perform_scoped_edit now
// delegates to plan_scoped_edit + apply_planned_edits); still exercised by
// #[cfg(test)] tests. Unused for now -- a later task strips this allow.
#[allow(dead_code)]
/// Join a non-tail slice of lines back into a string.
/// Always appends a '\n' after the last element to act as a separator.
pub(crate) fn join_lines(lines: &[&str]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    format!("{}\n", lines.join("\n"))
}

/// Maps 0-based line indices (per `content.split('\n')`) to byte offsets in `content`.
/// `line_start(i)` is the byte where line `i` begins; `line_start(line_count)` and any
/// larger index return `content.len()`. Because `join_lines(&lines[..i])` re-adds the
/// newline that `split('\n')` removed, `content[..line_start(i)] == join_lines(&lines[..i])`
/// and `content[line_start(j)..] == join_lines_tail(&lines[j..])` — this is what lets a
/// byte-offset splice reproduce the existing before/section/after boundaries exactly.
pub(crate) struct LineOffsets {
    starts: Vec<usize>,
    len: usize,
}

impl LineOffsets {
    pub(crate) fn new(content: &str) -> Self {
        let mut starts = vec![0usize];
        for (i, b) in content.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Self {
            starts,
            len: content.len(),
        }
    }

    pub(crate) fn line_start(&self, idx: usize) -> usize {
        self.starts.get(idx).copied().unwrap_or(self.len)
    }
}

// No longer called from production code as of Task 4 (perform_scoped_edit now
// delegates to plan_scoped_edit + apply_planned_edits). Unused for now -- a
// later task strips this allow.
#[allow(dead_code)]
/// Join a tail slice (including any trailing "" from split('\n')).
fn join_lines_tail(lines: &[&str]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    lines.join("\n")
}

/// Ensure `s` ends with exactly one newline.
fn ensure_trailing_newline(s: &str) -> String {
    if s.ends_with('\n') {
        s.to_owned()
    } else {
        format!("{}\n", s)
    }
}

/// Normalise the final result to end with exactly one newline.
fn normalize_trailing_newline(s: &str) -> String {
    let trimmed = s.trim_end_matches('\n');
    format!("{}\n", trimmed)
}
#[derive(Debug, Clone)]
pub(crate) struct PlannedEdit {
    pub span: Range<usize>,
    pub replacement: String,
    pub edit_index: usize, // user-facing edits[] index, for error messages
    pub order: usize,      // collection order; tie-break for coincident inserts
}

/// Byte range of each CRLF-tolerant match of `old_string` within `text`: exact
/// except for a lone trailing `\r` per line. Mirrors `edit_file`'s
/// `find_crlf_tolerant_windows` (see that function's doc comment for the
/// real-world trigger) — a Windows-checked-out markdown file has `\r\n` line
/// endings, but a multi-line `old_string` arrives with bare `\n` newlines (the
/// normal shape for an MCP payload), so the exact byte match fails at every
/// line boundary even though the content is otherwise identical. Returned
/// ranges are relative to `text` (a section slice, not the whole file).
fn find_crlf_tolerant_ranges(text: &str, old_string: &str) -> Vec<(usize, usize)> {
    let old_lines: Vec<&str> = old_string
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let k = old_lines.len();
    if k == 0 {
        return Vec::new();
    }
    let mut spans: Vec<(&str, usize, usize)> = Vec::new();
    let mut offset = 0usize;
    for raw in text.split_inclusive('\n') {
        let no_lf = raw.strip_suffix('\n').unwrap_or(raw);
        let line_text = no_lf.strip_suffix('\r').unwrap_or(no_lf);
        spans.push((line_text, offset, offset + line_text.len()));
        offset += raw.len();
    }
    let mut out = Vec::new();
    if spans.len() < k {
        return out;
    }
    for i in 0..=(spans.len() - k) {
        if (0..k).all(|j| spans[i + j].0 == old_lines[j]) {
            out.push((spans[i].1, spans[i + k - 1].2));
        }
    }
    out
}

fn spans_conflict(a: &Range<usize>, b: &Range<usize>) -> bool {
    let a_zero = a.start == a.end;
    let b_zero = b.start == b.end;
    match (a_zero, b_zero) {
        (true, true) => false,
        (true, false) => b.start < a.start && a.start < b.end,
        (false, true) => a.start < b.start && b.start < a.end,
        (false, false) => a.start < b.end && b.start < a.end,
    }
}

pub(crate) fn detect_overlaps(edits: &[PlannedEdit]) -> anyhow::Result<()> {
    for i in 0..edits.len() {
        for j in (i + 1)..edits.len() {
            if spans_conflict(&edits[i].span, &edits[j].span) {
                return Err(RecoverableError::with_hint(
                    format!(
                        "edits[{}] and edits[{}] rewrite overlapping regions (bytes {:?} and {:?})",
                        edits[i].edit_index, edits[j].edit_index, edits[i].span, edits[j].span
                    ),
                    "Split into separate edit_file calls, or target disjoint regions.",
                )
                .into());
            }
        }
    }
    Ok(())
}

pub(crate) fn apply_planned_edits(original: &str, mut edits: Vec<PlannedEdit>) -> String {
    // End-to-start (highest start first) so an applied splice never shifts a
    // not-yet-applied lower offset. At an EQUAL start, a non-zero span MUST apply
    // before a coincident zero-width insert: applying the insert first would shift
    // the shared start byte and make the span's replace_range corrupt the buffer
    // (C-1). Among coincident zero-width inserts, higher `order` first so the lower
    // `order` ends up leftmost in the final document.
    edits.sort_by(|a, b| {
        let a_zero = a.span.start == a.span.end;
        let b_zero = b.span.start == b.span.end;
        b.span
            .start
            .cmp(&a.span.start)
            .then(a_zero.cmp(&b_zero)) // false (non-zero) sorts before true (zero-width)
            .then(b.order.cmp(&a.order))
    });
    let mut out = original.to_string();
    for e in &edits {
        out.replace_range(e.span.clone(), &e.replacement);
    }
    normalize_trailing_newline(&out)
}

/// Plan a whole `edits[]` batch against a single, unmutated `snapshot`.
///
/// Every entry resolves its `heading` against `snapshot` — never against a
/// running mutated buffer — which is what makes batch application
/// order-independent: a rename of a heading and an edit scoped under that
/// same heading both resolve against the pre-edit document, so either
/// ordering in the input array produces the same planned spans (and thus,
/// after `apply_planned_edits`, the same output). Collects every edit's
/// `PlannedEdit`s, runs `detect_overlaps` once over the full set, and
/// returns the validated plan for the caller to apply.
pub(crate) fn plan_batch(snapshot: &str, edits: &[Value], force: bool) -> Result<Vec<PlannedEdit>> {
    let off = LineOffsets::new(snapshot);
    let mut planned: Vec<PlannedEdit> = Vec::new();

    for (i, edit) in edits.iter().enumerate() {
        let heading = edit["heading"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("edits[{}]: missing required 'heading' field", i))?;
        let action = edit["action"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("edits[{}]: missing required 'action' field", i))?;
        // 1-indexed selector among identical headings. Absent keeps today's contract:
        // one match resolves, several are an ambiguity error.
        let query = crate::tools::file_summary::HeadingQuery::new(
            heading,
            edit["occurrence"].as_u64().map(|n| n as usize),
        );

        let mut sub = if action == "edit" {
            let old_string = edit["old_string"].as_str().ok_or_else(|| {
                anyhow::anyhow!("edits[{}]: old_string is required for action='edit'", i)
            })?;
            let new_string = require_new_string(edit, &format!("edits[{i}]: "))?;
            let replace_all = edit["replace_all"].as_bool().unwrap_or(false);
            plan_scoped_edit(
                snapshot,
                &off,
                query,
                old_string,
                new_string,
                replace_all,
                i,
            )
            .map_err(|e| {
                prefix_scoped_error(
                    e,
                    &format!("edits[{i}]: "),
                    "Check heading name and old_string content.",
                )
            })?
        } else {
            if action == "replace" && !edit["include_subsections"].as_bool().unwrap_or(false) {
                if let Ok(victims) = find_consumed_subsections(snapshot, query) {
                    if !victims.is_empty() {
                        return Err(subsection_guard_error(Some(i), heading, &victims).into());
                    }
                }
            }
            plan_section_edit(
                snapshot,
                &off,
                query,
                action,
                edit["content"].as_str(),
                edit["at"].as_str(),
                force,
                i,
            )
            .map_err(|e| {
                RecoverableError::with_hint(
                    format!("edits[{}]: {}", i, e),
                    "Check heading name and action.",
                )
            })?
        };
        planned.append(&mut sub);
    }

    detect_overlaps(&planned)?;
    Ok(planned)
}

/// Reserved `heading` value naming the PREAMBLE region — text between frontmatter and
/// the first heading — in `plan_scoped_edit`'s `action="edit"` path. No real heading text
/// can equal this: `HeadingInfo::text` always carries its leading `#` markers.
/// docs/issues/archive/2026-08-19-guarded-artifact-preamble-cannot-be-edited.md
pub(crate) const PREAMBLE_SENTINEL: &str = "^";

/// Human-readable label for the preamble region in `diagnose_scoped_miss`'s error text,
/// standing in for `heading_query` when that query is `PREAMBLE_SENTINEL`.
const PREAMBLE_LABEL: &str = "(preamble)";

/// Plan the byte-span edit(s) that a heading-scoped `old_string` -> `new_string`
/// replacement would produce, without applying them. Finds the section identified
/// by `heading_query`, locates `old_string` within its byte range, and emits one
/// `PlannedEdit` per match. An ambiguous `old_string` -- more than one match in the
/// section, with `replace_all` false -- is REFUSED rather than resolved to the first,
/// because picking silently writes to a well-formed wrong target (see the ambiguity
/// gate below for the measured cost). `replace_all` true emits one edit per
/// non-overlapping match. Behavior-identical to the historical `perform_scoped_edit`
/// (see that function's doc comment) -- `perform_scoped_edit` is now a thin wrapper
/// delegating here + `apply_planned_edits`.
///
/// `heading_query == PREAMBLE_SENTINEL` (`"^"`) targets the PREAMBLE instead of a named
/// section: the text between frontmatter and the first heading, which otherwise has no
/// section to name and so is unreachable by this function at all. No real heading text
/// can equal the bare sentinel — `HeadingInfo::text` always carries its `#` markers — so
/// there is no collision to guard against. A file with no headings at all treats its
/// entire content as the preamble.
/// docs/issues/archive/2026-08-19-guarded-artifact-preamble-cannot-be-edited.md
pub(crate) fn plan_scoped_edit<'q, Q: Into<crate::tools::file_summary::HeadingQuery<'q>>>(
    content: &str,
    off: &LineOffsets,
    heading_query: Q,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
    edit_index: usize,
) -> Result<Vec<PlannedEdit>> {
    use crate::tools::file_summary::{parse_all_headings, resolve_section_range};

    // Destructure before the sentinel comparison: the PREAMBLE check is on the text,
    // and every diagnostic below echoes the text, not the selector.
    let query = heading_query.into();
    let heading_query = query.text;

    let (sec_start, sec_end, diag_label) = if heading_query == PREAMBLE_SENTINEL {
        let sec_end = parse_all_headings(content)
            .first()
            .map(|h| off.line_start(h.line - 1))
            .unwrap_or(content.len());
        (0, sec_end, PREAMBLE_LABEL.to_string())
    } else {
        let range = resolve_section_range(content, query).map_err(|e| anyhow::anyhow!("{}", e))?;
        let lines: Vec<&str> = content.split('\n').collect();
        let heading_idx = range.heading_line - 1;
        let end_idx = compute_section_end(&lines, heading_idx + 1, range.level);
        // Name the heading the resolver actually BOUND, and disclose the query when the two
        // differ. Tiers 3-4 are fuzzy and first-match-wins, so a query can bind a section the
        // caller never named -- and every diagnostic below is then a TRUE statement about the
        // bound section that reads as a FALSE one about the intended section. Measured: a
        // caller who queried "Index" got `not found in section 'Index'` for a section that was
        // never Index, and re-read their own `old_string` twice before questioning the heading.
        // docs/issues/2026-09-03-a-bare-heading-query-cannot-reach-the-exact-match-tiers.md
        let label = if range.heading_text == heading_query {
            range.heading_text.clone()
        } else {
            format!(
                "{} (resolved from \"{}\")",
                range.heading_text, heading_query
            )
        };
        (off.line_start(heading_idx), off.line_start(end_idx), label)
    };
    let section = &content[sec_start..sec_end];

    if !section.contains(old_string) {
        // CRLF-tolerant fallback: only kicks in when the exact match failed and
        // there's exactly one tolerant match (same conservative uniqueness gate
        // edit_file uses), so it never silently picks among ambiguous candidates.
        let crlf_ranges = find_crlf_tolerant_ranges(section, old_string);
        if crlf_ranges.len() == 1 {
            let (rel_start, rel_end) = crlf_ranges[0];
            let matched = &section[rel_start..rel_end];
            // Adapt the replacement's line endings to match this region's convention
            // so the edit doesn't leave a mixed CRLF/LF block behind.
            let adapted = if matched.contains("\r\n") {
                new_string.replace("\r\n", "\n").replace('\n', "\r\n")
            } else {
                new_string.replace("\r\n", "\n")
            };
            let mstart = sec_start + rel_start;
            let mend = sec_start + rel_end;
            let mut edits = vec![PlannedEdit {
                span: mstart..mend,
                replacement: adapted,
                edit_index,
                order: edit_index * 1_000,
            }];
            if !new_string.ends_with('\n') {
                if let Some(last) = edits.last_mut() {
                    if last.span.end == sec_end {
                        last.replacement.push('\n');
                    }
                }
            }
            return Ok(edits);
        }

        return Err(diagnose_scoped_miss(section, old_string, &diag_label).into());
    }

    // AMBIGUITY GATE. Without it the loop below takes the FIRST match and the call
    // returns `status: "ok"`, so a short anchor performs a well-formed write to the
    // wrong target with nothing for the caller to notice. This is the same conservative
    // uniqueness rule the CRLF fallback directly above already cites -- "so it never
    // silently picks among ambiguous candidates" -- and that `edit_file`'s text grammar
    // enforces (`src/tools/edit_file/mod.rs`). The markdown grammar arrived beside that
    // guard without inheriting it; `doc(action="update", patch={body_edits})` and
    // `edit_file`'s heading form both funnel here, so this is the one site.
    //
    // Measured cost of its absence: `old_string="---"` is a horizontal rule, a
    // frontmatter delimiter AND a substring of every GFM table separator row, so the
    // first match split a table's header from its separator row in a live tracker.
    // docs/issues/archive/2026-09-03-scoped-edit-silently-takes-the-first-of-several-old-string-matches.md
    //
    // The count is SECTION-scoped because the edit is; the reported lines are
    // FILE-relative because that is the coordinate the caller navigates to.
    if !replace_all {
        let hits: Vec<usize> = section
            .match_indices(old_string)
            .map(|(rel, _)| sec_start + rel)
            .collect();
        if hits.len() > 1 {
            let lines = hits
                .iter()
                // NOT `content[..off].lines().count() + 1`: `lines()` counts a partial
                // trailing line, so a match starting mid-line reports N+1. Counting
                // newlines is correct at every offset.
                .map(|&off| (content[..off].matches('\n').count() + 1).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(RecoverableError::with_hint(
                format!(
                    "old_string found {} times in {diag_label} (lines {lines}) — refusing \
                     rather than editing the first",
                    hits.len()
                ),
                "Expand old_string with a neighbouring line so exactly one match remains, \
                 or set replace_all: true to change every occurrence. A short anchor like \
                 \"---\" also matches every table separator row.",
            )
            .into());
        }
    }

    let mut edits = Vec::new();
    let mut search_from = 0usize;
    let mut order = edit_index * 1_000; // headroom so multi-span edits keep global order
    while let Some(rel) = section[search_from..].find(old_string) {
        let mstart = sec_start + search_from + rel;
        let mend = mstart + old_string.len();
        edits.push(PlannedEdit {
            span: mstart..mend,
            replacement: new_string.to_string(),
            edit_index,
            order,
        });
        order += 1;
        search_from += rel + old_string.len().max(1);
        if !replace_all {
            break;
        }
    }

    // Class-A fusion guard (mirrors legacy's `ensure_trailing_newline(&new_section)`):
    // if the rightmost match consumes the section's trailing newline (the byte just
    // before the next heading, or EOF) and `new_string` doesn't restore it, splice in
    // a corrective newline so the edited section never fuses onto what follows.
    if !new_string.ends_with('\n') {
        if let Some(last) = edits.last_mut() {
            if last.span.end == sec_end {
                last.replacement.push('\n');
            }
        }
    }

    Ok(edits)
}

/// Prefix a scoped-edit error's message while PRESERVING a rich `RecoverableError`
/// (its tier-adaptive hint + `extra`). Only `old_string` misses arrive as a
/// downcastable `RecoverableError` (from `diagnose_scoped_miss`); a heading-not-found
/// arrives as a plain `anyhow` and takes the generic `fallback_hint`.
pub(crate) fn prefix_scoped_error(
    e: anyhow::Error,
    prefix: &str,
    fallback_hint: &str,
) -> anyhow::Error {
    match e.downcast::<RecoverableError>() {
        Ok(mut rec) => {
            if !prefix.is_empty() {
                rec.message = format!("{prefix}{}", rec.message);
            }
            rec.into()
        }
        Err(other) => RecoverableError::with_hint(format!("{prefix}{other}"), fallback_hint).into(),
    }
}

/// Perform a heading-scoped string replacement within a markdown file.
///
/// Finds the section identified by `heading_query`, locates `old_string` within it,
/// and replaces with `new_string`. If `replace_all` is true, replaces every occurrence
/// within the section; otherwise the match must be UNIQUE within the section -- two or
/// more is an error naming the count and each match's file-relative line, never a
/// silent edit of the first.
///
/// Returns the full modified file content.
///
/// Thin wrapper over `plan_scoped_edit` + `apply_planned_edits`: plans the edit(s) at
/// `edit_index=0` and applies them immediately, preserving the historical single-edit
/// string-in-string-out contract for callers that don't need batch-mode overlap detection.
pub(crate) fn perform_scoped_edit<'q, Q: Into<crate::tools::file_summary::HeadingQuery<'q>>>(
    content: &str,
    heading_query: Q,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<String> {
    let off = LineOffsets::new(content);
    let edits = plan_scoped_edit(
        content,
        &off,
        heading_query,
        old_string,
        new_string,
        replace_all,
        0,
    )?;
    Ok(apply_planned_edits(content, edits))
}

// ---------------------------------------------------------------------------
// Markdown edit implementation
// ---------------------------------------------------------------------------

/// Apply a JSON-shaped frontmatter mutation request to a markdown source.
///
/// `param` is the value of the tool's `frontmatter` field — expected to be an
/// object with optional `set` (object: key → JSON value) and `delete` (array of
/// strings) sub-fields. At least one of the two must be non-empty.
///
/// When the file has no existing frontmatter block, `set:` operations
/// synthesize a new block at the head of the file; `delete:`-only operations
/// are an idempotent no-op (nothing to delete from a non-existent block).
///
/// Returns the rewritten file content with the frontmatter block updated and
/// the body preserved verbatim.
pub(super) fn apply_frontmatter_mutation(content: &str, param: &Value) -> Result<String> {
    let obj = param.as_object().ok_or_else(|| {
        RecoverableError::with_hint(
            "frontmatter param must be an object",
            "Pass `frontmatter: {set: {key: value}, delete: [keys]}`.",
        )
    })?;

    let set: serde_json::Map<String, Value> = obj
        .get("set")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let delete: Vec<String> = obj
        .get("delete")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    if set.is_empty() && delete.is_empty() {
        return Err(RecoverableError::with_hint(
            "frontmatter param requires at least one of `set` or `delete`",
            "Pass `frontmatter: {set: {key: value}}` or `frontmatter: {delete: [keys]}`.",
        )
        .into());
    }

    match frontmatter::extract_frontmatter(content)? {
        Some(fm) => {
            let new_block = frontmatter::apply_ops(&fm.lines, &set, &delete)?;
            Ok(frontmatter::splice_back(content, &new_block, &fm))
        }
        None => {
            if set.is_empty() {
                return Ok(content.to_string());
            }
            let new_block = frontmatter::apply_ops(&[], &set, &delete)?;
            let mut out = String::from("---\n");
            for line in &new_block {
                out.push_str(line);
                out.push('\n');
            }
            out.push_str("---\n");
            if !content.is_empty() && !content.starts_with('\n') {
                out.push('\n');
            }
            out.push_str(content);
            Ok(out)
        }
    }
}
/// Normalized Levenshtein similarity in [0.0, 1.0] (1.0 = identical). Used only
/// to LOCATE the closest line for a miss diagnostic — never to alter bytes.
pub(crate) fn similarity(a: &str, b: &str) -> f64 {
    strsim::normalized_levenshtein(a, b)
}

/// Strip whitespace + look-alike/invisible chars, leaving only "visible" glyphs.
/// Two lines with equal visible projections but different bytes differ ONLY in
/// whitespace/invisibles — the Tier A signal.
fn visible_projection(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{00A0}' && *c != '\u{200B}' && *c != '\u{FEFF}')
        .collect()
}

pub(crate) fn render_visible_whitespace(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for c in line.chars() {
        match c {
            ' ' => out.push('·'),
            '\t' => out.push('→'),
            '\r' => out.push_str("⟨CR⟩"),
            '\u{00A0}' => out.push_str("⟨NBSP⟩"),
            '\u{200B}' => out.push_str("⟨ZWSP⟩"),
            '\u{FEFF}' => out.push_str("⟨BOM⟩"),
            other => out.push(other),
        }
    }
    out
}

fn leading_ws(s: &str) -> String {
    s.chars()
        .take_while(|c| c.is_whitespace() || *c == '\u{00A0}')
        .collect()
}

/// `Some(reason)` iff `want`/`have` have identical visible projections but differ
/// byte-wise (Tier A). `None` when visible content differs (Tier B).
pub(crate) fn classify_whitespace_diff(want: &str, have: &str) -> Option<String> {
    if want == have || visible_projection(want) != visible_projection(have) {
        return None;
    }
    let mut notes: Vec<String> = Vec::new();
    if have.contains('\u{00A0}') || want.contains('\u{00A0}') {
        notes.push("non-breaking space (U+00A0) present — looks like a normal space".into());
    }
    if have.contains('\u{200B}')
        || want.contains('\u{200B}')
        || have.contains('\u{FEFF}')
        || want.contains('\u{FEFF}')
    {
        notes.push("zero-width / BOM character present (U+200B / U+FEFF)".into());
    }
    if have.contains('\r') != want.contains('\r') {
        notes.push("line endings differ (a CR is present on one side: CRLF vs LF)".into());
    }
    let (wi, hi) = (leading_ws(want), leading_ws(have));
    if wi != hi {
        notes.push(format!(
            "leading indentation differs — file: \"{}\", old_string: \"{}\"",
            render_visible_whitespace(&hi),
            render_visible_whitespace(&wi),
        ));
    }
    let want_trail = want.len() - want.trim_end().len();
    let have_trail = have.len() - have.trim_end().len();
    if want_trail != have_trail {
        notes.push(format!(
            "trailing whitespace differs (file has {have_trail} trailing ws byte(s), old_string has {want_trail})"
        ));
    }
    if notes.is_empty() {
        notes.push("interior whitespace differs (tabs vs spaces / repeated spaces)".into());
    }
    Some(notes.join("; "))
}

/// True iff `line_idx` (0-based into section.split('\n')) is inside a fenced
/// `` ``` `` or `~~~` block or is an indented (≥4 leading spaces / a tab) code
/// line.
/// Whitespace there is significant — the caller warns the agent not to
/// normalize it.
pub(crate) fn line_in_code_block(section: &str, line_idx: usize) -> bool {
    let mut fence = crate::util::markdown_fence::FenceState::new();
    for (i, line) in section.split('\n').enumerate() {
        let t = line.trim_start();
        if fence.feed(t) {
            // The delimiter line itself is code.
            if i == line_idx {
                return true;
            }
            continue;
        }
        if i == line_idx {
            if fence.in_fence() {
                return true;
            }
            return line.starts_with("    ") || line.starts_with('\t');
        }
    }
    false
}

const SIM_THRESHOLD: f64 = 0.5;
const SECTION_LINE_CAP: usize = 400;
const SECTION_BYTE_CAP: usize = 65_536;
const OLD_STRING_CAP: usize = 8192;

fn truncate_snippet(s: &str) -> String {
    const MAX: usize = 200;
    if s.chars().count() <= MAX {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(MAX).collect();
        t.push('…');
        t
    }
}

/// Locate the closest line/window to `old_string` within `section`, classify
/// the miss into a tier (whitespace-only / visible drift / no close match),
/// and build a `RecoverableError` carrying a tier-adaptive hint plus
/// `extra["scoped_miss_tier"]` for callers (Task 5) to route on.
///
/// Each bail-out cause gets its own tier value and message — a caller error
/// (empty/oversized `old_string`), a declined search (section over the line or
/// byte cap), and a genuine no-match are different problems with different
/// fixes, and reporting them identically forces a blind retry regardless of
/// which one actually happened.
pub(crate) fn diagnose_scoped_miss(
    section: &str,
    old_string: &str,
    heading: &str,
) -> RecoverableError {
    use serde_json::json;

    let no_close = |extra_note: &str, tier: &str| {
        RecoverableError::with_hint(
            format!(
                "old_string not found in section '{heading}'. The text must match exactly (whitespace-sensitive). {extra_note}"
            ),
            "old_string isn't in this section — verify the heading, or re-read the current section text and retry.",
        )
        .with_extra("scoped_miss_tier", json!(tier))
    };

    if old_string.is_empty() {
        return no_close("old_string is empty.", "old_string_empty");
    }
    if old_string.len() > OLD_STRING_CAP {
        return no_close(
            "old_string exceeds 8 KB, so no closest-match search was run — target a smaller, unique anchor.",
            "old_string_too_large",
        );
    }

    let lines: Vec<&str> = section.split('\n').collect();
    if lines.len() > SECTION_LINE_CAP {
        return no_close(
            "this section has more than 400 lines, so no closest-match search was attempted — re-read the section and copy an exact anchor.",
            "section_too_many_lines",
        );
    }
    if section.len() > SECTION_BYTE_CAP {
        return no_close(
            "this section is larger than 64 KB, so no closest-match search was attempted — re-read the section and copy an exact anchor.",
            "section_too_many_bytes",
        );
    }

    let old_lines: Vec<&str> = old_string.split('\n').collect();
    let n = old_lines.len();
    if n > lines.len() {
        return no_close(
            "old_string has more lines than the section itself, so it cannot match anything in it.",
            "old_string_longer_than_section",
        );
    }

    let mut best_idx = 0usize;
    let mut best_score = -1.0f64;
    for start in 0..=(lines.len() - n) {
        let window = lines[start..start + n].join("\n");
        let s = similarity(&window, old_string);
        if s > best_score {
            best_score = s;
            best_idx = start;
        }
    }
    if best_score < SIM_THRESHOLD {
        return no_close(
            "I looked, and nothing scored above 0.5 similarity.",
            "no_similar_match",
        );
    }

    let have_window = lines[best_idx..best_idx + n].join("\n");
    let in_code = (best_idx..best_idx + n).any(|i| line_in_code_block(section, i));
    let code_note = if in_code {
        "\nnote: inside a code block — whitespace is significant; copy the bytes exactly."
    } else {
        ""
    };

    let all_ws = old_lines
        .iter()
        .zip(have_window.split('\n'))
        .all(|(w, h)| w == &h || classify_whitespace_diff(w, h).is_some());

    if all_ws {
        let classes: Vec<String> = old_lines
            .iter()
            .zip(have_window.split('\n'))
            .filter_map(|(w, h)| classify_whitespace_diff(w, h))
            .collect();
        RecoverableError::with_hint(
            format!(
                "old_string not found in section '{heading}'. Closest line differs only in whitespace/invisible characters: {cls}.\n  want: {w}\n  have: {h}{code_note}",
                cls = classes.join("; "),
                w = render_visible_whitespace(&truncate_snippet(old_string)),
                h = render_visible_whitespace(&truncate_snippet(&have_window)),
            ),
            "Copy the exact bytes shown for `have` — the only difference is invisible whitespace.",
        )
        .with_extra("scoped_miss_tier", json!("whitespace_invisible"))
    } else {
        RecoverableError::with_hint(
            format!(
                "old_string not found in section '{heading}'. Closest text (did it change since you read it?):\n  want: {w}\n  have: {h}{code_note}",
                w = truncate_snippet(old_string),
                h = truncate_snippet(&have_window),
            ),
            "The text changed since you last read it — re-read this section for the current value, then retry with it.",
        )
        .with_extra("scoped_miss_tier", json!("visible_drift"))
    }
}
pub(crate) const LONG_DOCS: &str =
    "### Workflow: Editing a Markdown Document\n\n\
     | Step | Tool | Purpose |\n\
     |------|------|---------|\n\
     | 1 | `read_file(path)` | Get heading map — see all sections |\n\
     | 2 | `read_file(path, headings=[...])` | Read target sections (one call, multiple sections) |\n\
     | 3a | `edit_file(path, heading, action, content)` | Whole-section: replace (body only — heading preserved), insert, remove |\n\
     | 3b | `edit_file(path, heading, action=\"edit\", old_string, new_string)` | Surgical: scoped string replacement within a section |\n\
     | 3c | `edit_file(path, edits=[...])` | Batch: multiple edits across sections, atomic |\n\
     | 3d | `edit_file(path, frontmatter={set: {status: \"fixed\"}})` | Mutate the YAML frontmatter block (status flips, closed dates, etc.) without sed. Combinable with any body edit above — one atomic write covers both. |\n\n\
     ### Action semantics — pick the right verb\n\n\
     | Action | Effect on target section | Use when |\n\
     |---|---|---|\n\
     | `replace` | **OVERWRITES the entire body** (from line after the heading until next sibling heading). Heading preserved; subsections refused unless `include_subsections=true`. | The whole section body should be rewritten from scratch (e.g. refreshing a stale memory table). |\n\
     | `insert_before` / `insert_after` | Adds a new sibling section before/after the target. Target body **preserved**. `at=\"end-of-section\"` (default) or `\"after-heading-line\"` for `insert_after`. | Adding adjacent sections without touching the target's body. |\n\
     | `remove` | Deletes target section (heading + body). | Removing a section entirely. |\n\
     | `edit` | Surgical text replacement within the section via `old_string` / `new_string`. Surrounding body preserved. | Fixing a typo, updating a single line, scoped substring change. |\n\n\
     **Common footgun:** reaching for `action=\"replace\"` when you meant `action=\"insert_after\"`. `replace` destroys the existing body; `insert_after` adds adjacent without loss. Verify-after-edit with `read_file(path, heading=\"...\")` on any non-trivial mutation.";

/// Heading-addressed markdown edit. Reached through `edit_file`, which has already run
/// `guard_worktree_write` and `maybe_replay_ack` and decided the call is markdown grammar.
pub(crate) async fn edit(input: Value, ctx: &ToolContext) -> Result<Value> {
    let path = crate::tools::require_str_param_or_hint(
        &input,
        "path",
        crate::fs::PATH_PARAM_ALIASES,
        "edit_file(path=\"docs/x.md\", heading=\"## Section\", action=\"replace\", content=\"...\"). path is required on every call.",
    )?;

    let resolved =
        match crate::tools::resolve_write_or_capture(ctx, "edit_file", &input, path).await? {
            crate::tools::WriteOutcome::Write(p) => p,
            crate::tools::WriteOutcome::Pending(env) => return Ok(env),
        };

    let file_content = std::fs::read_to_string(&resolved)?;

    // Reject librarian-managed artifacts — use doc(action="update") instead.
    // Passing the resolved path also catches augmented artifacts with no
    // frontmatter id, where a direct write desynchronises file from params.
    //
    // `access` is what the caller's own arguments already say: `frontmatter` present
    // means this call mutates catalog-indexed keys, absent means it is body-only.
    // That distinction is the whole reason a merely-STAMPED file no longer refuses
    // an ordinary prose edit — the drift BL-48 describes is a frontmatter drift, and
    // this is the one call site that can prove it is not doing one.
    // docs/issues/archive/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md
    let access = if input["frontmatter"].is_object() {
        crate::util::librarian_guard::Access::FrontmatterWrite
    } else {
        crate::util::librarian_guard::Access::BodyWrite
    };
    crate::util::librarian_guard::guard_not_librarian_managed(
        path,
        &file_content,
        Some(&resolved),
        access,
    )?;

    // Working buffer — frontmatter mutation (if requested) lands here first,
    // then body edits run on the result. One atomic_write at the end keeps
    // mixed frontmatter+body edits transactional.
    let mut new_content = file_content.clone();

    // ── Frontmatter mutation (optional) ──────────────────────────
    let frontmatter_changed = if input["frontmatter"].is_object() {
        new_content = apply_frontmatter_mutation(&new_content, &input["frontmatter"])?;
        true
    } else {
        false
    };

    // ── Body edit mode detection ─────────────────────────────────
    let has_edits = input["edits"].is_array();
    let has_heading = input["heading"].is_string();
    let has_action = input["action"].is_string();

    if has_edits && (has_heading || has_action) {
        return Err(RecoverableError::with_hint(
            "edits array is mutually exclusive with top-level heading/action",
            "Use either edits=[] for batch mode, or heading+action for single edit.",
        )
        .into());
    }

    let has_body_edit = has_edits || has_heading || has_action;
    if !frontmatter_changed && !has_body_edit {
        return Err(RecoverableError::with_hint(
            "no operation specified",
            "Pass `frontmatter: {set:{...}, delete:[...]}`, `edits=[...]`, or `heading`+`action`.",
        )
        .into());
    }

    if has_edits {
        // ── Batch mode ───────────────────────────────────────────
        // Every edit resolves its heading against `snapshot` (the buffer as it
        // stood right after frontmatter mutation, before any body edit) rather
        // than a running mutated buffer — this is what makes the batch
        // order-independent. See `plan_batch`.
        let edits = input["edits"].as_array().unwrap();
        let snapshot = new_content.clone();
        let planned = plan_batch(&snapshot, edits, input["force"].as_bool().unwrap_or(false))?;
        new_content = apply_planned_edits(&snapshot, planned);
    } else if has_body_edit {
        // ── Single edit mode ─────────────────────────────────────
        let heading = crate::tools::require_str_param_or_hint(
            &input,
            "heading",
            &[],
            "Name the section to edit, e.g. heading=\"## Section\". For multiple edits use edits=[{heading, action, content}].",
        )?;
        // 1-indexed selector among identical headings — the only way to reach
        // either of two byte-identical ones. See `HeadingQuery`.
        let query = crate::tools::file_summary::HeadingQuery::new(
            heading,
            input["occurrence"].as_u64().map(|n| n as usize),
        );
        let action = crate::tools::require_str_param_or_hint(
            &input,
            "action",
            &[],
            "Set action to one of: replace | insert_before | insert_after | remove | edit. E.g. action=\"replace\", content=\"...\".",
        )?;

        new_content = if action == "edit" {
            let old_string = crate::tools::require_str_param(&input, "old_string")?;
            let new_string = require_new_string(&input, "")?;
            let replace_all_val = parse_bool_param(&input["replace_all"]);
            perform_scoped_edit(&new_content, query, old_string, new_string, replace_all_val)
                .map_err(|e| prefix_scoped_error(e, "", "Check heading name and old_string."))?
        } else {
            let content = input["content"].as_str();
            if action == "replace" && !input["include_subsections"].as_bool().unwrap_or(false) {
                if let Ok(victims) = find_consumed_subsections(&new_content, query) {
                    if !victims.is_empty() {
                        return Err(subsection_guard_error(None, heading, &victims).into());
                    }
                }
            }
            perform_section_edit_ext(
                &new_content,
                query,
                action,
                content,
                input["at"].as_str(),
                input["force"].as_bool().unwrap_or(false),
            )
            .map_err(|e| {
                RecoverableError::with_hint(e.to_string(), "Check heading name and action.")
            })?
        };
    }

    // ── Body-shrink guard ──────────────────────────────────────
    // Refuse a write that cuts the file by >50% in EITHER bytes or lines,
    // unless the caller passed `force: true`. The predicate, the 200-byte
    // floor and the reason there are two dimensions all live in
    // crate::util::shrink_guard, shared with doc(update) and
    // memory(write) — three private copies is how the line-truncation gap
    // survived being fixed once.
    let force = input["force"].as_bool().unwrap_or(false);
    if !force {
        if let Some(report) = crate::util::shrink_guard::check(&file_content, &new_content) {
            return Err(RecoverableError::with_hint(
                format!(
                    "body-shrink guard: write to {} {}",
                    resolved.display(),
                    report.describe()
                ),
                "Use action='edit' with old_string/new_string for surgical mutation, or pass force=true if the shrinkage is intentional.",
            )
            .into());
        }
    }

    crate::util::fs::atomic_write(&resolved, &new_content)?;

    // A frontmatter write can move catalog-INDEXED columns (status, title, tags,
    // time_scope), and nothing else brings the row back into step: the guard above
    // deliberately lets a plain catalogued file through, so this is the population
    // whose row can silently contradict its own file. See `librarian_sync` and
    // `open-issue-work-queue:BL-48`.
    //
    // Gated on `frontmatter_changed` because a body-only edit cannot move an indexed
    // column, and a catalog write per body edit would be pure cost.
    //
    // Return value deliberately dropped: `false` means "no librarian, or not an
    // artifact", both ordinary. The file write has already landed, so a failed sync
    // must not turn a successful edit into an error.
    if frontmatter_changed {
        crate::util::librarian_sync::sync_after_frontmatter_write(&resolved);
    }

    if let Ok(mut cov) = ctx.section_coverage.lock() {
        cov.update_mtime(&resolved);
    }

    ctx.agent
        .reload_config_if_project_toml_for(ctx.workspace_override.as_deref(), &resolved)
        .await;
    ctx.lsp.notify_file_changed(&resolved).await;
    ctx.agent
        .invalidate_call_edges_for(ctx.workspace_override.as_deref(), &resolved)
        .await;
    ctx.agent
        .mark_file_dirty_for(ctx.workspace_override.as_deref(), resolved.clone())
        .await;

    // Coverage hint: warn about unread sections.
    let all_headings = crate::tools::file_summary::parse_all_headings(&new_content);
    if !all_headings.is_empty() {
        let heading_texts: Vec<String> = all_headings.iter().map(|h| h.text.clone()).collect();
        if let Ok(mut cov) = ctx.section_coverage.lock() {
            if let Some(hint) = cov.unread_hint(&resolved, &heading_texts) {
                return Ok(json!({"status": "ok", "hint": hint}));
            }
        }
    }

    Ok(json!("ok"))
}
