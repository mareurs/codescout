//! Flat-YAML frontmatter mutation for markdown files.
//!
//! Scope is intentionally narrow: one-key-per-line, scalar / string / inline-array
//! values only. Designed to close the recurring "sed-shuffle for `status:` flips"
//! friction on `docs/issues/*.md` and tracker files. Nested YAML, multi-line values,
//! and anchors/aliases are out of scope — if they're ever needed, replace this module
//! with `serde_yaml` and revisit.

use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Frontmatter {
    pub lines: Vec<String>,
    pub body_start_line_idx: usize,
}

/// Extract a frontmatter block from the start of a markdown file.
///
/// Returns `Ok(None)` if no frontmatter is present (file does not start with
/// `---`). Returns `Err` if the file starts with `---` but no closing delimiter
/// is found — better to surface a malformed file than to silently truncate it.
pub fn extract_frontmatter(content: &str) -> Result<Option<Frontmatter>> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return Ok(None);
    }
    let end_idx = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| l.trim_end() == "---")
        .map(|(i, _)| i)
        .ok_or_else(|| {
            anyhow!(
                "frontmatter is malformed — file starts with `---` but no closing `---` delimiter found"
            )
        })?;
    let block: Vec<String> = lines[1..end_idx].iter().map(|s| s.to_string()).collect();
    Ok(Some(Frontmatter {
        lines: block,
        body_start_line_idx: end_idx + 1,
    }))
}

/// Apply `set` / `delete` operations to a frontmatter block, preserving the
/// order of existing keys. Keys present in `set` but not in the block are
/// appended at the end; keys in `delete` not present are silently ignored
/// (idempotent-friendly).
///
/// Each value in `set` is serialized to its YAML inline form:
///   - String → bare unless it needs quoting (contains `:`, `#`, `"`, leading
///     whitespace, leading reserved-indicator, or matches reserved literal
///     `true|false|null|~`); then double-quoted with `\"`/`\\` escaping.
///   - Number / Bool → bare.
///   - Null → empty (key written as `key:`).
///   - Array → `[v1, v2, ...]` with each element serialized recursively.
///   - Object → rejected (this module is flat-only).
pub fn apply_ops(
    block: &[String],
    set: &HashMap<String, Value>,
    delete: &[String],
) -> Result<Vec<String>> {
    // Preserve order; flag which keys we mutated so we can append the rest.
    let mut out: Vec<String> = Vec::with_capacity(block.len() + set.len());
    let mut applied: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let delete_set: std::collections::HashSet<&str> = delete.iter().map(|s| s.as_str()).collect();

    for line in block {
        match line_key(line) {
            Some(key) if delete_set.contains(key) => {
                // skip — deletes the line
            }
            Some(key) if set.contains_key(key) => {
                let v = &set[key];
                out.push(format!("{key}: {}", serialize_value(v)?));
                applied.insert(key);
            }
            _ => out.push(line.clone()),
        }
    }
    // Append any set keys that didn't already exist.
    for (key, value) in set {
        if applied.contains(key.as_str()) {
            continue;
        }
        // Validate key shape — no whitespace, no leading punctuation.
        if key.is_empty() || key.chars().any(|c| c.is_whitespace() || c == ':') {
            bail!(
                "invalid frontmatter key '{}' — must be non-empty with no whitespace or colons",
                key
            );
        }
        out.push(format!("{key}: {}", serialize_value(value)?));
    }
    Ok(out)
}

/// Splice a rewritten frontmatter block back into the original content,
/// preserving the body (and any trailing newline behaviour of the original).
pub fn splice_back(original: &str, new_block: &[String], fm: &Frontmatter) -> String {
    let lines: Vec<&str> = original.lines().collect();
    let body: Vec<&str> = lines.iter().skip(fm.body_start_line_idx).copied().collect();

    let mut out = String::new();
    out.push_str("---\n");
    for line in new_block {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("---\n");
    for line in &body {
        out.push_str(line);
        out.push('\n');
    }
    // Preserve trailing-newline absence if the original didn't have one.
    if !original.ends_with('\n') {
        // pop the last '\n' we just added
        if out.ends_with('\n') {
            out.pop();
        }
    }
    out
}

/// Extract the key from a frontmatter line, or None if it's a comment / blank.
fn line_key(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    // Indented lines are continuations (multi-line YAML); we don't support
    // those, so return None to leave them alone.
    if line.starts_with(' ') || line.starts_with('\t') {
        return None;
    }
    let colon = trimmed.find(':')?;
    Some(&trimmed[..colon])
}

/// Serialize a JSON value to its YAML inline form. Errors on nested objects
/// (this module is flat-only by contract).
fn serialize_value(v: &Value) -> Result<String> {
    match v {
        Value::Null => Ok(String::new()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) => Ok(n.to_string()),
        Value::String(s) => Ok(serialize_string(s)),
        Value::Array(items) => {
            // Array elements are always quoted when they are strings, to match
            // the existing convention in `docs/issues/*.md` frontmatter
            // (`tags: ["lsp", "cold-start"]`). Numbers / bools / nulls still
            // serialize bare.
            let parts: Result<Vec<String>> = items.iter().map(serialize_array_elem).collect();
            Ok(format!("[{}]", parts?.join(", ")))
        }
        Value::Object(_) => {
            bail!("nested objects are not supported — this frontmatter editor is flat-only")
        }
    }
}

/// Always-quote-strings variant for use inside flow arrays.
fn serialize_array_elem(v: &Value) -> Result<String> {
    match v {
        Value::String(s) => {
            let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
            Ok(format!("\"{escaped}\""))
        }
        _ => serialize_value(v),
    }
}

/// Decide whether a string needs quoting, and emit the safe form.
fn serialize_string(s: &str) -> String {
    let needs_quoting = s.is_empty()
        || s != s.trim()
        || s.contains(':')
        || s.contains('#')
        || s.contains('"')
        || s.contains('\n')
        || s.starts_with(['[', '{', '*', '?', '&', '!', '|', '>', '\'', '-'])
        || matches!(s, "true" | "false" | "null" | "yes" | "no" | "~");
    if needs_quoting {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fm(content: &str) -> Frontmatter {
        extract_frontmatter(content).unwrap().unwrap()
    }

    #[test]
    fn extract_returns_none_when_no_frontmatter() {
        let r = extract_frontmatter("# Title\n\nbody\n").unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn extract_returns_block_for_well_formed_frontmatter() {
        let src = "---\nstatus: open\nseverity: medium\n---\n# Title\nbody\n";
        let f = fm(src);
        assert_eq!(f.lines, vec!["status: open", "severity: medium"]);
        // Lines: 0=`---`, 1=`status:…`, 2=`severity:…`, 3=`---`, 4=`# Title`
        // Body starts at line 4.
        assert_eq!(f.body_start_line_idx, 4);
    }

    #[test]
    fn extract_errors_when_no_closing_delimiter() {
        let src = "---\nstatus: open\n# Title\n";
        let err = extract_frontmatter(src).unwrap_err().to_string();
        assert!(err.contains("no closing"), "got: {err}");
    }

    #[test]
    fn set_updates_existing_key_in_place_preserving_order() {
        let block = vec![
            "status: open".to_string(),
            "opened: 2026-04-24".to_string(),
            "closed:".to_string(),
        ];
        let mut set = HashMap::new();
        set.insert("status".to_string(), json!("fixed"));
        set.insert("closed".to_string(), json!("2026-05-17"));
        let out = apply_ops(&block, &set, &[]).unwrap();
        assert_eq!(
            out,
            vec![
                "status: fixed".to_string(),
                "opened: 2026-04-24".to_string(),
                "closed: 2026-05-17".to_string(),
            ]
        );
    }

    #[test]
    fn set_appends_new_key_at_end() {
        let block = vec!["status: open".to_string()];
        let mut set = HashMap::new();
        set.insert("owner".to_string(), json!("marius"));
        let out = apply_ops(&block, &set, &[]).unwrap();
        assert_eq!(out, vec!["status: open", "owner: marius"]);
    }

    #[test]
    fn delete_removes_line() {
        let block = vec![
            "status: open".to_string(),
            "legacy: yes".to_string(),
            "severity: low".to_string(),
        ];
        let out = apply_ops(&block, &HashMap::new(), &["legacy".to_string()]).unwrap();
        assert_eq!(out, vec!["status: open", "severity: low"]);
    }

    #[test]
    fn delete_of_missing_key_is_silent_idempotent() {
        let block = vec!["status: open".to_string()];
        let out = apply_ops(&block, &HashMap::new(), &["nonexistent".to_string()]).unwrap();
        assert_eq!(out, vec!["status: open"]);
    }

    #[test]
    fn array_values_serialize_inline() {
        let block: Vec<String> = vec![];
        let mut set = HashMap::new();
        set.insert("tags".to_string(), json!(["lsp", "cold-start"]));
        let out = apply_ops(&block, &set, &[]).unwrap();
        assert_eq!(out, vec!["tags: [\"lsp\", \"cold-start\"]"]);
    }

    #[test]
    fn nested_object_value_errors() {
        let block: Vec<String> = vec![];
        let mut set = HashMap::new();
        set.insert("foo".to_string(), json!({"a": 1}));
        let err = apply_ops(&block, &set, &[]).unwrap_err().to_string();
        assert!(err.contains("flat-only"), "got: {err}");
    }

    #[test]
    fn comments_and_blank_lines_preserved_through_set() {
        let block = vec![
            "# project metadata".to_string(),
            "status: open".to_string(),
            "".to_string(),
            "# review fields".to_string(),
            "owner: marius".to_string(),
        ];
        let mut set = HashMap::new();
        set.insert("status".to_string(), json!("fixed"));
        let out = apply_ops(&block, &set, &[]).unwrap();
        assert_eq!(
            out,
            vec![
                "# project metadata",
                "status: fixed",
                "",
                "# review fields",
                "owner: marius",
            ]
        );
    }

    #[test]
    fn reserved_literal_strings_get_quoted() {
        let block: Vec<String> = vec![];
        let mut set = HashMap::new();
        set.insert("a".to_string(), json!("true"));
        set.insert("b".to_string(), json!("null"));
        let out = apply_ops(&block, &set, &[]).unwrap();
        // HashMap iteration order is unstable — check membership not position.
        assert!(out.contains(&"a: \"true\"".to_string()));
        assert!(out.contains(&"b: \"null\"".to_string()));
    }

    #[test]
    fn strings_with_colons_get_quoted() {
        let block: Vec<String> = vec![];
        let mut set = HashMap::new();
        set.insert("note".to_string(), json!("see: BUG-049"));
        let out = apply_ops(&block, &set, &[]).unwrap();
        assert_eq!(out, vec!["note: \"see: BUG-049\""]);
    }

    #[test]
    fn invalid_key_rejected() {
        let block: Vec<String> = vec![];
        let mut set = HashMap::new();
        set.insert("has space".to_string(), json!("v"));
        let err = apply_ops(&block, &set, &[]).unwrap_err().to_string();
        assert!(err.contains("invalid frontmatter key"), "got: {err}");
    }

    #[test]
    fn splice_preserves_body_verbatim() {
        let src = "---\nstatus: open\n---\n# Title\n\nbody line 1\nbody line 2\n";
        let f = fm(src);
        let new_block = vec!["status: fixed".to_string()];
        let out = splice_back(src, &new_block, &f);
        assert_eq!(
            out,
            "---\nstatus: fixed\n---\n# Title\n\nbody line 1\nbody line 2\n"
        );
    }

    #[test]
    fn splice_preserves_missing_trailing_newline() {
        let src = "---\nstatus: open\n---\nbody";
        let f = fm(src);
        let new_block = vec!["status: fixed".to_string()];
        let out = splice_back(src, &new_block, &f);
        assert_eq!(out, "---\nstatus: fixed\n---\nbody");
    }
}
