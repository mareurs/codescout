# edit_markdown Batch Order-Independence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `edit_markdown` batch mode (`edits: [...]`) order-independent by resolving every heading against the original snapshot and applying edits as non-overlapping byte-span splices.

**Architecture:** Each edit compiles — against the *original* document snapshot — to one or more `PlannedEdit { span: Range<usize>, replacement }` (byte offsets). All planned edits are collected, checked for genuine overlap, then spliced **end-to-start** so earlier splices never shift unapplied offsets. A single final `normalize_trailing_newline` replaces the per-edit normalize. Single-edit mode routes through the same plan→apply core, so the two modes cannot diverge.

**Tech Stack:** Rust; existing `src/tools/markdown/edit_markdown.rs` + `src/tools/file_summary/file_summary.rs` (`resolve_section_range`, `SectionRange`); `RecoverableError` (`src/tools/core/types.rs`).

## Global Constraints

- Pre-commit gate on every commit: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` (per CLAUDE.md).
- No public tool-schema change to `edit_markdown`. `edits: [...]` keeps the same shape.
- Preserve verbatim (relocate, do not rewrite) these guard behaviors currently in `perform_section_edit_ext`: F-3 trailing-HR-separator exclusion on `replace`; F-7 surface-marker preservation (`find_lost_surface_markers`); `replace_heading` same-level logic; `insert_after` `at` modes; `remove` trailing-blank consumption; `compute_section_end` fenced-code-block awareness.
- `RecoverableError` for user-facing failures (routes as `isError:false`); `anyhow::bail!`/`?` for internal invariants (per `get_guide("error-handling")`).
- Design of record: `docs/superpowers/specs/2026-07-17-edit-markdown-batch-ordering-design.md`. Session log: `docs/trackers/edit-markdown-batch-ordering-session-log.md` (F-1/W-1).

---

### Task 1: `LineOffsets` — line-index ⇄ byte-offset map

**Files:**
- Modify: `src/tools/markdown/edit_markdown.rs` (add near the `join_lines` helpers, ~line 364)
- Test: `src/tools/markdown/tests.rs` (append)

**Interfaces:**
- Produces: `struct LineOffsets` with `fn new(content: &str) -> LineOffsets` and `fn line_start(&self, idx: usize) -> usize`. `line_start(i)` = byte offset where the i-th line (per `content.split('\n')`) begins; `line_start(line_count)` = `content.len()`; any larger idx also returns `content.len()`.

- [ ] **Step 1: Write the failing test**

```rust
// in src/tools/markdown/tests.rs
use super::edit_markdown::LineOffsets; // adjust path to wherever LineOffsets is exported

#[test]
fn line_offsets_maps_indices_to_byte_starts() {
    // "a\nb\nc" -> lines ["a","b","c"], starts at 0,2,4; past-end = len 5
    let off = LineOffsets::new("a\nb\nc");
    assert_eq!(off.line_start(0), 0);
    assert_eq!(off.line_start(1), 2);
    assert_eq!(off.line_start(2), 4);
    assert_eq!(off.line_start(3), 5); // == content.len()
    assert_eq!(off.line_start(99), 5);
}

#[test]
fn line_offsets_handles_trailing_newline() {
    // "a\nb\n" -> split yields ["a","b",""], 3 lines; len = 4
    let off = LineOffsets::new("a\nb\n");
    assert_eq!(off.line_start(0), 0);
    assert_eq!(off.line_start(1), 2);
    assert_eq!(off.line_start(2), 4); // the trailing empty line starts at len
    assert_eq!(off.line_start(3), 4);
}

#[test]
fn line_offsets_reproduces_join_boundaries() {
    // Invariant the whole design rests on: content[..line_start(i)] == join_lines(&lines[..i])
    let content = "# H\n\nbody line\n## Sub\nmore\n";
    let lines: Vec<&str> = content.split('\n').collect();
    let off = LineOffsets::new(content);
    for i in 0..=lines.len() {
        assert_eq!(&content[..off.line_start(i)], super::edit_markdown::join_lines(&lines[..i]));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib line_offsets`
Expected: FAIL — `LineOffsets` not found. (`join_lines` may need `pub(crate)` visibility for the third test; make it `pub(crate)` in this step's implementation.)

- [ ] **Step 3: Write minimal implementation**

```rust
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
        Self { starts, len: content.len() }
    }

    pub(crate) fn line_start(&self, idx: usize) -> usize {
        self.starts.get(idx).copied().unwrap_or(self.len)
    }
}
```

Also change `fn join_lines` → `pub(crate) fn join_lines` (and `join_lines_tail` if the test references it).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib line_offsets`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/tools/markdown/edit_markdown.rs src/tools/markdown/tests.rs
git commit -m "feat(edit_markdown): add LineOffsets line-index<->byte-offset map"
```

---

### Task 2: `PlannedEdit` + overlap detection + apply engine

**Files:**
- Modify: `src/tools/markdown/edit_markdown.rs`
- Test: `src/tools/markdown/tests.rs` (append)

**Interfaces:**
- Consumes: nothing from prior tasks except `normalize_trailing_newline` (existing, line ~389).
- Produces:
  - `struct PlannedEdit { span: std::ops::Range<usize>, replacement: String, edit_index: usize, order: usize }`
  - `fn detect_overlaps(edits: &[PlannedEdit]) -> anyhow::Result<()>` — `Err(RecoverableError)` naming the two conflicting `edit_index`es on a genuine overlap.
  - `fn apply_planned_edits(original: &str, edits: Vec<PlannedEdit>) -> String` — splices end-to-start, then normalizes trailing newline once.
- Conflict rule: non-empty spans conflict iff half-open ranges intersect; a zero-width insert at `X` conflicts with a non-empty span only if `start < X < end` (strictly interior); two zero-width inserts never conflict. Equal-offset zero-width inserts appear in the final document in ascending `order`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn detect_overlaps_allows_disjoint_and_boundary_inserts() {
    let e = |s: usize, en: usize, order: usize| PlannedEdit {
        span: s..en, replacement: "X".into(), edit_index: order, order,
    };
    // disjoint non-empty spans: OK
    assert!(super::edit_markdown::detect_overlaps(&[e(0, 5, 0), e(5, 10, 1)]).is_ok());
    // zero-width insert at a boundary (== end of the other span): OK
    let ins = PlannedEdit { span: 5..5, replacement: "I".into(), edit_index: 1, order: 1 };
    assert!(super::edit_markdown::detect_overlaps(&[e(0, 5, 0), ins]).is_ok());
}

#[test]
fn detect_overlaps_rejects_true_intersection() {
    let a = PlannedEdit { span: 0..8, replacement: "A".into(), edit_index: 0, order: 0 };
    let b = PlannedEdit { span: 4..12, replacement: "B".into(), edit_index: 1, order: 1 };
    let err = super::edit_markdown::detect_overlaps(&[a, b]).unwrap_err().to_string();
    assert!(err.contains("edits[0]") && err.contains("edits[1]"), "got: {err}");
}

#[test]
fn detect_overlaps_rejects_interior_insert() {
    let span = PlannedEdit { span: 0..8, replacement: "A".into(), edit_index: 0, order: 0 };
    let ins = PlannedEdit { span: 4..4, replacement: "I".into(), edit_index: 1, order: 1 };
    assert!(super::edit_markdown::detect_overlaps(&[span, ins]).is_err());
}

#[test]
fn apply_planned_edits_splices_end_to_start() {
    let original = "0123456789";
    // replace [2,4) with "XY" and [6,8) with "ZZ"; disjoint, any input order
    let edits = vec![
        PlannedEdit { span: 6..8, replacement: "ZZ".into(), edit_index: 1, order: 1 },
        PlannedEdit { span: 2..4, replacement: "XY".into(), edit_index: 0, order: 0 },
    ];
    // trailing newline is normalized on; strip for comparison
    let out = super::edit_markdown::apply_planned_edits(original, edits);
    assert_eq!(out.trim_end_matches('\n'), "01XY45ZZ89");
}

#[test]
fn apply_planned_edits_orders_coincident_inserts_by_order() {
    let original = "AB";
    let edits = vec![
        PlannedEdit { span: 1..1, replacement: "b".into(), edit_index: 1, order: 1 },
        PlannedEdit { span: 1..1, replacement: "a".into(), edit_index: 0, order: 0 },
    ];
    let out = super::edit_markdown::apply_planned_edits(original, edits);
    assert_eq!(out.trim_end_matches('\n'), "AabB"); // order 0 ("a") before order 1 ("b")
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib planned_edits detect_overlaps apply_planned`
Expected: FAIL — items not found.

- [ ] **Step 3: Write minimal implementation**

```rust
use std::ops::Range;

#[derive(Debug, Clone)]
pub(crate) struct PlannedEdit {
    pub span: Range<usize>,
    pub replacement: String,
    pub edit_index: usize, // user-facing edits[] index, for error messages
    pub order: usize,      // collection order; tie-break for coincident inserts
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
    use crate::tools::core::types::RecoverableError;
    for i in 0..edits.len() {
        for j in (i + 1)..edits.len() {
            if spans_conflict(&edits[i].span, &edits[j].span) {
                return Err(RecoverableError::with_hint(
                    format!(
                        "edits[{}] and edits[{}] rewrite overlapping regions (bytes {:?} and {:?})",
                        edits[i].edit_index, edits[j].edit_index, edits[i].span, edits[j].span
                    ),
                    "Split into separate edit_markdown calls, or target disjoint regions.",
                )
                .into());
            }
        }
    }
    Ok(())
}

pub(crate) fn apply_planned_edits(original: &str, mut edits: Vec<PlannedEdit>) -> String {
    // Apply end-to-start: highest start first. For coincident starts, apply the
    // higher `order` first so that after all splices the lower `order` sits first.
    edits.sort_by(|a, b| b.span.start.cmp(&a.span.start).then(b.order.cmp(&a.order)));
    let mut out = original.to_string();
    for e in &edits {
        out.replace_range(e.span.clone(), &e.replacement);
    }
    normalize_trailing_newline(&out)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib planned_edits detect_overlaps apply_planned`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src/tools/markdown/edit_markdown.rs src/tools/markdown/tests.rs
git commit -m "feat(edit_markdown): PlannedEdit + overlap detection + end-to-start apply engine"
```

---

### Task 3: Extract `plan_section_edit` from `perform_section_edit_ext`

**Files:**
- Modify: `src/tools/markdown/edit_markdown.rs:78-268` (`perform_section_edit_ext`)
- Test: `src/tools/markdown/tests.rs`

**Interfaces:**
- Consumes: `LineOffsets` (Task 1), `PlannedEdit`/`apply_planned_edits` (Task 2), existing `resolve_section_range`, `compute_section_end`, `find_lost_surface_markers`, `heading_level`, `ensure_trailing_newline`.
- Produces: `fn plan_section_edit(content: &str, off: &LineOffsets, heading_query: &str, action: &str, new_content: Option<&str>, at: Option<&str>, force: bool, edit_index: usize) -> anyhow::Result<Vec<PlannedEdit>>`. `perform_section_edit_ext` is kept as a thin wrapper: `let off = LineOffsets::new(content); let edits = plan_section_edit(content, &off, heading_query, action, new_content, at, force, 0)?; Ok(apply_planned_edits(content, edits))`.

**Mechanical transform.** Keep the entire body of the current `perform_section_edit_ext` (every guard block — F-3 HR walk, F-7 marker gate, `replace_heading`, `at` validation, `remove_end` blank consumption). Change **only** the terminal of each `match action` arm: instead of building `result` via `format!(before, mid, after)` + `normalize_trailing_newline`, return `Ok(vec![PlannedEdit { .. }])` with these exact spans/replacements (all offsets via `off.line_start(_)`; `heading_idx`, `end_idx`, `replace_end_idx`, `insert_idx`, `remove_end` are the same locals as today):

| Arm | `span` | `replacement` |
|---|---|---|
| `replace`, `replace_heading == true` | `off.line_start(heading_idx)..off.line_start(replace_end_idx)` | `ensure_trailing_newline(new)` |
| `replace`, `replace_heading == false` | `off.line_start(heading_idx)..off.line_start(replace_end_idx)` | `format!("{}{}{}", lines[heading_idx], separator, ensure_trailing_newline(new))` |
| `insert_before` | `off.line_start(heading_idx)..off.line_start(heading_idx)` (zero-width) | `ensure_trailing_newline(new)` |
| `insert_after` | `off.line_start(insert_idx)..off.line_start(insert_idx)` (zero-width) | `ensure_trailing_newline(new)`, **prefixed with `"\n"` iff `insert_idx == lines.len()`** — the EOF-append case (see note ‡ below) |
| `remove` | `off.line_start(heading_idx)..off.line_start(remove_end)` | `String::new()` |

> **‡ EOF-append rule (critical — verified during Task 1 review):** `insert_after` with `at="end-of-section"` on the **last** section sets `insert_idx == lines.len()` (that is `compute_section_end`'s fallback return). Legacy then computes `before = join_lines(&lines[..insert_idx])`, which equals `content + "\n"` — an extra blank line — because `join_lines` unconditionally appends `"\n"`. The byte-splice model must reproduce that: **when `insert_idx == lines.len()`, prefix the replacement with `"\n"`.** Concretely for that arm: `let prefix = if insert_idx == lines.len() { "\n" } else { "" }; replacement = format!("{prefix}{}", ensure_trailing_newline(new));`. This is the ONE case where `content[..off.line_start(i)] == join_lines(&lines[..i])` does not hold (it holds only for `i < lines.len()`), and it affects `insert_after` ONLY — every other arm's before-boundary is `heading_idx < lines.len()`. Verified: `join_lines(&lines[..lines.len()]) == content + "\n"` for content both with and without a trailing newline. Do NOT trust the (incorrect) comment in Task 1's `line_offsets_reproduces_join_boundaries` test that claims all call sites use an index `< lines.len()`.

Each returned `PlannedEdit` sets `edit_index` = the `edit_index` param and `order` = the `edit_index` param (single span per arm; scoped-edit multi-span is Task 4). The whole-document `normalize_trailing_newline` is **removed** from the arms — it now happens once in `apply_planned_edits`.

- [ ] **Step 1: Write the failing test**

```rust
// plan_section_edit produces a span+replacement; apply reproduces old behavior.
#[test]
fn plan_section_edit_replace_matches_legacy_output() {
    let content = "# Doc\n\n## A\nold body\n\n## B\ntail\n";
    // legacy: perform_section_edit_ext(content, "## A", "replace", Some("new body"), None, false)
    let legacy = super::edit_markdown::perform_section_edit_ext(
        content, "## A", "replace", Some("new body"), None, false,
    ).unwrap();
    let off = super::edit_markdown::LineOffsets::new(content);
    let planned = super::edit_markdown::plan_section_edit(
        content, &off, "## A", "replace", Some("new body"), None, false, 0,
    ).unwrap();
    let via_plan = super::edit_markdown::apply_planned_edits(content, planned);
    assert_eq!(via_plan, legacy);
}

#[test]
fn plan_section_edit_insert_after_and_remove_match_legacy() {
    let content = "## A\nbody\n## B\nmore\n";
    for (action, arg, at) in [
        ("insert_after", Some("added"), None),
        ("insert_before", Some("added"), None),
        ("remove", None, None),
    ] {
        let legacy = super::edit_markdown::perform_section_edit_ext(
            content, "## A", action, arg, at, false,
        ).unwrap();
        let off = super::edit_markdown::LineOffsets::new(content);
        let planned = super::edit_markdown::plan_section_edit(
            content, &off, "## A", action, arg, at, false, 0,
        ).unwrap();
        assert_eq!(super::edit_markdown::apply_planned_edits(content, planned), legacy,
                   "mismatch for action {action}");
    }
}

#[test]
fn plan_section_edit_insert_after_last_section_matches_legacy() {
    // EOF-append edge (note ‡): insert_after end-of-section on the LAST section,
    // file ending in '\n'. insert_idx == lines.len(), so legacy emits a blank line
    // before the inserted text; the span model must reproduce it via the "\n" prefix.
    let content = "## A\nbody\n"; // A is the last (and only) section
    let legacy = super::edit_markdown::perform_section_edit_ext(
        content, "## A", "insert_after", Some("added"), None, false,
    ).unwrap();
    let off = super::edit_markdown::LineOffsets::new(content);
    let planned = super::edit_markdown::plan_section_edit(
        content, &off, "## A", "insert_after", Some("added"), None, false, 0,
    ).unwrap();
    assert_eq!(super::edit_markdown::apply_planned_edits(content, planned), legacy,
               "insert_after on the last section must reproduce legacy EOF blank-line behavior");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib plan_section_edit`
Expected: FAIL — `plan_section_edit` not found.

- [ ] **Step 3: Write minimal implementation**

Perform the mechanical transform above: rename `perform_section_edit_ext`'s body into `plan_section_edit` with the new signature (add `off: &LineOffsets` and `edit_index: usize` params; drop the internal `let lines: Vec<&str> = content.split('\n')` only if unused — it is still needed for `compute_section_end`, `heading_level`, and the HR walk, so keep it), replace each arm's terminal per the table, and add the thin wrapper:

```rust
pub fn perform_section_edit_ext(
    content: &str,
    heading_query: &str,
    action: &str,
    new_content: Option<&str>,
    at: Option<&str>,
    force: bool,
) -> Result<String> {
    let off = LineOffsets::new(content);
    let edits = plan_section_edit(content, &off, heading_query, action, new_content, at, force, 0)?;
    Ok(apply_planned_edits(content, edits))
}
```

Keep `perform_section_edit` (the 3-arg variant at line ~56) delegating to `perform_section_edit_ext` unchanged.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib` (runs the new tests AND the full existing markdown suite — the regression gate)
Expected: PASS — new `plan_section_edit_*` tests pass AND every pre-existing `tests.rs` test still passes (proves all guards preserved through the transform).

- [ ] **Step 5: Commit**

```bash
git add src/tools/markdown/edit_markdown.rs src/tools/markdown/tests.rs
git commit -m "refactor(edit_markdown): extract plan_section_edit returning PlannedEdit; wrapper preserves single-edit behavior"
```

---

### Task 4: Extract `plan_scoped_edit` from `perform_scoped_edit` (fine-grained spans)

**Files:**
- Modify: `src/tools/markdown/edit_markdown.rs:401-447` (`perform_scoped_edit`)
- Test: `src/tools/markdown/tests.rs`

**Interfaces:**
- Produces: `fn plan_scoped_edit(content: &str, off: &LineOffsets, heading_query: &str, old_string: &str, new_string: &str, replace_all: bool, edit_index: usize) -> anyhow::Result<Vec<PlannedEdit>>`. `perform_scoped_edit` kept as a thin wrapper (`plan_scoped_edit(..., 0)` then `apply_planned_edits`).
- Behavior: resolve the section span `sec = off.line_start(heading_idx)..off.line_start(end_idx)`; search `&content[sec.clone()]` for `old_string`. If absent → same "old_string not found in section" error as today. First-only → one `PlannedEdit`; `replace_all` → one `PlannedEdit` per non-overlapping match (each `order` monotonically increasing from `edit_index`-derived base). Each span is `(sec.start + match_pos)..(sec.start + match_pos + old_string.len())`, replacement `new_string`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn plan_scoped_edit_first_only_matches_legacy() {
    let content = "## A\nrow one\nrow one\n## B\n";
    let legacy = super::edit_markdown::perform_scoped_edit(content, "## A", "row one", "row X", false).unwrap();
    let off = super::edit_markdown::LineOffsets::new(content);
    let planned = super::edit_markdown::plan_scoped_edit(content, &off, "## A", "row one", "row X", false, 0).unwrap();
    assert_eq!(super::edit_markdown::apply_planned_edits(content, planned), legacy);
}

#[test]
fn plan_scoped_edit_replace_all_matches_legacy() {
    let content = "## A\nx\nx\n## B\nx\n"; // only the two x's under A should change
    let legacy = super::edit_markdown::perform_scoped_edit(content, "## A", "x", "y", true).unwrap();
    let off = super::edit_markdown::LineOffsets::new(content);
    let planned = super::edit_markdown::plan_scoped_edit(content, &off, "## A", "x", "y", true, 0).unwrap();
    assert_eq!(super::edit_markdown::apply_planned_edits(content, planned), legacy);
}

#[test]
fn plan_scoped_edit_missing_old_string_errors() {
    let content = "## A\nbody\n";
    let off = super::edit_markdown::LineOffsets::new(content);
    let err = super::edit_markdown::plan_scoped_edit(content, &off, "## A", "nope", "z", false, 0);
    assert!(err.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib plan_scoped_edit`
Expected: FAIL — `plan_scoped_edit` not found.

- [ ] **Step 3: Write minimal implementation**

```rust
pub(crate) fn plan_scoped_edit(
    content: &str,
    off: &LineOffsets,
    heading_query: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
    edit_index: usize,
) -> Result<Vec<PlannedEdit>> {
    use crate::tools::file_summary::resolve_section_range;

    let range = resolve_section_range(content, heading_query).map_err(|e| anyhow::anyhow!("{}", e))?;
    let lines: Vec<&str> = content.split('\n').collect();
    let heading_idx = range.heading_line - 1;
    let end_idx = compute_section_end(&lines, heading_idx + 1, range.level);

    let sec_start = off.line_start(heading_idx);
    let sec_end = off.line_start(end_idx);
    let section = &content[sec_start..sec_end];

    if !section.contains(old_string) {
        return Err(anyhow::anyhow!(
            "old_string not found in section '{}'. The text must match exactly (whitespace-sensitive).",
            heading_query
        ));
    }

    let mut edits = Vec::new();
    let mut search_from = 0usize;
    let mut order = edit_index * 1_000; // leave headroom so multi-span edits keep global order
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
        search_from = search_from + rel + old_string.len().max(1);
        if !replace_all {
            break;
        }
    }
    Ok(edits)
}

pub(crate) fn perform_scoped_edit(
    content: &str,
    heading_query: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<String> {
    let off = LineOffsets::new(content);
    let edits = plan_scoped_edit(content, &off, heading_query, old_string, new_string, replace_all, 0)?;
    Ok(apply_planned_edits(content, edits))
}
```

Note: the legacy `perform_scoped_edit` extracted the section as `join_lines_tail(..) + "\n"`, adding a synthetic trailing newline only relevant at EOF-without-newline. Searching `&content[sec_start..sec_end]` matches legacy for every case except an `old_string` that depends on that synthetic char at end-of-file — call this out in the PR description; it is arguably more correct and is covered by Task 5's regression run.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib`
Expected: PASS — new scoped tests + full existing suite green.

- [ ] **Step 5: Commit**

```bash
git add src/tools/markdown/edit_markdown.rs src/tools/markdown/tests.rs
git commit -m "refactor(edit_markdown): extract plan_scoped_edit with fine-grained byte-span matches"
```

---

### Task 5: Rewire the batch loop to resolve-against-original + collect/detect/apply

**Files:**
- Modify: `src/tools/markdown/edit_markdown.rs` — extract a new pure `plan_batch` fn, and shrink the `if has_edits { ... }` block inside `EditMarkdown::call` (~lines 689-746) to call it.
- Test: `src/tools/markdown/tests.rs`

**Interfaces:**
- Consumes: `plan_scoped_edit` (Task 4), `plan_section_edit` (Task 3), `detect_overlaps`/`apply_planned_edits`/`PlannedEdit` (Task 2), `LineOffsets` (Task 1), existing `find_consumed_subsections`/`subsection_guard_error`, `serde_json::Value` (already imported at module top).
- Produces: `pub(crate) fn plan_batch(snapshot: &str, edits: &[Value], force: bool) -> anyhow::Result<Vec<PlannedEdit>>` — resolves every edit against `snapshot`, collects planned edits, runs `detect_overlaps`, returns the validated Vec (or a `RecoverableError` on a bad edit / overlap). This pure function is the unit-testable core; `call()` does `apply_planned_edits` on its result.
- Behavior: the snapshot is `new_content` **after** any frontmatter mutation and **before** any body edit (preserves the existing frontmatter-then-body ordering). Atomicity is preserved structurally: `plan_batch` errors propagate out of `call()` before `atomic_write`, so a rejected batch leaves the file untouched.

- [ ] **Step 1: Write the failing tests** (append to `tests.rs`; match its import style)

```rust
#[test]
fn batch_rename_heading_and_add_row_is_order_independent() {
    use serde_json::json;
    let doc = "# T\n\n## The 7 cases\n| 7 | seven |\n";
    let rename = json!({"heading":"The 7 cases","action":"edit",
        "old_string":"## The 7 cases","new_string":"## The 8 cases"});
    let addrow = json!({"heading":"The 7 cases","action":"edit",
        "old_string":"| 7 | seven |","new_string":"| 7 | seven |\n| 8 | eight |"});

    let plan_a = super::edit_markdown::plan_batch(doc, json!([rename, addrow]).as_array().unwrap(), false).unwrap();
    let plan_b = super::edit_markdown::plan_batch(doc, json!([addrow, rename]).as_array().unwrap(), false).unwrap();
    let out_a = super::edit_markdown::apply_planned_edits(doc, plan_a);
    let out_b = super::edit_markdown::apply_planned_edits(doc, plan_b);

    assert_eq!(out_a, out_b, "batch must be order-independent");
    assert!(out_a.contains("## The 8 cases"), "heading renamed: {out_a:?}");
    assert!(out_a.contains("| 8 | eight |"), "row 8 added: {out_a:?}");
    assert!(out_a.contains("| 7 | seven |"), "row 7 kept: {out_a:?}");
}

#[test]
fn batch_true_overlap_is_rejected() {
    use serde_json::json;
    let doc = "## A\nhello world\n";
    // replace whole section AND scoped-edit inside it -> interior overlap
    let arr = json!([
        {"heading":"A","action":"replace","content":"brand new"},
        {"heading":"A","action":"edit","old_string":"hello","new_string":"HELLO"}
    ]);
    let res = super::edit_markdown::plan_batch(doc, arr.as_array().unwrap(), false);
    assert!(res.is_err(), "overlapping replace+edit on same section must be rejected");
}

#[test]
fn batch_mixed_actions_scrambled_order_is_stable() {
    use serde_json::json;
    let doc = "## A\naaa\n## B\nbbb\n## C\nccc\n";
    let edits = json!([
        {"heading":"C","action":"edit","old_string":"ccc","new_string":"CCC"},
        {"heading":"A","action":"insert_after","content":"after-a","at":"after-heading-line"},
        {"heading":"B","action":"remove"}
    ]);
    let rev = json!([
        {"heading":"B","action":"remove"},
        {"heading":"A","action":"insert_after","content":"after-a","at":"after-heading-line"},
        {"heading":"C","action":"edit","old_string":"ccc","new_string":"CCC"}
    ]);
    let out1 = super::edit_markdown::apply_planned_edits(doc,
        super::edit_markdown::plan_batch(doc, edits.as_array().unwrap(), false).unwrap());
    let out2 = super::edit_markdown::apply_planned_edits(doc,
        super::edit_markdown::plan_batch(doc, rev.as_array().unwrap(), false).unwrap());
    assert_eq!(out1, out2, "disjoint edits must be permutation-invariant");
    assert!(out1.contains("CCC") && out1.contains("after-a") && !out1.contains("bbb"), "got: {out1:?}");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib batch_rename batch_true_overlap batch_mixed`
Expected: FAIL — `plan_batch` not found. (After it exists but before the `call()` rewrite, these already pass, since they test `plan_batch` directly; the `call()` change in Step 3 is what makes the live tool order-independent — covered by Task 6's live verify.)

- [ ] **Step 3: Implement**

Add the pure helper (place it just after `apply_planned_edits`):

```rust
pub(crate) fn plan_batch(snapshot: &str, edits: &[Value], force: bool) -> Result<Vec<PlannedEdit>> {
    use crate::tools::core::types::RecoverableError;
    let off = LineOffsets::new(snapshot);
    let mut planned: Vec<PlannedEdit> = Vec::new();

    for (i, edit) in edits.iter().enumerate() {
        let heading = edit["heading"].as_str()
            .ok_or_else(|| anyhow::anyhow!("edits[{}]: missing required 'heading' field", i))?;
        let action = edit["action"].as_str()
            .ok_or_else(|| anyhow::anyhow!("edits[{}]: missing required 'action' field", i))?;

        let mut sub = if action == "edit" {
            let old_string = edit["old_string"].as_str().ok_or_else(|| {
                anyhow::anyhow!("edits[{}]: old_string is required for action='edit'", i)
            })?;
            let new_string = edit["new_string"].as_str().unwrap_or("");
            let replace_all = edit["replace_all"].as_bool().unwrap_or(false);
            plan_scoped_edit(snapshot, &off, heading, old_string, new_string, replace_all, i)
                .map_err(|e| RecoverableError::with_hint(
                    format!("edits[{}]: {}", i, e),
                    "Check heading name and old_string content.",
                ))?
        } else {
            if action == "replace" && !edit["include_subsections"].as_bool().unwrap_or(false) {
                if let Ok(victims) = find_consumed_subsections(snapshot, heading) {
                    if !victims.is_empty() {
                        return Err(subsection_guard_error(Some(i), heading, &victims).into());
                    }
                }
            }
            plan_section_edit(
                snapshot, &off, heading, action,
                edit["content"].as_str(), edit["at"].as_str(), force, i,
            )
            .map_err(|e| RecoverableError::with_hint(
                format!("edits[{}]: {}", i, e),
                "Check heading name and action.",
            ))?
        };
        planned.append(&mut sub);
    }

    detect_overlaps(&planned)?;
    Ok(planned)
}
```

Then replace the body of the `if has_edits { ... }` block in `EditMarkdown::call` with:

```rust
if has_edits {
    let edits = input["edits"].as_array().unwrap();
    // Snapshot = content after frontmatter mutation, before any body edit.
    let snapshot = new_content.clone();
    let planned = plan_batch(&snapshot, edits, input["force"].as_bool().unwrap_or(false))?;
    new_content = apply_planned_edits(&snapshot, planned);
}
```

Everything downstream (body-shrink guard, atomic write) is unchanged — it operates on the final `new_content`. The mutual-exclusivity / no-op validation earlier in `call()` is unchanged.

**Dead-code cleanup:** now that `plan_batch` (and `call()`) consume them, remove the `#[allow(dead_code)]` attributes added in Tasks 1-2 from `LineOffsets`, `PlannedEdit`, `spans_conflict`, `detect_overlaps`, and `apply_planned_edits`. Keep `#[allow(clippy::too_many_arguments)]` on `plan_section_edit`. Confirm `cargo clippy -- -D warnings` stays clean after removal.

- [ ] **Step 4: Run the full gate**

Run: `cargo test --lib` then `cargo fmt` then `cargo clippy -- -D warnings`
Expected: PASS — the three batch tests pass, full suite green, clippy clean (including after the `#[allow(dead_code)]` removals).

- [ ] **Step 5: Commit** (add ONLY the two paths; never `git add -A`)

```bash
git add src/tools/markdown/edit_markdown.rs src/tools/markdown/tests.rs
git commit -m "fix(edit_markdown): resolve batch edits against original snapshot; order-independent apply"
```

---
---

### Task 6: End-to-end verification + session-log closeout

**Files:**
- Modify: `docs/trackers/edit-markdown-batch-ordering-session-log.md` (flip F-1 status)
- Verify: live MCP path

- [ ] **Step 1: Full gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all green.

- [ ] **Step 2: Live MCP smoke test**

Run: `cargo rb` then `/mcp` to reconnect. In a scratch markdown file, issue the original failing batch (rename `## The 7 cases`→`## The 8 cases` + add row 8) in rename-first order via the live `edit_markdown` tool.
Expected: `"ok"` (no "heading not found"); file shows `## The 8 cases`, rows 7 and 8.

- [ ] **Step 3: Mixed-action scramble (manual)**

Issue one batch with `insert_after` in an early section, `edit` in a late section, and `remove` of a middle section, in scrambled array order.
Expected: all three land correctly; re-running with a permuted order yields byte-identical output.

- [ ] **Step 4: Flip F-1 status**

```
edit_markdown(path="docs/trackers/edit-markdown-batch-ordering-session-log.md",
  heading="## F-1 — ...", action="edit",
  old_string="**Status:** open", new_string="**Status:** fixed-verified")
```
Also update the Index table row's Status cell to `fixed-verified`.

- [ ] **Step 5: Commit**

```bash
git add docs/trackers/edit-markdown-batch-ordering-session-log.md
git commit -m "docs(tracker): mark F-1 fixed-verified — edit_markdown batch order-independence shipped"
```

---

## Self-Review

**Spec coverage:**
- §3.1 snapshot resolution → Task 5 (resolve against `snapshot`). ✅
- §3.2 write-span byte model → Tasks 1 (`LineOffsets`), 3+4 (per-action spans). ✅
- §3.3 overlap + end-to-start + normalize-once → Task 2. ✅
- §4 plan/apply split, single-edit shares core → Tasks 3+4 wrappers. ✅
- §5 invariants preserved → Task 3 "mechanical transform" keeps guard blocks; Task 3/4 Step 4 run the full existing suite as the regression gate. ✅
- §6 testing strategy → Tasks 2–6 cover order-independence, disjoint same-section, genuine overlap, mixed-action scramble, normalize-once. ✅
- §7 open questions (EOF synthetic newline, replace_all multi-span) → noted in Task 4 + tested. ✅

**Placeholder scan:** No TBD/TODO; every code step shows real code; every run step shows the command + expected output. ✅

**Type consistency:** `PlannedEdit { span, replacement, edit_index, order }` used identically across Tasks 2–5; `plan_section_edit`/`plan_scoped_edit`/`detect_overlaps`/`apply_planned_edits`/`LineOffsets::line_start` signatures match their definitions and call sites. ✅

**Note for implementers:** the visibility (`pub(crate)`) of `plan_*`, `detect_overlaps`, `apply_planned_edits`, `LineOffsets`, and `join_lines` must be sufficient for `tests.rs` to reference them via `super::edit_markdown::`. If `tests.rs` is an inner `#[cfg(test)] mod`, adjust the `super::` paths accordingly — match the file's existing import style.
