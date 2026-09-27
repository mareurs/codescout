---
id: e897df90a0143cf5
kind: plan
status: draft
title: TAXONOMY append_entry recipe test — implementation plan
tags:
- tests
- taxonomy
- append-entry
- doc-to-code
- ic-11
topic: taxonomy-recipe-gate
---

# TAXONOMY `append_entry` recipe test — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Valid:** dated 2026-09-27

**Goal:** A `cargo test` that fails whenever an `append_entry` recipe in the *Main taxonomy* table of `docs/TAXONOMY.md` would be refused by `append_entry` itself.

**Architecture:** One `#[cfg(test)] mod taxonomy_recipes` appended to `src/librarian/tools/append_entry.rs`. A small scanner turns table rows into `Recipe`s (prose / params / template); three checks apply the code contract per shape through the production readers the tool itself uses. Joined on each row's *Lives in* cell, never on its recipe `id`.

**Tech Stack:** Rust (edition 2021), in-crate unit tests, `tempfile` (already a dev-dependency), `scripts/with-slot.sh`, `scripts/mutation-probe.sh`, `scripts/gate.sh`.

**Spec:** `docs/superpowers/specs/2026-09-27-taxonomy-append-recipes-test-design.md` (artifact `66842613e8620a86`). Read it first; this plan argues from it.

## Global Constraints

- The test lives in `src/librarian/tools/append_entry.rs` as `#[cfg(test)] mod taxonomy_recipes`, after the existing `mod tests`. It compiles only with the `librarian` feature, so it runs in the DEFAULT lane only; `LEAN exit=0` says nothing about it.
- Call production readers, never copies: `crate::librarian::catalog::augmentation::declared_prefixes_from_frontmatter`, `crate::util::librarian_guard::is_citable_entry_prefix`, `crate::librarian::tools::doctor::{parse_declaration, Declaration}`, `crate::librarian::augmentation_sidecar::read`, `crate::librarian::frontmatter::parse`, `crate::util::markdown_fence::FenceState`.
- Never join on a recipe's `id="…"` — it is `sha256` of one machine's absolute path (ADR `docs/adrs/2026-09-14-an-id-keyed-on-an-absolute-path-cannot-be-checked-off-the-machine.md`).
- Never split a row into cells on `|` beyond the Lives-in cell: the F row holds unescaped pipes inside `index_row="| {id} | … |"`.
- Params recipes: do NOT assert `entry_prefix` (`bug-fix-session-log:F-176`).
- Targeted test runs: `scripts/with-slot.sh cargo test --lib taxonomy_recipes`. Never the path a gate printed, never the shared `target/`.
- Never mutate the shared tree to watch a test fail — every observed red goes through `./scripts/mutation-probe.sh` (isolated worktree).
- After any `edit_code` insert, run `./scripts/fmt-mine.sh`. `edit_code` has been observed to over-indent an inserted body by one level. If `fmt-mine.sh` refuses the file as another session's, read `git diff -- src/librarian/tools/append_entry.rs`; only if every dirty hunk is yours, run `rustfmt --edition 2021 src/librarian/tools/append_entry.rs`.
- Commits on this shared checkout: `git add -- <paths>`, then `git diff --cached --name-only`, then `git commit -m "…" -- <paths>` — three SEPARATE calls, never chained with `&&`, never `git add -A`, never `git reset` a peer's staged path. Before committing a shared tracker file, read its `git diff`; if it carries hunks you did not write, do not commit it — ask its owner.
- Bug files and trackers are written through `doc(...)`; archive with `doc(action="move")`, never `git mv`.
- In any `docs/` prose, backtick only paths that exist — `audit_doc_refs` treats a backticked path as a live citation and a dead one is `high`.
- Before completing: `./scripts/gate.sh` must print `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

## Review Focus

1. **A tracker path outside the Lives-in cell** (for example in the Captures column) must never become the target; a row whose Lives-in cell names none is unparseable. Pinned in Task 1 (`a_target_is_read_only_from_the_lives_in_cell`).
2. **`## Main taxonomy` renamed or removed** must fail loudly, not pass on zero rows. Pinned in Task 1 (`a_renamed_section_yields_no_rows`) plus the non-vacuity assert in Task 2, whose message names the section heading.
3. **A CRLF checkout** (the repo has Windows CI lanes) must scan identically to LF. Pinned in Task 1 (`crlf_scans_like_lf`).
4. **An exemption naming a log that was renamed or deleted** must be reported, so the list cannot rot silently. Pinned in Task 2 (`template_check_enforces_f_and_w_with_a_shrink_only_exemption_list`).
5. **A params tracker declaring bare `expects_augmentation: true`** (no committed shape) must fail — a fresh clone re-attaches nothing. Pinned in Task 2 (`params_check_reads_the_committed_sidecar_not_entry_prefix`).

---

### Task 1: Claim, pre-action snapshot, and the recipe scanner

**Files:**
- Modify: `src/librarian/tools/append_entry.rs` (append a new `#[cfg(test)] mod taxonomy_recipes` after the existing `mod tests`, which ends at the end of the file)
- Modify (via `doc`): `docs/issues/2026-09-24-residual-taxonomy-recipes-declare-entry-prefix-test.md`, `docs/trackers/deep-agent-workflow-observations.md`

**Interfaces:**
- Produces (used by Task 2, all private to `taxonomy_recipes`):
  - `const CALL_OPENER: &str`, `const SECTION: &str`
  - `enum Shape { Prose, Params { collection: String }, Template }` — derives `Debug, Clone, PartialEq`
  - `struct Recipe { line: usize, label: String, id_prefix: String, target: String, shape: Shape }` — derives `Debug, Clone, PartialEq`
  - `struct Scan { recipes: Vec<Recipe>, unparseable: Vec<String>, non_recipe_rows: Vec<String>, rows_seen: usize }` — derives `Debug, Default`
  - `fn at(line: usize, label: &str) -> String` → `"docs/TAXONOMY.md:<line> (<label>)"`
  - `fn scan_main_taxonomy(text: &str) -> Scan`
  - `fn parse_row(line: &str) -> Result<(String, Option<String>, String), String>` → `(id_prefix, entry_collection, target)`
  - `fn call_args(text: &str, open: usize) -> Option<&str>`, `fn top_level_arg(args: &str, key: &str) -> Option<String>`, `fn first_tracker_path(cell: &str) -> Option<String>`

- [ ] **Step 1: Claim the bug**

```
doc(action="update", id="5820a75840dd2d52", patch={"status": "taken", "extra": {"claimed_by": "<your session id>", "claimed_at": "<today>"}})
```

- [ ] **Step 2: Record the observation-window snapshot BEFORE writing code** (CLAUDE.md § *Deep-agent observation window*; this is an `enrichment` sample — the session's first eligible task was not captured, which the Task 4 receipt records)

```
doc(action="append_entry", id="1b770cb6462acde6", id_prefix="DWF",
    anchor_heading="## Template for new entries",
    title="TAXONOMY append_entry recipe gate — pre-action packet",
    body="**Status:** pending-outcome\n**Valid:** dated <today>\n\n**Sampling / capture mode:** enrichment; prospective.\n\n**Identity / key:** session <your session id>; key `<session-prefix>:taxonomy-recipes-test`.\n\n**Task / authority / substrate:** implement the operator-approved plan for residual `5820a75840dd2d52`; experiments at <HEAD sha>, shared dirty tree.\n\n**Pre-action evidence:** spec `docs/superpowers/specs/2026-09-27-taxonomy-append-recipes-test-design.md`; `bug-fix-session-log:F-176`.\n\n**Initial next action / completion check:** write the scanner test-first; completion = corpus test green, 11 mutations each KILLED with the named message, gate 0/0/0/0.",
    index_row="| {id} | <today> | workflow | enrichment | <session-prefix>:taxonomy-recipes-test |")
```

- [ ] **Step 3: Write the failing scanner tests.** Append this module to the end of `src/librarian/tools/append_entry.rs` (after `mod tests`'s closing brace) with `edit_code(action="insert", symbol="tests", position="after", path="src/librarian/tools/append_entry.rs", body=…)`, then run `./scripts/fmt-mine.sh`. The module is tests only for now — Step 5 adds the implementation into it. The fence below is FOUR backticks because the fixture contains a three-backtick fence.

````rust
/// Doc-to-code gate: every `append_entry` recipe in `docs/TAXONOMY.md`'s *Main taxonomy* table
/// must be one `call` accepts. Spec: docs/superpowers/specs/2026-09-27-taxonomy-append-recipes-test-design.md.
#[cfg(test)]
mod taxonomy_recipes {
    use crate::util::markdown_fence::FenceState;

    /// A miniature TAXONOMY. Load-bearing details: the fenced ZZ row and the Q row under the
    /// NEXT section must be ignored; the R row's `index_row="| {id} | … |"` has unescaped pipes
    /// that break any cell-splitting parser; A has no call (not a recipe); B has a call with no
    /// id_prefix (unparseable, line 12).
    const FIXTURE: &str = r##"# T
## Main taxonomy
```sh
| **ZZ-N** | `docs/trackers/in-fence.md` | c | `doc(action="append_entry", id="z", id_prefix="ZZ")` | p |
```
| Prefix | Lives in | Captures | Append tool | Promotes to |
|---|---|---|---|---|
| **R-N** | `docs/trackers/r.md` (artifact `abc`) | c | `doc(action="append_entry", id="abc", id_prefix="R", index_row="| {id} | … |", title=…)` | p |
| **T-N** | `docs/trackers/t.md` | c | `doc(action="append_entry", id="t", entry_collection="observations", id_prefix="T", entry={…})` | p |
| **F-N** | `docs/trackers/<topic>-session-log.md` | c | `doc(action="append_entry", id=<log's artifact id>, id_prefix="F", title=…)` | p |
| **A-N** | `docs/trackers/a.md` | c | per the tracker's convention | p |
| **B-N** | `docs/trackers/b.md` | c | `doc(action="append_entry", id="b", title=…)` | p |
## Next section
| **Q-N** | `docs/trackers/q.md` | c | `doc(action="append_entry", id="q", id_prefix="Q")` | p |
"##;

    fn open_of(row: &str) -> usize {
        row.find(CALL_OPENER).unwrap() + "doc".len()
    }

    #[test]
    fn a_call_span_closes_over_a_nested_literal() {
        let row = r##"x `doc(action="append_entry", id_prefix="T", entry={a: [1, (2)]})` tail"##;
        assert_eq!(
            call_args(row, open_of(row)),
            Some(r##"action="append_entry", id_prefix="T", entry={a: [1, (2)]}"##)
        );
    }

    #[test]
    fn a_paren_inside_a_quoted_string_does_not_close_the_call() {
        let row = r##"`doc(action="append_entry", title="a ) b", id_prefix="R")`"##;
        let args = call_args(row, open_of(row)).expect("closes at the real paren");
        assert_eq!(top_level_arg(args, "id_prefix").as_deref(), Some("R"));
    }

    #[test]
    fn an_unclosed_call_span_is_none() {
        let row = r##"`doc(action="append_entry", id_prefix="R", entry={…)`"##;
        assert_eq!(call_args(row, open_of(row)), None);
    }

    #[test]
    fn only_a_top_level_argument_at_a_word_boundary_counts() {
        // Three decoys, each load-bearing: a quoted mention, a nested object and a longer key all
        // spell `id_prefix="…"`; only the last, top-level one is the argument.
        let args = r#"body="id_prefix=\"Q\"", entry={id_prefix="Z"}, xid_prefix="Y", id_prefix="R""#;
        assert_eq!(top_level_arg(args, "id_prefix").as_deref(), Some("R"));
    }

    #[test]
    fn the_scanner_classifies_each_row_and_ignores_fenced_and_foreign_rows() {
        let scan = scan_main_taxonomy(FIXTURE);
        let got: Vec<(&str, &str, Shape)> = scan
            .recipes
            .iter()
            .map(|r| (r.label.as_str(), r.id_prefix.as_str(), r.shape.clone()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("R-N", "R", Shape::Prose),
                ("T-N", "T", Shape::Params { collection: "observations".into() }),
                ("F-N", "F", Shape::Template),
            ]
        );
        assert_eq!(scan.recipes[0].target, "docs/trackers/r.md");
        assert_eq!(scan.recipes[0].line, 8);
        assert_eq!(scan.non_recipe_rows, vec!["A-N".to_string()]);
        assert_eq!(scan.unparseable.len(), 1, "{:?}", scan.unparseable);
        assert!(
            scan.unparseable[0].starts_with("docs/TAXONOMY.md:12 (B-N)"),
            "{:?}",
            scan.unparseable
        );
        assert_eq!(scan.rows_seen, 5, "the fenced ZZ row and the Q row under the next section are not rows");
    }

    #[test]
    fn two_calls_that_disagree_are_unparseable_not_a_guess() {
        let row = r##"| **X-N** | `docs/trackers/x.md` | c | `doc(action="append_entry", id_prefix="X")` or `doc(action="append_entry", id_prefix="Y")` | p |"##;
        let err = parse_row(row).unwrap_err();
        assert!(err.contains("disagree"), "{err}");
    }

    #[test]
    fn a_target_is_read_only_from_the_lives_in_cell() {
        // Load-bearing: the Captures cell names a real-looking tracker. Binding to it would
        // check the wrong file and pass.
        let row = r##"| **X-N** | Same file as F-N | see `docs/trackers/other.md` | `doc(action="append_entry", id_prefix="X")` | p |"##;
        let err = parse_row(row).unwrap_err();
        assert!(err.contains("Lives-in cell names no backticked docs/trackers"), "{err}");
    }

    #[test]
    fn a_renamed_section_yields_no_rows() {
        let scan = scan_main_taxonomy(&FIXTURE.replace("## Main taxonomy", "## Main taxonomy (renamed)"));
        assert_eq!(scan.rows_seen, 0);
        assert!(scan.recipes.is_empty());
    }

    #[test]
    fn crlf_scans_like_lf() {
        let lf = scan_main_taxonomy(FIXTURE);
        let crlf = scan_main_taxonomy(&FIXTURE.replace('\n', "\r\n"));
        assert_eq!(crlf.recipes, lf.recipes);
        assert_eq!(crlf.unparseable, lf.unparseable);
        assert_eq!(crlf.rows_seen, lf.rows_seen);
    }
}
````

- [ ] **Step 4: Run to verify it fails**

Run: `scripts/with-slot.sh cargo test --lib taxonomy_recipes`
Expected: compile error — `cannot find value CALL_OPENER`, `cannot find function call_args` / `scan_main_taxonomy` / `parse_row` / `top_level_arg`, `cannot find type Shape`.

- [ ] **Step 5: Implement the scanner.** Insert these items at the top of `mod taxonomy_recipes`, directly after its `use crate::util::markdown_fence::FenceState;` line (use `edit_code(action="insert", symbol="taxonomy_recipes/FIXTURE", position="before", …)`, then `./scripts/fmt-mine.sh`):

```rust
    const CALL_OPENER: &str = "doc(action=\"append_entry\"";
    const SECTION: &str = "## Main taxonomy";

    #[derive(Debug, Clone, PartialEq)]
    enum Shape {
        Prose,
        Params { collection: String },
        /// The F-N row: `<topic>` in its target stands for every session log. The W-N row has
        /// no call of its own ("Same"), so the template check requires W alongside F.
        Template,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Recipe {
        line: usize,
        label: String,
        id_prefix: String,
        target: String,
        shape: Shape,
    }

    #[derive(Debug, Default)]
    struct Scan {
        recipes: Vec<Recipe>,
        unparseable: Vec<String>,
        non_recipe_rows: Vec<String>,
        rows_seen: usize,
    }

    fn at(line: usize, label: &str) -> String {
        format!("docs/TAXONOMY.md:{line} ({label})")
    }

    /// Argument text of the call whose `(` is at byte `open`; `None` if it never closes.
    /// Tracks double-quoted strings (with `\` escapes) and `([{` depth.
    fn call_args(text: &str, open: usize) -> Option<&str> {
        let bytes = text.as_bytes();
        let (mut depth, mut in_str, mut i) = (0usize, false, open);
        while i < bytes.len() {
            let b = bytes[i];
            if in_str {
                match b {
                    b'\\' => i += 1,
                    b'"' => in_str = false,
                    _ => {}
                }
            } else {
                match b {
                    b'"' => in_str = true,
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => {
                        depth = depth.checked_sub(1)?;
                        if depth == 0 {
                            if b == b')' {
                                return Some(&text[open + 1..i]);
                            }
                            return None;
                        }
                    }
                    _ => {}
                }
            }
            i += 1;
        }
        None
    }

    /// Value of `key="…"` at depth 0 of `args`, outside strings, at a word boundary.
    fn top_level_arg(args: &str, key: &str) -> Option<String> {
        let bytes = args.as_bytes();
        let needle = format!("{key}=\"");
        let (mut depth, mut in_str, mut i) = (0usize, false, 0);
        while i < bytes.len() {
            let b = bytes[i];
            if in_str {
                match b {
                    b'\\' => i += 1,
                    b'"' => in_str = false,
                    _ => {}
                }
            } else if depth == 0
                && bytes[i..].starts_with(needle.as_bytes())
                && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_'))
            {
                let start = i + needle.len();
                let len = args[start..].find('"')?;
                return Some(args[start..start + len].to_string());
            } else {
                match b {
                    b'"' => in_str = true,
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                    _ => {}
                }
            }
            i += 1;
        }
        None
    }

    fn first_tracker_path(cell: &str) -> Option<String> {
        cell.split('`')
            .skip(1)
            .step_by(2)
            .find(|s| s.starts_with("docs/trackers/") && s.ends_with(".md"))
            .map(str::to_string)
    }

    fn parse_row(line: &str) -> Result<(String, Option<String>, String), String> {
        let mut calls: Vec<(String, Option<String>)> = Vec::new();
        for (open, _) in line.match_indices(CALL_OPENER) {
            let args = call_args(line, open + "doc".len())
                .ok_or("the append_entry call never closes its `(`")?;
            let prefix = top_level_arg(args, "id_prefix")
                .ok_or("the append_entry call passes no id_prefix=\"…\"")?;
            calls.push((prefix, top_level_arg(args, "entry_collection")));
        }
        if calls.windows(2).any(|w| w[0] != w[1]) {
            return Err(format!("the row holds append_entry calls that disagree: {calls:?}"));
        }
        // Only the Lives-in cell (the second) is split out: it never holds a pipe, while the
        // append-tool cell can hold unescaped ones inside a code span.
        let lives_in = line.split(" | ").nth(1).unwrap_or_default();
        let target = first_tracker_path(lives_in)
            .ok_or("the Lives-in cell names no backticked docs/trackers/…md target")?;
        let (prefix, collection) = calls.swap_remove(0);
        Ok((prefix, collection, target))
    }

    fn scan_main_taxonomy(text: &str) -> Scan {
        let mut scan = Scan::default();
        let mut fence = FenceState::new();
        let mut in_section = false;
        for (idx, line) in text.lines().enumerate() {
            if fence.feed(line) || fence.in_fence() {
                continue;
            }
            if line.starts_with("## ") {
                in_section = line.trim_end() == SECTION;
                continue;
            }
            if !in_section || !line.starts_with("| **") {
                continue;
            }
            scan.rows_seen += 1;
            let label = line["| **".len()..]
                .split(['*', '|'])
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
            if !line.contains(CALL_OPENER) {
                scan.non_recipe_rows.push(label);
                continue;
            }
            match parse_row(line) {
                Ok((id_prefix, collection, target)) => {
                    let shape = match collection {
                        Some(collection) => Shape::Params { collection },
                        None if target.contains("<topic>") => Shape::Template,
                        None => Shape::Prose,
                    };
                    scan.recipes.push(Recipe { line: idx + 1, label, id_prefix, target, shape });
                }
                Err(why) => scan.unparseable.push(format!("{}: {why}", at(idx + 1, &label))),
            }
        }
        scan
    }
```

- [ ] **Step 6: Run to verify it passes**

Run: `scripts/with-slot.sh cargo test --lib taxonomy_recipes`
Expected: `test result: ok. 9 passed; 0 failed`, and all nine names appear under `librarian::tools::append_entry::taxonomy_recipes::`.

- [ ] **Step 7: Lint**

Run: `scripts/with-slot.sh cargo clippy --workspace --all-targets --features local-embed -- -D warnings`
Expected: exit 0. A `dead_code` warning on any Task 1 item means a test does not reach it — fix by asserting on it, not by `#[allow]`.

- [ ] **Step 8: Commit** (three separate calls). The DWF tracker goes in only if its `git diff` holds nothing but your entry.

```bash
git add -- src/librarian/tools/append_entry.rs
git diff --cached --name-only
git commit -m "test(librarian): scanner for TAXONOMY append_entry recipes

Parses docs/TAXONOMY.md's Main taxonomy rows into prose / params /
template recipes without splitting cells on '|' (the F row holds
unescaped pipes in a code span); the target is read only from the
Lives-in cell. Unparseable rows are recorded, never skipped. Checks
follow in the next commit. Residual 5820a75840dd2d52." -- src/librarian/tools/append_entry.rs
```

---

### Task 2: The three checks and the corpus test

**Files:**
- Modify: `src/librarian/tools/append_entry.rs` (inside `mod taxonomy_recipes`)

**Interfaces:**
- Consumes (Task 1): `Shape`, `Recipe`, `Scan`, `at`, `scan_main_taxonomy`, `SECTION`.
- Produces: `const TEMPLATE_EXEMPT: &[&str]`, `impl Recipe { fn at(&self) -> String }`, `fn read_fm(path: &Path) -> Result<Option<Frontmatter>, String>`, `fn check_prose(root: &Path, r: &Recipe) -> Option<String>`, `fn check_params(root: &Path, r: &Recipe, collection: &str) -> Option<String>`, `fn check_template(root: &Path, r: &Recipe, exempt: &[&str]) -> Vec<String>`, test `every_taxonomy_append_entry_recipe_is_one_the_code_accepts`.

- [ ] **Step 1: Write the failing check tests.** Insert after `taxonomy_recipes/crlf_scans_like_lf` (`edit_code(action="insert", symbol="taxonomy_recipes/crlf_scans_like_lf", position="after", …)`, then `./scripts/fmt-mine.sh`):

```rust
    fn recipe(target: &str, id_prefix: &str, shape: Shape) -> Recipe {
        Recipe { line: 1, label: "X-N".into(), id_prefix: id_prefix.into(), target: target.into(), shape }
    }

    fn put(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn prose_check_mirrors_both_allocator_refusals() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        put(root, "docs/trackers/ok.md", "---\nkind: tracker\nentry_prefix: R\n---\n# ok\n");
        put(root, "docs/trackers/none.md", "---\nkind: tracker\n---\n# none\n");
        put(root, "docs/trackers/other.md", "---\nkind: tracker\nentry_prefix: Q\n---\n# other\n");
        assert_eq!(check_prose(root, &recipe("docs/trackers/ok.md", "R", Shape::Prose)), None);
        let none = check_prose(root, &recipe("docs/trackers/none.md", "R", Shape::Prose)).unwrap();
        assert!(none.contains("does not declare an entry_prefix"), "{none}");
        let other = check_prose(root, &recipe("docs/trackers/other.md", "R", Shape::Prose)).unwrap();
        assert!(other.contains("is not declared by this ledger") && other.contains("declares Q"), "{other}");
        let gone = check_prose(root, &recipe("docs/trackers/gone.md", "R", Shape::Prose)).unwrap();
        assert!(gone.contains("archived or moved"), "{gone}");
    }

    #[test]
    fn params_check_reads_the_committed_sidecar_not_entry_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // No entry_prefix on purpose: the params path never reads one (bug-fix-session-log:F-176).
        put(root, "docs/trackers/p.md", "---\nkind: tracker\nexpects_augmentation: docs/augmentations/p.yaml\n---\n# p\n");
        put(root, "docs/augmentations/p.yaml", "prompt: \"p\"\nentry_collection: issues\n");
        put(root, "docs/trackers/bare.md", "---\nkind: tracker\n---\n# bare\n");
        put(root, "docs/trackers/yes.md", "---\nkind: tracker\nexpects_augmentation: true\n---\n# yes\n");
        let p = |c: &str| recipe("docs/trackers/p.md", "P", Shape::Params { collection: c.into() });
        assert_eq!(check_params(root, &p("issues"), "issues"), None);
        let wrong = check_params(root, &p("items"), "items").unwrap();
        assert!(wrong.contains("declares Some(\"issues\")"), "{wrong}");
        let bare = check_params(root, &recipe("docs/trackers/bare.md", "P", Shape::Prose), "issues").unwrap();
        assert!(bare.contains("declares no `expects_augmentation` sidecar"), "{bare}");
        let yes = check_params(root, &recipe("docs/trackers/yes.md", "P", Shape::Prose), "issues").unwrap();
        assert!(yes.contains("names no committed sidecar"), "{yes}");
    }

    #[test]
    fn template_check_enforces_f_and_w_with_a_shrink_only_exemption_list() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        put(root, "docs/trackers/a-session-log.md", "---\nentry_prefix: [F, W]\n---\n");
        put(root, "docs/trackers/b-session-log.md", "---\nkind: tracker\n---\n");
        put(root, "docs/trackers/c-session-log.md", "---\nentry_prefix: F\n---\n");
        put(root, "docs/trackers/d-session-log.md", "---\nentry_prefix: [F, W]\n---\n");
        let r = recipe("docs/trackers/<topic>-session-log.md", "F", Shape::Template);
        let out = check_template(root, &r, &["d-session-log.md", "gone-session-log.md"]);
        let joined = out.join("\n");
        assert!(!joined.contains("a-session-log.md"), "a declares [F, W]: {joined}");
        assert!(
            joined.contains("b-session-log.md` declares no entry_prefix") && joined.contains("If this log is YOURS"),
            "{joined}"
        );
        assert!(joined.contains("c-session-log.md` declares F —"), "{joined}");
        assert!(joined.contains("d-session-log.md` now declares F, W"), "{joined}");
        assert!(joined.contains("gone-session-log.md`, which no longer exists"), "{joined}");
        assert_eq!(out.len(), 4, "{joined}");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `scripts/with-slot.sh cargo test --lib taxonomy_recipes`
Expected: compile error — `cannot find function check_prose` / `check_params` / `check_template`, `cannot find type Path`.

- [ ] **Step 3: Implement the checks and the corpus test.** Replace the module's single `use` line with the block below, and insert the items after `taxonomy_recipes/scan_main_taxonomy` (`edit_code(action="insert", symbol="taxonomy_recipes/scan_main_taxonomy", position="after", …)`; `edit_file` for the `use` lines). Then `./scripts/fmt-mine.sh`.

```rust
    use crate::librarian::catalog::augmentation::declared_prefixes_from_frontmatter;
    use crate::librarian::frontmatter::{self, Frontmatter};
    use crate::librarian::tools::doctor::{parse_declaration, Declaration};
    use crate::util::librarian_guard::is_citable_entry_prefix;
    use crate::util::markdown_fence::FenceState;
    use std::path::{Path, PathBuf};
```

```rust
    /// Session logs that declared no `entry_prefix` when this gate landed (2026-09-27), so the
    /// F-N recipe is refused there. SHRINK-ONLY: declare `entry_prefix: [F, W]` in one, then
    /// delete its line; the test reds until you do.
    const TEMPLATE_EXEMPT: &[&str] = &[
        "local-onnx-embedding-session-log.md",
        "pr-review-session-log.md",
        "release-promotion-session-log.md",
        "structural-edit-gate-session-log.md",
        "worktree-semantic-search-session-log.md",
    ];

    impl Recipe {
        fn at(&self) -> String {
            at(self.line, &self.label)
        }
    }

    fn read_fm(path: &Path) -> Result<Option<Frontmatter>, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("cannot be read ({e})"))?;
        frontmatter::parse(&text)
            .map(|(fm, _)| fm)
            .map_err(|e| format!("has frontmatter that does not parse ({e})"))
    }

    /// Mirrors `allocate_entry_id`, the only reader of the declaration: the prose path refuses
    /// an empty declared set, and one lacking the recipe's prefix.
    fn check_prose(root: &Path, r: &Recipe) -> Option<String> {
        let fm = match read_fm(&root.join(&r.target)) {
            Ok(fm) => fm,
            Err(e) => {
                return Some(format!(
                    "{}: routes {}-N writes to `{}`, which {e} — archived or moved? Update the row.",
                    r.at(),
                    r.id_prefix,
                    r.target
                ))
            }
        };
        let declared = declared_prefixes_from_frontmatter(fm.as_ref());
        if declared.contains(&r.id_prefix) {
            return None;
        }
        let refusal = if declared.is_empty() {
            format!("allocate_entry_id: `{}` does not declare an entry_prefix", r.target)
        } else {
            format!(
                "allocate_entry_id: `{}` is not declared by this ledger (it declares {})",
                r.id_prefix,
                declared.join(", ")
            )
        };
        Some(format!(
            "{}: prose recipe for id_prefix=\"{p}\" is refused — {refusal}. Repair ONE side: declare \
             it (doc(action=\"update\", id=<artifact id of {t}>, patch={{extra: {{\"entry_prefix\": \
             \"{p}\"}}}})), or correct the TAXONOMY row if the recipe is what is wrong.",
            r.at(),
            p = r.id_prefix,
            t = r.target
        ))
    }

    /// The params path checks no declaration; it refuses when no augmentation declares the
    /// collection. Offline, the committed sidecar is what a fresh clone re-attaches.
    fn check_params(root: &Path, r: &Recipe, collection: &str) -> Option<String> {
        let fm = match read_fm(&root.join(&r.target)) {
            Ok(fm) => fm.unwrap_or_default(),
            Err(e) => return Some(format!("{}: routes writes to `{}`, which {e}.", r.at(), r.target)),
        };
        let missing = |what: &str| {
            format!(
                "{}: params recipe (entry_collection=\"{collection}\") targets `{}`, which {what} — on a \
                 fresh clone no augmentation re-attaches, so append_entry refuses it. Export the shape: \
                 librarian(action=\"doctor\", fix=\"export_augmentations\") (a dry run; then confirm=true).",
                r.at(),
                r.target
            )
        };
        let sidecar = match fm.extra.get("expects_augmentation").map(parse_declaration) {
            Some(Declaration::Declared { sidecar: Some(rel) }) => rel,
            Some(Declaration::Declared { sidecar: None }) => {
                return Some(missing("declares `expects_augmentation: true` but names no committed sidecar"))
            }
            Some(Declaration::Unparseable) => {
                return Some(missing("carries an `expects_augmentation` value that declares nothing"))
            }
            Some(Declaration::Absent) | None => {
                return Some(missing("declares no `expects_augmentation` sidecar"))
            }
        };
        match crate::librarian::augmentation_sidecar::read(&root.join(&sidecar)) {
            Err(e) => Some(format!("{}: sidecar `{sidecar}` does not read: {e:#}", r.at())),
            Ok(s) if s.entry_collection.as_deref() == Some(collection) => None,
            Ok(s) => Some(format!(
                "{}: names entry_collection=\"{collection}\", but `{sidecar}` declares {:?} — append_entry \
                 refuses a collection the augmentation does not declare. Repair ONE side: the TAXONOMY \
                 row, or the augmentation.",
                r.at(),
                s.entry_collection
            )),
        }
    }

    fn check_template(root: &Path, r: &Recipe, exempt: &[&str]) -> Vec<String> {
        let Some((dir, suffix)) = r.target.split_once("<topic>") else {
            return vec![format!("{}: template target `{}` has no `<topic>`", r.at(), r.target)];
        };
        let dir_path = root.join(dir);
        let mut names: Vec<String> = match std::fs::read_dir(&dir_path) {
            Ok(entries) => entries
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.ends_with(suffix))
                .collect(),
            Err(e) => return vec![format!("{}: cannot list `{dir}`: {e}", r.at())],
        };
        names.sort();
        let mut out = Vec::new();
        if names.is_empty() {
            out.push(format!("{}: `{}` matches no file — the template shape went vacuous", r.at(), r.target));
        }
        for name in &names {
            let rel = format!("{dir}{name}");
            let declared = match read_fm(&dir_path.join(name)) {
                Ok(fm) => declared_prefixes_from_frontmatter(fm.as_ref()),
                Err(e) => {
                    out.push(format!("{}: `{rel}` {e}", r.at()));
                    continue;
                }
            };
            let is_exempt = exempt.contains(&name.as_str());
            let has_both = ["F", "W"].iter().all(|p| declared.iter().any(|d| d == p));
            match (is_exempt, declared.is_empty(), has_both) {
                (true, true, _) | (false, _, true) => {}
                (true, false, _) => out.push(format!(
                    "`{rel}` now declares {} — delete its line from TEMPLATE_EXEMPT \
                     (src/librarian/tools/append_entry.rs); the list only shrinks.",
                    declared.join(", ")
                )),
                (false, true, _) => out.push(format!(
                    "{}: `{rel}` declares no entry_prefix, so the F-N recipe is refused there. If this log \
                     is YOURS: declare it before appending, as docs/templates/session-log.md says \
                     (doc(action=\"update\", id=<its artifact id>, patch={{extra: {{\"entry_prefix\": \
                     [\"F\", \"W\"]}}}})). If it is NOT yours, it is likely a peer's log in progress — this \
                     test reads the working tree: attribute it with scripts/file-provenance.py and ask \
                     them; do not declare it for them.",
                    r.at()
                )),
                (false, false, false) => out.push(format!(
                    "{}: `{rel}` declares {} — a session log owns both F and W (the W-N row is \"Same\").",
                    r.at(),
                    declared.join(", ")
                )),
            }
        }
        for ex in exempt {
            if !names.iter().any(|n| n == ex) {
                out.push(format!("TEMPLATE_EXEMPT names `{dir}{ex}`, which no longer exists — delete the line."));
            }
        }
        out
    }

    /// Asserts the CODE contract per recipe shape. Params recipes are deliberately NOT required
    /// to declare `entry_prefix` — only the prose path reads it (bug-fix-session-log:F-176).
    /// Covers TAXONOMY only: CLAUDE.md, sidecar prompts and ledger templates also route writers
    /// to append_entry and are not read here.
    #[test]
    fn every_taxonomy_append_entry_recipe_is_one_the_code_accepts() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(root.join("docs/TAXONOMY.md"))
            .expect("docs/TAXONOMY.md is the surface under test");
        let scan = scan_main_taxonomy(&text);

        let count = |want: fn(&Shape) -> bool| scan.recipes.iter().filter(|r| want(&r.shape)).count();
        let prose = count(|s| matches!(s, Shape::Prose));
        let params = count(|s| matches!(s, Shape::Params { .. }));
        let template = count(|s| matches!(s, Shape::Template));
        let population = format!(
            "examined {} rows under `{SECTION}`: {prose} prose, {params} params, {template} template \
             recipe(s); rows with no recipe: {:?}",
            scan.rows_seen, scan.non_recipe_rows
        );
        assert!(
            prose > 0 && params > 0 && template > 0,
            "a recipe shape is missing — the scanner lost it or the section moved; this is not a \
             clean corpus. {population}"
        );

        let mut failures = scan.unparseable.clone();
        for r in &scan.recipes {
            if !is_citable_entry_prefix(&r.id_prefix) {
                failures.push(format!(
                    "{}: id_prefix=\"{}\" is refused by append_entry before either branch — an entry \
                     token is `[A-Z]{{1,3}}-<n>`. Correct the TAXONOMY row.",
                    r.at(),
                    r.id_prefix
                ));
                continue;
            }
            match &r.shape {
                Shape::Prose => failures.extend(check_prose(&root, r)),
                Shape::Params { collection } => failures.extend(check_params(&root, r, collection)),
                Shape::Template => failures.extend(check_template(&root, r, TEMPLATE_EXEMPT)),
            }
        }
        assert!(
            failures.is_empty(),
            "{} TAXONOMY append_entry recipe finding(s) — each would be refused, or the gate cannot \
             read it:\n  {}\n\n{population}",
            failures.len(),
            failures.join("\n  ")
        );
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `scripts/with-slot.sh cargo test --lib taxonomy_recipes`
Expected: `test result: ok. 13 passed; 0 failed`. The corpus test passes on first run because the corpus is clean today (spec § *Facts*) — its observed reds come in Task 3, not here.

- [ ] **Step 5: Lint**

Run: `scripts/with-slot.sh cargo clippy --workspace --all-targets --features local-embed -- -D warnings`
Expected: exit 0.

- [ ] **Step 6: Commit** (three separate calls)

```bash
git add -- src/librarian/tools/append_entry.rs
git diff --cached --name-only
git commit -m "test(librarian): TAXONOMY append_entry recipes must be ones the code accepts

Prose recipe => id_prefix declared (allocate_entry_id's two refusals);
params recipe => committed sidecar declares the entry_collection, and
entry_prefix is deliberately not asserted (bug-fix-session-log:F-176);
F template => every session log declares [F, W] except a shrink-only
exemption list of five. Production readers throughout. Residual
5820a75840dd2d52." -- src/librarian/tools/append_entry.rs
```

---

### Task 3: One observed red per guarded site

**Files:** none modified in the shared tree. Every mutation runs in `mutation-probe.sh`'s isolated worktree.

**Interfaces:**
- Consumes (Task 2): `every_taxonomy_append_entry_recipe_is_one_the_code_accepts` and the messages it emits.
- Produces: a verdict table pasted into the bug file's `## Tests added` in Task 4.

Each run: `./scripts/mutation-probe.sh --file <F> --find <X> --replace <Y> -- cargo test --lib taxonomy_recipes::every_taxonomy_append_entry_recipe_is_one_the_code_accepts`. A row passes only when the verdict is `KILLED (… 1 test(s) ran)` **and** the panic text contains the expected substring — the substring is what attributes the kill to the right guard. `SURVIVED` or `INCONCLUSIVE` stops the task: report it, do not weaken the assertion. The script refuses a `--find` that is not exactly once; every literal below was verified unique on 2026-09-27, so a refusal means the file moved — re-read it rather than guessing a new literal.

- [ ] **Step 1: prose, nothing declared**
`--file docs/trackers/reconnaissance-patterns.md --find 'entry_prefix: R' --replace ''`
Expect: `does not declare an entry_prefix` on the `R-N` line.

- [ ] **Step 2: prose, declared but not the recipe's prefix**
`--file docs/trackers/operator-rules.md --find 'entry_prefix: OP' --replace 'entry_prefix: OQ'`
Expect: `` `OP` is not declared by this ledger (it declares OQ) ``.

- [ ] **Step 3: params, collection mismatch**
`--file docs/augmentations/docs-trackers-windows-platform-support.yaml --find 'entry_collection: issues' --replace 'entry_collection: issuez'`
Expect: `declares Some("issuez")` on the `WIN-N` line.

- [ ] **Step 4: params, no sidecar declared**
`--file docs/trackers/provenance-subsystem.md --find 'expects_augmentation: docs/augmentations/docs-trackers-provenance-subsystem.yaml' --replace ''`
Expect: ``declares no `expects_augmentation` sidecar`` on the `PV-N` line.

- [ ] **Step 5: template, declared log missing W**
`--file docs/trackers/response-envelope-session-log.md --find 'entry_prefix: ["F", "W"]' --replace 'entry_prefix: ["F"]'`
Expect: `` response-envelope-session-log.md` declares F — ``.

- [ ] **Step 6: template, stale exemption**
`--file docs/trackers/pr-review-session-log.md --find 'topic: pr-review' --replace "$(printf 'topic: pr-review\nentry_prefix: [F, W]')"`
Expect: `` pr-review-session-log.md` now declares F, W ``.

- [ ] **Step 7: template, undeclared non-exempt log** (same guard input as a brand-new log)
`--file src/librarian/tools/append_entry.rs --find '"pr-review-session-log.md",' --replace ''`
Expect: `` pr-review-session-log.md` declares no entry_prefix `` and `If this log is YOURS`.

- [ ] **Step 8: unparseable row**
`--file docs/TAXONOMY.md --find 'id_prefix="OB"' --replace 'idprefix="OB"'`
Expect: `passes no id_prefix` on the `OB-N` line.

- [ ] **Step 9: citable, on a params row** (a prose input cannot show this guard is load-bearing: `declared_prefixes_from_frontmatter` already drops a non-citable prefix, so with the citable guard deleted the prose check would still refuse it and the mutation would prove nothing about the guard; a params row is refused by this guard alone)
`--file docs/TAXONOMY.md --find 'id_prefix="WIN"' --replace 'id_prefix="WINX"'`
Expect: `id_prefix="WINX" is refused by append_entry before either branch`, and no `WIN-N` params finding.

- [ ] **Step 10: the production reader is really called**
`--file src/librarian/catalog/augmentation.rs --find '.filter(|p| crate::util::librarian_guard::is_citable_entry_prefix(p))' --replace '.filter(|_| false)'`
Expect: `does not declare an entry_prefix` findings.

- [ ] **Step 11: non-vacuity**
`--file src/librarian/tools/append_entry.rs --find 'top_level_arg(args, "entry_collection")' --replace 'top_level_arg(args, "entry_collectionX")'`
Expect: `a recipe shape is missing` (params recipes collapse into prose, and the shape assert fires first).

- [ ] **Step 12: Confirm the shared tree is untouched**

Run: `git status --porcelain -- docs/TAXONOMY.md docs/trackers docs/augmentations src/librarian`
Expected: only the paths you changed deliberately in Tasks 1–2 and your own bug-file claim, and none of the eleven mutated files. Any mutated path showing modified means the probe did not isolate — stop and report.

---

### Task 4: Follow-ups, gate, close-out

**Files:**
- Create (via `doc`): two follow-up bug files under `docs/issues/`
- Modify (via `doc`): the residual bug file, the spec (citation repoint), `docs/trackers/deep-agent-workflow-observations.md`

**Interfaces:**
- Consumes: Task 3's verdict table; the Task 2 commit SHA.

- [ ] **Step 1: File the uncovered-surfaces follow-up**

```
doc(action="create", kind="bug", status="open",
    rel_path="docs/issues/2026-09-27-append-entry-recipes-outside-taxonomy-are-unchecked.md",
    title="BUG: append_entry recipes outside TAXONOMY are not checked by the recipe gate",
    tags=["cluster/selector-narrower-than-its-population"],
    body="## Summary\n\nThe TAXONOMY recipe gate (`every_taxonomy_append_entry_recipe_is_one_the_code_accepts`) reads only the Main taxonomy table. Three other surfaces also route writers to `append_entry` and are unchecked: CLAUDE.md, committed augmentation-sidecar prompts under `docs/augmentations/`, and each ledger's own template section. The parent of residual `5820a75840dd2d52` found a sidecar prompt instructing a refused call.\n\n## Fix\n\nNot started. Reuse the gate's scanner and checks with a second input per surface; the sidecar prompts are YAML strings, not table rows.\n\n## References\n\n- `docs/superpowers/specs/2026-09-27-taxonomy-append-recipes-test-design.md`")
```

- [ ] **Step 2: File the template follow-up**

```
doc(action="create", kind="bug", status="open",
    rel_path="docs/issues/2026-09-27-session-log-template-recipe-is-refused-on-a-fresh-copy.md",
    title="BUG: the session-log template's own append_entry recipe is refused on a fresh copy",
    tags=["cluster/doc-contradicted-by-code"],
    body="## Summary\n\n`docs/templates/session-log.md` ships without frontmatter and prescribes `doc(action=\"append_entry\", …, id_prefix=\"F\")`. On a fresh copy that call is refused (`does not declare an entry_prefix`) until the writer declares `entry_prefix: [F, W]`. Five session logs in this repo never took that step and sit on the recipe gate's shrink-only exemption list.\n\n## Fix\n\nNot started, and a decision rather than a patch: shipping the template declared closes the window but ends its deliberate 'directly editable until declared' property, which the reconnaissance skill also states.\n\n## References\n\n- `docs/superpowers/specs/2026-09-27-taxonomy-append-recipes-test-design.md`")
```

- [ ] **Step 3: Run the full gate**

Run: `./scripts/gate.sh` (background it and wait for `GATE EXITS`).
Expected: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, and `grep -E 'taxonomy_recipes::' <default-lane output>` lists all 13 test names as `ok`.

- [ ] **Step 4: Write the residual's Fix and Tests sections**

```
doc(action="update", id="5820a75840dd2d52", patch={"body_edits": [
  {"heading": "## Fix", "action": "replace", "content": "<what shipped: the module, the code contract per shape, the five exemptions; Task 2 SHA; the two follow-up files>"},
  {"heading": "## References", "action": "insert_before", "content": "## Tests added\n\n<the Task 3 verdict table: step, file, mutation, verdict, matched substring>\n"}]})
```

- [ ] **Step 5: Record SHA and patch-id, close**

```bash
git show <task-2-sha> > <scratchpad>/fix.patch
git patch-id --stable < <scratchpad>/fix.patch
```

```
doc(action="update", id="5820a75840dd2d52", patch={"status": "fixed", "extra": {"claimed_by": null, "claimed_at": null, "closed": "<today>", "fix_sha": "<sha>", "fix_patch_id": "<patch-id>"}})
```

- [ ] **Step 6: Commit the bookkeeping** (three separate calls; include the DWF tracker only if its diff is yours alone)

```bash
git add -- docs/issues/2026-09-24-residual-taxonomy-recipes-declare-entry-prefix-test.md docs/issues/2026-09-27-append-entry-recipes-outside-taxonomy-are-unchecked.md docs/issues/2026-09-27-session-log-template-recipe-is-refused-on-a-fresh-copy.md
git diff --cached --name-only
git commit -m "docs(issues): close the TAXONOMY recipe residual; file its two follow-ups" -- docs/issues/2026-09-24-residual-taxonomy-recipes-declare-entry-prefix-test.md docs/issues/2026-09-27-append-entry-recipes-outside-taxonomy-are-unchecked.md docs/issues/2026-09-27-session-log-template-recipe-is-refused-on-a-fresh-copy.md
```

- [ ] **Step 7: Archive and repoint**

```
doc(action="move", id="5820a75840dd2d52", new_rel_path="docs/issues/archive/2026-09-24-residual-taxonomy-recipes-declare-entry-prefix-test.md")
```

Read the response's `inbound_path_citations` minus `inbound_citations_cleared`, and repoint each remaining citer to the archive path — at minimum the spec's Goal line. Stage both halves of the move (`git add -- <old> <new>`, confirm column-1 letters in `git status --short`) plus the repointed files, check `git diff --cached --name-only`, then:

```bash
git commit -m "docs(issues): archive the TAXONOMY recipe residual; repoint its citations" -- <old path> <new path> <each repointed file>
```

- [ ] **Step 8: Close the observation sample and the session receipt**

Update the Task 1 DWF entry with a dated outcome paragraph (commits, the Task 3 table, gate result) via heading-scoped `doc(action="update", id="1b770cb6462acde6", patch={"body_edits": [...]})`. Then append a DCS coverage receipt with the same `append_entry` shape and `id_prefix="DCS"`, recording that this session's first eligible task — the `long_docs()` gate extension, commits `b990f177` and `a43b467e` — was NOT captured pre-action (capture gap), and naming the DWF id selected here.
