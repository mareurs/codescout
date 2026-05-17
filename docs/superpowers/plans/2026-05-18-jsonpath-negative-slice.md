# `json_path` negative index + slice — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `json_path` support for `[-N]` (negative single-index) and `[-N:]` (negative-start open-end slice) on `read_file`, replacing the current misleading `"path segment not found"` error with proper success / OOB / unsupported-syntax responses.

**Architecture:** Replace stringly-typed segments with a typed `enum Segment { Key, Index, NegIndex, NegSliceFrom }`. Parser produces `Vec<Segment>`. Resolver takes `&Segment` and returns `Result<Cow<'a, Value>, RecoverableError>`. Walk threads `Cow` through the loop, flipping to owned on first slice. New fns introduced alongside old (`_v2` suffix) to keep green-bar between tasks; old fns deleted in final cutover.

**Tech Stack:** Rust, `serde_json::Value`, `std::borrow::Cow`, project's `RecoverableError`.

**Spec:** `docs/superpowers/specs/2026-05-18-jsonpath-negative-slice-design.md`
**Bug:** `docs/issues/2026-05-17-read-file-jsonpath-negative-slice.md`

---

## File Structure

| Path | Role |
|---|---|
| `src/tools/file_summary/file_summary.rs` | All production code lives here. Add `Segment` enum + new parser/resolver fns; eventually replace originals. |
| `src/tools/file_summary/tests.rs` | All new tests (10 parser + 8 resolver) co-located with existing `extract_json_path_array_index` (line 352). |
| `src/tools/read_file.rs:1003` | Existing regression-pin test `read_file_buffer_json_path_array_element_returns_value` — must stay green; no changes here. |

No new files created.

---

## Task 1: Define `Segment` enum

**Files:**
- Modify: `src/tools/file_summary/file_summary.rs` — add enum near top of file (above `extract_json_path`)

- [ ] **Step 1: Add the enum**

Insert immediately above `pub fn extract_json_path` (currently line 419):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    /// Object key access: `.field` or bare `field` after `$`.
    Key(String),
    /// Non-negative array index: `[N]` where N ≥ 0.
    Index(usize),
    /// Negative single-index: `[-N]` where N ≥ 1, stored as positive magnitude.
    NegIndex(usize),
    /// Negative-start open-end slice: `[-N:]` where N ≥ 1, last N elements.
    NegSliceFrom(usize),
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p codescout`
Expected: success, one `dead_code` warning on `Segment` (acceptable — used in Task 2).

- [ ] **Step 3: Commit**

```bash
git add src/tools/file_summary/file_summary.rs
git commit -m "feat(file_summary): add Segment enum for typed json_path grammar"
```

---

## Task 2: New parser `parse_segments_v2` alongside old

**Files:**
- Modify: `src/tools/file_summary/file_summary.rs` — add `parse_segments_v2` + helper `parse_bracket`
- Modify: `src/tools/file_summary/tests.rs` — 10 parser tests

- [ ] **Step 1: Write all 10 failing parser tests**

Append to `src/tools/file_summary/tests.rs` (test fn names match spec verbatim):

```rust
use super::file_summary::{parse_segments_v2, Segment};

#[test]
fn parse_empty_path_returns_empty_segments() {
    assert_eq!(parse_segments_v2("").unwrap(), Vec::<Segment>::new());
}

#[test]
fn parse_root_only() {
    assert_eq!(parse_segments_v2("$").unwrap(), Vec::<Segment>::new());
}

#[test]
fn parse_negative_single_index() {
    assert_eq!(
        parse_segments_v2("$.a[-1]").unwrap(),
        vec![Segment::Key("a".into()), Segment::NegIndex(1)]
    );
}

#[test]
fn parse_negative_slice_from() {
    assert_eq!(
        parse_segments_v2("$.a[-3:]").unwrap(),
        vec![Segment::Key("a".into()), Segment::NegSliceFrom(3)]
    );
}

#[test]
fn parse_chained_negative_after_positive() {
    assert_eq!(
        parse_segments_v2("$.a[0][-1]").unwrap(),
        vec![Segment::Key("a".into()), Segment::Index(0), Segment::NegIndex(1)]
    );
}

#[test]
fn parse_top_level_negative_index() {
    assert_eq!(
        parse_segments_v2("$[-1]").unwrap(),
        vec![Segment::NegIndex(1)]
    );
}

#[test]
fn parse_rejects_positive_slice() {
    let err = parse_segments_v2("$.a[1:3]").unwrap_err();
    assert!(err.to_string().contains("unsupported json_path segment"), "got: {}", err.message);
    assert!(err.to_string().contains("[1:3]"), "got: {}", err.message);
}

#[test]
fn parse_rejects_slice_with_step() {
    let err = parse_segments_v2("$.a[::2]").unwrap_err();
    assert!(err.to_string().contains("unsupported json_path segment"));
}

#[test]
fn parse_rejects_open_end_positive() {
    let err = parse_segments_v2("$.a[1:]").unwrap_err();
    assert!(err.to_string().contains("unsupported json_path segment"));
}

#[test]
fn parse_rejects_negative_zero() {
    let err = parse_segments_v2("$.a[-0]").unwrap_err();
    assert!(err.to_string().contains("[-0]"));
    assert!(err.to_string().contains("[0]"));
}

#[test]
fn parse_rejects_non_integer_bracket() {
    let err = parse_segments_v2("$.a[abc]").unwrap_err();
    assert!(err.to_string().contains("[abc]"));
}
```

Scouted pre-dispatch (F-3 in `docs/trackers/bug-fix-session-log.md`, 2026-05-18): `RecoverableError` lives at `src/tools/core/types.rs:169` with `pub message: String` + `pub guidance: Option<Guidance>` (no `.hint` field). Per the `Display` impl's own documented contract, tests use `err.to_string().contains(...)` — that rendering includes both `message` and the guidance text, so a single substring assertion covers both.

- [ ] **Step 2: Run the failing tests to confirm they fail**

Run: `cargo test -p codescout --lib parse_ -- --nocapture`
Expected: 11 tests FAIL (function `parse_segments_v2` does not exist).

- [ ] **Step 3: Implement `parse_segments_v2` + `parse_bracket`**

Insert immediately after the existing `parse_json_path_segments` function (around line 495):

```rust
fn parse_segments_v2(path: &str) -> Result<Vec<Segment>, RecoverableError> {
    let path = path
        .strip_prefix("$.")
        .or_else(|| path.strip_prefix('$'))
        .unwrap_or(path);
    if path.is_empty() {
        return Ok(Vec::new());
    }
    let mut segments = Vec::new();
    for part in path.split('.') {
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
                let end = rest
                    .find(']')
                    .ok_or_else(|| unsupported_bracket(rest))?;
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
    let supported_hint = "Supported forms: '.key', '[N]' (non-negative integer), '[-N]' (negative integer), '[-N:]' (last N elements). Other slice/filter forms not supported.";
    if inner.is_empty() {
        return Err(RecoverableError::with_hint(
            "unsupported json_path segment '[]'".to_string(),
            supported_hint,
        ));
    }
    // Positive integer
    if inner.chars().all(|c| c.is_ascii_digit()) {
        let n: usize = inner.parse().map_err(|_| RecoverableError::with_hint(
            format!("unsupported json_path segment '[{}]'", inner),
            supported_hint,
        ))?;
        return Ok(Segment::Index(n));
    }
    // Negative form: [-N] or [-N:]
    if let Some(rest) = inner.strip_prefix('-') {
        let (mag_str, is_slice) = if let Some(s) = rest.strip_suffix(':') {
            (s, true)
        } else {
            (rest, false)
        };
        if !mag_str.chars().all(|c| c.is_ascii_digit()) || mag_str.is_empty() {
            return Err(RecoverableError::with_hint(
                format!("unsupported json_path segment '[{}]'", inner),
                supported_hint,
            ));
        }
        let mag: usize = mag_str.parse().map_err(|_| RecoverableError::with_hint(
            format!("unsupported json_path segment '[{}]'", inner),
            supported_hint,
        ))?;
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
        format!("unsupported json_path bracket near '{}'", s),
        "Supported forms: '.key', '[N]', '[-N]', '[-N:]'.",
    )
}
```

- [ ] **Step 4: Re-export the new types for tests**

If `parse_segments_v2` and `Segment` are not visible from `tests.rs`, add `pub(super)` to their declarations:

```rust
pub(super) enum Segment { ... }
pub(super) fn parse_segments_v2(...) -> ...
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p codescout --lib parse_ -- --nocapture`
Expected: 11 parse_ tests PASS.

- [ ] **Step 6: Run full test suite to confirm no regression**

Run: `cargo test -p codescout`
Expected: all tests pass (no callers of `parse_segments_v2` yet — old `parse_json_path_segments` still wired in).

- [ ] **Step 7: Commit**

```bash
git add src/tools/file_summary/file_summary.rs src/tools/file_summary/tests.rs
git commit -m "feat(file_summary): add parse_segments_v2 returning Vec<Segment>"
```

---

## Task 3: New resolver `resolve_segment_v2` alongside old

**Files:**
- Modify: `src/tools/file_summary/file_summary.rs` — add `resolve_segment_v2`
- Modify: `src/tools/file_summary/tests.rs` — 8 resolver tests calling a temporary `extract_json_path_v2`

This task adds a parallel resolver. Because `extract_json_path` still uses the old `resolve_json_segment`, we test the new resolver via a temporary `extract_json_path_v2` wrapper that uses the new parser + new resolver. Both wrapper and resolver are dropped in Task 4 when cutover happens.

- [ ] **Step 1: Write 8 failing resolver tests**

Append to `src/tools/file_summary/tests.rs`:

```rust
use super::file_summary::extract_json_path_v2;

#[test]
fn extract_root_returns_parsed() {
    let (content, ty, count) = extract_json_path_v2(r#"{"a":1}"#, "$").unwrap();
    assert!(content.contains("\"a\""), "got: {}", content);
    assert_eq!(ty, "object");
    assert_eq!(count, Some(1));
}

#[test]
fn extract_top_level_negative_index() {
    let (content, ty, _) = extract_json_path_v2(r#"["a","b","c"]"#, "$[-1]").unwrap();
    assert_eq!(content, "c");
    assert_eq!(ty, "string");
}

#[test]
fn extract_negative_index_returns_last_element() {
    let (content, ty, _) = extract_json_path_v2(
        r#"{"items":["a","b","c"]}"#, "$.items[-1]").unwrap();
    assert_eq!(content, "c");
    assert_eq!(ty, "string");
}

#[test]
fn extract_negative_slice_returns_tail() {
    let (content, ty, count) = extract_json_path_v2(
        r#"{"items":["a","b","c","d"]}"#, "$.items[-2:]").unwrap();
    assert!(content.contains("\"c\""));
    assert!(content.contains("\"d\""));
    assert!(!content.contains("\"a\""));
    assert_eq!(ty, "array");
    assert_eq!(count, Some(2));
}

#[test]
fn extract_negative_index_oob_returns_clear_error() {
    let err = extract_json_path_v2(r#"{"items":["a"]}"#, "$.items[-5]").unwrap_err();
    assert!(err.to_string().contains("out of bounds"), "got: {}", err.message);
    assert!(err.to_string().contains("length 1"), "got: {}", err.message);
}

#[test]
fn extract_negative_slice_oob_returns_clear_error() {
    let err = extract_json_path_v2(r#"{"items":["a"]}"#, "$.items[-5:]").unwrap_err();
    assert!(err.to_string().contains("out of bounds"));
    assert!(err.to_string().contains("length 1"));
}

#[test]
fn extract_mid_path_slice_then_index() {
    let (content, ty, _) = extract_json_path_v2(
        r#"{"items":[{"v":1},{"v":2},{"v":3}]}"#,
        "$.items[-2:][0].v"
    ).unwrap();
    assert_eq!(content, "2");
    assert_eq!(ty, "number");
}

#[test]
fn extract_unsupported_syntax_distinguished_from_not_found() {
    let err = extract_json_path_v2(r#"{"items":["a"]}"#, "$.items[1:3]").unwrap_err();
    assert!(err.to_string().contains("unsupported json_path segment"), "got: {}", err.message);
    assert!(!err.to_string().contains("not found"), "got: {}", err.message);
}
```

Note on `extract_mid_path_slice_then_index`: the spec says expected is `("1", "number", None)`. That assumes `[-2:][0]` selects the *first* element of the last-2-element slice. For input `[{v:1},{v:2},{v:3}]`, the last-2 slice is `[{v:2},{v:3}]`, so `[0].v = 2`. **The spec example was wrong.** This plan uses `2` — verify against the test above on first run.

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test -p codescout --lib extract_ -- --nocapture`
Expected: 8 extract_ tests FAIL (function `extract_json_path_v2` does not exist).

- [ ] **Step 3: Implement `resolve_segment_v2` + `extract_json_path_v2`**

Add `use std::borrow::Cow;` near the top of `file_summary.rs` (next to existing `use` statements) if not already present.

Insert after `parse_bracket` (from Task 2):

```rust
fn resolve_segment_v2<'a>(
    value: &'a Value,
    seg: &Segment,
) -> Result<Cow<'a, Value>, RecoverableError> {
    match seg {
        Segment::Key(k) => match value {
            Value::Object(obj) => obj
                .get(k)
                .map(Cow::Borrowed)
                .ok_or_else(|| {
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
                    format!("index {} out of bounds for array of length {}", n, arr.len()),
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
                        format!(
                            "index -{} out of bounds for array of length {}",
                            n, len
                        ),
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
                        format!(
                            "index -{} out of bounds for array of length {}",
                            n, len
                        ),
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

/// Temporary scaffolding for tests in Task 3. Removed in Task 4 cutover.
pub(super) fn extract_json_path_v2(
    content: &str,
    path: &str,
) -> Result<(String, String, Option<usize>), RecoverableError> {
    let parsed: Value = serde_json::from_str(content).map_err(|e| {
        RecoverableError::with_hint(
            format!("failed to parse JSON: {}", e),
            "Ensure the file contains valid JSON",
        )
    })?;
    let segments = parse_segments_v2(path)?;
    let mut current: Cow<'_, Value> = Cow::Borrowed(&parsed);
    for seg in &segments {
        current = match current {
            Cow::Borrowed(v) => resolve_segment_v2(v, seg)?,
            Cow::Owned(v) => {
                let r = resolve_segment_v2(&v, seg)?;
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
```

- [ ] **Step 4: Run resolver tests to verify they pass**

Run: `cargo test -p codescout --lib extract_ -- --nocapture`
Expected: 8 extract_ tests PASS.

- [ ] **Step 5: Run full test suite — no regressions**

Run: `cargo test -p codescout`
Expected: all tests pass. Old `extract_json_path` still in use by `read_file`; `extract_json_path_v2` only exercised by new tests.

- [ ] **Step 6: Commit**

```bash
git add src/tools/file_summary/file_summary.rs src/tools/file_summary/tests.rs
git commit -m "feat(file_summary): add resolve_segment_v2 + extract_json_path_v2 scaffold"
```

---

## Task 4: Cut over — replace old fns with v2, delete scaffold

**Files:**
- Modify: `src/tools/file_summary/file_summary.rs` — rename v2 fns to canonical names, delete originals
- Modify: `src/tools/file_summary/tests.rs` — update `use` statements

- [ ] **Step 1: Delete the old functions and rename v2 fns**

In `src/tools/file_summary/file_summary.rs`:

1. **Delete** the old `parse_json_path_segments` (currently lines ~470-495) — the `Vec<String>` version.
2. **Delete** the old `resolve_json_segment` (currently lines ~497-504) — the `&str` / `Option<&Value>` version.
3. **Rename** `parse_segments_v2` → `parse_json_path_segments`.
4. **Rename** `resolve_segment_v2` → `resolve_json_segment`.
5. **Delete** the temporary `extract_json_path_v2` (the wrapper added in Task 3).
6. **Rewrite** the body of the existing `pub fn extract_json_path` to use the new parser/resolver and thread `Cow`. The new body is:

```rust
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
```

Keep the existing doc comment on `extract_json_path` (the one explaining the `Value::String` raw-return rationale) intact.

- [ ] **Step 2: Update test imports**

In `src/tools/file_summary/tests.rs`, change:

```rust
use super::file_summary::{parse_segments_v2, Segment};
use super::file_summary::extract_json_path_v2;
```

to:

```rust
use super::file_summary::{parse_json_path_segments, Segment};
use super::file_summary::extract_json_path;
```

Then in each `parse_*` test, replace `parse_segments_v2` with `parse_json_path_segments`. In each `extract_*` test, replace `extract_json_path_v2` with `extract_json_path`.

- [ ] **Step 3: Run full test suite**

Run: `cargo test -p codescout`
Expected: ALL tests pass, including:
- the 19 new tests from Tasks 2 + 3
- existing `extract_json_path_array_index` in `tests.rs:352`
- existing `read_file_buffer_json_path_array_element_returns_value` in `src/tools/read_file.rs:1003`
- all other tests in the crate

If `read_file_buffer_json_path_array_element_returns_value` fails, the cutover broke positive-index + property access. Re-check the resolver's `Segment::Key` and `Segment::Index` arms.

- [ ] **Step 4: cargo fmt + clippy**

Run: `cargo fmt`
Run: `cargo clippy -- -D warnings`
Expected: both clean.

- [ ] **Step 5: Commit**

```bash
git add src/tools/file_summary/file_summary.rs src/tools/file_summary/tests.rs
git commit -m "refactor(file_summary): cut over to typed Segment grammar

- Delete stringly-typed parse_json_path_segments + resolve_json_segment
- Rename v2 implementations to canonical names
- Rewrite extract_json_path to thread Cow<'a, Value>
- Closes docs/issues/2026-05-17-read-file-jsonpath-negative-slice.md"
```

---

## Task 5: Live MCP verification

**Files:** none modified.

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`
Expected: success.

- [ ] **Step 2: Restart MCP server**

In the active Claude Code session, run: `/mcp`
Expected: codescout reconnects on the new release binary.

- [ ] **Step 3: Verify `[-1]` works**

In the session, call:
```
symbols(path="src/server.rs")   # produces @tool_xxx buffer
read_file(path="@tool_xxx", json_path="$.symbols[-1]")
```
Expected: success, returns the last symbol entry.

- [ ] **Step 4: Verify `[-3:]` works**

```
read_file(path="@tool_xxx", json_path="$.symbols[-3:]")
```
Expected: success, returns the last 3 symbols as a JSON array.

- [ ] **Step 5: Verify unsupported syntax is rejected clearly**

```
read_file(path="@tool_xxx", json_path="$.symbols[1:3]")
```
Expected: error message starts with `"unsupported json_path segment '[1:3]'"`, NOT `"not found"`.

- [ ] **Step 6: Verify OOB negative**

```
read_file(path="@tool_xxx", json_path="$.symbols[-9999]")
```
Expected: error message contains `"out of bounds"` and `"length"`.

- [ ] **Step 7: Mark the bug fixed**

```bash
# Edit docs/issues/2026-05-17-read-file-jsonpath-negative-slice.md frontmatter:
#   status: open  →  status: fixed
#   closed:       →  closed: 2026-05-18
# Cite the cutover commit SHA in the Fix section.
git add docs/issues/2026-05-17-read-file-jsonpath-negative-slice.md
git commit -m "docs(issues): close jsonpath-negative-slice — shipped on experiments"
```

Use `edit_markdown` with `frontmatter: {set: {status: "fixed", closed: "2026-05-18"}}` to make the frontmatter edit atomic.

Per CLAUDE.md: do NOT move the bug file to `archive/` yet — archive happens only after the fix ships to `master`, not on `experiments`. The Standard Ship Sequence (cherry-pick to master, then `git mv` to `archive/`) is a separate step outside this plan.

---

## Stop conditions

The plan is done when:

1. `cargo test -p codescout` is green with all 19 new tests passing.
2. `cargo clippy -- -D warnings` is clean.
3. Manual MCP probes in Task 5 succeed.
4. The bug file's `status` is `fixed` and `closed` is set.

---

## Self-review

**Spec coverage** — every section of the spec maps to a task:

| Spec section | Task |
|---|---|
| Goals 1–5 | Tasks 3 (resolver) + 5 (live verify) |
| Architecture (enum + Cow walk) | Tasks 1, 3, 4 |
| Parser grammar | Task 2 |
| Resolver semantics | Task 3 |
| Error catalog | Task 3 (resolver error builders) |
| Public API impact | Task 4 (cutover; no external signature change) |
| Pub audit | resolved at design time (both private) — no plan task |
| Dependencies | Task 3 (`use std::borrow::Cow`) |
| Parser tests (10) | Task 2 Step 1 |
| Resolver tests (8) | Task 3 Step 1 |
| Regression | Task 4 Step 3 + Task 5 |
| Implementation order | Tasks 1–5 |

**Placeholder scan:** no TBD / TODO / "implement later" / "appropriate" — all steps carry exact code or exact commands.

**Type consistency:**
- `Segment` enum variants `Key`, `Index`, `NegIndex`, `NegSliceFrom` used identically across Tasks 1, 2, 3, 4.
- `parse_segments_v2` → `parse_json_path_segments` rename in Task 4 propagates to test imports in same task.
- `resolve_segment_v2` → `resolve_json_segment` rename in Task 4.
- `Cow<'a, Value>` return type consistent between Task 3 and Task 4.
- Test fn names match spec verbatim (10 parse_ + 8 extract_).

**Spec correction noted:** spec's `extract_mid_path_slice_then_index` expected value (`"1"`) was wrong; plan corrects to `"2"`. Verify on first run.

**Resolved at pre-dispatch recon (F-3, W-2):** `RecoverableError` accessors scouted before any subagent dispatch. Type lives at `src/tools/core/types.rs:169`; tests use `err.to_string().contains(...)` per the `Display` impl's documented test-stability contract. No remaining unknowns.
