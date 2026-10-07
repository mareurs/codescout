use std::borrow::Cow;

use serde_json::Value;

use crate::tools::RecoverableError;

pub struct SectionResult {
    pub content: String,
    pub line_range: (usize, usize), // 1-indexed, inclusive
    /// The heading actually BOUND, which is not always the one queried — tiers 3-4 of
    /// `resolve_section_range` are fuzzy. A caller that reports the query back to its user
    /// cannot distinguish an exact hit from a substring hit on some other section, which is
    /// how `doc(action="get")` came to echo `body_meta.heading` while `read_file` disclosed
    /// the real bind. Report this, not the request.
    /// docs/issues/2026-09-03-a-bare-heading-query-cannot-reach-the-exact-match-tiers.md
    pub heading_text: String,
    pub breadcrumb: Vec<String>,
    pub siblings: Vec<String>,
    pub format: String,
}

#[derive(PartialEq)]
pub enum FileSummaryType {
    Source,
    Markdown,
    Json,
    Yaml,
    Toml,
    Config, // remaining: .xml, .ini, .env, .lock, .cfg
    Generic,
}

// Stubs — implementations replaced in GREEN phase
pub fn detect_file_type(path: &str) -> FileSummaryType {
    let lower = path.to_lowercase();
    const SOURCE_EXTS: &[&str] = &[
        ".rs", ".py", ".ts", ".tsx", ".js", ".jsx", ".go", ".java", ".kt", ".kts", ".c", ".cpp",
        ".cc", ".cxx", ".h", ".swift", ".rb", ".cs", ".php", ".scala", ".ex", ".exs", ".hs",
        ".lua", ".sh", ".bash",
    ];
    const CONFIG_EXTS: &[&str] = &[".xml", ".ini", ".env", ".lock", ".cfg"];
    if SOURCE_EXTS.iter().any(|e| lower.ends_with(e)) {
        FileSummaryType::Source
    } else if lower.ends_with(".md") || lower.ends_with(".mdx") {
        FileSummaryType::Markdown
    } else if lower.ends_with(".json") {
        FileSummaryType::Json
    } else if lower.ends_with(".yaml") || lower.ends_with(".yml") {
        FileSummaryType::Yaml
    } else if lower.ends_with(".toml") {
        FileSummaryType::Toml
    } else if CONFIG_EXTS.iter().any(|e| lower.ends_with(e)) {
        FileSummaryType::Config
    } else {
        FileSummaryType::Generic
    }
}

pub fn summarize_source(path: &str, content: &str) -> Value {
    let p = std::path::Path::new(path);
    let language = crate::ast::detect_language(p);
    let symbols =
        crate::ast::parser::extract_symbols_from_source(content, language, p).unwrap_or_default();

    if symbols.is_empty() {
        let mut result = summarize_generic_file(content);
        result["type"] = serde_json::json!("source");
        return result;
    }

    let names: Vec<serde_json::Value> = symbols
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name_path,
                "kind": format!("{:?}", s.kind),
                "line": s.start_line + 1,
            })
        })
        .collect();

    serde_json::json!({
        "type": "source",
        "line_count": content.lines().count(),
        "symbols": names,
    })
}

pub fn summarize_markdown(content: &str) -> Value {
    let line_count = content.lines().count();
    let all_headings = parse_all_headings(content);
    let mut headings: Vec<Value> = all_headings
        .iter()
        .map(|h| {
            serde_json::json!({
                "heading": h.text,
                "level": h.level,
                "line": h.line,
                "end_line": h.end_line,
            })
        })
        .collect();
    let total_headings = all_headings.len();
    headings.truncate(30);
    let mut out = serde_json::json!({
        "type": "markdown",
        "line_count": line_count,
        "headings": headings,
    });
    if total_headings > 30 {
        out["total_headings"] = serde_json::json!(total_headings);
        out["headings_truncated"] = serde_json::json!(true);
    }
    out
}

pub fn heading_level(line: &str) -> Option<usize> {
    if !line.starts_with('#') {
        return None;
    }
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes) && line.as_bytes().get(hashes) == Some(&b' ') {
        Some(hashes)
    } else {
        None
    }
}

/// A heading's text with its `#` level marker removed — `"## Index"` → `"Index"`. A string
/// carrying no marker comes back unchanged, which is the whole point: `HeadingInfo::text` is
/// the RAW line (see `parse_all_headings`), so without normalising both sides a caller's bare
/// `Index` can never equal a stored `## Index`. Tiers 1-2 were therefore unreachable for any
/// query written without its markers, leaving such queries to the fuzzy tiers'
/// first-match-wins — which binds whichever EARLIER heading merely contains the word.
///
/// Applied in tier 2 only. Tier 1 still compares raw text, so a caller who passes `## Foo`
/// keeps exact level semantics and `### Foo` cannot answer it. Where both `## Foo` and
/// `### Foo` exist, a bare `Foo` now matches both at tier 2 and raises the duplicate error
/// naming each line, which is the honest answer rather than a silent pick.
/// docs/issues/2026-09-03-a-bare-heading-query-cannot-reach-the-exact-match-tiers.md
fn strip_heading_marker(s: &str) -> &str {
    let trimmed = s.trim_start_matches('#');
    if trimmed.len() == s.len() {
        s
    } else {
        trimmed.trim_start()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeadingInfo {
    pub text: String,    // e.g. "## Setup"
    pub level: usize,    // 1-6
    pub line: usize,     // 1-indexed
    pub end_line: usize, // 1-indexed, inclusive
}

#[derive(Debug, Clone, PartialEq)]
pub struct SectionRange {
    pub heading_line: usize,    // 1-indexed
    pub body_start_line: usize, // heading_line + 1
    pub end_line: usize,        // inclusive, last line of section
    pub heading_text: String,   // raw heading text (with formatting)
    pub level: usize,           // 1-6
}

/// A heading query, optionally narrowed to one of several equally-good matches.
///
/// `occurrence` is 1-indexed and exists because two byte-identical headings admit
/// **no** distinguishing query string: `resolve_section_range`'s exact-match tiers
/// return the ambiguity error before the fuzzier prefix/substring tiers ever run, and
/// even those could not separate two equal strings. Without a positional selector such
/// a section is unreachable through every heading-addressed surface — including
/// `doc(update, patch={body_edits})`, which is the *only* edit path a
/// librarian-managed artifact has, so the section becomes permanently uneditable.
/// See `docs/issues/archive/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md`.
///
/// `&str` converts in, so every caller with nothing to disambiguate keeps passing a
/// bare string and keeps today's behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeadingQuery<'a> {
    /// The heading text to match, e.g. `"## Fix"`.
    pub text: &'a str,
    /// 1-indexed selector among matches. `None` keeps the historical contract:
    /// a single match resolves, several are an error.
    pub occurrence: Option<usize>,
}

impl<'a> HeadingQuery<'a> {
    pub fn new(text: &'a str, occurrence: Option<usize>) -> Self {
        Self { text, occurrence }
    }
}

impl<'a> From<&'a str> for HeadingQuery<'a> {
    fn from(text: &'a str) -> Self {
        Self {
            text,
            occurrence: None,
        }
    }
}

impl<'a> From<&'a String> for HeadingQuery<'a> {
    fn from(text: &'a String) -> Self {
        Self {
            text: text.as_str(),
            occurrence: None,
        }
    }
}

/// Parse all markdown headings with their line ranges. No truncation.
/// Skips headings inside fenced code blocks.
pub fn parse_all_headings(content: &str) -> Vec<HeadingInfo> {
    let line_count = content.lines().count();

    // Pre-scan: an unclosed code block means the file carries a half-fence
    // (typical during in-flight batch edits whose intermediate new_string
    // contains one). CommonMark would extend that fence to EOF, hiding every
    // heading after it. For an editor tool, that's brittle: we'd rather treat
    // unbalanced fences as plain text and still find the headings. Bug:
    // docs/issues/archive/2026-05-21-edit-markdown-last-heading-unaddressable.md
    //
    // This is a real fence scan, not a parity count — a nested shorter run is
    // content, not a delimiter, so counting fence-ish lines called balanced
    // files unbalanced and silently disabled tracking. See
    // docs/issues/archive/2026-08-11-artifact-nested-fence-closes-outer-fence.md
    let fences_balanced = crate::util::markdown_fence::fences_balanced(content.lines());

    let mut fence = crate::util::markdown_fence::FenceState::new();
    let mut raw: Vec<(String, usize, usize)> = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        if fences_balanced && fence.feed(line) {
            continue;
        }
        if fence.in_fence() {
            continue;
        }
        if let Some(level) = heading_level(line) {
            raw.push((line.to_string(), level, idx + 1));
        }
    }
    raw.iter()
        .enumerate()
        .map(|(i, (text, level, line))| {
            let end_line = raw[i + 1..]
                .iter()
                .find(|(_, l, _)| *l <= *level)
                .map(|(_, _, next_line)| next_line - 1)
                .unwrap_or(line_count);
            HeadingInfo {
                text: text.clone(),
                level: *level,
                line: *line,
                end_line,
            }
        })
        .collect()
}

/// Strip inline markdown formatting from a heading string.
/// Removes backtick spans, bold/italic markers, collapses spaces, trims.
pub fn strip_inline_formatting(s: &str) -> String {
    let mut result = s.to_string();
    // Remove backtick spans: `code` → code
    while let Some(start) = result.find('`') {
        let Some(end) = result[start + 1..].find('`') else {
            break;
        };
        let inner = result[start + 1..start + 1 + end].to_string();
        result = format!(
            "{}{}{}",
            &result[..start],
            inner,
            &result[start + 1 + end + 1..]
        );
    }
    // Remove bold/italic: **text** → text, __text__ → text, *text* → text, _text_ → text
    // Order matters: ** before *, __ before _
    for marker in &["**", "__", "*", "_"] {
        while let Some(start) = result.find(marker) {
            if let Some(end) = result[start + marker.len()..].find(marker) {
                let inner = result[start + marker.len()..start + marker.len() + end].to_string();
                result = format!(
                    "{}{}{}",
                    &result[..start],
                    inner,
                    &result[start + marker.len() + end + marker.len()..]
                );
            } else {
                break;
            }
        }
    }
    // Collapse multiple spaces to single, trim
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Resolve a heading query to a precise line range in a markdown document.
/// Uses a 4-tier matching cascade: exact raw → exact stripped → prefix stripped → substring stripped.
///
/// Several matches in an exact tier are an error unless the caller supplies
/// [`HeadingQuery::occurrence`], which selects among them positionally. That selector is
/// the only way to reach either of two byte-identical headings: no query string can
/// separate them, and the fuzzier tiers below never run once an exact tier has matched.
/// The prefix/substring tiers keep their historical first-match-wins contract for an
/// unqualified query — they are fuzzy by design and have never treated several hits as
/// ambiguous.
pub fn resolve_section_range<'a>(
    content: &str,
    query: impl Into<HeadingQuery<'a>>,
) -> Result<SectionRange, RecoverableError> {
    let HeadingQuery {
        text: heading_query,
        occurrence,
    } = query.into();

    let headings = parse_all_headings(content);

    if headings.is_empty() {
        return Err(RecoverableError::with_hint(
            "no headings found in file",
            "The file contains no Markdown headings to navigate",
        ));
    }

    let query_stripped = strip_inline_formatting(heading_query);
    let query_stripped_lower = query_stripped.to_lowercase();

    // Helper to build SectionRange from HeadingInfo
    let make_range = |h: &HeadingInfo| -> SectionRange {
        SectionRange {
            heading_line: h.line,
            body_start_line: h.line + 1,
            end_line: h.end_line,
            heading_text: h.text.clone(),
            level: h.level,
        }
    };

    // Helper to build duplicate error.
    //
    // The `heading_ambiguous` / `occurrences` extras exist because a caller one frame up
    // may only see `Option` -- `.ok()` collapses this error into the same `None` as a
    // genuine miss, and a caller that then reports "missing" states the false one and
    // sends the reader hunting for a heading that is present twice. The discriminant has
    // to ride on the error, not be re-derived from its message text.
    // docs/issues/archive/2026-08-27-artifact-get-reports-a-doubly-defined-heading-as-missing.md
    let dup_error = |indices: &[usize]| -> RecoverableError {
        let lines: Vec<String> = indices
            .iter()
            .map(|&i| headings[i].line.to_string())
            .collect();
        let line_nums: Vec<usize> = indices.iter().map(|&i| headings[i].line).collect();
        RecoverableError::with_hint(
            format!(
                "heading '{}' found {} times (lines {})",
                heading_query,
                indices.len(),
                lines.join(", ")
            ),
            format!(
                "Pass occurrence=N to target one (1-indexed, here 1..={}), or query a heading text unique to the section you mean. Identical headings admit no distinguishing query, so occurrence is the only way to reach either.",
                indices.len()
            ),
        )
        .with_extra("heading_ambiguous", serde_json::json!(true))
        .with_extra("occurrences", serde_json::json!(line_nums))
    };

    // Choose among `indices` (document order), honouring `occurrence`.
    let select = |indices: &[usize]| -> Result<SectionRange, RecoverableError> {
        match occurrence {
            None if indices.len() == 1 => Ok(make_range(&headings[indices[0]])),
            None => Err(dup_error(indices)),
            Some(0) => Err(RecoverableError::with_hint(
                "occurrence is 1-indexed, got 0",
                "Pass occurrence=1 to target the first match.",
            )),
            Some(n) if n <= indices.len() => Ok(make_range(&headings[indices[n - 1]])),
            Some(n) => Err(RecoverableError::with_hint(
                format!(
                    "occurrence {} requested but heading '{}' matches {} time(s)",
                    n,
                    heading_query,
                    indices.len()
                ),
                format!("Valid occurrence values here are 1..={}.", indices.len()),
            )),
        }
    };

    // Tier 1: Exact match (raw)
    let exact_raw: Vec<usize> = headings
        .iter()
        .enumerate()
        .filter(|(_, h)| h.text == heading_query)
        .map(|(i, _)| i)
        .collect();
    if !exact_raw.is_empty() {
        return select(&exact_raw);
    }

    // Tier 2: Exact match (stripped)
    //
    // Both sides pass through `strip_heading_marker` so a bare `Index` reaches this
    // EXACT tier instead of falling through to tier 4's first-match-wins. Tier 1 above
    // still compares raw text, so level semantics survive for a `## Foo` query.
    // docs/issues/2026-09-03-a-bare-heading-query-cannot-reach-the-exact-match-tiers.md
    let exact_stripped: Vec<usize> = headings
        .iter()
        .enumerate()
        .filter(|(_, h)| {
            let h_stripped = strip_inline_formatting(&h.text);
            strip_heading_marker(&h_stripped) == strip_heading_marker(&query_stripped)
        })
        .map(|(i, _)| i)
        .collect();
    if !exact_stripped.is_empty() {
        return select(&exact_stripped);
    }

    // Tier 3: Prefix match (stripped, case-insensitive)
    let prefix_matches: Vec<usize> = headings
        .iter()
        .enumerate()
        .filter(|(_, h)| {
            strip_inline_formatting(&h.text)
                .to_lowercase()
                .starts_with(&query_stripped_lower)
        })
        .map(|(i, _)| i)
        .collect();
    if !prefix_matches.is_empty() {
        return match occurrence {
            None => Ok(make_range(&headings[prefix_matches[0]])),
            Some(_) => select(&prefix_matches),
        };
    }

    // Tier 4: Substring match (stripped, case-insensitive)
    let substring_matches: Vec<usize> = headings
        .iter()
        .enumerate()
        .filter(|(_, h)| {
            strip_inline_formatting(&h.text)
                .to_lowercase()
                .contains(&query_stripped_lower)
        })
        .map(|(i, _)| i)
        .collect();
    if !substring_matches.is_empty() {
        return match occurrence {
            None => Ok(make_range(&headings[substring_matches[0]])),
            Some(_) => select(&substring_matches),
        };
    }

    // No match.
    //
    // The list is windowed, and the window used to be `take(15)` — head-only. That
    // direction is not neutral: a heading-addressed append targets the document's
    // LAST stanza (`append_entry` inserts *before* its anchor), so on a long ledger
    // the one heading the caller needed was the one this list dropped, every time.
    // Measured on `reconnaissance-patterns.md`: 92 headings, anchor at line 4038 of
    // 4100. Keep both ends, and say how many were elided so the gap is legible
    // rather than merely absent.
    // docs/issues/archive/2026-08-27-append-entry-anchor-is-undiscoverable-through-the-surface-its-error-names.md
    const HEAD: usize = 12;
    const TAIL: usize = 3;
    let names: Vec<&str> = headings.iter().map(|h| h.text.as_str()).collect();
    let available = if names.len() > HEAD + TAIL {
        format!(
            "{} … (+{} more) … {}",
            names[..HEAD].join(", "),
            names.len() - HEAD - TAIL,
            names[names.len() - TAIL..].join(", "),
        )
    } else {
        names.join(", ")
    };
    Err(RecoverableError::with_hint(
        format!("heading '{}' not found", heading_query),
        format!("Available headings: {available}"),
    ))
}

pub fn extract_markdown_section<'q, Q: Into<HeadingQuery<'q>>>(
    content: &str,
    heading_query: Q,
) -> Result<SectionResult, RecoverableError> {
    let range = resolve_section_range(content, heading_query)?;
    let all_headings = parse_all_headings(content);

    // Extract content
    let lines: Vec<&str> = content.lines().collect();
    let start = (range.heading_line - 1).min(lines.len());
    let end = range.end_line.min(lines.len());
    let section_content = lines[start..end].join("\n");

    // Build breadcrumb: walk backwards collecting parents (lower level numbers)
    let mut breadcrumb = Vec::new();
    let mut current_level = range.level;
    for h in all_headings.iter().rev() {
        if h.line > range.heading_line {
            continue;
        }
        if h.level < current_level || h.line == range.heading_line {
            breadcrumb.push(h.text.clone());
            current_level = h.level;
        }
    }
    breadcrumb.reverse();

    // Find siblings: same level headings (excluding the matched one)
    let siblings: Vec<String> = all_headings
        .iter()
        .filter(|h| h.level == range.level && h.text != range.heading_text)
        .map(|h| h.text.clone())
        .collect();

    Ok(SectionResult {
        content: section_content,
        line_range: (range.heading_line, range.end_line),
        heading_text: range.heading_text.clone(),
        breadcrumb,
        siblings,
        format: "markdown".to_string(),
    })
}

pub fn summarize_json(content: &str) -> Value {
    let line_count = content.lines().count();

    let parsed: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => {
            let mut fallback = summarize_generic_file(content);
            fallback["type"] = serde_json::json!("json");
            return fallback;
        }
    };

    let schema = match &parsed {
        Value::Object(map) => {
            let keys: Vec<Value> = map
                .iter()
                .take(30)
                .map(|(k, v)| {
                    let mut entry = serde_json::json!({
                        "path": format!("$.{}", k),
                        "type": json_type_name(v),
                    });
                    match v {
                        Value::Object(m) => {
                            entry["count"] = serde_json::json!(m.len());
                        }
                        Value::Array(a) => {
                            entry["count"] = serde_json::json!(a.len());
                        }
                        _ => {}
                    }
                    entry
                })
                .collect();
            let mut obj = serde_json::json!({ "root_type": "object", "keys": keys });
            if map.len() > 30 {
                obj["total_keys"] = serde_json::json!(map.len());
                obj["keys_truncated"] = serde_json::json!(true);
            }
            obj
        }
        Value::Array(arr) => {
            let element_type = arr
                .first()
                .map(json_type_name)
                .unwrap_or_else(|| "unknown".to_string());
            serde_json::json!({
                "root_type": "array",
                "count": arr.len(),
                "element_type": element_type,
            })
        }
        other => serde_json::json!({ "root_type": json_type_name(other) }),
    };

    serde_json::json!({
        "type": "json",
        "line_count": line_count,
        "schema": schema,
    })
}

fn json_type_name(v: &Value) -> String {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
    .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Segment {
    /// Object key access: `.field` or bare `field` after `$`.
    Key(String),
    /// Non-negative array index: `[N]` where N ≥ 0.
    Index(usize),
    /// Negative single-index: `[-N]` where N ≥ 1, stored as positive magnitude.
    NegIndex(usize),
    /// Negative-start open-end slice: `[-N:]` where N ≥ 1, last N elements.
    NegSliceFrom(usize),
    /// `[*]` — project the REMAINING path over every element of an array.
    ///
    /// Unlike the other segments this one is not a simple narrowing step: it
    /// changes how the rest of the path is evaluated, so `resolve_json_segment`
    /// never sees it. `eval_segments` splits on it instead.
    Wildcard,
}

/// Evaluate a parsed path against a value.
///
/// Without a `Wildcard` this is the original sequential narrowing: each segment
/// resolves against the previous result. `[*]` is different in kind — it changes
/// how the *remaining* path is evaluated — so the list is split on the first one:
/// the prefix narrows normally, the result must be an array, and the suffix is
/// evaluated against every element and collected.
///
/// Recursion handles nesting, and nesting is **preserved rather than flattened**:
/// `$.groups[*].rows[*].v` yields `[[1,2],[3]]`, not `[1,2,3]`. Flattening would
/// discard which group a value came from, which is usually the question a grouped
/// projection is being asked.
fn eval_segments(root: &Value, segments: &[Segment]) -> Result<Value, RecoverableError> {
    let Some(pos) = segments.iter().position(|s| matches!(s, Segment::Wildcard)) else {
        let mut current: Cow<'_, Value> = Cow::Borrowed(root);
        for seg in segments {
            current = match current {
                Cow::Borrowed(v) => resolve_json_segment(v, seg)?,
                Cow::Owned(v) => Cow::Owned(resolve_json_segment(&v, seg)?.into_owned()),
            };
        }
        return Ok(current.into_owned());
    };

    let base = eval_segments(root, &segments[..pos])?;
    let Some(arr) = base.as_array() else {
        // The hint says what WAS found, because "found object" alone sends a caller to guess.
        // An object lists its keys through the same `describe_keys` the key-miss error uses;
        // a string says how big it is and that it is text — a JSON document inside a string
        // is one string value to `json_path`, which is exactly how `run_command`'s `stdout`
        // looked to it. The message text itself is classified by `usage/db.rs`
        // (`json_path_shape_mismatch`) and stays as it was.
        let hint = match &base {
            Value::Object(obj) => format!(
                "Use '[*]' only where the value is an array. This object has the keys: {}. \
                 Name one of them first, or drop '[*]' to address the object itself.",
                describe_keys(obj)
            ),
            Value::String(s) => format!(
                "This value is text of {} bytes, not an array, so '[*]' cannot project it. \
                 Drop '[*]' to read the text. JSON stored inside a string cannot be projected \
                 by json_path; parse the text with a shell tool such as jq.",
                s.len()
            ),
            _ => "Use '[*]' only where the value is an array. Drop it to address the value itself."
                .to_string(),
        };
        return Err(RecoverableError::with_hint(
            format!(
                "json_path '[*]' needs an array, found {}",
                json_type_name(&base)
            ),
            hint,
        ));
    };

    let rest = &segments[pos + 1..];
    let mut out = Vec::with_capacity(arr.len());
    for (i, el) in arr.iter().enumerate() {
        // Fail loudly, naming the element. A projection that silently skipped rows
        // would return a short array that reads as complete — the same defect class
        // as an unmarked truncation, and harder to notice because the result is
        // still well-formed.
        let projected = eval_segments(el, rest).map_err(|e| {
            RecoverableError::with_hint(
                format!("json_path '[*]' failed at element {i}: {e}"),
                "Every element must satisfy the path after '[*]'. Inspect one with '[N]' first.",
            )
        })?;
        out.push(projected);
    }
    Ok(Value::Array(out))
}

/// Extract a JSON subtree by path. Returns (pretty-printed content, type name, optional count).
///
/// For `Value::String` nodes the raw string is returned unescaped — not the JSON-quoted form.
/// `serde_json::to_string_pretty` on `Value::String("fn foo(){\n}")` produces
/// `"\"fn foo(){\\n}\""` — quoted with `\n` escapes — which is unreadable as code.
/// Returning the raw string means `json_path="$.symbols[0].body"` gives actual
/// source lines that can be browsed, grepped, and displayed directly.
pub fn extract_json_path(
    content: &str,
    path: &str,
) -> Result<(String, String, Option<usize>), RecoverableError> {
    let parsed: Value = serde_json::from_str(content).map_err(|e| {
        RecoverableError::with_hint(
            format!("failed to parse JSON: {}", e),
            "Ensure the file contains valid JSON",
        )
    })?;
    let segments = parse_json_path_segments(path)?;
    let resolved = eval_segments(&parsed, &segments)?;
    let final_ref: &Value = &resolved;
    let pretty = match final_ref {
        Value::String(s) => s.clone(),
        _ => serde_json::to_string_pretty(final_ref).unwrap_or_else(|_| final_ref.to_string()),
    };
    let type_name = json_type_name(final_ref);
    let count = match final_ref {
        Value::Object(m) => Some(m.len()),
        Value::Array(a) => Some(a.len()),
        _ => None,
    };
    Ok((pretty, type_name, count))
}

/// Split a json_path body on `.` separators that sit OUTSIDE `[...]`
/// brackets, so a quoted bracket key containing dots (e.g. `["2.1.5"]`) is
/// not fragmented. Bracket depth is tracked; dots inside brackets are kept.
/// Bug 2026-07-01-read-file-jsonpath-dotted-object-keys-unreachable.
fn split_on_unbracketed_dot(path: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: usize = 0;
    let mut start = 0usize;
    for (i, c) in path.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            '.' if depth == 0 => {
                parts.push(&path[start..i]);
                start = i + 1; // '.' is one byte (ASCII)
            }
            _ => {}
        }
    }
    parts.push(&path[start..]);
    parts
}

/// If `s` is wrapped in a matching pair of single or double quotes, return
/// the unquoted inner slice; otherwise `None`. Backs `["key"]` / `['key']`
/// bracket keys — the only way to address object keys containing `.`.
fn strip_matching_quotes(s: &str) -> Option<&str> {
    let b = s.as_bytes();
    if b.len() >= 2 {
        let (first, last) = (b[0], b[b.len() - 1]);
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return Some(&s[1..s.len() - 1]);
        }
    }
    None
}

pub(crate) fn parse_json_path_segments(path: &str) -> Result<Vec<Segment>, RecoverableError> {
    let path = path
        .strip_prefix("$.")
        .or_else(|| path.strip_prefix('$'))
        .unwrap_or(path);
    if path.is_empty() {
        return Ok(Vec::new());
    }
    let mut segments = Vec::new();
    for part in split_on_unbracketed_dot(path) {
        if part.is_empty() {
            continue;
        }
        if let Some(bracket_pos) = part.find('[') {
            let key = &part[..bracket_pos];
            if !key.is_empty() {
                segments.push(Segment::Key(key.to_string()));
            }
            let mut rest = &part[bracket_pos..];
            while !rest.is_empty() {
                if !rest.starts_with('[') {
                    return Err(unsupported_bracket(rest));
                }
                let end = rest.find(']').ok_or_else(|| unsupported_bracket(rest))?;
                let inner = &rest[1..end];
                segments.push(parse_bracket(inner)?);
                rest = &rest[end + 1..];
            }
        } else {
            segments.push(Segment::Key(part.to_string()));
        }
    }
    Ok(segments)
}

fn parse_bracket(inner: &str) -> Result<Segment, RecoverableError> {
    let supported_hint = "Supported forms: '.key', '[\"key\"]' / '['key']' (quoted key — use for keys containing '.'), '[N]' (non-negative integer), '[-N]' (negative integer), '[-N:]' (last N elements), '[*]' (every element — projects the rest of the path over the array). Forward slices '[a:b]' and filters '[?(...)]' are not supported.";
    // `[*]` projects the remaining path over every element. The results that
    // overflow are overwhelmingly arrays of records, so "this field from every
    // element" is the common recovery — measured 2026-08-15 as 73% of all rejected
    // segments, while the printed hint recommended a scalar extraction.
    if inner == "*" {
        return Ok(Segment::Wildcard);
    }
    if inner.is_empty() {
        return Err(RecoverableError::with_hint(
            "unsupported json_path segment '[]'".to_string(),
            supported_hint,
        ));
    }
    // Quoted string key: ["key"] or ['key']. Reaches object keys the bare
    // `.key` form cannot express (dots, leading digits, etc). A quoted
    // numeric string is a KEY, not an array index.
    if let Some(key) = strip_matching_quotes(inner) {
        return Ok(Segment::Key(key.to_string()));
    }
    if inner.chars().all(|c| c.is_ascii_digit()) {
        let n: usize = inner.parse().map_err(|_| {
            RecoverableError::with_hint(
                format!("unsupported json_path segment '[{}]'", inner),
                supported_hint,
            )
        })?;
        return Ok(Segment::Index(n));
    }
    if let Some(rest) = inner.strip_prefix('-') {
        let (mag_str, is_slice) = if let Some(s) = rest.strip_suffix(':') {
            (s, true)
        } else {
            (rest, false)
        };
        if mag_str.is_empty() || !mag_str.chars().all(|c| c.is_ascii_digit()) {
            return Err(RecoverableError::with_hint(
                format!("unsupported json_path segment '[{}]'", inner),
                supported_hint,
            ));
        }
        let mag: usize = mag_str.parse().map_err(|_| {
            RecoverableError::with_hint(
                format!("unsupported json_path segment '[{}]'", inner),
                supported_hint,
            )
        })?;
        if mag == 0 {
            return Err(RecoverableError::with_hint(
                format!("unsupported json_path segment '[{}]'", inner),
                "Use [0] for the first element",
            ));
        }
        return Ok(if is_slice {
            Segment::NegSliceFrom(mag)
        } else {
            Segment::NegIndex(mag)
        });
    }
    Err(RecoverableError::with_hint(
        format!("unsupported json_path segment '[{}]'", inner),
        supported_hint,
    ))
}

fn unsupported_bracket(s: &str) -> RecoverableError {
    RecoverableError::with_hint(
        format!("unsupported json_path segment near '{}'", s),
        "Supported forms: '.key', '[\"key\"]' / '['key']' (quoted key), '[N]', '[-N]', '[-N:]', '[*]' (every element).",
    )
}

/// The keys of `obj` as a hint line: all of them when there are few, otherwise the first
/// `HEAD`, a count of what was elided, and the last `TAIL`.
///
/// ONE function for every error that tells a caller what keys an object has: the key-miss
/// error in [`resolve_json_segment`] and the `[*]`-on-an-object error in [`eval_segments`].
/// They were two readers of the same fact, and the second had no way to name a key at all,
/// so a caller who projected `$[*].name` over a `run_command` envelope could not learn the
/// data sat under `stdout`. A copy of the window here would drift from the original, so there
/// is exactly one, and `wildcard_on_a_wide_object_states_its_elision_and_keeps_the_tail`
/// fails if the two errors ever render a different one.
///
/// The window is the decision recorded at the key-miss site: `serde_json` is built with
/// `preserve_order`, so `keys()` is insertion order and head-only truncation drops the keys
/// added LAST, which in a report object are the newest and the ones a caller is reaching
/// for. Keep both ends and state the elision.
fn describe_keys(obj: &serde_json::Map<String, Value>) -> String {
    const HEAD: usize = 7;
    const TAIL: usize = 3;
    let names: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();
    if names.len() > HEAD + TAIL {
        format!(
            "{} … (+{} more) … {}",
            names[..HEAD].join(", "),
            names.len() - HEAD - TAIL,
            names[names.len() - TAIL..].join(", "),
        )
    } else {
        names.join(", ")
    }
}

fn resolve_json_segment<'a>(
    value: &'a Value,
    seg: &Segment,
) -> Result<Cow<'a, Value>, RecoverableError> {
    match seg {
        Segment::Key(k) => match value {
            Value::Object(obj) => obj.get(k).map(Cow::Borrowed).ok_or_else(|| {
                // WINDOWED, AND THE WINDOW SAYS SO. This was `take(10)` with no marker:
                // the same defect `resolve_section_range` above already fixed for
                // headings, at the sibling site, with the same bias. `serde_json` is
                // built with `preserve_order`, so `keys()` yields INSERTION order —
                // head-only truncation drops the keys added LAST, which in a report
                // object are the newest and therefore exactly the ones a caller is
                // reaching for.
                //
                // Measured 2026-09-07 on `doctor`'s `catalog_health`: 13 keys, and
                // `take(10)` elided `open_bug_source_citations`, `audit` and `hint`. A
                // session then read the resulting complete-looking list as evidence that
                // the running binary did not contain the code that inserts them, and came
                // one step from reporting a shipped fix as missing. The list was not
                // wrong about any key it named; it was wrong by being read as total.
                //
                // The same file prints `absence from a cut list is not evidence` above a
                // truncated `violations` array, so the rule was already stated one field
                // away from where it was needed. Keep both ends and state the elision;
                // the rationale and the archived bug file for the heading half live on
                // `resolve_section_range`'s own HEAD/TAIL constants.
                RecoverableError::with_hint(
                    format!("path segment '{}' not found", k),
                    format!("Available keys: {}", describe_keys(obj)),
                )
            }),
            other => Err(RecoverableError::with_hint(
                format!(
                    "cannot apply key '{}' to {} (expected object)",
                    k,
                    json_type_name(other)
                ),
                "Use [N] to index into an array.",
            )),
        },
        Segment::Index(n) => match value {
            Value::Array(arr) => arr.get(*n).map(Cow::Borrowed).ok_or_else(|| {
                RecoverableError::with_hint(
                    format!(
                        "index {} out of bounds for array of length {}",
                        n,
                        arr.len()
                    ),
                    format!("Use an index in 0..{}", arr.len()),
                )
            }),
            other => Err(RecoverableError::with_hint(
                format!(
                    "cannot apply index '[{}]' to {} (expected array)",
                    n,
                    json_type_name(other)
                ),
                "Use .key to access an object field.",
            )),
        },
        Segment::NegIndex(n) => match value {
            Value::Array(arr) => {
                if *n >= 1 && *n <= arr.len() {
                    Ok(Cow::Borrowed(&arr[arr.len() - *n]))
                } else {
                    let len = arr.len();
                    Err(RecoverableError::with_hint(
                        format!("index -{} out of bounds for array of length {}", n, len),
                        format!(
                            "Use a non-negative index in 0..{} or a negative index in -{}..-1",
                            len, len
                        ),
                    ))
                }
            }
            other => Err(RecoverableError::with_hint(
                format!(
                    "cannot apply index '[-{}]' to {} (expected array)",
                    n,
                    json_type_name(other)
                ),
                "Use .key to access an object field.",
            )),
        },
        Segment::NegSliceFrom(n) => match value {
            Value::Array(arr) => {
                if *n >= 1 && *n <= arr.len() {
                    let start = arr.len() - *n;
                    Ok(Cow::Owned(Value::Array(arr[start..].to_vec())))
                } else {
                    let len = arr.len();
                    Err(RecoverableError::with_hint(
                        format!("index -{} out of bounds for array of length {}", n, len),
                        format!("For slice '[-N:]', N must be in 1..={}", len),
                    ))
                }
            }
            other => Err(RecoverableError::with_hint(
                format!(
                    "cannot apply slice '[-{}:]' to {} (expected array)",
                    n,
                    json_type_name(other)
                ),
                "Slice requires an array.",
            )),
        },
        // Unreachable by construction: `eval_segments` splits the path on the
        // first Wildcard and never passes one down here. Stated explicitly rather
        // than swept up by a catch-all arm, so that adding a future segment kind
        // still fails to compile until it is handled.
        Segment::Wildcard => Err(RecoverableError::new(
            "internal: '[*]' reached the per-segment resolver".to_string(),
        )),
    }
}

pub fn summarize_toml(content: &str) -> Value {
    let line_count = content.lines().count();

    // Scan for TOML table headers: [name] or [[name]]
    let mut sections: Vec<(String, usize)> = Vec::new(); // (header, line_1indexed)
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            sections.push((trimmed.to_string(), idx + 1));
        }
    }

    // If no table headers found, try parsing as TOML and list top-level keys
    if sections.is_empty() {
        if let Ok(table) = content.parse::<toml::Table>() {
            let total_keys = table.len();
            let keys: Vec<Value> = table
                .keys()
                .take(20)
                .map(|k| {
                    let line = find_toml_key_line(content, k);
                    serde_json::json!({ "key": k, "line": line.unwrap_or(0) })
                })
                .collect();
            let mut out = serde_json::json!({
                "type": "toml",
                "format": "toml",
                "line_count": line_count,
                "keys": keys,
            });
            if total_keys > 20 {
                out["total_keys"] = serde_json::json!(total_keys);
                out["keys_truncated"] = serde_json::json!(true);
            }
            return out;
        }
        let mut fallback = summarize_generic_file(content);
        fallback["type"] = serde_json::json!("toml");
        fallback["format"] = serde_json::json!("toml");
        return fallback;
    }

    // Compute end_line for each section
    let mut result_sections: Vec<Value> = Vec::new();
    for (i, (header, line)) in sections.iter().enumerate() {
        let end_line = sections
            .get(i + 1)
            .map(|(_, next)| next - 1)
            .unwrap_or(line_count);
        result_sections.push(serde_json::json!({
            "key": header,
            "line": line,
            "end_line": end_line,
        }));
    }
    let total_sections = result_sections.len();
    result_sections.truncate(30);

    let mut out = serde_json::json!({
        "type": "toml",
        "format": "toml",
        "line_count": line_count,
        "sections": result_sections,
    });
    if total_sections > 30 {
        out["total_sections"] = serde_json::json!(total_sections);
        out["sections_truncated"] = serde_json::json!(true);
    }
    out
}

fn find_toml_key_line(content: &str, key: &str) -> Option<u64> {
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with(key) && trimmed[key.len()..].trim_start().starts_with('=') {
            return Some((idx + 1) as u64);
        }
    }
    None
}

/// Scan a YAML document's top-level keys (column-0 `key:` lines), returning
/// `(key, 1-indexed line)` for each, UNCAPPED. `summarize_yaml` truncates the
/// result for display; `extract_yaml_key` uses the full list so key resolution
/// isn't limited by the display cap (the false-"not found" bug).
fn yaml_top_level_keys(content: &str) -> Vec<(String, usize)> {
    let mut keys = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() || trimmed == "---" || trimmed == "..." {
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') {
            if let Some(colon_pos) = trimmed.find(':') {
                let key = trimmed[..colon_pos].trim().to_string();
                if !key.is_empty() && !key.starts_with('-') {
                    keys.push((key, idx + 1));
                }
            }
        }
    }
    keys
}

pub fn summarize_yaml(content: &str) -> Value {
    let line_count = content.lines().count();
    let sections = yaml_top_level_keys(content);

    if sections.is_empty() {
        let mut fallback = summarize_generic_file(content);
        fallback["type"] = serde_json::json!("yaml");
        fallback["format"] = serde_json::json!("yaml");
        return fallback;
    }

    // Compute end_line for each section
    let mut result_sections: Vec<Value> = Vec::new();
    for (i, (key, line)) in sections.iter().enumerate() {
        let end_line = sections
            .get(i + 1)
            .map(|(_, next)| next - 1)
            .unwrap_or(line_count);
        result_sections.push(serde_json::json!({
            "key": key,
            "line": line,
            "end_line": end_line,
        }));
    }
    result_sections.truncate(30);

    serde_json::json!({
        "type": "yaml",
        "format": "yaml",
        "line_count": line_count,
        "sections": result_sections,
    })
}

pub fn summarize_config(content: &str) -> Value {
    let line_count = content.lines().count();
    let preview: String = content.lines().take(30).collect::<Vec<_>>().join("\n");
    serde_json::json!({
        "type": "config",
        "line_count": line_count,
        "preview": preview,
    })
}

pub fn summarize_generic_file(content: &str) -> Value {
    let lines: Vec<&str> = content.lines().collect();
    let line_count = lines.len();
    let head: String = lines
        .iter()
        .take(20)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let tail_start = line_count.saturating_sub(10);
    let tail: String = lines[tail_start..].join("\n");
    serde_json::json!({
        "type": "generic",
        "line_count": line_count,
        "head": head,
        "tail": tail,
    })
}
/// Serialized-size target for a whole `read_file`/`read_markdown` summary ENVELOPE (the summary
/// plus the keys `read_full_file` and the markdown tiers add around it: `file_id`, `complete`,
/// `overflow`, `hint`, `coverage`): `INLINE_BYTE_BUDGET`, which leaves the slack
/// `call_content` needs for path stripping and the guide hint it may still add.
///
/// The summarizers above bound their output by COUNT: every symbol, the first 20 and last 10
/// lines, 30 lines of a config, 30 sections. A count has no size. A 1,500-function file or a
/// file of a few very wide lines made the summary JSON exceed `TOOL_OUTPUT_BUFFER_THRESHOLD`,
/// `call_content` buffered it a second time under `@tool_*`, and the caller held two handles
/// for one read plus a `json_path="$.field"` hint that reaches nothing. Measured 2026-10-05:
/// `read_file` on a 6,206-line source file returned `@tool_0bce7715` (20,636 B) beside the
/// tool's own `@file_0bce76f9`. It is the same defect `summarize_generic` had in
/// `command_summary.rs` and the same one `GENERIC_FIELD_BYTE_BUDGET` fixed there.
///
/// **The target is applied to the MEASURED envelope, never to a guess about it.** An earlier
/// version cut every summary to a fixed 6,000 B "to leave room for the keys", but the keys cost
/// ~300-420 B, so summaries of 6,000-9,600 B that had always come back whole were cut: a
/// contract change for ordinary files. [`fit_envelope`] builds the envelope, and only if that
/// is over the INLINE LIMIT does it cut, down to this target.
// cap-class: RESULT_CAP file_summary.summary_bytes — probed
pub(crate) const SUMMARY_ENVELOPE_BUDGET: usize = crate::tools::INLINE_BYTE_BUDGET;

/// Build the response envelope around `summary` and make it fit the inline limit by cutting
/// only the EXCESS, so a summary that fits comes back byte-for-byte as it always did.
///
/// `finish(summary, notes)` is the caller's own construction of the whole response (the keys
/// it adds around the summary, the cut notes it puts in its hint). Passing it in is what lets
/// this measure the REAL envelope: the budget handed to [`bound_summary`] is
/// [`SUMMARY_ENVELOPE_BUDGET`] minus what `finish` costs around an empty summary, not a
/// constant guessed to be about right.
///
/// 1. The envelope with the summary as built fits the inline limit: returned untouched.
/// 2. Otherwise the summary is cut to the budget and the envelope re-measured WITH the notes
///    the cut produced, because those lengthen the hint; if it is still over, the budget
///    shrinks by exactly the excess and the cut is redone from the ORIGINAL summary (up to
///    four times, which every case measured needs one or two).
/// 3. If that never converges, the minimal `summary_omitted` summary (budget 0) is used, which
///    always fits. So the result is never oversized and never silently so.
pub(crate) fn fit_envelope(
    summary: Value,
    file_id: &str,
    finish: impl Fn(Value, &[String]) -> Value,
) -> Value {
    let over_limit = |v: &Value| !crate::tools::response_fits(v);
    let whole = finish(summary.clone(), &[]);
    if !over_limit(&whole) {
        return whole;
    }
    let overhead = finish(Value::Object(serde_json::Map::new()), &[])
        .to_string()
        .len();
    let mut budget = SUMMARY_ENVELOPE_BUDGET.saturating_sub(overhead);
    for _ in 0..4 {
        let (bounded, notes) = bound_summary(summary.clone(), file_id, budget);
        let envelope = finish(bounded, &notes);
        let size = envelope.to_string().len();
        if size <= SUMMARY_ENVELOPE_BUDGET {
            return envelope;
        }
        budget = budget.saturating_sub(size - SUMMARY_ENVELOPE_BUDGET + 16);
    }
    let (minimal, notes) = bound_summary(summary, file_id, 0);
    finish(minimal, &notes)
}

/// Bound a whole-file summary to `budget` SERIALIZED bytes, keeping both ends of whatever is
/// cut, and say what was cut.
///
/// Returns the bounded summary and one note per cut array, for the caller to put in the
/// `overflow` hint. A summary that already fits is returned untouched.
///
/// **The result always fits `budget`.** Every size here is measured on the serialized JSON,
/// never estimated from raw bytes: JSON escaping turns one raw byte into 2 (`"`, `\`, tab) or 6
/// (`\x01`, ANSI `\x1b`), so a cut sized in raw bytes left a 6 KB wide line at 11 KB or more
/// and the response was buffered a second time. What the two passes below cannot cut is
/// replaced by a minimal summary marked `summary_omitted` (the last resort), so an oversized
/// summary is never returned silently.
///
/// **Strings first** (`head`, `tail`, `preview`, any top-level string over 500 B): each is
/// middle-elided with `elide_middle_bytes`, its marker naming the buffer handle. They share
/// ONE raw cut size, the largest whose whole summary serializes within the budget, so `head`
/// and `tail` are cut alike (largest-first would crush one) and a short string is left whole.
///
/// **Then arrays** (`symbols`, `sections`, `headings`, `keys`, at the top level or one level
/// down, where `summarize_json` keeps `schema.keys`): every array is taken
/// out, what is left is the fixed part, and the rest of the budget is shared between the arrays
/// by max-min fairness. Smallest first, each gets at most an equal share of what remains, so an
/// array that fits its share is returned whole and what it leaves unused goes to the larger
/// ones. Each cut array loses its MIDDLE entries: the first and last halves of its allowance
/// survive. Entries
/// stay objects of the same shape, so no consumer sees a foreign element; the gap is described
/// in structured keys beside the array (`<key>_truncated`, `total_<key>` if absent, and
/// `<key>_omitted {after, count, from_line, to_line}`) and rendered where it falls.
pub(crate) fn bound_summary(
    mut summary: Value,
    file_id: &str,
    budget: usize,
) -> (Value, Vec<String>) {
    let size = |v: &Value| v.to_string().len();
    let mut notes: Vec<String> = Vec::new();
    if size(&summary) <= budget {
        return (summary, notes);
    }
    let original_bytes = size(&summary);

    // ---- strings ----
    // A string is "wide" above 500 B: below that it is part of the fixed cost of the summary.
    let wide: Vec<(String, String)> = summary
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| !matches!(k.as_str(), "type" | "format"))
        .filter_map(|(k, v)| {
            v.as_str()
                .filter(|s| s.len() > 500)
                .map(|s| (k.clone(), s.to_owned()))
        })
        .collect();
    if !wide.is_empty() {
        let remedy = format!("the rest: read_file(path=\"{file_id}\", start_line=N, end_line=M)");
        let with_share = |base: &Value, share: usize| -> Value {
            let mut cut = base.clone();
            for (key, original) in &wide {
                cut[key.as_str()] = Value::String(crate::util::text::elide_middle_bytes(
                    original.clone(),
                    original.len(),
                    share,
                    key,
                    &remedy,
                ));
            }
            cut
        };
        // The share is RAW bytes (that is what `elide_middle_bytes` clips), but the budget is
        // SERIALIZED bytes, and JSON escaping turns one raw byte into 2 (`"`, `\`, tab, newline)
        // or 6 (`\x01`, `\x1b`). So the share is found by measuring what each candidate really
        // serializes to, not by dividing the budget: the largest share whose whole summary fits.
        // One common share for every wide string, so a small one is simply left whole and its
        // unused room goes to the larger ones.
        let longest = wide.iter().map(|(_, s)| s.len()).max().unwrap_or(0);
        let (mut lo, mut hi) = (0usize, longest); // `hi` is known not to fit: nothing is cut
        let mut best: Option<usize> = None;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if size(&with_share(&summary, mid)) <= budget {
                best = Some(mid);
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        // No share fits: take the smallest one (markers only) and let the arrays, or the last
        // resort below, finish the job.
        summary = with_share(&summary, best.unwrap_or(0));
    }
    if size(&summary) <= budget {
        return (summary, notes);
    }

    // ---- arrays ----
    // Top level, and one level down: `summarize_json` nests its key list under `schema`.
    let mut found: Vec<(String, String)> = Vec::new(); // (parent pointer, key)
    for (k, v) in summary.as_object().into_iter().flatten() {
        match v {
            Value::Array(a) if !a.is_empty() => found.push((String::new(), k.clone())),
            Value::Object(inner) => {
                for (ik, iv) in inner {
                    if iv.as_array().is_some_and(|a| !a.is_empty()) {
                        found.push((format!("/{k}"), ik.clone()));
                    }
                }
            }
            _ => {}
        }
    }
    // Take every array OUT first, so what is left is the fixed part, then share what is left
    // between them. Cutting the largest while the others are still whole would leave it no room
    // at all: with two arrays in one summary, the larger lost EVERY entry.
    let mut taken: Vec<(String, String, Vec<Value>, usize)> = Vec::new();
    for (parent, key) in found {
        let Some(entries) = summary
            .pointer(&parent)
            .and_then(|h| h.get(&key))
            .and_then(Value::as_array)
            .cloned()
        else {
            continue;
        };
        let bytes = size(&Value::Array(entries.clone()));
        if let Some(slot) = summary.pointer_mut(&parent).and_then(|h| h.get_mut(&key)) {
            *slot = Value::Array(Vec::new());
        }
        taken.push((parent, key, entries, bytes));
    }
    let count = taken.len();
    // 300 B per array for the structured markers a cut adds beside it.
    let mut remaining = budget.saturating_sub(size(&summary) + 300 * count);
    // Smallest first, each getting at most an equal share of what is left: an array that fits
    // its share is returned whole, and what it leaves unused goes to the larger ones.
    taken.sort_by_key(|(_, _, _, bytes)| *bytes);
    for (i, (parent, key, entries, bytes)) in taken.into_iter().enumerate() {
        let fair = remaining / (count - i);
        if bytes <= fair {
            // Fits its share whole: put it back untouched. Cutting by halves of its own size
            // would lose up to an entry of what fits, because each half rounds down.
            remaining -= bytes;
            if let Some(slot) = summary.pointer_mut(&parent).and_then(|h| h.get_mut(&key)) {
                *slot = Value::Array(entries);
            }
            continue;
        }
        remaining -= fair;
        if let Some(note) = cut_array_middle(&mut summary, &parent, &key, entries, fair, file_id) {
            notes.push(note);
        }
    }

    // ---- last resort ----
    // Whatever is left over budget cannot be cut by the two passes above: a wide value nested
    // deeper than one level, or a fixed part larger than the budget on its own. Returning it
    // would hand the caller an oversized summary with nothing saying so, so it is replaced by
    // the smallest summary that still names what the file is, marked as dropped. Loud by
    // construction: `summary_omitted` is a key a caller and a test can both read, and the
    // note says where to read the file instead.
    if size(&summary) > budget {
        let mut minimal = serde_json::Map::new();
        for key in ["type", "format", "line_count", "lines"] {
            if let Some(v) = summary.get(key) {
                minimal.insert(key.to_string(), v.clone());
            }
        }
        minimal.insert("summary_omitted".to_string(), Value::Bool(true));
        notes.push(format!(
            "summary: {original_bytes} bytes omitted entirely (too wide to cut); read the file in \
             ranges with read_file(path=\"{file_id}\", start_line=N, end_line=M)."
        ));
        return (Value::Object(minimal), notes);
    }
    (summary, notes)
}

/// Cut the MIDDLE entries out of `entries`, the array `key` inside the object at JSON pointer
/// `parent` ("" is the summary itself; `/schema` for a JSON summary's key list), keeping what
/// fits `allowance` bytes, half from each end. The caller took the array out and priced it.
/// Returns the note for the caller's hint, or `None` when `key` is not an array.
pub(crate) fn cut_array_middle(
    summary: &mut Value,
    parent: &str,
    key: &str,
    entries: Vec<Value>,
    allowance: usize,
    file_id: &str,
) -> Option<String> {
    let total = entries.len();
    let half = allowance / 2;
    let cost = |e: &Value| e.to_string().len() + 1; // +1 for the comma

    let (mut head, mut used) = (0usize, 0usize);
    while head < total && used + cost(&entries[head]) <= half {
        used += cost(&entries[head]);
        head += 1;
    }
    let (mut tail, mut used) = (0usize, 0usize);
    while head + tail < total && used + cost(&entries[total - 1 - tail]) <= half {
        used += cost(&entries[total - 1 - tail]);
        tail += 1;
    }
    // The array fit its allowance (or only the brackets and commas it was priced with did not):
    // put it back whole and say nothing, rather than mark a cut that removed no entry.
    if head + tail >= total {
        *summary.pointer_mut(parent)?.get_mut(key)? = Value::Array(entries);
        return None;
    }

    let omitted = total - head - tail;
    // Entries name their line `line` (symbols, sections, headings in a file summary) or `l`
    // (the compact heading map `read_markdown` returns).
    let line_of = |e: &Value| e["line"].as_u64().or_else(|| e["l"].as_u64());
    // The gap is "the line after the last kept head entry up to the line before the first kept
    // tail entry", which is the omitted entries' lines ONLY when lines strictly ascend through the
    // WHOLE array: kept AND omitted, since an omitted entry out of order moves the gap too. Flat
    // TOML keys are listed through `toml::Table`, which is alphabetical, so a file written z..a
    // produced `from_line: 15, to_line: 4` and a ready-made `read_file(start_line=15,
    // end_line=4)` that fails with "invalid line range". Without a monotonic array no range is
    // reported (the markers are null and the note is the generic one, which carries no numbers).
    let lines: Vec<Option<u64>> = entries.iter().map(line_of).collect();
    let in_line_order = lines.iter().all(Option::is_some) && lines.windows(2).all(|w| w[0] < w[1]);
    let from = if in_line_order { lines[head] } else { None };
    let to = if in_line_order && tail > 0 {
        lines[total - tail].map(|l| l.saturating_sub(1))
    } else {
        None
    };

    // The markers go BESIDE the array, in whichever object holds it: `schema` for a JSON
    // summary's key list, the summary itself otherwise.
    let holder = summary.pointer_mut(parent)?;
    let grand_total = holder[format!("total_{key}")]
        .as_u64()
        .map(|t| t as usize)
        .unwrap_or(total);
    let mut kept: Vec<Value> = entries[..head].to_vec();
    kept.extend_from_slice(&entries[total - tail..]);
    holder[key] = Value::Array(kept);
    holder[format!("{key}_truncated").as_str()] = Value::Bool(true);
    if holder.get(format!("total_{key}")).is_none() {
        holder[format!("total_{key}").as_str()] = serde_json::json!(total);
    }
    holder[format!("{key}_omitted").as_str()] = serde_json::json!({
        "after": head,
        "count": omitted,
        "from_line": from,
        "to_line": to,
    });

    let label = if parent.is_empty() {
        key.to_string()
    } else {
        format!("{}.{key}", parent.trim_start_matches('/'))
    };
    Some(match (from, to) {
        (Some(f), Some(t)) => format!(
            "{label}: {omitted} of {grand_total} entries omitted (lines {f}-{t}); read them with \
             read_file(path=\"{file_id}\", start_line={f}, end_line={t})."
        ),
        _ => format!(
            "{label}: {omitted} of {grand_total} entries omitted; read the file in ranges with \
             read_file(path=\"{file_id}\", start_line=N, end_line=M)."
        ),
    })
}

pub fn extract_toml_key(content: &str, key: &str) -> Result<SectionResult, RecoverableError> {
    let summary = summarize_toml(content);

    // Fast path: a table header present in the (display-capped) summary gives
    // precise source line ranges + sibling headers. Enrichment only — the
    // full-parse fallback below is the authoritative resolver.
    if let Some(sections) = summary["sections"].as_array() {
        let table_name = format!("[{}]", key);
        let array_name = format!("[[{}]]", key);
        if let Some(matched) = sections.iter().find(|s| {
            let k = s["key"].as_str().unwrap_or("");
            k == table_name || k == array_name || k == key
        }) {
            let line = matched["line"].as_u64().unwrap_or(1) as usize;
            let end_line = matched["end_line"].as_u64().unwrap_or(1) as usize;
            let lines: Vec<&str> = content.lines().collect();
            let start = (line - 1).min(lines.len());
            let end = end_line.min(lines.len());
            let section_content = lines[start..end].join("\n");
            let siblings: Vec<String> = sections
                .iter()
                .filter_map(|s| s["key"].as_str())
                .filter(|k| *k != matched["key"].as_str().unwrap_or(""))
                .map(|s| s.to_string())
                .collect();
            return Ok(SectionResult {
                content: section_content,
                line_range: (line, end_line),
                heading_text: matched["key"].as_str().unwrap_or("?").to_string(),
                breadcrumb: vec![matched["key"].as_str().unwrap_or("?").to_string()],
                siblings,
                format: "toml".to_string(),
            });
        }
    }

    // Authoritative resolution against the FULL parse — independent of
    // summarize_toml's 30-section display cap and its sections-XOR-keys
    // branching. Fixes false "not found" for tables past the cap, and the
    // dead dotted/flat-key fallback for files mixing top-level scalars with
    // tables. See docs/issues/archive/2026-07-10-toml-yaml-key-false-not-found-past-summary-cap.md
    // and docs/issues/archive/2026-07-10-extract-toml-key-branch-order-mixed-files-unreachable.md.
    let table = content.parse::<toml::Table>().map_err(|e| {
        RecoverableError::with_hint(
            format!("failed to parse TOML: {e}"),
            "File could not be parsed as TOML",
        )
    })?;
    let root = toml::Value::Table(table);
    let available: Vec<String> = match &root {
        toml::Value::Table(t) => t.keys().cloned().collect(),
        _ => Vec::new(),
    };
    let segments: Vec<&str> = key.split('.').collect();
    let mut current: &toml::Value = &root;
    for seg in &segments {
        current = current.get(seg).ok_or_else(|| {
            RecoverableError::with_hint(
                format!("key '{}' not found in TOML", key),
                format!("Available top-level keys: {}", available.join(", ")),
            )
        })?;
    }
    let pretty = toml::to_string_pretty(current).unwrap_or_else(|_| format!("{current:?}"));
    Ok(SectionResult {
        content: pretty,
        line_range: (1, content.lines().count()),
        heading_text: segments
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join("."),
        breadcrumb: segments.iter().map(|s| s.to_string()).collect(),
        siblings: Vec::new(),
        format: "toml".to_string(),
    })
}

pub fn extract_yaml_key(content: &str, key: &str) -> Result<SectionResult, RecoverableError> {
    // Resolve against the FULL (uncapped) top-level key scan, not
    // summarize_yaml's 30-key display cap — fixes false "not found" for keys
    // past position 30. See
    // docs/issues/archive/2026-07-10-toml-yaml-key-false-not-found-past-summary-cap.md.
    // (Nested-key resolution remains unsupported — a pre-existing feature gap,
    // not a regression; no YAML deserializer is a dependency here.)
    let keys = yaml_top_level_keys(content);
    if keys.is_empty() {
        return Err(RecoverableError::with_hint(
            format!("key '{}' not found in YAML", key),
            "No top-level keys found in file",
        ));
    }
    let line_count = content.lines().count();
    if let Some(pos) = keys.iter().position(|(k, _)| k == key) {
        let (_, line) = keys[pos];
        let end_line = keys
            .get(pos + 1)
            .map(|(_, next)| next - 1)
            .unwrap_or(line_count);
        let lines: Vec<&str> = content.lines().collect();
        let start = (line - 1).min(lines.len());
        let end = end_line.min(lines.len());
        let section_content = lines[start..end].join("\n");
        let siblings: Vec<String> = keys
            .iter()
            .map(|(k, _)| k.clone())
            .filter(|k| k != key)
            .collect();
        return Ok(SectionResult {
            content: section_content,
            line_range: (line, end_line),
            heading_text: key.to_string(),
            breadcrumb: vec![key.to_string()],
            siblings,
            format: "yaml".to_string(),
        });
    }
    let available: Vec<String> = keys.into_iter().map(|(k, _)| k).collect();
    Err(RecoverableError::with_hint(
        format!("key '{}' not found in YAML", key),
        format!("Available keys: {}", available.join(", ")),
    ))
}
