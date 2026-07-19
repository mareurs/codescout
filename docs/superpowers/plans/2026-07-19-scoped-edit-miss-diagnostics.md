# Scoped-Edit Miss Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a scoped `action="edit"` `old_string` misses, return a tiered diagnostic (invisible-whitespace classification / closest visible-content diff / wrong-heading nudge) plus a context-aware, tier-adaptive hint — at the shared `plan_scoped_edit` chokepoint, so `edit_markdown` and the librarian's `apply_body_edits` both benefit.

**Architecture:** A pure `diagnose_scoped_miss` builds a `RecoverableError` (message = diagnostic, `Hint` = tier base-hint, `extra["scoped_miss_tier"]` = tier). `plan_scoped_edit`'s not-found branch returns it. The three caller wrap-sites preserve it (prefix message, keep hint) via one shared helper instead of re-wrapping with a generic hint. The librarian's update `call` — which has the augmentation loaded — appends a params nudge when `scoped_miss_tier == "visible_drift"` on an augmented artifact.

**Tech Stack:** Rust; `strsim::normalized_levenshtein`; existing `RecoverableError` (`src/tools/core/types.rs`), `plan_scoped_edit`/`compute_section_end` (`src/tools/markdown/edit_markdown.rs`), `apply_body_edits` (`src/librarian/tools/update.rs`).

## Global Constraints

- Pre-commit gate every commit: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` (full, not just `--lib` — integration tests live under `tests/`).
- **Diagnose-only.** Never auto-repair, never retarget a write to a fuzzy match, never normalize displayed/applied bytes. Displayed text is byte-exact with whitespace made *visible*.
- **Tolerant to find, exact to show.** Whitespace-tolerance is confined to *scoring/locating*; classification and display operate on raw bytes.
- No tool-schema change to `edit_markdown` or `artifact`. Success path of `plan_scoped_edit` is unchanged — only the `!section.contains(old_string)` error branch changes.
- Tunable constants (pin with tests): `SIM_THRESHOLD = 0.5`, `SECTION_LINE_CAP = 400`, `SECTION_BYTE_CAP = 65_536`, snippet truncation at 200 chars.
- Design of record: `docs/superpowers/specs/2026-07-19-scoped-edit-miss-diagnostics-design.md`.

---

### Task 1: `strsim` dep + `similarity` wrapper

**Files:**
- Modify: `Cargo.toml` (add `strsim` to `[dependencies]`, version matching `Cargo.lock`)
- Modify: `src/tools/markdown/edit_markdown.rs` (add `similarity`)
- Test: `src/tools/markdown/tests.rs`

**Interfaces:**
- Produces: `fn similarity(a: &str, b: &str) -> f64` — `strsim::normalized_levenshtein` in `[0.0, 1.0]` (1.0 = identical).

- [ ] **Step 1: Confirm the locked version**

Run: `grep -A2 'name = "strsim"' Cargo.lock`
Note the `version = "X.Y.Z"`; use `strsim = "X.Y"` in Cargo.toml.

- [ ] **Step 2: Write the failing test** (append to `tests.rs`)

```rust
#[test]
fn similarity_ranks_closeness() {
    use super::edit_markdown::similarity;
    assert!((similarity("abc", "abc") - 1.0).abs() < 1e-9);
    // one SHA differs from another by ~half its chars but shares the frame
    let a = "_Last refresh: `8481bea`_";
    let b = "_Last refresh: `ddf8215`_";
    assert!(similarity(a, b) > 0.6, "framed lines with a changed token stay similar");
    assert!(similarity("totally different", "xxxxxxxxxxxxx") < 0.4);
}
```

- [ ] **Step 3: Add dep + wrapper**

In `Cargo.toml` `[dependencies]`: `strsim = "X.Y"` (locked version from Step 1).

In `edit_markdown.rs`:

```rust
/// Normalized Levenshtein similarity in [0.0, 1.0] (1.0 = identical). Used only
/// to LOCATE the closest line for a miss diagnostic — never to alter bytes.
pub(crate) fn similarity(a: &str, b: &str) -> f64 {
    strsim::normalized_levenshtein(a, b)
}
```

- [ ] **Step 4: Verify**

Run: `cargo test --lib similarity_ranks_closeness`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/tools/markdown/edit_markdown.rs src/tools/markdown/tests.rs
git commit -m "feat(edit_markdown): add strsim similarity helper for miss diagnostics"
```

---

### Task 2: Whitespace/invisible classification + visible rendering

**Files:**
- Modify: `src/tools/markdown/edit_markdown.rs`
- Test: `src/tools/markdown/tests.rs`

**Interfaces:**
- Produces:
  - `fn render_visible_whitespace(line: &str) -> String` — space→`·`, tab→`→`, NBSP(U+00A0)→`⟨NBSP⟩`, ZWSP(U+200B)→`⟨ZWSP⟩`, CR→`⟨CR⟩`; a trailing run is still shown via the space/tab markers. Non-whitespace chars pass through.
  - `fn classify_whitespace_diff(want: &str, have: &str) -> Option<String>` — `Some(reason)` iff `want` and `have` have identical *visible* projections but differ byte-wise (→ Tier A); `None` if visible content differs (→ Tier B).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn render_visible_whitespace_marks_invisibles() {
    use super::edit_markdown::render_visible_whitespace;
    assert_eq!(render_visible_whitespace("a b\tc"), "a·b→c");
    assert_eq!(render_visible_whitespace("x\u{00A0}y"), "x⟨NBSP⟩y");
    assert_eq!(render_visible_whitespace("trail "), "trail·");
}

#[test]
fn classify_whitespace_diff_names_the_culprit() {
    use super::edit_markdown::classify_whitespace_diff;
    // NBSP masquerading as space
    let c = classify_whitespace_diff("a b", "a\u{00A0}b").unwrap();
    assert!(c.to_lowercase().contains("non-breaking") || c.contains("U+00A0"), "got: {c}");
    // tab vs spaces in indent
    let c = classify_whitespace_diff("    x", "\tx").unwrap();
    assert!(c.to_lowercase().contains("tab") || c.to_lowercase().contains("indent"), "got: {c}");
    // trailing space
    let c = classify_whitespace_diff("done", "done ").unwrap();
    assert!(c.to_lowercase().contains("trailing"), "got: {c}");
    // CRLF vs LF (a stray CR in the line)
    let c = classify_whitespace_diff("row", "row\r").unwrap();
    assert!(c.to_lowercase().contains("cr") || c.to_lowercase().contains("line ending"), "got: {c}");
    // visible content differs -> None (this is Tier B, not a whitespace miss)
    assert!(classify_whitespace_diff("v1.0", "v2.0").is_none());
}
```

- [ ] **Step 2: Verify they fail**

Run: `cargo test --lib render_visible_whitespace classify_whitespace_diff`
Expected: FAIL — not found.

- [ ] **Step 3: Implement**

```rust
/// Strip whitespace + look-alike/invisible chars, leaving only the "visible" glyphs.
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
    s.chars().take_while(|c| c.is_whitespace() || *c == '\u{00A0}').collect()
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
    if have.contains('\u{200B}') || want.contains('\u{200B}') || have.contains('\u{FEFF}') || want.contains('\u{FEFF}') {
        notes.push("zero-width / BOM character present (U+200B / U+FEFF)".into());
    }
    if have.contains('\r') != want.contains('\r') {
        notes.push("line endings differ (a CR is present on one side: CRLF vs LF)".into());
    }
    let (wi, hi) = (leading_ws(want), leading_ws(have));
    if wi != hi {
        notes.push(format!(
            "leading whitespace differs — file: \"{}\", old_string: \"{}\"",
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
```

- [ ] **Step 4: Verify**

Run: `cargo test --lib render_visible_whitespace classify_whitespace_diff`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tools/markdown/edit_markdown.rs src/tools/markdown/tests.rs
git commit -m "feat(edit_markdown): whitespace/invisible-char classification + visible rendering"
```

---

### Task 3: Code-block awareness

**Files:**
- Modify: `src/tools/markdown/edit_markdown.rs`
- Test: `src/tools/markdown/tests.rs`

**Interfaces:**
- Produces: `fn line_in_code_block(section: &str, line_idx: usize) -> bool` — true iff the 0-based `line_idx` (into `section.split('\n')`) sits inside a ``` fenced block or is a ≥4-space indented code line. Mirrors `compute_section_end`'s fence tracking (toggle on lines starting ```` ``` ````).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn line_in_code_block_detects_fence_and_indent() {
    use super::edit_markdown::line_in_code_block;
    let section = "## H\nprose\n```\ncode line\n```\nmore prose\n    indented code\n";
    let lines: Vec<&str> = section.split('\n').collect();
    let idx = |t: &str| lines.iter().position(|l| *l == t).unwrap();
    assert!(!line_in_code_block(section, idx("prose")));
    assert!(line_in_code_block(section, idx("code line")));
    assert!(!line_in_code_block(section, idx("more prose")));
    assert!(line_in_code_block(section, idx("    indented code")));
}
```

- [ ] **Step 2: Verify it fails**

Run: `cargo test --lib line_in_code_block`
Expected: FAIL — not found.

- [ ] **Step 3: Implement**

```rust
/// True iff `line_idx` (0-based into section.split('\n')) is inside a fenced
/// ``` block or is an indented (≥4 leading spaces / a tab) code line. Whitespace
/// there is significant — the caller warns the agent not to normalize it.
pub(crate) fn line_in_code_block(section: &str, line_idx: usize) -> bool {
    let mut in_fence = false;
    for (i, line) in section.split('\n').enumerate() {
        if line.trim_start().starts_with("```") {
            if i == line_idx {
                return true;
            }
            in_fence = !in_fence;
            continue;
        }
        if i == line_idx {
            if in_fence {
                return true;
            }
            return line.starts_with("    ") || line.starts_with('\t');
        }
    }
    false
}
```

- [ ] **Step 4: Verify**

Run: `cargo test --lib line_in_code_block`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tools/markdown/edit_markdown.rs src/tools/markdown/tests.rs
git commit -m "feat(edit_markdown): line_in_code_block detection for miss diagnostics"
```

---

### Task 4: `diagnose_scoped_miss` — assemble tier + diagnostic + hint

**Files:**
- Modify: `src/tools/markdown/edit_markdown.rs`
- Test: `src/tools/markdown/tests.rs`

**Interfaces:**
- Consumes: `similarity` (T1), `classify_whitespace_diff` / `render_visible_whitespace` (T2), `line_in_code_block` (T3), `RecoverableError` (`crate::tools::core::types`).
- Produces: `fn diagnose_scoped_miss(section: &str, old_string: &str, heading: &str) -> RecoverableError`. Returns a `RecoverableError` with `message` = the tiered diagnostic, `Hint` = the tier base-hint, and `extra["scoped_miss_tier"]` = `"whitespace_invisible" | "visible_drift" | "no_close"`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn diagnose_visible_drift_shows_want_have_and_tier() {
    use super::edit_markdown::diagnose_scoped_miss;
    let section = "## State\n\n_Last refresh: `ddf8215`_\n";
    let e = diagnose_scoped_miss(section, "_Last refresh: `8481bea`_", "## State");
    let msg = e.to_string();
    assert!(msg.contains("8481bea") && msg.contains("ddf8215"), "want/have shown: {msg}");
    assert_eq!(e.extra.get("scoped_miss_tier").and_then(|v| v.as_str()), Some("visible_drift"));
    assert!(msg.to_lowercase().contains("re-read") || msg.to_lowercase().contains("changed"), "tier-B hint: {msg}");
}

#[test]
fn diagnose_whitespace_tier_classifies_and_flags_code() {
    use super::edit_markdown::diagnose_scoped_miss;
    // NBSP in a fenced code line
    let section = "## H\n```\nlet x =\u{00A0}1;\n```\n";
    let e = diagnose_scoped_miss(section, "let x = 1;", "## H");
    let msg = e.to_string();
    assert_eq!(e.extra.get("scoped_miss_tier").and_then(|v| v.as_str()), Some("whitespace_invisible"));
    assert!(msg.contains("U+00A0") || msg.to_lowercase().contains("non-breaking"), "classified: {msg}");
    assert!(msg.to_lowercase().contains("code"), "code-block significance note: {msg}");
}

#[test]
fn diagnose_no_close_nudges_heading() {
    use super::edit_markdown::diagnose_scoped_miss;
    let section = "## State\n\n_Last refresh: `ddf8215`_\n";
    let e = diagnose_scoped_miss(section, "completely unrelated content xyz", "## State");
    assert_eq!(e.extra.get("scoped_miss_tier").and_then(|v| v.as_str()), Some("no_close"));
    assert!(e.to_string().to_lowercase().contains("heading"), "wrong-heading nudge: {e}");
}
```

- [ ] **Step 2: Verify they fail**

Run: `cargo test --lib diagnose_`
Expected: FAIL — `diagnose_scoped_miss` not found.

- [ ] **Step 3: Implement**

```rust
const SIM_THRESHOLD: f64 = 0.5;
const SECTION_LINE_CAP: usize = 400;
const SECTION_BYTE_CAP: usize = 65_536;

fn truncate_snippet(s: &str) -> String {
    const MAX: usize = 200;
    if s.chars().count() <= MAX { s.to_string() } else {
        let mut t: String = s.chars().take(MAX).collect();
        t.push('…');
        t
    }
}

pub(crate) fn diagnose_scoped_miss(section: &str, old_string: &str, heading: &str) -> RecoverableError {
    use crate::tools::core::types::RecoverableError;
    use serde_json::json;

    let no_close = |extra_note: &str| {
        RecoverableError::with_hint(
            format!(
                "old_string not found in section '{heading}'. The text must match exactly (whitespace-sensitive). {extra_note}"
            ),
            "old_string isn't in this section — verify the heading, or re-read the current section text and retry.",
        )
        .with_extra("scoped_miss_tier", json!("no_close"))
    };

    let lines: Vec<&str> = section.split('\n').collect();
    if old_string.is_empty() || lines.len() > SECTION_LINE_CAP || section.len() > SECTION_BYTE_CAP {
        return no_close("");
    }

    let old_lines: Vec<&str> = old_string.split('\n').collect();
    let n = old_lines.len();
    if n == 0 || n > lines.len() {
        return no_close("");
    }

    // Locate the best contiguous n-line window by raw similarity.
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
        return no_close("");
    }

    let have_window = lines[best_idx..best_idx + n].join("\n");
    let in_code = (best_idx..best_idx + n).any(|i| line_in_code_block(section, i));
    let code_note = if in_code {
        "\nnote: inside a code block — whitespace is significant; copy the bytes exactly."
    } else {
        ""
    };

    // Tier A vs B: whitespace-only if every line pair classifies as whitespace-diff.
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
```

- [ ] **Step 4: Verify**

Run: `cargo test --lib diagnose_`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/tools/markdown/edit_markdown.rs src/tools/markdown/tests.rs
git commit -m "feat(edit_markdown): diagnose_scoped_miss builds tiered miss diagnostic + tier-adaptive hint"
```

---

### Task 5: Wire the chokepoint + preserve the hint across all three call sites

**Files:**
- Modify: `src/tools/markdown/edit_markdown.rs` (`plan_scoped_edit` branch; add `prefix_scoped_error` helper; the batch + single-edit wrap sites)
- Modify: `src/librarian/tools/update.rs` (`apply_body_edits` wrap site; augmented enrichment in the update `call` handler)
- Test: `src/tools/markdown/tests.rs`, `src/librarian/tools/update.rs` tests

**Interfaces:**
- Consumes: `diagnose_scoped_miss` (T4).
- Produces: `pub(crate) fn prefix_scoped_error(e: anyhow::Error, prefix: &str, fallback_hint: &str) -> anyhow::Error` — if `e` downcasts to `RecoverableError`, prepend `prefix` to its `message` and **keep** its guidance + extra; else wrap the display with `fallback_hint`. Replaces the three ad-hoc `.map_err(|e| RecoverableError::with_hint(format!("...{e}..."), <generic>))` sites for scoped edits.

- [ ] **Step 1: Write the failing tests**

```rust
// edit_markdown surface: the rich diagnostic + tier-B hint reach the caller.
#[test]
fn scoped_edit_miss_surfaces_rich_diagnostic() {
    use super::edit_markdown::perform_scoped_edit;
    let doc = "## State\n\n_Last refresh: `ddf8215`_\n";
    let err = perform_scoped_edit(doc, "State", "_Last refresh: `8481bea`_", "x", false).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("ddf8215"), "shows current text: {msg}");
    assert!(msg.to_lowercase().contains("re-read") || msg.to_lowercase().contains("changed"),
            "carries tier-B hint, not the generic one: {msg}");
}
```

Librarian test (in `update.rs` tests) — augmented + visible-drift appends the params nudge; a non-augmented or whitespace miss does not. Mirror the existing `body_edits_*` async test setup; assert the returned error string contains "params" only in the augmented+visible_drift case.

- [ ] **Step 2: Verify the edit_markdown test fails**

Run: `cargo test --lib scoped_edit_miss_surfaces_rich_diagnostic`
Expected: FAIL — currently the generic "Check heading name and old_string" hint is shown, and the diagnostic lacks the want/have body.

- [ ] **Step 3: Implement**

3a. In `plan_scoped_edit`, replace the not-found branch:

```rust
    if !section.contains(old_string) {
        return Err(diagnose_scoped_miss(section, old_string, heading_query).into());
    }
```

3b. Add the shared preserving-wrapper:

```rust
pub(crate) fn prefix_scoped_error(e: anyhow::Error, prefix: &str, fallback_hint: &str) -> anyhow::Error {
    use crate::tools::core::types::RecoverableError;
    match e.downcast::<RecoverableError>() {
        Ok(mut rec) => {
            rec.message = format!("{prefix}{}", rec.message);
            rec.into()
        }
        Err(other) => {
            RecoverableError::with_hint(format!("{prefix}{other}"), fallback_hint).into()
        }
    }
}
```

3c. Update the two `edit_markdown.rs` scoped-`edit` wrap sites (batch loop in `plan_batch`, and the single-edit branch) from the current `.map_err(|e| RecoverableError::with_hint(format!("...{e}..."), "Check heading name and old_string content."))` to:

```rust
    .map_err(|e| prefix_scoped_error(e, &format!("edits[{i}]: "), "Check heading name and old_string content."))?
```

(single-edit branch uses an empty `""` prefix.) Leave the non-`edit` (`plan_section_edit`) wrap sites unchanged.

3d. In `src/librarian/tools/update.rs` `apply_body_edits`, change the scoped-`edit` `.map_err` to:

```rust
    crate::tools::markdown::edit_markdown::prefix_scoped_error(
        e, &format!("body_edits[{i}]: "), "Check heading name and old_string content.",
    )
```

3e. In the update `call` handler, after `apply_body_edits(&working, edits)` errors AND when the artifact is augmented, append the params nudge if the error carries `extra["scoped_miss_tier"] == "visible_drift"`:

```rust
// pseudocode — adapt to the actual error-handling shape in `call`
let working = apply_body_edits(&working, edits).map_err(|e| {
    if is_augmented {
        if let Some(rec) = e.downcast_ref::<RecoverableError>() {
            if rec.extra.get("scoped_miss_tier").and_then(|v| v.as_str()) == Some("visible_drift") {
                // rebuild with an augmentation-aware hint appended
                return augment_params_nudge(e);
            }
        }
    }
    e
})?;
```

where `augment_params_nudge` replaces the guidance text with the base hint + " This artifact is augmented — a drifted value usually means the body is a render of `params`; update patch={params:{…}} and re-render rather than hand-editing the rendered text." Determine `is_augmented` from the artifact record already loaded in `call`.

- [ ] **Step 4: Verify**

Run: `cargo test --lib scoped_edit_miss_surfaces_rich_diagnostic` then the librarian test, then `cargo test` (full), `cargo fmt`, `cargo clippy -- -D warnings`.
Expected: PASS; full suite green (existing scoped-edit + body_edits tests still pass — success path unchanged). Do not modify a pre-existing test to make it pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/markdown/edit_markdown.rs src/librarian/tools/update.rs src/tools/markdown/tests.rs
git commit -m "feat(edit_markdown,librarian): tier-adaptive miss hints preserved across edit_markdown + body_edits; augmented params nudge"
```

---

### Task 6: Full verify + live confirmation (controller-run)

**Files:** none (verification only)

- [ ] **Step 1: Full gate** — `cargo fmt && cargo clippy -- -D warnings && cargo test` (all green).
- [ ] **Step 2: Live** — `cargo rb`, reconnect `/mcp`. On a scratch `.md` with `` _Last refresh: `ddf8215`_ ``, issue an `edit_markdown` scoped edit whose `old_string` uses a stale SHA; confirm the error now shows `have: _Last refresh: `ddf8215`_` and a re-read hint (Tier B).
- [ ] **Step 3: Live invisible-char** — put an NBSP in a scratch code fence; scoped-edit with a normal space; confirm the Tier A classification + code-significance note.
- [ ] **Step 4: Commit** any doc/tracker closeout.

---

## Self-Review

**Spec coverage:** §3 tolerant-to-find/exact-to-show → T1 locate + T2 byte-exact display. §4 tiers → T4. §4b context-aware hint → T4 base hints + T5 librarian nudge. §5 algorithm/bounds → T4 (`SIM_THRESHOLD`, caps, truncation). §6 mechanism → T4 (`RecoverableError`+`extra` supersedes the `ScopedEditMiss` sketch — noted) + T5 preserving wrapper. §7 tests → T2/T4/T5 cover each invisible class, tier gating, code-fence note, cross-surface, hint-precision matrix. Non-goal (no auto-repair) honored — nothing applies a fuzzy match.

**Placeholder scan:** the only non-literal is Task 5 §3e (`call` handler enrichment), explicitly marked pseudocode because the exact error-handling shape must be read from `update.rs` at implementation time; the intent, condition, and hint text are fully specified. Every other step is complete code.

**Type consistency:** `similarity`, `classify_whitespace_diff`, `render_visible_whitespace`, `line_in_code_block`, `diagnose_scoped_miss`, `prefix_scoped_error` signatures are consistent across definition and call sites; tier strings `"whitespace_invisible" | "visible_drift" | "no_close"` are identical in T4 (set) and T5 (read).

**Note for implementers:** Task 5 §3c/§3e must first READ the current wrap sites — `plan_batch`'s scoped-edit `.map_err`, `EditMarkdown::call`'s single-edit branch, and `update.rs`'s `call` — to match their exact local variable names and error-flow before editing. `RecoverableError` fields (`message`, `extra`) are `pub`; `extra` is a `Box<serde_json::Map>` — mutate/read via `rec.extra.get(...)`.
