//! Heading-based markdown navigation, folded into `read_file` (formerly the standalone
//! `read_markdown` tool). `read_file` routes `.md` reads and any `heading`/`headings`
//! call here via `read()`.

use anyhow::Result;
use serde_json::{json, Value};

use super::super::{optional_u64_param, RecoverableError, ToolContext};
use crate::util::text::extract_lines;

/// Resolve the `path` argument to `(resolved_path, text)`: an `@file_` buffer
/// ref loads from the output buffer; otherwise validate, stat, and read the
/// `.md` file from disk. The only async phase of `markdown::read`.
async fn resolve_markdown_source(
    path: &str,
    ctx: &ToolContext,
) -> Result<(std::path::PathBuf, String)> {
    if path.starts_with("@file_") {
        let buf = ctx
            .output_buffer
            .get(path)
            .ok_or_else(|| {
                RecoverableError::with_hint(
                    format!("buffer reference not found: '{}'", path),
                    "Buffer refs expire when the session resets. Re-run read_file on the file to get a fresh ref.",
                )
            })?;
        let resolved = buf
            .source_path
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from(path));
        Ok((resolved, buf.stdout.clone()))
    } else {
        // Gate: .md files only. Case-insensitive to match `is_markdown_target`'s own
        // lowercasing — `read_file` dispatches here whenever that function says "this
        // is markdown", so a gate that disagrees with the dispatcher it serves refuses
        // every uppercase-extension file the caller was just routed to (e.g.
        // `README.MD`), with a hint that tells them to do exactly what they did.
        let lower = path.to_ascii_lowercase();
        if !lower.ends_with(".md") && !lower.ends_with(".markdown") {
            return Err(RecoverableError::with_hint(
                format!(
                    "heading/headings address markdown sections, and '{}' is not a markdown file",
                    path
                ),
                "Drop heading/headings to read it as text, or pass a .md path.",
            )
            .into());
        }

        let project_root = ctx
            .agent
            .project_root_for(ctx.workspace_override.as_deref())
            .await;
        let security = ctx
            .agent
            .security_config_for(ctx.workspace_override.as_deref())
            .await;
        let resolved = crate::util::path_security::validate_read_path(
            path,
            project_root.as_deref(),
            &security,
        )?;

        if resolved.is_dir() {
            return Err(RecoverableError::with_hint(
                format!("'{}' is a directory, not a file", path),
                "Use tree to browse directory contents, or provide a specific file path",
            )
            .into());
        }

        let text = std::fs::read_to_string(&resolved).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => RecoverableError::with_hint(
                format!(
                    "file not found: '{}' (searched {})",
                    path,
                    resolved.display()
                ),
                "Check the path with tree, or use tree with `glob` to locate the file. \
                 If the root above is not the project you meant, a subagent sharing \
                 this session's process may have changed the active project — call \
                 workspace(action='status') to check.",
            )
            .into(),
            _ => anyhow::anyhow!("failed to read {}: {}", resolved.display(), e),
        })?;
        Ok((resolved, text))
    }
}

/// Multi-heading navigation: extract each requested section, join them, and
/// either return the combined content (+ coverage) or, when the join exceeds
/// the inline limit, buffer it and return a paginating hint error.
fn read_markdown_multi_heading(
    text: &str,
    resolved: &std::path::PathBuf,
    ctx: &ToolContext,
    headings_arr: &[Value],
) -> Result<Value> {
    let heading_queries: Vec<String> = headings_arr
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    let mut sections = Vec::new();
    let mut seen_headings = Vec::new();

    for query in &heading_queries {
        let section = crate::tools::file_summary::extract_markdown_section(text, query)?;
        seen_headings.push(
            section
                .breadcrumb
                .last()
                .cloned()
                .unwrap_or_else(|| query.clone()),
        );
        sections.push(section.content);
    }

    let content = sections.join("\n\n");

    // Oversized multi-heading join — fall back to hint. Measured on the SMALLEST response this
    // read can return: `content` with `sections` and `coverage` dropped and EVERY key that
    // then rides along (the omission markers, the hint, `format`). Measuring `content` alone
    // missed those keys, and a content of 9,820-10,003 B came back 10,016-10,136 B and was
    // parked under `@tool_*`. If even that does not fit, the read takes this error path.
    let smallest = finalize_multi(json!({ "content": &content }), &["sections", "coverage"]);
    if crate::tools::exceeds_inline_limit(&smallest.to_string()) {
        let file_id = ctx
            .output_buffer
            .store_file_excerpt(resolved.to_string_lossy().to_string(), content.clone());
        let lines = content.lines().count();
        let hint = format!(
            "use {:?} — request one heading at a time, or slice with start_line/end_line",
            file_id
        );
        let next_actions: Vec<String> = seen_headings
            .iter()
            .take(3)
            .map(|h| format!("read_file({:?}, heading={})", file_id, h))
            .collect();
        let err = crate::tools::RecoverableError::with_hint(
            format!(
                "combined headings span {} lines — exceeds inline threshold",
                lines
            ),
            hint,
        )
        .with_extra("file_id", serde_json::json!(file_id))
        .with_extra("requested_headings", serde_json::json!(seen_headings))
        .with_extra("next_actions", serde_json::json!(next_actions));
        return Err(err.into());
    }

    // Record coverage
    if !seen_headings.is_empty() {
        if let Ok(mut cov) = ctx.section_coverage.lock() {
            cov.mark_seen(resolved, &seen_headings);
        }
    }

    // `sections` carries each requested section's content individually (not just the
    // joined `content` string) so a caller can tell which section produced what without
    // re-splitting on its own delimiter guess.
    let mut result = json!({
        "content": content,
        "sections": sections,
    });

    // Coverage hint
    let all_headings = crate::tools::file_summary::parse_all_headings(text);
    if !all_headings.is_empty() {
        let all_texts: Vec<String> = all_headings.iter().map(|h| h.text.clone()).collect();
        if let Ok(mut cov) = ctx.section_coverage.lock() {
            if let Some(status) = cov.status(resolved, &all_texts) {
                if !status.unread.is_empty() {
                    result["coverage"] = json!({
                        "read": status.read_count,
                        "total": status.total_count,
                        "unread": status.unread,
                    });
                }
            }
        }
    }

    // `sections` REPEATS the text already in `content` (it exists so a caller can tell which
    // section produced what). Two copies of a 9,208 B read serialized to 18,869 B, and the whole
    // response was buffered under a `@tool_*` handle. When the response does not fit, the
    // duplicate goes first, then `coverage`, each marked `<key>_omitted: true` so its absence is
    // a statement and not a silence. Every candidate is the FINAL response (`finalize_multi`
    // adds the markers, the hint and `format`), so what is measured is what is returned; the
    // smallest candidate fits, by the check above.
    Ok(drop_to_fit(
        &result,
        &["sections", "coverage"],
        finalize_multi,
    ))
}

/// The multi-heading response as it will be RETURNED once `dropped` keys are gone: each
/// removal marked `<key>_omitted: true`, the hint that explains a dropped `sections`, and the
/// `format` key `read()` would add. The one place these keys are written, so the size measured
/// is the size returned.
fn finalize_multi(mut result: Value, dropped: &[&'static str]) -> Value {
    if let Some(obj) = result.as_object_mut() {
        for key in dropped {
            obj.insert(format!("{key}_omitted"), json!(true));
        }
        if dropped.contains(&"sections") {
            obj.insert(
                "hint".into(),
                json!(
                    "`sections` omitted to fit: `content` holds every requested section in order; \
                     request one heading at a time for a per-section value"
                ),
            );
        }
        obj.insert("format".into(), json!("markdown"));
    }
    result
}
/// A single-response candidate as RETURNED once `dropped` keys are gone: each removal marked
/// `<key>_omitted: true`, and the `format` key `read()` adds, so the size [`drop_to_fit`]
/// measures is the size returned.
fn finalize_dropped(mut result: Value, dropped: &[&'static str]) -> Value {
    if let Some(obj) = result.as_object_mut() {
        for key in dropped {
            obj.insert(format!("{key}_omitted"), json!(true));
        }
    }
    with_format(result)
}

/// Remove top-level `keys` from `base`, in order, until the FINAL response (`finalize` applied
/// to what is left and what was dropped) fits the inline limit. Returns the first candidate
/// that fits, or the last when none does (the caller guarantees the smallest fits).
fn drop_to_fit(
    base: &Value,
    keys: &[&'static str],
    finalize: impl Fn(Value, &[&'static str]) -> Value,
) -> Value {
    let mut work = base.clone();
    let mut dropped: Vec<&'static str> = Vec::new();
    loop {
        let candidate = finalize(work.clone(), &dropped);
        if !crate::tools::exceeds_inline_limit(&candidate.to_string()) {
            return candidate;
        }
        let next = keys
            .iter()
            .find(|k| !dropped.contains(k) && work.get(**k).is_some());
        let Some(key) = next else {
            return candidate;
        };
        if let Some(obj) = work.as_object_mut() {
            obj.remove(*key);
        }
        dropped.push(key);
    }
}

/// How much of a heading's text the oversized-section error echoes back (in `section_map`,
/// `next_actions`, `breadcrumb` and the message). Heading text has no length: a 12 KB sub-heading
/// made the error body 12,700 B, which `fit_envelope` cannot shrink because it is part of what
/// the error adds AROUND the map. A prefix is enough to name the heading, and still resolves it:
/// `read_file(heading=<prefix>)` matches by prefix. A clipped entry carries the true length as
/// `h_bytes`, so the clip is a statement and not a silent edit. The cap counts SERIALIZED bytes
/// (`json_escaped_len`), the unit the response is measured in: in raw bytes, 200 `\x01` escaped
/// to 1,200 B per echo.
// cap-class: RESULT_CAP read_markdown.heading_echo_bytes — probed
const HEADING_ECHO_CLIP: usize = 200;

/// `text` clipped to [`HEADING_ECHO_CLIP`] SERIALIZED bytes (its JSON-escaped length) on a
/// character boundary, and its original raw length when it was clipped. The unit is what the
/// echo costs in the response: a raw-byte clip of 200 control characters serialized to 1,200 B,
/// once in each place the heading is echoed.
fn clip_heading(text: &str) -> (String, Option<usize>) {
    if crate::util::text::json_escaped_len(text) <= HEADING_ECHO_CLIP {
        return (text.to_string(), None);
    }
    (
        crate::util::text::clip_head_escaped(text, HEADING_ECHO_CLIP).to_string(),
        Some(text.len()),
    )
}

/// Single-heading navigation: extract one section. Returns a `headings` list on
/// not-found, a buffered hint + `section_map` when the match is oversized, or
/// the section content (+ coverage).
fn read_markdown_single_heading(
    path: &str,
    text: &str,
    resolved: &std::path::PathBuf,
    ctx: &ToolContext,
    heading_query: &str,
) -> Result<Value> {
    let section_result =
        match crate::tools::file_summary::extract_markdown_section(text, heading_query) {
            Ok(s) => s,
            Err(e) => {
                let msg = e.message.clone();
                if msg.contains("not found") {
                    let headings_json: Vec<serde_json::Value> =
                        crate::tools::file_summary::parse_all_headings(text)
                            .iter()
                            .map(|h| serde_json::json!({"h": h.text, "l": h.line}))
                            .collect();
                    // The list has no length of its own (27.6 to 32.7 KB measured), and
                    // `call_content` buffered it under `@tool_*`. `fit_envelope` builds the
                    // whole response, measures it serialized, and cuts the list's MIDDLE only
                    // by the excess, with an `entries omitted` note naming the lines to read
                    // in the file the caller asked about (`path`, the route they already hold;
                    // never a new handle). The query is echoed clipped, as every heading echo is.
                    let error = format!("heading '{}' not found", clip_heading(heading_query).0);
                    let finish = |mut result: Value, notes: &[String]| -> Value {
                        let mut hint = String::from(
                            "pick a heading from the list above, or use start_line/end_line",
                        );
                        for note in notes {
                            hint.push(' ');
                            hint.push_str(note);
                        }
                        result["ok"] = json!(false);
                        result["error"] = json!(error);
                        result["hint"] = json!(hint);
                        with_format(result)
                    };
                    return Ok(crate::tools::file_summary::fit_envelope(
                        json!({ "headings": headings_json }),
                        path,
                        finish,
                    ));
                }
                return Err(e.into());
            }
        };
    let cov = crate::tools::read_file::markdown_coverage(
        text,
        resolved,
        ctx,
        Some(heading_query),
        None,
        None,
    );

    // The success response, built BEFORE deciding, because whether it fits is a fact about
    // the SERIALIZED response and not the section's raw bytes: JSON escaping doubles every
    // quote, backslash and newline and turns a control character into six bytes, so a section
    // of 6,000 quotes (6 KB raw) was returned as a 12 KB success and buffered under `@tool_*`.
    // It carries `format`, which `read()` adds, so the size measured is the size returned.
    let success = {
        let mut val = json!({
            "content": section_result.content,
            "lines": section_result.content.lines().count(),
            "line_range": [section_result.line_range.0, section_result.line_range.1],
            "breadcrumb": section_result.breadcrumb,
            "siblings": section_result.siblings,
        });
        if let Some(c) = cov {
            val["coverage"] = c;
        }
        // `coverage` (every unread heading) and `siblings` have no length of their own: a
        // short section of a 1,000-heading file must not be refused for them, so they are
        // dropped first, each marked `<key>_omitted`, and only a section that does not fit
        // without them takes the oversized arm.
        drop_to_fit(&val, &["coverage", "siblings"], finalize_dropped)
    };

    // Oversized match — return ok:false with hint + nested section_map
    // + next_actions. The agent must pick a sub-heading or a line range, not
    // retry against the original path.
    if crate::tools::exceeds_inline_limit(&success.to_string()) {
        let file_id = ctx.output_buffer.store_file_excerpt(
            resolved.to_string_lossy().to_string(),
            section_result.content.clone(),
        );
        let section_lines = section_result.content.lines().count();

        let (start_ln, end_ln) = section_result.line_range;
        // Every number below addresses `file_id`, which holds ONLY this section,
        // so they are stated in that buffer's frame — where the section's first
        // line is 1, not `start_ln`. The server already reports it that way: ask
        // the handle for a heading that does not exist and the listing comes
        // back `### Sub A  L3`, not L306. `line_range` stays file-relative on
        // purpose; it is the one field here that describes where the section
        // lives rather than how to address the handle.
        let all_headings = crate::tools::file_summary::parse_all_headings(text);
        let nested: Vec<serde_json::Value> = all_headings
            .iter()
            .filter(|h| h.line > start_ln && h.line <= end_ln)
            .map(|h| {
                let (text, clipped_from) = clip_heading(&h.text);
                let mut entry = json!({"h": text, "l": h.line - start_ln + 1});
                if let Some(n) = clipped_from {
                    entry["h_bytes"] = json!(n);
                }
                entry
            })
            .collect();

        let heading_label = clip_heading(
            &section_result
                .breadcrumb
                .last()
                .cloned()
                .unwrap_or_else(|| heading_query.to_string()),
        )
        .0;
        let breadcrumb: Vec<String> = section_result
            .breadcrumb
            .iter()
            .map(|b| clip_heading(b).0)
            .collect();

        let hint = format!(
            "use {:?} — pick a sub-heading from `section_map` or start_line/end_line",
            file_id
        );

        let next_actions: Vec<String> = {
            let mut actions = Vec::new();
            if let Some(first) = nested.first() {
                if let Some(h) = first.get("h").and_then(|v| v.as_str()) {
                    // The heading is quoted as a JSON string, the form a caller passes it in:
                    // `{:?}` wrote a control character as `\u{1}`, which no JSON parser reads,
                    // and an unquoted `heading=### Sub A` is not a call the caller can paste back.
                    let quoted = serde_json::to_string(h).unwrap_or_else(|_| format!("{h:?}"));
                    actions.push(format!("read_file({:?}, heading={quoted})", file_id));
                }
            }
            // The range is sized in SERIALIZED bytes, the unit the read it names is judged
            // in: `end_line=100` of a control-heavy section is 24 KB once escaped, and that
            // read came back under a second `@tool_*` handle. At most 100 lines, at least one.
            let (_, fit_lines, _) = crate::util::text::extract_lines_to_json_budget(
                &section_result.content,
                1,
                100,
                crate::tools::INLINE_BYTE_BUDGET,
            );
            actions.push(format!(
                "read_file({:?}, start_line=1, end_line={})",
                file_id,
                fit_lines.max(1)
            ));
            actions
        };

        let message = format!(
            "section {:?} spans {} lines — exceeds inline threshold",
            heading_label, section_lines
        );
        // An `Err` never reaches `call_content`'s buffering: `server.rs` puts `extra` inline in
        // the error body, so a 300-sub-heading section returned a 26,629 B body in context. The
        // body is therefore bounded HERE, through the same `fit_envelope` the summaries use:
        // `finish` builds the whole body (message, hint with the cut notes, `file_id`, the map
        // and its markers, `next_actions`, `breadcrumb`, `line_range`), it is measured, and only
        // `section_map` is cut, by the excess. The hint's route still works after a cut: the
        // first sub-heading is kept, which holds because every heading echoed here is clipped to
        // `HEADING_ECHO_CLIP` first (an entry larger than its half of the allowance is DROPPED
        // by `cut_array_middle`, and so is anything `fit_envelope` cannot shrink: it never
        // re-checks the response around an empty summary). The note adds
        // `read_file(path=<file_id>, start_line=.., end_line=..)` for the middle, in the buffer's
        // own line frame, which is the frame `section_map`'s `l` values are in.
        let finish = |map: Value, notes: &[String]| -> Value {
            let mut body = serde_json::Map::new();
            body.insert("error".into(), json!(message));
            let mut hint = hint.clone();
            for note in notes {
                hint.push(' ');
                hint.push_str(note);
            }
            body.insert("hint".into(), json!(hint));
            body.insert("file_id".into(), json!(file_id));
            if let Some(m) = map.as_object() {
                for (k, v) in m {
                    body.insert(k.clone(), v.clone());
                }
            }
            body.insert("next_actions".into(), json!(next_actions));
            body.insert("breadcrumb".into(), json!(breadcrumb));
            body.insert("line_range".into(), json!([start_ln, end_ln]));
            Value::Object(body)
        };
        let body = crate::tools::file_summary::fit_envelope(
            json!({ "section_map": nested }),
            &file_id,
            finish,
        );
        let mut err = crate::tools::RecoverableError::with_hint(
            message.clone(),
            body["hint"].as_str().unwrap_or_default().to_string(),
        );
        if let Some(fields) = body.as_object() {
            for (k, v) in fields {
                if k != "error" && k != "hint" {
                    err = err.with_extra(k.clone(), v.clone());
                }
            }
        }
        return Err(err.into());
    }

    Ok(success)
}

/// Line-range read: validate the 1-indexed range, extract the slice, and either
/// return it (+ coverage) or buffer + paginate when oversized.
fn read_markdown_line_range(
    path: &str,
    text: &str,
    resolved: &std::path::PathBuf,
    ctx: &ToolContext,
    start: u64,
    end: u64,
) -> Result<Value> {
    if start == 0 || end < start {
        return Err(RecoverableError::with_hint(
            format!(
                "invalid line range: start_line={} end_line={} \
                 (start_line must be >= 1 and end_line >= start_line)",
                start, end
            ),
            "Lines are 1-indexed. Example: start_line=1, end_line=50",
        )
        .into());
    }
    let file_total_lines = text.lines().count();
    if (start as usize) > file_total_lines {
        return Err(RecoverableError::with_hint(
            format!(
                "start_line {} exceeds file length {}",
                start, file_total_lines
            ),
            format!(
                "valid range is 1..={}; use read_file(path, start_line=N, end_line=M) within bounds",
                file_total_lines
            ),
        )
        .with_extra("lines", serde_json::json!(file_total_lines))
        .into());
    }
    let content = extract_lines(text, start as usize, end as usize);
    let md_cov = crate::tools::read_file::markdown_coverage(
        text,
        resolved,
        ctx,
        None,
        Some(start),
        Some(end),
    );

    // The inline response, built BEFORE deciding: whether it fits is a fact about the
    // SERIALIZED response, not the extract's raw bytes. 43 lines of `\x01` are under 2 KB raw
    // and over 10 KB serialized, and were returned inline and buffered under `@tool_*`. The
    // `coverage` list names every unread heading and has no length of its own, so it goes
    // first (marked `coverage_omitted`); a response still over the limit is buffered here.
    let inline = {
        let mut result = json!({ "content": content });
        if let Some(c) = &md_cov {
            result["coverage"] = c.clone();
        }
        drop_to_fit(&result, &["coverage"], finalize_dropped)
    };

    // Buffer large extracts
    if crate::tools::exceeds_inline_limit(&inline.to_string()) {
        let file_id = ctx
            .output_buffer
            .store_file_excerpt(resolved.to_string_lossy().to_string(), content.clone());
        // Budget on the ESCAPED size: this chunk is returned inline as JSON and
        // measured against TOOL_OUTPUT_BUFFER_THRESHOLD after serialization, so a
        // raw-byte budget lets a line-dense extract overshoot and get re-wrapped
        // as a `@tool_*` envelope.
        let (chunk, lines_shown, complete) = crate::util::text::extract_lines_to_json_budget(
            &content,
            1,
            usize::MAX,
            crate::tools::INLINE_BYTE_BUDGET,
        );
        let orig_start = start as usize;
        let orig_end = orig_start + lines_shown.saturating_sub(1);
        let mut result = json!({
            "content": chunk,
            "file_id": file_id,
            "total_lines": file_total_lines,
            "shown_lines": [orig_start, orig_end],
            "complete": complete,
        });
        if !complete {
            // Continue against the file, in the line numbers `shown_lines` just
            // reported. Phrasing `next` in the slice buffer's own 1-based frame
            // is off by `start - 1` and re-serves lines the caller has seen.
            result["next"] = json!(format!(
                "read_file(\"{path}\", start_line={}, end_line={end})",
                orig_end + 1
            ));
        }
        if let Some(c) = md_cov {
            result["coverage"] = c;
        }
        // The chunk is bounded; `coverage` is not, so it is dropped when it is what overflows.
        return Ok(drop_to_fit(&result, &["coverage"], finalize_dropped));
    }

    Ok(inline)
}

/// Add the `format` key every markdown response carries. `read()` adds it too, on whatever a
/// builder returns, which is harmless when it is already there; the builders call it so the key
/// is inside the response they MEASURE. Added only afterwards, it made a response judged to fit
/// (<= 10,003 B) come back up to 20 B over and be parked under `@tool_*`.
fn with_format(mut result: Value) -> Value {
    if let Some(obj) = result.as_object_mut() {
        obj.insert("format".into(), json!("markdown"));
    }
    result
}

/// Default (no nav/range) read: adaptive tiers — tier 3 (oversized → heading
/// map + buffer, no body), tier 2 (medium → full content + soft hint), tier 1
/// (small → full content).
fn read_markdown_default_tiers(
    text: &str,
    resolved: &std::path::PathBuf,
    ctx: &ToolContext,
) -> Result<Value> {
    let total_lines = text.lines().count();
    let oversized = crate::tools::exceeds_inline_limit(text);
    let all_headings = crate::tools::file_summary::parse_all_headings(text);
    let oversized_by_headings = all_headings.len() > crate::tools::HEADINGS_HARD_CAP;

    let md_cov = crate::tools::read_file::markdown_coverage(text, resolved, ctx, None, None, None);

    // `read_file`'s contract is "heading map by default" on markdown — every tier below
    // carries this, not just the oversized one, so a caller can always see the section
    // list without paying for a second round-trip. Small/medium tiers still return the
    // full body too (cheaper than a follow-up read_file(heading=...) call for content
    // that already fit inline); only the oversized tier omits content.
    let headings_json: Vec<Value> = all_headings
        .iter()
        .map(|h| json!({"h": h.text, "l": h.line}))
        .collect();

    // ── Tiers 1 and 2: the body inline, with the heading map ─────────
    // Built BEFORE deciding, because whether it FITS is a fact about the SERIALIZED response:
    // `exceeds_inline_limit(text)` measures the raw body alone, but the response is the body
    // plus the heading map, JSON-escaped (every newline and quote doubles), and a 9,900 B file
    // serialized to 14,062 B and was buffered under `@tool_*` with no handle of its own. A
    // response that does not fit falls through to tier 3, the tier built for exactly that.
    let inline = if oversized || oversized_by_headings {
        None
    } else if total_lines > crate::tools::LINE_SOFT_CAP {
        // Tier 2: medium — full content + heading map + soft hint.
        let heading_count = all_headings.len();
        let hint = if heading_count == 0 {
            format!(
                "{} lines, no headings — read_file(path, start_line=N, end_line=M) to focus",
                total_lines
            )
        } else {
            format!(
                "{} lines, {} sections — read_file(path, heading=\"## Section\") to focus",
                total_lines, heading_count
            )
        };
        let mut result = json!({
            "content": text,
            "lines": total_lines,
            "headings": headings_json,
            "hint": hint,
        });
        if let Some(c) = &md_cov {
            result["coverage"] = c.clone();
        }
        Some(with_format(result))
    } else {
        // Tier 1: small — full content + heading map.
        let mut result = json!({
            "content": text,
            "lines": total_lines,
            "headings": headings_json,
        });
        if let Some(c) = &md_cov {
            result["coverage"] = c.clone();
        }
        let heading_count = all_headings.len();
        if heading_count >= 2 {
            result["hint"] = serde_json::json!(format!(
                "{} lines, {} sections — read_file(path, heading=\"## Section\") to focus",
                total_lines, heading_count
            ));
        }
        Some(with_format(result))
    };
    if let Some(result) = inline {
        if !crate::tools::exceeds_inline_limit(&result.to_string()) {
            return Ok(result);
        }
    }

    // ── Tier 3: large — heading map + hint, no body ──────────────────
    let file_id = ctx
        .output_buffer
        .store_file(resolved.to_string_lossy().to_string(), text.to_string());

    let hint = if all_headings.is_empty() {
        format!("use {:?} — start_line/end_line", file_id)
    } else {
        format!(
            "use {:?} — heading=\"## Section\" or start_line/end_line",
            file_id
        )
    };

    // The map lists EVERY heading, and `HEADINGS_HARD_CAP` only chooses this tier: it bounds
    // nothing in it. Heading text has no length, so the map crosses the inline limit at about
    // 109 headings of 60 characters (map and `lines`, before `file_id` and the hint: 100 ->
    // 9,190 B, 120 -> 11,030 B, 200 -> 18,390 B; computed, and within 25 B of the reviewer's
    // live probe), and `call_content` buffered it again under `@tool_*` beside `file_id`.
    // `fit_envelope` builds the whole response, measures it, and cuts the map only by the
    // excess, with the note added to the hint the renderer prints as `next:`. (`coverage`
    // cannot add a second list here: a default read counts every heading as seen, so
    // `markdown_coverage` returns `None`. It sits in the measured envelope anyway, so if that
    // ever changed the map would simply get less room.)
    let summary = json!({ "lines": total_lines, "headings": headings_json });
    let finish = |mut result: Value, notes: &[String]| -> Value {
        result["file_id"] = json!(file_id);
        let mut hint = hint.clone();
        for note in notes {
            hint.push(' ');
            hint.push_str(note);
        }
        result["hint"] = json!(hint);
        if let Some(c) = &md_cov {
            result["coverage"] = c.clone();
        }
        with_format(result)
    };
    Ok(crate::tools::file_summary::fit_envelope(
        summary, &file_id, finish,
    ))
}

/// Heading-addressed markdown read. Reached through `read_file`, which routes here for
/// `.md`/`.markdown` paths, `@file_` buffers that came from one, or any call carrying
/// `heading`/`headings`. Results carry `"format": "markdown"` so `ReadFile::format_compact`
/// can pick the markdown renderer.
pub(crate) async fn read(input: Value, ctx: &ToolContext) -> Result<Value> {
    let path = crate::tools::require_str_param_or_hint(
        &input,
        "path",
        crate::fs::PATH_PARAM_ALIASES,
        "read_file(path=\"docs/x.md\") — add heading=\"## Section\" to read one section.",
    )?;

    // Resolve path → (resolved PathBuf, text String): @file_ buffer ref or disk read.
    let (resolved, text) = resolve_markdown_source(path, ctx).await?;

    // Reject librarian-managed artifacts — use doc(action="get") instead.
    // The resolved path lets the guard also catch AUGMENTED artifacts whose
    // frontmatter carries no id: for those the file is only a snapshot of
    // params held in the catalog, so a direct read returns stale state with
    // no signal that it is stale.
    crate::util::librarian_guard::guard_not_librarian_managed(
        path,
        &text,
        Some(&resolved),
        crate::util::librarian_guard::Access::Read,
    )?;

    // Extract params
    let heading = input["heading"].as_str();
    let headings_param = crate::tools::optional_array_param(&input, "headings");
    let start_line = optional_u64_param(&input, "start_line");
    let end_line = optional_u64_param(&input, "end_line");

    // Mutual exclusivity checks
    if heading.is_some() && headings_param.is_some() {
        return Err(RecoverableError::with_hint(
            "heading and headings are mutually exclusive",
            "Use heading for a single section, or headings for multiple sections.",
        )
        .into());
    }

    let has_nav = heading.is_some() || headings_param.is_some();
    let has_range = start_line.is_some() || end_line.is_some();

    if has_nav && has_range {
        return Err(RecoverableError::with_hint(
            "navigation parameters are mutually exclusive with start_line/end_line",
            "Use heading/headings OR start_line+end_line, not both",
        )
        .into());
    }

    if start_line.is_some() != end_line.is_some() {
        return Err(RecoverableError::with_hint(
            "both start_line and end_line are required",
            "Provide both start_line and end_line for a line range, e.g. start_line=1, end_line=50",
        )
        .into());
    }

    // ── Dispatch to the matching read strategy ────────────────────────
    let res = if let Some(headings_arr) = headings_param {
        read_markdown_multi_heading(&text, &resolved, ctx, &headings_arr)
    } else if let Some(heading_query) = heading {
        read_markdown_single_heading(path, &text, &resolved, ctx, heading_query)
    } else if let (Some(start), Some(end)) = (start_line, end_line) {
        read_markdown_line_range(path, &text, &resolved, ctx, start, end)
    } else {
        read_markdown_default_tiers(&text, &resolved, ctx)
    };

    let mut res = res?;
    if let Some(obj) = res.as_object_mut() {
        obj.insert("format".into(), json!("markdown"));
    }
    Ok(res)
}

/// True when `read_file` should take the markdown path: a markdown extension, or an
/// `@file_` buffer whose source was one.
pub(crate) fn is_markdown_target(path: &str, ctx: &ToolContext) -> bool {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".md") || lower.ends_with(".markdown") {
        return true;
    }
    if path.starts_with("@file_") {
        return ctx
            .output_buffer
            .get(path)
            .and_then(|b| b.source_path.clone())
            .is_some_and(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("md")));
    }
    false
}

/// The former `ReadMarkdown::format_compact`, unchanged.
pub(crate) fn format_read(result: &Value) -> Option<String> {
    // ERROR branch — must run first so {ok:false, headings:[...]} doesn't fall through to MAP.
    let is_error = result
        .get("ok")
        .and_then(|v| v.as_bool())
        .map(|ok| !ok)
        .unwrap_or(false);
    if is_error {
        let mut out = String::from("error: ");
        if let Some(msg) = result.get("error").and_then(|v| v.as_str()) {
            out.push_str(msg);
        }
        out.push_str("\n\n");
        if let Some(headings) = result.get("headings").and_then(|v| v.as_array()) {
            out.push_str("available headings:\n");
            for entry in headings {
                let h = entry.get("h").and_then(|v| v.as_str()).unwrap_or("");
                let l = entry.get("l").and_then(|v| v.as_u64()).unwrap_or(0);
                let level = h.chars().take_while(|c| *c == '#').count().max(1);
                let indent = " ".repeat((level - 1) * 2);
                out.push_str(&format!("{indent}{h}  L{l}\n"));
            }
        }
        if let Some(hint) = result.get("hint").and_then(|v| v.as_str()) {
            out.push('\n');
            out.push_str("next: ");
            out.push_str(hint);
        }
        return Some(out);
    }

    // CONTENT branch — pass content through, with optional headers/footers.
    if let Some(content) = result.get("content").and_then(|v| v.as_str()) {
        let mut out = String::new();
        if let (Some(breadcrumb), Some(line_range)) = (
            result.get("breadcrumb").and_then(|v| v.as_array()),
            result.get("line_range").and_then(|v| v.as_array()),
        ) {
            if let (Some(last), Some(start), Some(end)) = (
                breadcrumb.last().and_then(|v| v.as_str()),
                line_range.first().and_then(|v| v.as_u64()),
                line_range.get(1).and_then(|v| v.as_u64()),
            ) {
                out.push_str(&format!("§ {last}  L{start}-L{end}\n\n"));
            }
        }
        out.push_str(content);
        if let Some(hint) = result.get("hint").and_then(|v| v.as_str()) {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
            out.push_str(hint);
        }
        return Some(out);
    }

    // MAP branch — indented heading tree + next cue.
    let headings = result
        .get("headings")
        .or_else(|| result.get("section_map"))
        .and_then(|v| v.as_array());
    if let Some(headings) = headings {
        let lines = result.get("lines").and_then(|v| v.as_u64()).unwrap_or(0);
        let file_id = result.get("file_id").and_then(|v| v.as_str()).unwrap_or("");
        let mut out = format!("{} lines  {}\n\n", lines, file_id);
        for (i, entry) in headings.iter().enumerate() {
            // The gap line replaces nothing: it sits between the two kept halves. Its helper
            // leads with a newline for the indented outline in `read_file`; here every entry
            // already ends with one.
            if let Some(gap) = crate::tools::read_file::omitted_gap(result, "headings", i) {
                out.push_str(gap.trim_start_matches('\n'));
                out.push('\n');
            }
            let h = entry.get("h").and_then(|v| v.as_str()).unwrap_or("");
            let l = entry.get("l").and_then(|v| v.as_u64()).unwrap_or(0);
            let level = h.chars().take_while(|c| *c == '#').count().max(1);
            let indent = " ".repeat((level - 1) * 2);
            out.push_str(&format!("{indent}{h}  L{l}\n"));
        }
        if let Some(hint) = result.get("hint").and_then(|v| v.as_str()) {
            out.push('\n');
            out.push_str("next: ");
            out.push_str(hint);
        }
        return Some(out);
    }

    // Fallback for shapes added later: serialize JSON.
    Some(result.to_string())
}

#[cfg(test)]
mod clip_heading_tests {
    use super::{clip_heading, HEADING_ECHO_CLIP};

    #[test]
    fn a_heading_of_exactly_the_clip_length_is_returned_whole() {
        let text = "h".repeat(HEADING_ECHO_CLIP);
        assert_eq!(clip_heading(&text), (text, None));
    }

    #[test]
    fn a_heading_one_byte_over_is_clipped_and_its_length_reported() {
        let text = "h".repeat(HEADING_ECHO_CLIP + 1);
        let (clipped, from) = clip_heading(&text);
        assert_eq!(clipped.len(), HEADING_ECHO_CLIP);
        assert_eq!(from, Some(HEADING_ECHO_CLIP + 1));
        assert!(
            text.starts_with(&clipped),
            "a clip must be a PREFIX so it still resolves"
        );
    }

    #[test]
    fn a_clip_never_splits_a_multibyte_character() {
        // 100 x '€' is 300 B; 200 is not a multiple of 3, so a raw cut lands inside a character.
        let text = "€".repeat(100);
        let (clipped, from) = clip_heading(&text);
        assert!(clipped.len() <= HEADING_ECHO_CLIP && clipped.len() > HEADING_ECHO_CLIP - 3);
        assert!(clipped.chars().all(|c| c == '€'));
        assert_eq!(from, Some(300));
    }
    #[test]
    fn the_clip_counts_serialized_bytes_not_raw_bytes() {
        // 100 quotes are 100 raw bytes and 200 serialized: exactly the cap, so returned whole.
        let fits = "\"".repeat(HEADING_ECHO_CLIP / 2);
        assert_eq!(clip_heading(&fits), (fits.clone(), None));
        // One more is 202 serialized: clipped back to the 100 that fit, with the raw length.
        let over = "\"".repeat(HEADING_ECHO_CLIP / 2 + 1);
        let (clipped, from) = clip_heading(&over);
        assert_eq!(clipped, fits);
        assert_eq!(from, Some(over.len()));
        // A control character serializes to six bytes, so 200 of them keep 33.
        let ctrl = "\u{1}".repeat(HEADING_ECHO_CLIP);
        let (clipped, from) = clip_heading(&ctrl);
        assert_eq!(clipped, "\u{1}".repeat(HEADING_ECHO_CLIP / 6));
        assert_eq!(from, Some(HEADING_ECHO_CLIP));
    }
}
