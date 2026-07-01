use std::borrow::Cow;

use serde_json::Value;

use crate::tools::RecoverableError;

pub struct SectionResult {
    pub content: String,
    pub line_range: (usize, usize), // 1-indexed, inclusive
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
    headings.truncate(30);
    serde_json::json!({
        "type": "markdown",
        "line_count": line_count,
        "headings": headings,
    })
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

/// Parse all markdown headings with their line ranges. No truncation.
/// Skips headings inside fenced code blocks.
pub fn parse_all_headings(content: &str) -> Vec<HeadingInfo> {
    let line_count = content.lines().count();

    // Pre-scan: an odd number of ``` fence markers means the file has an
    // unclosed code block (typical during in-flight batch edits whose
    // intermediate new_string contains a half-fence). CommonMark would
    // extend that fence to EOF, hiding every heading after it. For an
    // editor tool, that's brittle: we'd rather treat unbalanced fences
    // as plain text and still find the headings. Bug:
    // docs/issues/2026-05-21-edit-markdown-last-heading-unaddressable.md
    let fence_count = content.lines().filter(|l| l.starts_with("```")).count();
    let fences_balanced = fence_count % 2 == 0;

    let mut in_code_block = false;
    let mut raw: Vec<(String, usize, usize)> = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        if fences_balanced && line.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
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
/// Errors on duplicate exact matches (ambiguity) or no match.
pub fn resolve_section_range(
    content: &str,
    heading_query: &str,
) -> Result<SectionRange, RecoverableError> {
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

    // Helper to build duplicate error
    let dup_error = |indices: &[usize]| -> RecoverableError {
        let lines: Vec<String> = indices
            .iter()
            .map(|&i| headings[i].line.to_string())
            .collect();
        RecoverableError::with_hint(
            format!(
                "heading '{}' found {} times (lines {})",
                heading_query,
                indices.len(),
                lines.join(", ")
            ),
            "Provide a more specific heading or use edit_file with start_line/end_line to target a specific occurrence.",
        )
    };

    // Tier 1: Exact match (raw)
    let exact_raw: Vec<usize> = headings
        .iter()
        .enumerate()
        .filter(|(_, h)| h.text == heading_query)
        .map(|(i, _)| i)
        .collect();
    if exact_raw.len() == 1 {
        return Ok(make_range(&headings[exact_raw[0]]));
    }
    if exact_raw.len() > 1 {
        return Err(dup_error(&exact_raw));
    }

    // Tier 2: Exact match (stripped)
    let exact_stripped: Vec<usize> = headings
        .iter()
        .enumerate()
        .filter(|(_, h)| strip_inline_formatting(&h.text) == query_stripped)
        .map(|(i, _)| i)
        .collect();
    if exact_stripped.len() == 1 {
        return Ok(make_range(&headings[exact_stripped[0]]));
    }
    if exact_stripped.len() > 1 {
        return Err(dup_error(&exact_stripped));
    }

    // Tier 3: Prefix match (stripped, case-insensitive)
    if let Some(idx) = headings.iter().position(|h| {
        strip_inline_formatting(&h.text)
            .to_lowercase()
            .starts_with(&query_stripped_lower)
    }) {
        return Ok(make_range(&headings[idx]));
    }

    // Tier 4: Substring match (stripped, case-insensitive)
    if let Some(idx) = headings.iter().position(|h| {
        strip_inline_formatting(&h.text)
            .to_lowercase()
            .contains(&query_stripped_lower)
    }) {
        return Ok(make_range(&headings[idx]));
    }

    // No match
    let available: Vec<&str> = headings.iter().map(|h| h.text.as_str()).take(15).collect();
    Err(RecoverableError::with_hint(
        format!("heading '{}' not found", heading_query),
        format!("Available headings: {}", available.join(", ")),
    ))
}

pub fn extract_markdown_section(
    content: &str,
    heading_query: &str,
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
            serde_json::json!({ "root_type": "object", "keys": keys })
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
    let mut current: Cow<'_, Value> = Cow::Borrowed(&parsed);
    for seg in &segments {
        current = match current {
            Cow::Borrowed(v) => resolve_json_segment(v, seg)?,
            Cow::Owned(v) => {
                let r = resolve_json_segment(&v, seg)?;
                Cow::Owned(r.into_owned())
            }
        };
    }
    let final_ref: &Value = current.as_ref();
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
    let supported_hint = "Supported forms: '.key', '[\"key\"]' / '['key']' (quoted key — use for keys containing '.'), '[N]' (non-negative integer), '[-N]' (negative integer), '[-N:]' (last N elements). Other slice/filter forms not supported.";
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
        "Supported forms: '.key', '[\"key\"]' / '['key']' (quoted key), '[N]', '[-N]', '[-N:]'.",
    )
}

fn resolve_json_segment<'a>(
    value: &'a Value,
    seg: &Segment,
) -> Result<Cow<'a, Value>, RecoverableError> {
    match seg {
        Segment::Key(k) => match value {
            Value::Object(obj) => obj.get(k).map(Cow::Borrowed).ok_or_else(|| {
                let available = obj.keys().take(10).cloned().collect::<Vec<_>>().join(", ");
                RecoverableError::with_hint(
                    format!("path segment '{}' not found", k),
                    format!("Available keys: {}", available),
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
            let keys: Vec<Value> = table
                .keys()
                .take(20)
                .map(|k| {
                    let line = find_toml_key_line(content, k);
                    serde_json::json!({ "key": k, "line": line.unwrap_or(0) })
                })
                .collect();
            return serde_json::json!({
                "type": "toml",
                "format": "toml",
                "line_count": line_count,
                "keys": keys,
            });
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
    result_sections.truncate(30);

    serde_json::json!({
        "type": "toml",
        "format": "toml",
        "line_count": line_count,
        "sections": result_sections,
    })
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

pub fn summarize_yaml(content: &str) -> Value {
    let line_count = content.lines().count();

    // Scan for top-level keys: lines starting at column 0, containing ':'
    let mut sections: Vec<(String, usize)> = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() || trimmed == "---" || trimmed == "..." {
            continue;
        }
        // Top-level key: starts at column 0 (no leading whitespace), has a colon
        if !line.starts_with(' ') && !line.starts_with('\t') {
            if let Some(colon_pos) = trimmed.find(':') {
                let key = trimmed[..colon_pos].trim().to_string();
                if !key.is_empty() && !key.starts_with('-') {
                    sections.push((key, idx + 1));
                }
            }
        }
    }

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

pub fn extract_toml_key(content: &str, key: &str) -> Result<SectionResult, RecoverableError> {
    let summary = summarize_toml(content);

    // Check sections first (table headers like [dependencies])
    if let Some(sections) = summary["sections"].as_array() {
        // Match against table names: key could be "dependencies" matching "[dependencies]"
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
                breadcrumb: vec![matched["key"].as_str().unwrap_or("?").to_string()],
                siblings,
                format: "toml".to_string(),
            });
        }

        // Not found in sections — show available
        let available: Vec<&str> = sections.iter().filter_map(|s| s["key"].as_str()).collect();
        return Err(RecoverableError::with_hint(
            format!("key '{}' not found in TOML", key),
            format!("Available sections: {}", available.join(", ")),
        ));
    }

    // No sections found — check top-level keys
    if let Some(keys) = summary["keys"].as_array() {
        let available: Vec<&str> = keys.iter().filter_map(|k| k["key"].as_str()).collect();
        // Try to find the key and extract via parsing
        if let Ok(table) = content.parse::<toml::Table>() {
            let segments: Vec<&str> = key.split('.').collect();
            let mut current: &toml::Value = &toml::Value::Table(table);
            for seg in &segments {
                current = current.get(seg).ok_or_else(|| {
                    RecoverableError::with_hint(
                        format!("key '{}' not found in TOML", key),
                        format!("Available keys: {}", available.join(", ")),
                    )
                })?;
            }
            let pretty =
                toml::to_string_pretty(current).unwrap_or_else(|_| format!("{:?}", current));
            return Ok(SectionResult {
                content: pretty,
                line_range: (1, content.lines().count()),
                breadcrumb: segments.iter().map(|s| s.to_string()).collect(),
                siblings: Vec::new(),
                format: "toml".to_string(),
            });
        }
    }

    Err(RecoverableError::with_hint(
        format!("key '{}' not found in TOML", key),
        "File could not be parsed as TOML",
    ))
}

pub fn extract_yaml_key(content: &str, key: &str) -> Result<SectionResult, RecoverableError> {
    let summary = summarize_yaml(content);

    if let Some(sections) = summary["sections"].as_array() {
        if let Some(matched) = sections
            .iter()
            .find(|s| s["key"].as_str().unwrap_or("") == key)
        {
            let line = matched["line"].as_u64().unwrap_or(1) as usize;
            let end_line = matched["end_line"].as_u64().unwrap_or(1) as usize;
            let lines: Vec<&str> = content.lines().collect();
            let start = (line - 1).min(lines.len());
            let end = end_line.min(lines.len());
            let section_content = lines[start..end].join("\n");
            let siblings: Vec<String> = sections
                .iter()
                .filter_map(|s| s["key"].as_str())
                .filter(|k| *k != key)
                .map(|s| s.to_string())
                .collect();
            return Ok(SectionResult {
                content: section_content,
                line_range: (line, end_line),
                breadcrumb: vec![key.to_string()],
                siblings,
                format: "yaml".to_string(),
            });
        }

        let available: Vec<String> = sections
            .iter()
            .filter_map(|s| s["key"].as_str().map(|s| s.to_string()))
            .collect();
        return Err(RecoverableError::with_hint(
            format!("key '{}' not found in YAML", key),
            format!("Available keys: {}", available.join(", ")),
        ));
    }

    Err(RecoverableError::with_hint(
        format!("key '{}' not found in YAML", key),
        "No top-level keys found in file",
    ))
}
