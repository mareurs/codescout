//! Flat-YAML frontmatter mutation for markdown files.
//!
//! Scope is intentionally narrow: one-key-per-line, scalar / string / inline-array
//! values only. Designed to close the recurring "sed-shuffle for `status:` flips"
//! friction on `docs/issues/*.md` and tracker files. Nested YAML, multi-line values,
//! and anchors/aliases are out of scope — if they're ever needed, replace this module
//! with `serde_yaml` and revisit.

use anyhow::{anyhow, Result};
use serde_json::Value;

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
    set: &serde_json::Map<String, Value>,
    delete: &[String],
) -> Result<Vec<String>> {
    // Preserve order; flag which keys we mutated so we can append the rest.
    let mut out: Vec<String> = Vec::with_capacity(block.len() + set.len());
    let mut applied: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let delete_set: std::collections::HashSet<&str> = delete.iter().map(|s| s.as_str()).collect();

    // Index-based rather than `for line in block`: replacing or deleting a key must
    // also consume that key's continuation lines. Line-at-a-time iteration left them
    // behind, which produced invalid YAML on `set` and — when the preceding key was
    // itself a block sequence — silently reparented them on `delete`.
    let mut i = 0usize;
    while i < block.len() {
        let line = &block[i];
        let mutated = match line_key(line) {
            Some(key) if delete_set.contains(key) => {
                i += 1;
                true
            }
            Some(key) if set.contains_key(key) => {
                let v = &set[key];
                out.push(format!("{key}: {}", serialize_value(v)?));
                applied.insert(key);
                i += 1;
                true
            }
            _ => {
                out.push(line.clone());
                i += 1;
                false
            }
        };
        if mutated {
            // Drop the old value's continuation lines. Correct for every multi-line
            // shape, because both `set` and `delete` replace the value wholesale: a
            // nested map, a block scalar and a block sequence are all discarded the
            // same way.
            while i < block.len() && is_continuation(&block[i]) {
                i += 1;
            }
        }
    }
    // Append any set keys that didn't already exist, in caller order
    // (serde_json::Map preserves insertion order via preserve_order feature).
    for (key, value) in set {
        if applied.contains(key.as_str()) {
            continue;
        }
        if key.is_empty() || key.chars().any(|c| c.is_whitespace() || c == ':') {
            return Err(crate::tools::RecoverableError::with_hint(
                format!(
                    "invalid frontmatter key '{}' — must be non-empty with no whitespace or colons",
                    key
                ),
                "Use a flat key with no whitespace or ':' (e.g. `status`, `owner`).",
            )
            .into());
        }
        out.push(format!("{key}: {}", serialize_value(value)?));
    }

    // Backstop for multi-line shapes the continuation scan cannot fully consume (a
    // sequence with an interleaved comment, say). Compared against the input rather
    // than checked absolutely, so a file that arrived broken can still be repaired.
    let before = orphaned_sequence_items(block);
    let after = orphaned_sequence_items(&out);
    if after > before {
        return Err(crate::tools::RecoverableError::with_hint(
            "frontmatter edit would orphan a block-sequence item, producing invalid YAML — refusing to write"
                .to_string(),
            "This module is flat-only and cannot safely rewrite a multi-line YAML value it \
             did not fully recognise. For a librarian-managed artifact use \
             doc(action=\"update\", patch={...}); otherwise rewrite the frontmatter block \
             in one edit."
                .to_string(),
        )
        .into());
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

/// Does this line continue the previous key's value rather than begin a new key?
///
/// Two shapes, and both must be caught, or mutating the key above leaves orphans
/// behind — see
/// `docs/issues/archive/2026-08-08-edit-markdown-frontmatter-set-orphans-block-sequence.md`:
///
/// - **indented** — nested maps and block scalars (`|`, `>`).
/// - **`-` at column 0** — a block-sequence entry. YAML permits sequence items at the
///   parent key's own indentation, and that is the form the librarian writes.
///
/// Deliberately **not** expressed as `line_key(line).is_none()`. `line_key` answers a
/// different question and gets this one wrong: for a sequence-of-maps entry like
/// `- name: x` it finds a colon and returns `Some("- name")`, so a continuation line
/// would read as a fresh key and stop the scan mid-value.
///
/// Blank and comment lines are **not** continuations. Consuming them would silently
/// delete a comment belonging to the key below. The cost is that a sequence with an
/// interleaved comment is not fully consumed; `orphaned_sequence_items` is the
/// backstop for that.
fn is_continuation(line: &str) -> bool {
    if line.starts_with(' ') || line.starts_with('\t') {
        return true;
    }
    line.starts_with('-')
}

/// Count block-sequence items that have no parent key to attach to.
///
/// A `- item` at column 0 is well-formed only when the nearest preceding key line
/// declares an empty value (`tags:`), which is what opens a block sequence. If the
/// nearest preceding key already carries a value (`related: ["x"]`) — or there is no
/// preceding key at all — the item is orphaned and the block is invalid YAML.
///
/// This exists because there is no YAML parser in the dependency tree (the module is
/// hand-rolled and flat-only by design, and pulling one in for a validation pass is a
/// worse trade). It is a structural approximation, used only to compare before/after:
/// `apply_ops` refuses a write that *increases* the count, so an already-broken file
/// can still be repaired.
fn orphaned_sequence_items(block: &[String]) -> usize {
    let mut orphans = 0usize;
    // None = no key seen yet; Some(true) = last key opened a block (empty value).
    let mut last_key_opened_block: Option<bool> = None;

    for line in block {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            // Indented content belongs to whatever is above it; not our concern.
            continue;
        }
        if line.starts_with('-') {
            if last_key_opened_block != Some(true) {
                orphans += 1;
            }
            continue;
        }
        if let Some(colon) = trimmed.find(':') {
            last_key_opened_block = Some(trimmed[colon + 1..].trim().is_empty());
        }
    }
    orphans
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
        Value::Object(_) => Err(crate::tools::RecoverableError::with_hint(
            "nested objects are not supported — this frontmatter editor is flat-only",
            "Flatten the value: use a scalar or a flat array of strings.",
        )
        .into()),
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
        let mut set = serde_json::Map::new();
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
        let mut set = serde_json::Map::new();
        set.insert("owner".to_string(), json!("marius"));
        let out = apply_ops(&block, &set, &[]).unwrap();
        assert_eq!(out, vec!["status: open", "owner: marius"]);
    }

    #[test]
    fn bootstrap_emits_keys_in_caller_order() {
        // Regression for the HashMap iteration-order non-determinism bug
        // (docs/issues/archive/2026-05-18-frontmatter-bootstrap-key-order-nondeterminism.md).
        // The bootstrap path must emit keys in the order the caller inserted
        // them — guaranteed by serde_json::Map's preserve_order feature.
        let block: Vec<String> = vec![];
        let mut forward = serde_json::Map::new();
        forward.insert("alpha".to_string(), json!("1"));
        forward.insert("bravo".to_string(), json!("2"));
        forward.insert("charlie".to_string(), json!("3"));
        let out_f = apply_ops(&block, &forward, &[]).unwrap();
        assert_eq!(out_f, vec!["alpha: 1", "bravo: 2", "charlie: 3"]);

        let mut reverse = serde_json::Map::new();
        reverse.insert("charlie".to_string(), json!("3"));
        reverse.insert("bravo".to_string(), json!("2"));
        reverse.insert("alpha".to_string(), json!("1"));
        let out_r = apply_ops(&block, &reverse, &[]).unwrap();
        assert_eq!(out_r, vec!["charlie: 3", "bravo: 2", "alpha: 1"]);
    }

    #[test]
    fn delete_removes_line() {
        let block = vec![
            "status: open".to_string(),
            "legacy: yes".to_string(),
            "severity: low".to_string(),
        ];
        let out = apply_ops(&block, &serde_json::Map::new(), &["legacy".to_string()]).unwrap();
        assert_eq!(out, vec!["status: open", "severity: low"]);
    }

    /// The filed case. Measured live 2026-08-14 before the fix: the call returned
    /// `"ok"` and wrote `related: ["c.md"]` followed by the surviving `- a.md` /
    /// `- b.md` lines — invalid YAML, rejected by `yaml.safe_load`.
    #[test]
    fn set_on_a_block_sequence_consumes_its_items() {
        let block = vec![
            "owner: marius".to_string(),
            "related:".to_string(),
            "- a.md".to_string(),
            "- b.md".to_string(),
            "severity: medium".to_string(),
        ];
        let mut set = serde_json::Map::new();
        set.insert("related".to_string(), json!(["c.md"]));
        let out = apply_ops(&block, &set, &[]).unwrap();
        assert_eq!(
            out,
            vec!["owner: marius", "related: [\"c.md\"]", "severity: medium"],
            "the old sequence items must not survive beneath the new inline value"
        );
    }

    /// `delete` has two distinct failure modes and which one you get depends on the
    /// *neighbouring* key, not on the deleted one.
    ///
    /// Here the preceding key is a scalar, so orphaned items are invalid YAML —
    /// loud, if anyone looks.
    #[test]
    fn delete_of_a_block_sequence_after_a_scalar_key_consumes_its_items() {
        let block = vec![
            "owner: marius".to_string(),
            "related:".to_string(),
            "- a.md".to_string(),
            "severity: medium".to_string(),
        ];
        let out = apply_ops(&block, &serde_json::Map::new(), &["related".to_string()]).unwrap();
        assert_eq!(out, vec!["owner: marius", "severity: medium"]);
    }

    /// The dangerous mode, and the reason this bug is `severity: high`.
    ///
    /// When the preceding key is *itself* a block sequence, orphaned items reattach to
    /// it. The result is **valid YAML that means something else**, so nothing errors at
    /// any layer. Measured live 2026-08-14 before the fix: deleting `related` from
    /// `tags: [alpha] / related: [a.md]` yielded `{'tags': ['alpha', 'a.md']}`.
    #[test]
    fn delete_does_not_reparent_items_onto_a_preceding_sequence() {
        let block = vec![
            "tags:".to_string(),
            "- alpha".to_string(),
            "related:".to_string(),
            "- a.md".to_string(),
            "severity: medium".to_string(),
        ];
        let out = apply_ops(&block, &serde_json::Map::new(), &["related".to_string()]).unwrap();
        assert_eq!(
            out,
            vec!["tags:", "- alpha", "severity: medium"],
            "`a.md` must not end up as a member of `tags`"
        );
    }

    /// Block sequences are not the only multi-line shape. Both `set` and `delete`
    /// replace the value wholesale, so consuming indented continuations is correct for
    /// nested maps and block scalars too — neither of which the old loop handled.
    #[test]
    fn set_consumes_indented_continuations() {
        let block = vec![
            "cfg:".to_string(),
            "  a: 1".to_string(),
            "  b: 2".to_string(),
            "note: |".to_string(),
            "  first".to_string(),
            "  second".to_string(),
            "severity: low".to_string(),
        ];
        let mut set = serde_json::Map::new();
        set.insert("cfg".to_string(), json!("replaced"));
        set.insert("note".to_string(), json!("flat"));
        let out = apply_ops(&block, &set, &[]).unwrap();
        assert_eq!(out, vec!["cfg: replaced", "note: flat", "severity: low"]);
    }

    /// A sequence-of-maps entry is a continuation even though `line_key` reads a key
    /// out of it (`- name: x` → `Some("- name")`). If the scan used `line_key` as its
    /// terminator this item would survive as an orphan.
    #[test]
    fn set_consumes_a_sequence_of_maps_whose_items_contain_colons() {
        let block = vec![
            "steps:".to_string(),
            "- name: build".to_string(),
            "- name: test".to_string(),
            "severity: low".to_string(),
        ];
        let mut set = serde_json::Map::new();
        set.insert("steps".to_string(), json!(["only"]));
        let out = apply_ops(&block, &set, &[]).unwrap();
        assert_eq!(out, vec!["steps: [\"only\"]", "severity: low"]);
    }

    /// The backstop. A comment interleaved in a sequence stops the continuation scan —
    /// deliberately, since consuming comments would delete one belonging to the key
    /// below — so the write would orphan `- b.md`. Refuse rather than corrupt.
    #[test]
    fn orphan_backstop_refuses_a_write_it_cannot_make_safe() {
        let block = vec![
            "related:".to_string(),
            "- a.md".to_string(),
            "# keep this".to_string(),
            "- b.md".to_string(),
        ];
        let mut set = serde_json::Map::new();
        set.insert("related".to_string(), json!(["c.md"]));
        let err = apply_ops(&block, &set, &[]).unwrap_err().to_string();
        assert!(
            err.contains("orphan"),
            "expected the orphan backstop to fire; got: {err}"
        );
    }

    /// The backstop compares against the input rather than checking absolutely, so a
    /// block that arrived already broken can still be repaired.
    #[test]
    fn orphan_backstop_allows_repairing_an_already_broken_block() {
        let block = vec![
            "owner: marius".to_string(),
            "related: [old]".to_string(),
            "- stray.md".to_string(),
        ];
        let mut set = serde_json::Map::new();
        set.insert("owner".to_string(), json!("someone"));
        let out = apply_ops(&block, &set, &[])
            .expect("a pre-existing orphan must not block an unrelated edit");
        assert!(out.contains(&"owner: someone".to_string()));
    }

    #[test]
    fn delete_of_missing_key_is_silent_idempotent() {
        let block = vec!["status: open".to_string()];
        let out = apply_ops(
            &block,
            &serde_json::Map::new(),
            &["nonexistent".to_string()],
        )
        .unwrap();
        assert_eq!(out, vec!["status: open"]);
    }

    #[test]
    fn array_values_serialize_inline() {
        let block: Vec<String> = vec![];
        let mut set = serde_json::Map::new();
        set.insert("tags".to_string(), json!(["lsp", "cold-start"]));
        let out = apply_ops(&block, &set, &[]).unwrap();
        assert_eq!(out, vec!["tags: [\"lsp\", \"cold-start\"]"]);
    }

    #[test]
    fn nested_object_value_errors() {
        let block: Vec<String> = vec![];
        let mut set = serde_json::Map::new();
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
        let mut set = serde_json::Map::new();
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
        let mut set = serde_json::Map::new();
        set.insert("a".to_string(), json!("true"));
        set.insert("b".to_string(), json!("null"));
        let out = apply_ops(&block, &set, &[]).unwrap();
        assert!(out.contains(&"a: \"true\"".to_string()));
        assert!(out.contains(&"b: \"null\"".to_string()));
    }

    #[test]
    fn strings_with_colons_get_quoted() {
        let block: Vec<String> = vec![];
        let mut set = serde_json::Map::new();
        set.insert("note".to_string(), json!("see: BUG-049"));
        let out = apply_ops(&block, &set, &[]).unwrap();
        assert_eq!(out, vec!["note: \"see: BUG-049\""]);
    }

    #[test]
    fn invalid_key_rejected() {
        let block: Vec<String> = vec![];
        let mut set = serde_json::Map::new();
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
