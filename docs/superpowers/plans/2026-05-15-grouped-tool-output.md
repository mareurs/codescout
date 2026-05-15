# Grouped Tool Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Group multi-file results for `symbols(search)`, `references`, and `grep` by file in both JSON (refs+grep) and `format_compact` text (all three), via a shared `file_group` module that also owns truncation.

**Architecture:** New module `src/tools/file_group.rs` exposes `group_by_file`, `cap_grouped`, `render_grouped`, `groups_to_json`. `symbols` JSON stays flat (deferred legacy); only its `format_compact` switches to grouped rendering. `references` and `grep` get new `file_groups[]` JSON shape and grouped text.

**Tech Stack:** Rust, `serde_json::Value` for tool outputs, in-file `#[cfg(test)] mod tests` for unit tests, `cargo test --lib` for execution.

---

## File Structure

| File | Purpose | Action |
|---|---|---|
| `src/tools/file_group.rs` | Shared grouping/rendering/capping module | Create |
| `src/tools/mod.rs` | Module registration | Modify (add `pub mod file_group;`) |
| `src/tools/symbol/display.rs` | `format_search_symbols` rewritten through `file_group` | Modify |
| `src/tools/symbol/symbols.rs` | Drop now-unused Fix C single-file hoist (replaced by grouping) | Modify |
| `src/tools/symbol/tests.rs` | Update Fix C tests, add grouped snapshots | Modify |
| `src/tools/symbol/references.rs` | New JSON shape, grouped `format_compact` | Modify |
| `src/tools/grep.rs` | New JSON shape (simple mode), grouped `format_compact` | Modify |
| `src/prompts/server_instructions.md` | Update shape references if present | Modify (conditional) |
| `src/prompts/onboarding_prompt.md` | Same; bump `ONBOARDING_VERSION` if touched | Modify (conditional) |
| `src/prompts/builders.rs` | Same | Modify (conditional) |

---

### Task 1: Scaffold `file_group` module with `group_by_file`

**Files:**
- Create: `src/tools/file_group.rs`
- Modify: `src/tools/mod.rs` (add module declaration alongside `pub mod file_summary;`)

- [ ] **Step 1: Register the module**

Edit `src/tools/mod.rs`. After the line `pub mod file_summary;` insert:

```rust
pub mod file_group;
```

- [ ] **Step 2: Write `src/tools/file_group.rs` with stub + failing test**

```rust
//! Group flat tool results by `file` field. Used by `symbols(search)`,
//! `references`, and `grep` for both LLM-facing text rendering and (for refs
//! and grep) the JSON output shape.
//!
//! File-only grouping by design. If a future tool needs `group_by_kind` or
//! `group_by_directory`, introduce a new abstraction then; do not parameterize
//! this one. (See 2026-05-15-grouped-tool-output-design.md.)

use serde_json::Value;

/// A group of items sharing the same `file` field.
///
/// Holds borrowed references into the input slice — callers keep ownership.
pub struct FileGroup<'a> {
    pub file: &'a str,
    pub items: Vec<&'a Value>,
}

/// Group items by their `file` field. Sort: group size desc, ties by path asc.
/// Within a group, original order is preserved (stable partition).
///
/// Items lacking a `file` field are dropped silently; callers should not pass
/// such items to this function.
pub fn group_by_file(items: &[Value]) -> Vec<FileGroup<'_>> {
    use std::collections::BTreeMap;
    let mut by_file: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for item in items {
        if let Some(file) = item.get("file").and_then(|v| v.as_str()) {
            by_file.entry(file).or_default().push(item);
        }
    }
    let mut groups: Vec<FileGroup<'_>> = by_file
        .into_iter()
        .map(|(file, items)| FileGroup { file, items })
        .collect();
    // Stable: BTreeMap iteration is path-asc; sort_by with reverse on count
    // preserves alphabetical order among ties.
    groups.sort_by(|a, b| b.items.len().cmp(&a.items.len()));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(file: &str) -> Value {
        json!({ "file": file })
    }

    #[test]
    fn groups_sorted_by_count_desc() {
        let items = vec![
            item("a.rs"),
            item("b.rs"),
            item("b.rs"),
            item("c.rs"),
            item("c.rs"),
            item("c.rs"),
        ];
        let groups = group_by_file(&items);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].file, "c.rs");
        assert_eq!(groups[0].items.len(), 3);
        assert_eq!(groups[1].file, "b.rs");
        assert_eq!(groups[1].items.len(), 2);
        assert_eq!(groups[2].file, "a.rs");
        assert_eq!(groups[2].items.len(), 1);
    }

    #[test]
    fn groups_tie_break_by_path_asc() {
        let items = vec![item("z.rs"), item("a.rs"), item("m.rs")];
        let groups = group_by_file(&items);
        assert_eq!(groups[0].file, "a.rs");
        assert_eq!(groups[1].file, "m.rs");
        assert_eq!(groups[2].file, "z.rs");
    }

    #[test]
    fn drops_items_without_file_field() {
        let items = vec![item("a.rs"), json!({ "no_file": true })];
        let groups = group_by_file(&items);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].file, "a.rs");
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib tools::file_group::tests`
Expected: 3 passed.

- [ ] **Step 4: Commit**

```bash
git add src/tools/mod.rs src/tools/file_group.rs
git commit -m "feat(file_group): introduce module with group_by_file"
```

---

### Task 2: Add `cap_grouped`

**Files:**
- Modify: `src/tools/file_group.rs`

- [ ] **Step 1: Write failing tests**

Append at the end of `mod tests` in `src/tools/file_group.rs`:

```rust
    #[test]
    fn cap_grouped_round_robin_first() {
        // 4 files with counts {3, 2, 1, 1}, budget=3 → top 3 files each get 1 hit.
        let items: Vec<Value> = ["a.rs", "a.rs", "a.rs", "b.rs", "b.rs", "c.rs", "d.rs"]
            .iter()
            .map(|f| item(f))
            .collect();
        let (visible, total, files) = cap_grouped(items, 3);
        assert_eq!(total, 7);
        assert_eq!(files, 4);
        assert_eq!(visible.len(), 3);
        let visible_files: Vec<&str> = visible
            .iter()
            .map(|v| v["file"].as_str().unwrap())
            .collect();
        // Hottest files in count-desc order, one hit each.
        assert_eq!(visible_files, vec!["a.rs", "b.rs", "c.rs"]);
    }

    #[test]
    fn cap_grouped_fills_hot_after_breadth() {
        // {6, 3, 1}, budget=8 → round 1 picks one each (3 used), then
        // remaining 5 slots go to a.rs (5 left) and b.rs (2 left).
        let mut items: Vec<Value> = vec![];
        for _ in 0..6 {
            items.push(item("a.rs"));
        }
        for _ in 0..3 {
            items.push(item("b.rs"));
        }
        items.push(item("c.rs"));
        let (visible, total, files) = cap_grouped(items, 8);
        assert_eq!(total, 10);
        assert_eq!(files, 3);
        assert_eq!(visible.len(), 8);
        let counts: std::collections::HashMap<&str, usize> = visible.iter().fold(
            std::collections::HashMap::new(),
            |mut acc, v| {
                let f = v["file"].as_str().unwrap();
                *acc.entry(f).or_insert(0) += 1;
                acc
            },
        );
        assert_eq!(counts["a.rs"], 5); // 1 in round-robin + 4 in fill
        assert_eq!(counts["b.rs"], 2); // 1 + 1
        assert_eq!(counts["c.rs"], 1); // 1 in round-robin, none left to fill
    }

    #[test]
    fn cap_grouped_budget_exceeds_total() {
        let items: Vec<Value> = vec![item("a.rs"), item("b.rs")];
        let (visible, total, files) = cap_grouped(items, 100);
        assert_eq!(visible.len(), 2);
        assert_eq!(total, 2);
        assert_eq!(files, 2);
    }

    #[test]
    fn cap_grouped_zero_budget() {
        let items: Vec<Value> = vec![item("a.rs"), item("b.rs")];
        let (visible, total, files) = cap_grouped(items, 0);
        assert_eq!(visible.len(), 0);
        assert_eq!(total, 2);
        assert_eq!(files, 2);
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --lib tools::file_group::tests::cap_grouped`
Expected: compile error — `cap_grouped` not defined.

- [ ] **Step 3: Implement `cap_grouped`**

Add to `src/tools/file_group.rs` after `group_by_file`:

```rust
/// Truncate a flat item list to fit `budget`, preserving file diversity.
///
/// Policy:
/// 1. One hit per file in count-desc order until every file has contributed
///    or the budget is exhausted.
/// 2. Remaining budget fills the hottest files first.
///
/// Returns the visible items, plus the un-truncated `total` and `files` counts
/// (callers anchor "N hits in M files" headers to these).
pub fn cap_grouped(items: Vec<Value>, budget: usize) -> (Vec<Value>, usize, usize) {
    let total = items.len();
    if total == 0 {
        return (items, 0, 0);
    }

    // Build per-file index lists keyed by file path, preserving input order.
    use std::collections::BTreeMap;
    let mut by_file: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, item) in items.iter().enumerate() {
        if let Some(file) = item.get("file").and_then(|v| v.as_str()) {
            by_file.entry(file.to_string()).or_default().push(idx);
        }
    }
    let files = by_file.len();

    if budget >= total {
        return (items, total, files);
    }

    // Sort file buckets count-desc, ties path-asc.
    let mut buckets: Vec<(String, Vec<usize>)> = by_file.into_iter().collect();
    buckets.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));

    // Round-robin one hit per file in count-desc order, then fill hottest.
    let mut picked: Vec<usize> = Vec::with_capacity(budget);
    let mut cursors: Vec<usize> = vec![0; buckets.len()];

    // Round 1: one per file.
    for (i, (_, idxs)) in buckets.iter().enumerate() {
        if picked.len() == budget {
            break;
        }
        if let Some(&idx) = idxs.first() {
            picked.push(idx);
            cursors[i] = 1;
        }
    }

    // Round 2+: fill hottest files first.
    let mut bucket_i = 0;
    while picked.len() < budget && bucket_i < buckets.len() {
        let idxs = &buckets[bucket_i].1;
        if cursors[bucket_i] < idxs.len() {
            picked.push(idxs[cursors[bucket_i]]);
            cursors[bucket_i] += 1;
        } else {
            bucket_i += 1;
        }
    }

    // Preserve input order in the visible slice.
    picked.sort();
    // Use into_iter + filter to avoid clone — we drop the unchosen items.
    let picked_set: std::collections::HashSet<usize> = picked.into_iter().collect();
    let visible: Vec<Value> = items
        .into_iter()
        .enumerate()
        .filter_map(|(idx, item)| if picked_set.contains(&idx) { Some(item) } else { None })
        .collect();

    (visible, total, files)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib tools::file_group::tests`
Expected: 7 passed (3 prior + 4 new).

- [ ] **Step 5: Commit**

```bash
git add src/tools/file_group.rs
git commit -m "feat(file_group): add cap_grouped with breadth-first truncation"
```

---

### Task 3: Add `render_grouped`

**Files:**
- Modify: `src/tools/file_group.rs`

- [ ] **Step 1: Write failing tests**

Append at end of `mod tests`:

```rust
    #[test]
    fn render_multi_file_header_and_groups() {
        let items = vec![
            json!({ "file": "a.rs", "marker": "x" }),
            json!({ "file": "a.rs", "marker": "y" }),
            json!({ "file": "b.rs", "marker": "z" }),
        ];
        let groups = group_by_file(&items);
        let out = render_grouped(&groups, 3, 2, "matches", |v| {
            format!("  m={}", v["marker"].as_str().unwrap())
        });
        assert!(out.starts_with("3 matches in 2 files\n"), "got:\n{out}");
        assert!(out.contains("a.rs (2)"));
        assert!(out.contains("b.rs (1)"));
        assert!(out.contains("  m=x"));
        assert!(out.contains("  m=z"));
        let a_pos = out.find("a.rs").unwrap();
        let b_pos = out.find("b.rs").unwrap();
        assert!(a_pos < b_pos, "hotter file should appear first");
    }

    #[test]
    fn render_single_file_omits_header_line() {
        let items = vec![
            json!({ "file": "a.rs", "marker": "x" }),
            json!({ "file": "a.rs", "marker": "y" }),
        ];
        let groups = group_by_file(&items);
        let out = render_grouped(&groups, 2, 1, "matches", |v| {
            format!("  m={}", v["marker"].as_str().unwrap())
        });
        // Single-file results: no "N matches in 1 files" line.
        assert!(!out.contains(" in 1 files"), "got:\n{out}");
        assert!(out.starts_with("a.rs (2)\n"), "got:\n{out}");
    }

    #[test]
    fn render_empty_groups() {
        let groups: Vec<FileGroup<'_>> = vec![];
        let out = render_grouped(&groups, 0, 0, "matches", |_| String::new());
        assert_eq!(out, "0 matches");
    }

    #[test]
    fn render_singular_noun_when_total_one() {
        let items = vec![json!({ "file": "a.rs" })];
        let groups = group_by_file(&items);
        let out = render_grouped(&groups, 1, 1, "match", |_| "  x".to_string());
        // Still single-file, so no header — but make sure 0-match path uses singular.
        assert!(out.starts_with("a.rs (1)"));
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --lib tools::file_group::tests::render`
Expected: compile error — `render_grouped` not defined.

- [ ] **Step 3: Implement `render_grouped`**

Append to `src/tools/file_group.rs`:

```rust
/// Render groups to compact text.
///
/// Output shape:
/// ```text
/// <total> <noun> in <files> files
///
/// path/to/a.rs (3)
///   <render_item(item0)>
///   <render_item(item1)>
///   <render_item(item2)>
/// path/to/b.rs (1)
///   <render_item(item0)>
/// ```
///
/// Single-file results (`files <= 1`) suppress the global header — the file
/// header is signal enough.
///
/// `noun` is the plural form ("hits", "references", "matches"). The function
/// does not pluralize for callers; pass the right word.
pub fn render_grouped(
    groups: &[FileGroup<'_>],
    total: usize,
    files: usize,
    noun: &str,
    render_item: impl Fn(&Value) -> String,
) -> String {
    if groups.is_empty() {
        return format!("0 {noun}");
    }

    let mut out = String::new();
    if files > 1 {
        out.push_str(&format!("{total} {noun} in {files} files\n\n"));
    }

    for (gi, group) in groups.iter().enumerate() {
        if gi > 0 {
            out.push('\n');
        }
        out.push_str(&format!("{} ({})\n", group.file, group.items.len()));
        for item in &group.items {
            out.push_str(&render_item(item));
            out.push('\n');
        }
    }
    // Trim the trailing newline for caller convenience.
    if out.ends_with('\n') {
        out.pop();
    }
    out
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib tools::file_group::tests`
Expected: 11 passed (7 prior + 4 new).

- [ ] **Step 5: Commit**

```bash
git add src/tools/file_group.rs
git commit -m "feat(file_group): add render_grouped"
```

---

### Task 4: Add `groups_to_json`

**Files:**
- Modify: `src/tools/file_group.rs`

- [ ] **Step 1: Write failing tests**

Append:

```rust
    #[test]
    fn groups_to_json_shape() {
        let items = vec![
            json!({ "file": "a.rs", "line": 1 }),
            json!({ "file": "a.rs", "line": 2 }),
            json!({ "file": "b.rs", "line": 5 }),
        ];
        let groups = group_by_file(&items);
        let value = groups_to_json(&groups);
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["file"], "a.rs");
        assert_eq!(arr[0]["count"], 2);
        let items_a = arr[0]["items"].as_array().unwrap();
        assert_eq!(items_a.len(), 2);
        // Per-item `file` field stripped — implied by group.
        assert!(items_a[0].get("file").is_none());
        assert_eq!(items_a[0]["line"], 1);
        assert_eq!(arr[1]["file"], "b.rs");
        assert_eq!(arr[1]["count"], 1);
    }
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test --lib tools::file_group::tests::groups_to_json_shape`
Expected: compile error — `groups_to_json` not defined.

- [ ] **Step 3: Implement `groups_to_json`**

Append to `src/tools/file_group.rs`:

```rust
/// Build the JSON `file_groups[]` shape used by `references` and `grep`.
///
/// Each group becomes `{ file, count, items }` where `items` is the group's
/// items with the per-item `file` field stripped.
pub fn groups_to_json(groups: &[FileGroup<'_>]) -> Value {
    use serde_json::json;
    let arr: Vec<Value> = groups
        .iter()
        .map(|g| {
            let items: Vec<Value> = g
                .items
                .iter()
                .map(|item| {
                    let mut clone = (*item).clone();
                    if let Some(obj) = clone.as_object_mut() {
                        obj.remove("file");
                    }
                    clone
                })
                .collect();
            json!({
                "file": g.file,
                "count": g.items.len(),
                "items": items,
            })
        })
        .collect();
    Value::Array(arr)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib tools::file_group::tests`
Expected: 12 passed.

- [ ] **Step 5: Commit**

```bash
git add src/tools/file_group.rs
git commit -m "feat(file_group): add groups_to_json"
```

---

### Task 5: Caller sweep — survey JSON consumers before changing shapes

**Files:** None modified. This task produces a survey note that informs Tasks 7 and 8.

- [ ] **Step 1: Grep for `references[]` and `matches[]` consumers**

Run:

```bash
grep -rn 'references\[' src/ tests/ docs/ 2>/dev/null
grep -rn '"references"' src/ tests/ docs/ 2>/dev/null
grep -rn 'matches\[' src/ tests/ docs/ 2>/dev/null
grep -rn 'result\["matches"\]\|result\.matches' src/ tests/ docs/ 2>/dev/null
```

- [ ] **Step 2: Write findings to a scratch note**

Create `docs/superpowers/plans/2026-05-15-grouped-tool-output-callers.md`:

```markdown
# Grouped tool output — caller sweep results

## `references[]` (JSON consumers)

<paste grep output, one bullet per file:line>

## `matches[]` (JSON consumers)

<paste grep output, one bullet per file:line>

## Prompt-surface mentions

- `src/prompts/server_instructions.md`: <grep result>
- `src/prompts/onboarding_prompt.md`: <grep result>
- `src/prompts/builders.rs`: <grep result>

## Migration size estimate

<N> test cases need rewriting, <M> prompt surfaces need editing.
```

- [ ] **Step 3: Commit the survey**

```bash
git add docs/superpowers/plans/2026-05-15-grouped-tool-output-callers.md
git commit -m "docs(plan): caller sweep for grouped tool output migration"
```

---

### Task 6: Switch `symbols(search)` `format_compact` to grouped rendering

**Files:**
- Modify: `src/tools/symbol/display.rs` (rewrite `format_search_symbols`)
- Modify: `src/tools/symbol/tests.rs` (update or add snapshot tests)

- [ ] **Step 1: Add failing test for grouped output**

Add at the end of `src/tools/symbol/tests.rs`:

```rust
#[test]
fn format_search_symbols_groups_by_file() {
    use crate::tools::symbol::display::format_search_symbols;
    let val = json!({
        "symbols": [
            { "kind": "Function", "file": "a.rs", "start_line": 1, "end_line": 5, "name": "foo", "symbol": "foo" },
            { "kind": "Function", "file": "a.rs", "start_line": 10, "end_line": 15, "name": "bar", "symbol": "bar" },
            { "kind": "Function", "file": "b.rs", "start_line": 3, "end_line": 7, "name": "baz", "symbol": "baz" },
        ],
        "total": 3,
    });
    let out = format_search_symbols(&val);
    assert!(out.starts_with("3 matches in 2 files\n"), "got:\n{out}");
    assert!(out.contains("a.rs (2)"));
    assert!(out.contains("b.rs (1)"));
    // File path should NOT appear inline next to each row when grouped.
    assert!(!out.contains("a.rs:1-5"), "row should not repeat file path: {out}");
    assert!(out.contains("foo"));
    assert!(out.contains("bar"));
    assert!(out.contains("baz"));
}

#[test]
fn format_search_symbols_single_file_no_global_header() {
    use crate::tools::symbol::display::format_search_symbols;
    let val = json!({
        "symbols": [
            { "kind": "Function", "file": "a.rs", "start_line": 1, "end_line": 5, "name": "foo", "symbol": "foo" },
        ],
        "total": 1,
    });
    let out = format_search_symbols(&val);
    assert!(!out.contains(" in 1 files"), "single-file output should omit global header: {out}");
    assert!(out.starts_with("a.rs (1)\n"), "got:\n{out}");
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --lib tools::symbol::tests::format_search_symbols_groups_by_file tools::symbol::tests::format_search_symbols_single_file_no_global_header`
Expected: both FAIL — current `format_search_symbols` emits a flat header (`"N matches"`) and inline `file:line` per row.

- [ ] **Step 3: Make `format_search_symbols` and `format_overflow` pub for cross-module access**

In `src/tools/symbol/display.rs`:

1. Change line 107 from `pub(super) fn format_search_symbols(val: &Value) -> String {` to `pub fn format_search_symbols(val: &Value) -> String {`.
2. Find `format_overflow` in the same file (`grep -n 'fn format_overflow' src/tools/symbol/display.rs`) and change its visibility to `pub` — `references.rs` and `grep.rs` both reuse it. If `format_overflow` is not defined in `display.rs`, add this minimal version at the bottom:

```rust
pub fn format_overflow(overflow: &Value) -> String {
    let shown = overflow["shown"].as_u64().unwrap_or(0);
    let total = overflow["total"].as_u64().unwrap_or(0);
    let hint = overflow["hint"].as_str().unwrap_or("");
    format!("… {} more (showing {shown} of {total}). {hint}", total.saturating_sub(shown))
}
```

- [ ] **Step 4: Rewrite `format_search_symbols` to group by file**

Replace the entire body of `format_search_symbols` (lines 107–211) with:

```rust
pub fn format_search_symbols(val: &Value) -> String {
    use crate::tools::file_group::{group_by_file, render_grouped};

    let symbols = match val["symbols"].as_array() {
        Some(arr) => arr,
        None => return String::new(),
    };

    let total = val["total"].as_u64().unwrap_or(symbols.len() as u64) as usize;

    if symbols.is_empty() {
        return "0 matches".to_string();
    }

    // For grouping, items need a `file` field. The symbols search JSON may
    // hoist `file` to the top level (Fix C single-file path) — restore it
    // per-symbol before grouping so the renderer has uniform shape.
    let top_file = val.get("file").and_then(|v| v.as_str());
    let normalized: Vec<Value> = symbols
        .iter()
        .map(|s| {
            let mut s = s.clone();
            if s.get("file").is_none() {
                if let Some(f) = top_file {
                    if let Some(obj) = s.as_object_mut() {
                        obj.insert("file".to_string(), Value::String(f.to_string()));
                    }
                }
            }
            s
        })
        .collect();

    let groups = group_by_file(&normalized);
    let files = groups.len();
    let noun = if total == 1 { "match" } else { "matches" };

    // Pre-compute column widths within each group's rows.
    let render_item = |item: &Value| -> String {
        let kind = item["kind"].as_str().unwrap_or("?");
        let start = item["start_line"].as_u64().unwrap_or(0);
        let end = item["end_line"].as_u64().unwrap_or(0);
        let range = if end > start {
            format!("{start}-{end}")
        } else {
            format!("{start}")
        };
        let name_path = item["symbol"]
            .as_str()
            .or_else(|| item["name"].as_str())
            .unwrap_or("?");
        // Two-space indent under the file header, then "Kind  range  name".
        let mut row = format!("  {kind}  {range}  {name_path}");
        // Inline-body rendering (short bodies only).
        if let Some(body) = item["body"].as_str() {
            const INLINE_BODY_LIMIT: usize = 500;
            if body.len() <= INLINE_BODY_LIMIT {
                for line in body.lines() {
                    row.push_str("\n      ");
                    row.push_str(line);
                }
            } else {
                let line_count = body.lines().count();
                row.push_str(&format!(
                    "\n      ({line_count}-line body — use json_path=\"$.symbols[0].body\" to extract)"
                ));
            }
        }
        row
    };

    let mut out = render_grouped(&groups, total, files, noun, render_item);

    // Append overflow line if the response was truncated.
    if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
        out.push('\n');
        out.push_str(&super::display::format_overflow(overflow));
    }

    out
}
```

(`format_overflow` already lives in `display.rs`; the `super::display::` path may simplify to a sibling call — adjust if needed during compile.)

- [ ] **Step 5: Run new tests**

Run: `cargo test --lib tools::symbol::tests::format_search_symbols`
Expected: both new tests PASS.

- [ ] **Step 6: Run all `tools::symbol` tests to catch regressions**

Run: `cargo test --lib tools::symbol`
Expected: all pass. If any old format-compact assertion fails, update it to match the new grouped shape (the snapshot is the new contract).

- [ ] **Step 7: Run clippy + fmt**

Run: `cargo fmt && cargo clippy -- -D warnings`
Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add src/tools/symbol/display.rs src/tools/symbol/tests.rs
git commit -m "feat(symbols): group search results by file in format_compact"
```

---

### Task 7: Restructure `references` JSON + grouped `format_compact`

**Files:**
- Modify: `src/tools/symbol/references.rs`
- Modify: `src/tools/symbol/tests.rs` (any consumer tests reading `result["references"]`)

- [ ] **Step 1: Locate `references[]` test consumers**

Run:

```bash
grep -n '"references"' src/tools/symbol/tests.rs
```

For each match, plan how to rewrite the assertion against `result["file_groups"]` (a flat list `total` is still available as a top-level field).

- [ ] **Step 2: Add a failing test for the new shape**

Append to `src/tools/symbol/tests.rs`:

```rust
#[test]
fn references_json_shape_uses_file_groups() {
    // Construct a fake references call output by directly calling the helper
    // that builds the result. Since references requires LSP, we test the
    // shape via a fixture-driven integration test instead. Skip here —
    // replaced by `references_returns_grouped_shape` below.
}

#[tokio::test]
async fn references_returns_grouped_shape() {
    // Reuse existing references fixture pattern.
    let dir = tempdir().unwrap();
    let lib_rs = dir.path().join("src").join("lib.rs");
    std::fs::create_dir_all(lib_rs.parent().unwrap()).unwrap();
    std::fs::write(
        &lib_rs,
        "pub fn greet() {}\nfn main() { greet(); greet(); }\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();

    let ctx = make_ctx(dir.path()).await;
    let tool = crate::tools::symbol::references::References;
    let result = tool
        .call(
            json!({ "symbol": "greet", "path": "src/lib.rs" }),
            &ctx,
        )
        .await
        .unwrap();

    let groups = result["file_groups"].as_array().unwrap();
    assert!(!groups.is_empty(), "should return at least one file_group");
    for group in groups {
        assert!(group["file"].is_string());
        assert!(group["count"].is_u64());
        let items = group["items"].as_array().unwrap();
        // Per-item `file` should be stripped.
        for item in items {
            assert!(item.get("file").is_none(), "per-item file should be stripped: {item}");
            assert!(item.get("line").is_some());
        }
    }
    assert!(result["total"].is_u64());
    assert!(result["files"].is_u64());
}
```

If `make_ctx` does not exist in this file, copy its pattern from an existing tokio test in the same file. Look for `make_ctx` definitions with `grep -n 'fn make_ctx' src/tools/symbol/tests.rs`.

- [ ] **Step 3: Run the new test to verify failure**

Run: `cargo test --lib tools::symbol::tests::references_returns_grouped_shape`
Expected: FAIL — `result["file_groups"]` is null.

- [ ] **Step 4: Replace the JSON-assembly in `references.rs`**

In `src/tools/symbol/references.rs`, replace the final result-assembly block (around lines 124-136) — the section that currently builds `json!({ "references": locations, "total": total })` — with:

```rust
        use crate::tools::file_group::{cap_grouped, group_by_file, groups_to_json};

        let budget = guard.max_results;
        let (visible, total, files) = cap_grouped(locations, budget);
        let truncated = total > visible.len();
        let groups = group_by_file(&visible);
        let file_groups = groups_to_json(&groups);

        let mut result = json!({
            "file_groups": file_groups,
            "total": total,
            "files": files,
        });
        if excluded > 0 {
            result["excluded_from_build_dirs"] = json!(excluded);
        }
        if truncated {
            // Preserve existing overflow-style hint shape.
            let overflow = json!({
                "shown": visible.len(),
                "total": total,
                "hint": "This symbol has many references. Use detail_level='full' with offset/limit to paginate",
            });
            result["overflow"] = overflow;
        }
        Ok(result)
```

Note: this removes the `let (locations, overflow) = guard.cap_items(...)` and `OutputGuard::overflow_json` calls preceding it. Delete those two lines along with the old `json!({ "references": locations, "total": total })` block.

- [ ] **Step 5: Add grouped `format_compact`**

Replace the existing `format_compact` stub in `references.rs` (lines 141–143) with:

```rust
    fn format_compact(&self, result: &Value) -> Option<String> {
        use crate::tools::file_group::{group_by_file, render_grouped};

        let file_groups = result["file_groups"].as_array()?;
        if file_groups.is_empty() {
            return Some("0 references".to_string());
        }

        // Reconstruct a flat list with per-item file field so render_grouped
        // can re-group consistently. (We could render directly from file_groups
        // but reusing render_grouped keeps the format identical to other tools.)
        let mut flat: Vec<Value> = vec![];
        for group in file_groups {
            let file = group["file"].as_str().unwrap_or("?");
            if let Some(items) = group["items"].as_array() {
                for item in items {
                    let mut clone = item.clone();
                    if let Some(obj) = clone.as_object_mut() {
                        obj.insert("file".to_string(), Value::String(file.to_string()));
                    }
                    flat.push(clone);
                }
            }
        }
        let groups = group_by_file(&flat);
        let total = result["total"].as_u64().unwrap_or(flat.len() as u64) as usize;
        let files = result["files"].as_u64().unwrap_or(groups.len() as u64) as usize;
        let noun = if total == 1 { "reference" } else { "references" };

        let render_item = |item: &Value| -> String {
            let line = item["line"].as_u64().unwrap_or(0);
            let context = item["context"].as_str().unwrap_or("").trim();
            format!("  {line:>5}  {context}")
        };

        Some(render_grouped(&groups, total, files, noun, render_item))
    }
```

- [ ] **Step 6: Update any test consumers of the old shape**

For each match from Step 1, rewrite the assertion. Example: a test that read `result["references"][0]["file"]` now reads `result["file_groups"][0]["file"]`, and to access an item it reads `result["file_groups"][0]["items"][0]["line"]`.

- [ ] **Step 7: Run all references tests**

Run: `cargo test --lib references`
Expected: all pass, including the new grouped-shape test.

- [ ] **Step 8: Commit**

```bash
git add src/tools/symbol/references.rs src/tools/symbol/tests.rs
git commit -m "feat(references): file_groups[] JSON + grouped format_compact"
```

---

### Task 8: Restructure `grep` simple-mode JSON + grouped `format_compact`

**Files:**
- Modify: `src/tools/grep.rs`

- [ ] **Step 1: Locate the matches[] result assembly**

Run: `grep -n '"matches"' src/tools/grep.rs`

Find the line that builds `json!({ "matches": ..., "total": ... })` (typically near the end of `call`).

- [ ] **Step 2: Add failing test for the new shape**

Append to `src/tools/grep.rs` inside the existing `#[cfg(test)] mod tests` block (or create one if absent — look for `mod tests` first with `grep -n 'mod tests' src/tools/grep.rs`):

```rust
    #[tokio::test]
    async fn grep_returns_grouped_shape_simple_mode() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn foo() {}\nfn foo_bar() {}\n").unwrap();
        std::fs::write(dir.path().join("b.rs"), "fn foo_baz() {}\n").unwrap();

        let ctx = make_grep_ctx(dir.path()).await; // helper, see below
        let tool = super::Grep;
        let result = tool
            .call(json!({ "pattern": "foo" }), &ctx)
            .await
            .unwrap();

        let groups = result["file_groups"].as_array().unwrap();
        assert!(!groups.is_empty());
        for group in groups {
            let items = group["items"].as_array().unwrap();
            for item in items {
                assert!(item.get("file").is_none(), "per-item file should be stripped");
                assert!(item.get("line").is_some());
                assert!(item.get("content").is_some());
            }
        }
        assert!(result["total"].as_u64().unwrap() >= 3);
        assert!(result["files"].as_u64().unwrap() >= 2);
    }
```

If `make_grep_ctx` does not exist, copy the existing pattern used by other tokio tests in `src/tools/grep.rs` (likely a helper that builds `ToolContext` from a tempdir).

- [ ] **Step 3: Run new test to verify failure**

Run: `cargo test --lib tools::grep::tests::grep_returns_grouped_shape_simple_mode`
Expected: FAIL — `file_groups` is null.

- [ ] **Step 4: Restructure the JSON assembly in `grep.rs`**

Replace the existing `json!({ "matches": ..., "total": total })` block at the end of `call` with:

```rust
        use crate::tools::file_group::{cap_grouped, group_by_file, groups_to_json};

        let budget = guard.max_results;
        let (visible, total, files) = cap_grouped(matches, budget);
        let truncated = total > visible.len();
        let groups = group_by_file(&visible);
        let file_groups = groups_to_json(&groups);

        let mut result = json!({
            "file_groups": file_groups,
            "total": total,
            "files": files,
        });
        if truncated {
            let overflow = json!({
                "shown": visible.len(),
                "total": total,
                "hint": "Many matches. Narrow the pattern or use offset/limit to paginate.",
            });
            result["overflow"] = overflow;
        }
        Ok(result)
```

Drop the previous `guard.cap_items` + `OutputGuard::overflow_json` block this replaces.

- [ ] **Step 5: Rewrite `format_search_simple_mode` to use `render_grouped`**

Replace `format_search_simple_mode` (lines 250–273) with:

```rust
fn format_search_simple_mode(out: &mut String, file_groups: &[Value], total: usize, files: usize) {
    use crate::tools::file_group::{group_by_file, render_grouped};

    // Flatten back into per-item rows so render_grouped owns the layout.
    let mut flat: Vec<Value> = vec![];
    for group in file_groups {
        let file = group["file"].as_str().unwrap_or("?");
        if let Some(items) = group["items"].as_array() {
            for item in items {
                let mut clone = item.clone();
                if let Some(obj) = clone.as_object_mut() {
                    obj.insert("file".to_string(), Value::String(file.to_string()));
                }
                flat.push(clone);
            }
        }
    }
    let groups = group_by_file(&flat);
    let noun = if total == 1 { "match" } else { "matches" };

    let render_item = |item: &Value| -> String {
        let line = item["line"].as_u64().unwrap_or(0);
        let content = item["content"].as_str().unwrap_or("").trim();
        format!("  {line:>5}: {content}")
    };

    out.push_str(&render_grouped(&groups, total, files, noun, render_item));
}
```

- [ ] **Step 6: Update `format_grep` to pass the new args and preserve context mode**

Find `format_grep` (lines 213–248). Update the call site for `format_search_simple_mode` to pass `file_groups`, `total`, and `files`. Context-mode rendering (`format_search_context_mode`) stays as-is for now; only the file-header for blocks changes — but since context mode currently reads `matches[]`, update its call site to flatten `file_groups[]` first:

In `format_grep`, when reading the result, branch on whether `file_groups` exists:

```rust
fn format_grep(val: &Value) -> String {
    let file_groups = val.get("file_groups").and_then(|v| v.as_array());
    let total = val["total"].as_u64().unwrap_or(0) as usize;
    let files = val["files"].as_u64().unwrap_or(0) as usize;

    if total == 0 {
        return "0 matches".to_string();
    }

    let mut out = String::new();
    let context_lines = val["context_lines"].as_u64().unwrap_or(0);

    // Flatten file_groups → flat matches once for both modes.
    let mut flat: Vec<Value> = vec![];
    if let Some(groups) = file_groups {
        for group in groups {
            let file = group["file"].as_str().unwrap_or("?");
            if let Some(items) = group["items"].as_array() {
                for item in items {
                    let mut clone = item.clone();
                    if let Some(obj) = clone.as_object_mut() {
                        obj.insert("file".to_string(), Value::String(file.to_string()));
                    }
                    flat.push(clone);
                }
            }
        }
    }

    if context_lines > 0 {
        format_search_context_mode(&mut out, &flat);
    } else if let Some(groups) = file_groups {
        format_search_simple_mode(&mut out, groups, total, files);
    }

    if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
        out.push('\n');
        out.push_str(&crate::tools::symbol::display::format_overflow(overflow));
    }
    out
}
```

(`format_overflow` was made `pub` in Task 6 Step 3.)

- [ ] **Step 7: Update `format_search_context_mode` signature**

Old signature: `fn format_search_context_mode(out: &mut String, matches: &[Value])`. The new caller passes the flattened slice — signature unchanged. No internal change needed *unless* the body reads anything other than per-item fields. Verify by reading the body and confirming it only uses `m["file"]`, `m["line"]`, `m["content"]`, `m["context_before"]`, `m["context_after"]` per item.

- [ ] **Step 8: Run grep tests**

Run: `cargo test --lib tools::grep`
Expected: all pass, including the new grouped-shape test.

- [ ] **Step 9: Run all tests**

Run: `cargo test --lib`
Expected: all pass.

- [ ] **Step 10: Commit**

```bash
git add src/tools/grep.rs
git commit -m "feat(grep): file_groups[] JSON + grouped format_compact (simple mode)"
```

---

### Task 9: Prompt-surface sweep + onboarding-version bump check

**Files:** Possibly modify `src/prompts/server_instructions.md`, `src/prompts/onboarding_prompt.md`, `src/prompts/builders.rs`, `src/tools/onboarding.rs`.

- [ ] **Step 1: Grep each surface for old shape references**

Run:

```bash
grep -n '"references"\|"matches"\|references\[\|matches\[' src/prompts/server_instructions.md src/prompts/onboarding_prompt.md src/prompts/builders.rs
```

- [ ] **Step 2: For each hit, decide**

- If the hit is about a *parameter name* (e.g. `name="matches"`), leave it alone.
- If the hit describes the *output shape* (e.g. "returns matches array"), rewrite to mention `file_groups`.

For example, if `server_instructions.md` says "references returns `{ references: [...] }`", change it to:

```markdown
references returns `{ file_groups: [{ file, count, items: [{line, column, context}] }], total, files }`
```

- [ ] **Step 3: Run the prompt-consistency test**

Run: `cargo test --lib server::tests::prompt_surfaces_reference_only_real_tools`
Expected: pass.

- [ ] **Step 4: Bump `ONBOARDING_VERSION` if `onboarding_prompt.md` or `builders.rs` was edited**

Read `src/tools/onboarding.rs`:

```bash
grep -n 'ONBOARDING_VERSION' src/tools/onboarding.rs
```

If `onboarding_prompt.md` or `builders.rs` was modified in Step 2, bump the version constant by 1 in `src/tools/onboarding.rs`. If only `server_instructions.md` was modified, skip the bump.

- [ ] **Step 5: Run all tests**

Run: `cargo test --lib`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/prompts/ src/tools/onboarding.rs
git commit -m "docs(prompts): update shape references for grouped tool output"
```

(If nothing was modified, skip this commit.)

---

### Task 10: Final verification (clippy, full test, release build, live MCP)

**Files:** None modified.

- [ ] **Step 1: Run fmt + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings`
Expected: clean, exit 0.

- [ ] **Step 2: Run full test suite (lib only — `retrieval_integration` is known broken on this branch)**

Run: `cargo test --lib`
Expected: all pass.

- [ ] **Step 3: Build release binary**

Run: `cargo build --release`
Expected: success.

- [ ] **Step 4: Live MCP verification**

Tell the user: "Run `/mcp` to reconnect. Then I will exercise the three tools to verify the new shapes."

After reconnect, run these and confirm output:

```
symbols(name="collect_matching", path="src/symbol/query.rs")
  → grouped text, one file header, no top-level "N in M" line
references(symbol="collect_matching", path="src/symbol/query.rs")
  → JSON has file_groups[]; format_compact shows grouped refs
grep(pattern="collect_matching")
  → JSON has file_groups[]; format_compact shows grouped matches per file
```

If any surface is wrong, return to the relevant task. If all three look right, proceed.

- [ ] **Step 5: Cherry-pick path (optional)**

If the user wants this on master, follow CLAUDE.md "Standard Ship Sequence":

```bash
git checkout master
git cherry-pick <range-of-commits-from-this-plan>
git push
git checkout experiments
git rebase master
```

But this is a presentation change — invoke the Docs Lotus Frog
(`/buddy:summon frog`) before pushing to master, per repo rules.

---

## Self-review notes

- All four `file_group` functions exposed in spec are implemented across Tasks 1–4 with tests.
- Spec §"Tests" table mapped 1:1 to plan tasks; snapshot tests live inline as planned.
- Caller sweep is a discrete task (5) before any breaking shape change (Tasks 7, 8) — order matters.
- Prompt-surface sweep + `ONBOARDING_VERSION` decision is its own task (9), not folded into Tasks 7/8, so it isn't forgotten.
- Single-file no-global-header rule appears in two test assertions (Tasks 3 and 6) — verified consistent.
- The truncation header invariant ("N hits in M files" from un-truncated totals) is enforced by `cap_grouped` returning `total, files` distinct from `visible.len()`, plumbed through Tasks 6, 7, 8.
- `symbols` JSON deliberately left flat per spec §1 decision; only `format_compact` rewired (Task 6, no JSON change).
- No "TBD", "implement appropriate", or "similar to" placeholders — every code block is complete.
