# Ledger-Aware Librarian Guard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Teach `librarian_guard` the `entry_prefix` ledger declaration, so a prose ledger's `PREFIX-N` entry headings can only be written through the server's id allocator — while every other heading in that same file stays directly editable.

**Architecture:** Two changes to `src/util/librarian_guard.rs`, in order. Task 1 adds a third predicate to the guard's existing union (stamped `id:` OR augmented OR **declares `entry_prefix`**), which closes the hole for any newly-created prose ledger. Task 2 then narrows that third arm alone from whole-file to heading-structural: `edit_markdown` refuses only edits whose target heading matches `^<PREFIX>-\d+`, so a typo fix elsewhere in a 3,000-line ledger is not a ceremony. The `id:` and augmented arms keep their whole-file semantics untouched.

**Tech Stack:** Rust, `regex`, `cargo test --lib`. No new dependencies.

**Spec:** `docs/issues/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md` (artifact `88129ecc9c4c87a2`) — specifically its `## Status` → *"The shape the fix should take"* section. That bug file already carries one retraction; read the whole `## Status` section before starting, not just the summary.

---

## Global Constraints

These are corrections and hard limits established by reading the current code on `experiments` @ `dd788ce1`. Three of them contradict the spec's own prose — the spec was written 2026-08-17 and the substrate moved under it.

- **Spec pieces 1 and 2 are ALREADY SHIPPED. Do not re-implement them.** `ENTRY_PREFIX_KEY = "entry_prefix"` exists (`src/librarian/catalog/augmentation.rs:705`), `entry_high_water_<PREFIX>` exists (`ENTRY_HIGH_WATER_PREFIX`, same file :717), and `allocate_entry_id` (:762) is wired to the MCP surface at `src/librarian/tools/append_entry.rs:91` on the prose path (`entry_collection` omitted). The spec says "NOT yet called from any MCP tool" — that is stale. This plan implements only the spec's piece 3.
- **The guard MUST NOT call `crate::librarian::frontmatter::parse`.** `librarian` is a Cargo feature (`Cargo.toml [features]`, `src/lib.rs:32-33` gates `pub mod librarian` behind `#[cfg(feature = "librarian")]`). `src/util/librarian_guard.rs` compiles under `--no-default-features`, and its own doc comment promises it "degrades to its frontmatter check" there. Parse `entry_prefix` from raw text by hand, exactly as `is_librarian_artifact` hand-parses `id:`. This is why that function hand-rolls its parsing rather than reusing a YAML parser.
- **Drop the "declared index heading" half of the spec's piece 3.** There is no machine-readable index-heading declaration anywhere in the codebase (`grep 'index_heading|entry_index|ENTRY_INDEX' src/**/*.rs` → 0 matches), and `get_guide("tracker-conventions")` states "A hand-written index is never the answer." Guard entry headings only.
- **`a_catalogued_but_unaugmented_file_stays_directly_editable` (`src/util/librarian_guard.rs:332`) must stay green, unmodified.** It pins that `docs/trackers/skill-frictions.md` (a catalog row, no `id:`, no `entry_prefix`, no augmentation) and `docs/RELEASE.md` stay directly editable. The spec's earlier retracted advice would have broken exactly this; an earlier attempt to act on it stamped `id:` into a ledger and had to be reverted in `bb9a94d7`.
- **Task 2 must NOT relax augmented files.** An augmented artifact's params live in the catalog and the file is only a rendered snapshot, so even a prose-only edit desynchronises it. `artifact(action="update", patch={body_edits: [...]})` is the correct tool there and is not a ceremony. Only the new ledger arm from Task 1 gets heading-structural treatment.
- **Entry-heading shape is `^(#{1,6})[ \t]+[`*\[]*<PREFIX>-(\d+)\b`.** Match `body_claimed_indices` (`src/librarian/catalog/augmentation.rs:1012`) on the heading half of its pattern, so the guard and the allocator agree on what an entry is. Do not also match index-table rows (`| F-12 |`) — `edit_markdown` addresses headings, not rows.
- **`entry_prefix` accepts three YAML forms**, all of which `allocate_entry_id` honours (:798-802) and the guard must too: scalar `entry_prefix: R`, block sequence (`entry_prefix:` then indented `- F` / `- W` lines), and inline flow `entry_prefix: [F, W]`.
- **Pre-commit gate on every task:** `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib` all green. A concurrent peer session shares this checkout — commit with `git commit --only <paths>`, never `git add -A`, and run `git status --short` first to confirm you are not staging their files.

---

## File Structure

| File | Responsibility | Change |
|---|---|---|
| `src/util/librarian_guard.rs` | The guard predicate and its decision. Owns `is_librarian_artifact`, `guard_with_oracle`, the oracle plumbing, and all guard tests. | Modify — add ledger parsing + a third union arm (Task 1), then a heading-scoped decision for that arm (Task 2). |
| `src/tools/markdown/edit_markdown.rs` | The `edit_markdown` tool. Calls the guard at :1271 with `(path, file_content, Some(&resolved))`. | Modify in Task 2 only — pass the call's target headings to the guard. |
| `src/tools/markdown/read_markdown.rs` | The `read_markdown` tool. Calls the guard at :507. | Modify in Task 2 only — pass `&[]` (a read addresses no heading for guard purposes). |
| `src/tools/edit_file/mod.rs` | Text-level editing. Calls the guard at :692. | Modify in Task 2 only — pass `&[]` (no heading concept). |

No new files. The guard is ~400 lines including tests and stays one focused unit; splitting it would separate the predicate from the tests that pin its rationale.

---

### Task 1: Guard recognises a declared ledger

Closes the hole: a prose ledger created the documented way — frontmatter `entry_prefix`, no augmentation, no stamped `id:` — is currently invisible to the guard, so its `PREFIX-N` headings can be hand-written, bypassing the allocator and leaving `entry_high_water_<PREFIX>` stale.

Verified end-to-end on `experiments` @ `dd788ce1` before this plan was written: a file with `entry_prefix: ZZ` / `entry_high_water_ZZ: 3` / no `id:` / not augmented accepted `edit_markdown(action="insert_after")` writing a `## ZZ-4` heading, and the high-water mark stayed at 3. After a later compaction lowers `body_max` back to 3, `allocate_entry_id`'s `next = max(body_max+1, reserved_max+1, frontmatter_max+1, 1)` reissues `ZZ-4` — re-arming the silent-citation-repoint bug closed in `docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md`.

All five ledgers that exist *today* are incidentally guarded (`capability-proposals.md` and `tracker-hygiene-log.md` by a stamped `id:`; `codescout-usage-frictions.md`, `codescout-usage-hookify.md` and `reconnaissance-patterns.md` by augmentation — the last two confirmed by live probe). So this task changes no current behaviour; it makes the protection principled instead of accidental, and covers the next ledger anyone creates.

**Files:**
- Modify: `src/util/librarian_guard.rs:115-128` (add a sibling to `is_librarian_artifact`)
- Modify: `src/util/librarian_guard.rs:82-111` (`guard_with_oracle` — third union arm)
- Test: `src/util/librarian_guard.rs` `mod tests` (same file, existing test module)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces, for Task 2:
  - `pub(crate) fn declared_entry_prefixes(text: &str) -> Vec<String>` — the prefixes a file's frontmatter declares, empty when it declares none.
  - `guard_with_oracle`'s ledger arm, which Task 2 narrows.

- [ ] **Step 1: Write the failing tests**

Add to the bottom of `mod tests` in `src/util/librarian_guard.rs`:

```rust
    /// A prose ledger declares its id namespace in frontmatter and needs no
    /// augmentation and no stamped id — `allocate_entry_id`'s own error hint says
    /// exactly that ("No augmentation and no entry_collection are needed"). So the
    /// documented way to create a ledger produced a file both existing predicates
    /// were blind to, and its `PREFIX-N` headings could be hand-written straight
    /// past the allocator.
    ///
    /// Verified before this test existed: a `entry_prefix: ZZ` file with
    /// `entry_high_water_ZZ: 3` accepted an `## ZZ-4` heading via `edit_markdown`
    /// and left the mark at 3, which is the input compaction later reads back to
    /// reissue `ZZ-4`.
    /// docs/issues/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md
    #[test]
    fn a_declared_ledger_is_guarded_with_no_id_and_no_augmentation() {
        struct NothingIsAugmented;
        impl AugmentedArtifactOracle for NothingIsAugmented {
            fn is_augmented(&self, _: &std::path::Path) -> bool {
                false
            }
        }

        let text = "---\nkind: tracker\nstatus: active\nentry_prefix: ZZ\nentry_high_water_ZZ: 3\n---\n\n## ZZ-3 — an entry\n";
        assert!(
            !is_librarian_artifact(text),
            "precondition: no stamped id for the text predicate to find"
        );

        let abs = std::path::Path::new("/repo/docs/trackers/probe-ledger.md");
        let err = guard_with_oracle(
            "docs/trackers/probe-ledger.md",
            text,
            Some(abs),
            Some(&NothingIsAugmented),
        )
        .expect_err("a declared ledger must be guarded");
        let re = err.downcast_ref::<RecoverableError>().unwrap();
        assert!(
            re.message.contains("librarian-managed artifact"),
            "got: {}",
            re.message
        );
    }

    /// A session log legitimately owns two namespaces (F-N frictions, W-N wins), and
    /// YAML serialises one key three different ways depending only on which writer
    /// last touched the file. `allocate_entry_id` honours all three
    /// (`src/librarian/catalog/augmentation.rs:798-802`); a guard that honoured
    /// fewer would make protection depend on that formatting accident — the same
    /// mistake the quoted-id bug made (BL-33).
    #[test]
    fn every_yaml_form_of_entry_prefix_is_recognised() {
        for (label, text, want) in [
            (
                "scalar",
                "---\nkind: tracker\nentry_prefix: R\n---\n\n# L\n",
                vec!["R"],
            ),
            (
                "block sequence",
                "---\nkind: tracker\nentry_prefix:\n  - F\n  - W\n---\n\n# L\n",
                vec!["F", "W"],
            ),
            (
                "inline flow",
                "---\nkind: tracker\nentry_prefix: [F, W]\n---\n\n# L\n",
                vec!["F", "W"],
            ),
            (
                "quoted scalar",
                "---\nkind: tracker\nentry_prefix: 'HY'\n---\n\n# L\n",
                vec!["HY"],
            ),
        ] {
            assert_eq!(
                declared_entry_prefixes(text),
                want.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                "{label}: entry_prefix must parse whatever YAML form it was written in"
            );
        }
    }

    /// The declaration is a FRONTMATTER fact. A tracker that merely discusses
    /// `entry_prefix` in its prose — every convention doc in `docs/` does — owns no
    /// namespace, and inferring one from body text would make every such doc a
    /// ledger. Same boundary `allocate_entry_id` draws by scanning `fm.extra`
    /// rather than the document.
    #[test]
    fn entry_prefix_outside_frontmatter_declares_nothing() {
        for (label, text) in [
            (
                "after the closing delimiter",
                "---\nkind: tracker\n---\n\n# Guide\n\nDeclare it with `entry_prefix: R`.\n",
            ),
            (
                "no frontmatter block at all",
                "# Guide\n\nentry_prefix: R\n",
            ),
        ] {
            assert!(
                declared_entry_prefixes(text).is_empty(),
                "{label}: only a frontmatter key declares a ledger"
            );
        }
    }

    /// An empty or valueless declaration is not a namespace. Returning a prefix here
    /// would make the guard build an entry regex with no prefix in it, which matches
    /// any numbered heading in any file.
    #[test]
    fn a_valueless_entry_prefix_declares_nothing() {
        for (label, text) in [
            ("bare key", "---\nkind: tracker\nentry_prefix:\n---\n\n# L\n"),
            (
                "empty string",
                "---\nkind: tracker\nentry_prefix: ''\n---\n\n# L\n",
            ),
            (
                "empty flow list",
                "---\nkind: tracker\nentry_prefix: []\n---\n\n# L\n",
            ),
        ] {
            assert!(
                declared_entry_prefixes(text).is_empty(),
                "{label}: a valueless declaration owns no namespace"
            );
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib util::librarian_guard 2>&1`

Expected: FAIL — `cannot find function 'declared_entry_prefixes' in this scope` (three tests), and `a_declared_ledger_is_guarded_with_no_id_and_no_augmentation` panics with `a declared ledger must be guarded: Ok(())`.

- [ ] **Step 3: Implement `declared_entry_prefixes`**

Insert immediately after `is_librarian_artifact` (which ends at `src/util/librarian_guard.rs:128`):

```rust
/// The id namespaces a file's frontmatter declares via `entry_prefix`, or empty
/// when it declares none.
///
/// A **ledger** — an artifact owning a `PREFIX-N` id namespace — is the third
/// reason a markdown file is librarian-managed, and it is orthogonal to the other
/// two. It carries no stamped `id:` (the catalog derives ids from the path and does
/// not need one) and needs no augmentation: `allocate_entry_id`'s own error hint
/// tells authors "No augmentation and no entry_collection are needed". So the
/// documented way to create a ledger produced a file both existing predicates were
/// blind to, and its entry headings could be hand-written straight past the
/// allocator — which is how the R-N ledger came to reuse nine ids.
///
/// **Hand-parsed, not `serde_yml`, on purpose.** `librarian` is a Cargo feature
/// (`src/lib.rs:32`), and this guard compiles and must keep working under
/// `--no-default-features`, where `crate::librarian::frontmatter` does not exist.
/// That is also why [`is_librarian_artifact`] hand-parses `id:`.
///
/// Accepts all three YAML forms `allocate_entry_id` honours
/// (`src/librarian/catalog/augmentation.rs:798-802`) — scalar, inline flow, and
/// block sequence — because quoting and flow style are properties of whichever
/// writer last emitted the file, never of the artifact. Making protection depend on
/// that accident is exactly the quoted-id defect (BL-33).
pub(crate) fn declared_entry_prefixes(text: &str) -> Vec<String> {
    let Some(rest) = text.strip_prefix("---\n") else {
        return Vec::new();
    };
    let mut lines = rest.lines();
    while let Some(line) = lines.next() {
        if line == "---" {
            return Vec::new();
        }
        let Some(val) = line.strip_prefix("entry_prefix:") else {
            continue;
        };
        let val = val.trim();
        // Inline flow: `entry_prefix: [F, W]`.
        if let Some(inner) = val.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            return inner.split(',').filter_map(clean_prefix).collect();
        }
        // Scalar: `entry_prefix: R`.
        if !val.is_empty() {
            return clean_prefix(val).into_iter().collect();
        }
        // Block sequence: the key's value is the indented `- F` lines that follow.
        // Stops at the first line that is not one, so a sibling key cannot be
        // swallowed as a list item.
        let mut out = Vec::new();
        for next in lines {
            if next == "---" {
                break;
            }
            let t = next.trim_start();
            let Some(item) = t.strip_prefix("- ") else {
                break;
            };
            if next.len() == t.len() {
                // Not indented — a top-level sequence, so this key is not its parent.
                break;
            }
            out.extend(clean_prefix(item));
        }
        return out;
    }
    Vec::new()
}

/// One declared prefix, unquoted and validated. `None` for anything that is not a
/// usable namespace — empty after trimming, or carrying characters the entry-token
/// grammar (`\b[A-Z]{1,3}-\d+\b`) cannot represent.
///
/// The validation is load-bearing rather than defensive: an empty prefix would let
/// the entry-heading regex match every numbered heading in the file.
fn clean_prefix(raw: &str) -> Option<String> {
    let p = strip_matching_quotes(raw.trim()).trim();
    let ok = !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_uppercase());
    ok.then(|| p.to_string())
}
```

- [ ] **Step 4: Add the ledger arm to `guard_with_oracle`**

In `src/util/librarian_guard.rs`, replace the decision block inside `guard_with_oracle` (currently the `let augmented = ...` line through the end of the `let why = ...` block) with:

```rust
    // Three independent reasons a file is off-limits, and none implies another: a
    // stamped frontmatter id says the librarian wrote this file; augmentation says
    // the file is not where its state lives; a declared `entry_prefix` says the file
    // owns an id namespace whose counter only the server may advance. An augmented
    // tracker can carry no id at all, a plain bug file carries one without being
    // augmented, and a prose ledger is routinely neither.
    let augmented = matches!((abs_path, oracle), (Some(p), Some(o)) if o.is_augmented(p));
    let ledger = !declared_entry_prefixes(text).is_empty();
    if !augmented && !ledger && !is_librarian_artifact(text) {
        return Ok(());
    }
    let why = if augmented {
        " (augmented — its params live in the catalog, and this file is only a \
         rendered snapshot of them)"
    } else if ledger {
        " (a ledger — it declares an entry_prefix, and its PREFIX-N ids are \
         allocated by the server)"
    } else {
        ""
    };
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --lib util::librarian_guard 2>&1`

Expected: PASS, all tests in the module, including the unmodified `a_catalogued_but_unaugmented_file_stays_directly_editable` and `an_augmented_artifact_is_guarded_even_with_no_frontmatter_id`.

- [ ] **Step 6: Mutation-test the new guard arm**

Temporarily change `let ledger = !declared_entry_prefixes(text).is_empty();` to `let ledger = false;`, then run:

Run: `cargo test --lib util::librarian_guard::tests::a_declared_ledger_is_guarded_with_no_id_and_no_augmentation 2>&1`

Expected: FAIL with `a declared ledger must be guarded: Ok(())`. Restore the line and re-run to confirm PASS. A test that cannot fail is not coverage.

- [ ] **Step 7: Run the full gate**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings 2>&1 && cargo test --lib 2>&1`

Expected: fmt silent, clippy clean, all `--lib` tests pass. If `src/util/path_security.rs` or other files show unrelated failures, check `git status --short` — a peer session shares this checkout, and its uncommitted work is not yours to fix.

- [ ] **Step 8: Commit**

```bash
git status --short
git commit --only src/util/librarian_guard.rs -m "fix(librarian-guard): recognise a declared entry_prefix ledger

A prose ledger declares its id namespace in frontmatter and needs neither a
stamped id: nor an augmentation -- allocate_entry_id's own hint says so. Both
existing predicates were blind to that shape, so the documented way to create
a ledger produced an unguarded file: verified a entry_prefix: ZZ file with
entry_high_water_ZZ: 3 accepting a hand-written ## ZZ-4 heading via
edit_markdown, leaving the mark at 3. Compaction later lowers body_max back to
3 and allocate_entry_id reissues ZZ-4, re-arming the silent citation repoint
closed in 2026-08-17-ledger-id-reissue-silently-repoints-citations.md.

All five ledgers that exist today were already incidentally guarded (two by a
stamped id, three by augmentation), so this changes no current behaviour -- it
makes the protection principled rather than accidental.

Hand-parses the frontmatter because librarian is a Cargo feature and this
guard must keep working under --no-default-features."
```

---

### Task 2: The ledger arm guards entry headings, not the whole file

Task 1 leaves a declared ledger wholly refused for `edit_markdown`. For an augmented file that is right — the file is a rendered snapshot, so any edit desynchronises it. For a ledger that is merely *declared*, it overshoots: the file IS where its state lives, and refusing a typo fix in `## The seven laws` of a 3,000-line ledger is the "ceremony" objection the spec raises against a whole-file guard.

`edit_markdown` is heading-addressed, so the guard can decide mechanically. Refuse when a target heading names an entry (`^#{1,6} <PREFIX>-<N>`), allow otherwise.

**Files:**
- Modify: `src/util/librarian_guard.rs` (new `headings` parameter; ledger-arm decision)
- Modify: `src/tools/markdown/edit_markdown.rs:1268-1273` (pass target headings)
- Modify: `src/tools/markdown/read_markdown.rs:507` and `src/tools/edit_file/mod.rs:692` (pass `&[]` — neither addresses headings)
- Test: `src/util/librarian_guard.rs` `mod tests`

**Interfaces:**
- Consumes from Task 1: `declared_entry_prefixes(text) -> Vec<String>`, and the `ledger` arm of `guard_with_oracle`.
- Produces:
  - `pub fn guard_not_librarian_managed(path: &str, text: &str, abs_path: Option<&std::path::Path>, headings: &[&str]) -> Result<(), anyhow::Error>` — one new trailing parameter. Pass `&[]` from any caller that does not address headings; `&[]` keeps the whole-file refusal, which is the safe default.
  - `pub(crate) fn targets_a_ledger_entry(text: &str, headings: &[&str]) -> bool`

- [ ] **Step 1: Write the failing tests**

Add to the bottom of `mod tests` in `src/util/librarian_guard.rs`:

```rust
    /// `edit_markdown` is heading-addressed, so the ledger arm can be precise: the
    /// thing that must go through the allocator is an ENTRY, and an entry is a
    /// heading naming `PREFIX-N`. Everything else in the file — the distillation
    /// section, the reading guide, a typo in prose — is ordinary markdown the author
    /// owns. Refusing all of it is the "ceremony" objection the spec raises against
    /// a whole-file guard, and it is what trains authors to route around the guard.
    #[test]
    fn a_ledger_refuses_entry_headings_and_permits_every_other_heading() {
        struct NothingIsAugmented;
        impl AugmentedArtifactOracle for NothingIsAugmented {
            fn is_augmented(&self, _: &std::path::Path) -> bool {
                false
            }
        }

        let text = "---\nkind: tracker\nentry_prefix: R\nentry_high_water_R: 104\n---\n\n## The seven laws\n\n## R-104 — a lesson\n";
        let abs = std::path::Path::new("/repo/docs/trackers/recon.md");
        let display = "docs/trackers/recon.md";

        // Refused: the heading names an entry, so writing it by hand would bypass
        // the allocator and leave entry_high_water_R stale.
        for heading in [
            "## R-105 — a new lesson",
            "## R-104 — a lesson",
            "#### R-7 — deep heading",
            "## `R-9` — backticked",
        ] {
            assert!(
                guard_with_oracle(display, text, Some(abs), Some(&NothingIsAugmented), &[heading])
                    .is_err(),
                "must refuse an entry heading: {heading}"
            );
        }

        // Permitted: no target heading names an entry.
        for heading in [
            "## The seven laws",
            "# Reconnaissance patterns",
            "## Template for new entries",
        ] {
            assert!(
                guard_with_oracle(display, text, Some(abs), Some(&NothingIsAugmented), &[heading])
                    .is_ok(),
                "a non-entry heading in a ledger must stay directly editable: {heading}"
            );
        }
    }

    /// The batch form edits several headings in one atomic call, so one entry
    /// heading anywhere in the batch has to refuse the whole call — permitting it
    /// because a sibling edit was innocent would write the entry anyway.
    #[test]
    fn one_entry_heading_in_a_batch_refuses_the_whole_call() {
        struct NothingIsAugmented;
        impl AugmentedArtifactOracle for NothingIsAugmented {
            fn is_augmented(&self, _: &std::path::Path) -> bool {
                false
            }
        }

        let text = "---\nkind: tracker\nentry_prefix: R\n---\n\n## Index\n\n## R-9 — x\n";
        let abs = std::path::Path::new("/repo/docs/trackers/recon.md");
        assert!(
            guard_with_oracle(
                "docs/trackers/recon.md",
                text,
                Some(abs),
                Some(&NothingIsAugmented),
                &["## Index", "## R-10 — sneaked in"]
            )
            .is_err(),
            "a batch containing one entry heading must be refused whole"
        );
    }

    /// The relaxation belongs to the LEDGER arm alone. An augmented artifact's params
    /// live in the catalog and the file is only a rendered snapshot, so a prose-only
    /// edit desynchronises it just as much as an entry edit — and
    /// `artifact(action="update", patch={body_edits: [...]})` already serves that
    /// case without ceremony. A stamped id says the librarian wrote the file, which
    /// the heading being edited does not change either.
    #[test]
    fn heading_scoping_does_not_relax_the_augmented_or_stamped_arms() {
        struct EverythingIsAugmented;
        impl AugmentedArtifactOracle for EverythingIsAugmented {
            fn is_augmented(&self, _: &std::path::Path) -> bool {
                true
            }
        }
        struct NothingIsAugmented;
        impl AugmentedArtifactOracle for NothingIsAugmented {
            fn is_augmented(&self, _: &std::path::Path) -> bool {
                false
            }
        }

        let abs = std::path::Path::new("/repo/docs/trackers/t.md");

        // Augmented, and the heading is innocent prose.
        let augmented_text = "---\nkind: tracker\nentry_prefix: R\n---\n\n## Prose\n";
        assert!(
            guard_with_oracle(
                "docs/trackers/t.md",
                augmented_text,
                Some(abs),
                Some(&EverythingIsAugmented),
                &["## Prose"]
            )
            .is_err(),
            "an augmented artifact stays wholly refused whatever heading is targeted"
        );

        // Stamped id, not a ledger, innocent heading.
        let stamped_text = "---\nkind: bug\nid: 0123456789abcdef\n---\n\n## Summary\n";
        assert!(
            guard_with_oracle(
                "docs/issues/b.md",
                stamped_text,
                Some(abs),
                Some(&NothingIsAugmented),
                &["## Summary"]
            )
            .is_err(),
            "a stamped artifact stays wholly refused whatever heading is targeted"
        );
    }

    /// A caller that does not address headings passes `&[]`, and the safe reading of
    /// "no heading information" is the whole-file refusal — never a blanket permit.
    /// `read_markdown` and `edit_file` are both in this position.
    #[test]
    fn no_heading_information_keeps_the_whole_file_refusal() {
        struct NothingIsAugmented;
        impl AugmentedArtifactOracle for NothingIsAugmented {
            fn is_augmented(&self, _: &std::path::Path) -> bool {
                false
            }
        }

        let text = "---\nkind: tracker\nentry_prefix: R\n---\n\n## Prose\n";
        assert!(
            guard_with_oracle(
                "docs/trackers/recon.md",
                text,
                Some(std::path::Path::new("/repo/docs/trackers/recon.md")),
                Some(&NothingIsAugmented),
                &[]
            )
            .is_err(),
            "an empty heading list must not be read as permission"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib util::librarian_guard 2>&1`

Expected: FAIL to compile — `this function takes 4 arguments but 5 arguments were supplied` on every new call.

- [ ] **Step 3: Add `targets_a_ledger_entry` and thread the parameter**

Insert after `declared_entry_prefixes` in `src/util/librarian_guard.rs`:

```rust
/// `true` when any of `headings` names a `PREFIX-N` entry of a namespace this file
/// declares.
///
/// The heading shape mirrors the heading half of
/// `augmentation::body_claimed_indices` — `#{1,6}`, whitespace, optional
/// backtick/bold/link wrapping, the prefix, a hyphen, digits, word boundary — so the
/// guard and the allocator agree on what an entry is. Index-table rows are
/// deliberately not matched: `edit_markdown` addresses headings.
///
/// An empty `headings` slice means the caller had no heading information, and the
/// safe reading of that is "assume an entry" — a permit would be the guard failing
/// open.
pub(crate) fn targets_a_ledger_entry(text: &str, headings: &[&str]) -> bool {
    let prefixes = declared_entry_prefixes(text);
    if prefixes.is_empty() {
        return false;
    }
    if headings.is_empty() {
        return true;
    }
    prefixes.iter().any(|p| {
        let esc = regex::escape(p);
        let Ok(re) = regex::Regex::new(&format!(r"^#{{1,6}}[ \t]+[`*\[]*{esc}-\d+\b")) else {
            // A prefix that will not compile is one the guard cannot reason about;
            // refuse rather than permit.
            return true;
        };
        headings.iter().any(|h| re.is_match(h.trim()))
    })
}
```

Then change the two signatures. `guard_not_librarian_managed`:

```rust
pub fn guard_not_librarian_managed(
    path: &str,
    text: &str,
    abs_path: Option<&std::path::Path>,
    headings: &[&str],
) -> Result<(), anyhow::Error> {
    let oracle = oracle();
    guard_with_oracle(path, text, abs_path, oracle.as_deref(), headings)
}
```

`guard_with_oracle` — add the parameter and narrow the ledger arm:

```rust
fn guard_with_oracle(
    path: &str,
    text: &str,
    abs_path: Option<&std::path::Path>,
    oracle: Option<&dyn AugmentedArtifactOracle>,
    headings: &[&str],
) -> Result<(), anyhow::Error> {
    let augmented = matches!((abs_path, oracle), (Some(p), Some(o)) if o.is_augmented(p));
    let stamped = is_librarian_artifact(text);
    // The ledger arm is the only heading-scoped one. Augmentation means the file is
    // not where its state lives, and a stamped id means the librarian wrote it —
    // neither claim is about the section being edited, so neither narrows. What a
    // ledger owns is its ID NAMESPACE, and only an entry heading touches that.
    let ledger_entry = targets_a_ledger_entry(text, headings);
    if !augmented && !stamped && !ledger_entry {
        return Ok(());
    }
    let why = if augmented {
        " (augmented — its params live in the catalog, and this file is only a \
         rendered snapshot of them)"
    } else if ledger_entry {
        " (a ledger entry — PREFIX-N ids are allocated by the server, and a \
         hand-written entry leaves the committed high-water mark stale)"
    } else {
        ""
    };
    let hint = if ledger_entry && !augmented && !stamped {
        "This file's other headings are directly editable — only PREFIX-N entry \
         headings are not. To add an entry:\n\
         • Reserve: artifact(action=\"append_entry\", id=\"<id>\", id_prefix=\"<PREFIX>\")\n\
         • Then write the section yourself with the id it returns."
    } else {
        "Use artifact tools instead:\n\
         • Read:   artifact(action=\"get\", id=\"<id>\")\n\
         • Find:   artifact(action=\"find\", semantic=\"<topic>\")\n\
         • Edit:   artifact(action=\"update\", id=\"<id>\", patch={...})\n\
         Full guide: resources/read doc://librarian-guide"
    };
    Err(RecoverableError::with_hint(
        format!("'{path}' is a librarian-managed artifact{why} — do not read or edit it directly"),
        hint,
    )
    .into())
}
```

- [ ] **Step 4: Update the three call sites**

`src/tools/markdown/edit_markdown.rs` — replace the guard call at :1271 with:

```rust
        // Collect the headings this call targets so the guard can scope a ledger's
        // refusal to its PREFIX-N entries. Both shapes are covered: the single
        // `heading` param and every element of the `edits` batch. A batch is atomic,
        // so one entry heading anywhere in it refuses the whole call.
        let mut target_headings: Vec<&str> = Vec::new();
        if let Some(h) = input["heading"].as_str() {
            target_headings.push(h);
        }
        if let Some(edits) = input["edits"].as_array() {
            target_headings.extend(edits.iter().filter_map(|e| e["heading"].as_str()));
        }
        crate::util::librarian_guard::guard_not_librarian_managed(
            path,
            &file_content,
            Some(&resolved),
            &target_headings,
        )?;
```

`src/tools/markdown/read_markdown.rs:507` — a read addresses no heading for guard purposes, and `&[]` keeps the existing whole-file refusal:

```rust
        crate::util::librarian_guard::guard_not_librarian_managed(path, &text, Some(&resolved), &[])?;
```

`src/tools/edit_file/mod.rs:692` — `edit_file` is text-level, with no heading concept. Read the actual argument expressions at that site before editing; the fourth argument `&[]` is the only addition:

```rust
    crate::util::librarian_guard::guard_not_librarian_managed(
        path,
        &text,
        abs_path.as_deref(),
        &[],
    )?;
```

- [ ] **Step 5: Update Task 1's tests for the new arity**

The four tests added in Task 1 and the three pre-existing oracle tests call `guard_with_oracle` with four arguments. Add `&[]` as the fifth to each — `&[]` preserves whole-file semantics, which is what each of those tests asserts. `a_catalogued_but_unaugmented_file_stays_directly_editable` must keep passing unchanged apart from that argument: its two files declare no `entry_prefix`, so `targets_a_ledger_entry` returns `false` regardless.

Also update the three `guard_not_librarian_managed` call sites inside `mod tests` (lines 205, 214, 279) with a trailing `&[]`.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test --lib util::librarian_guard 2>&1`

Expected: PASS, every test in the module.

- [ ] **Step 7: Mutation-test the heading scoping**

Temporarily change `targets_a_ledger_entry`'s `if headings.is_empty() { return true; }` to `return false;`, then run:

Run: `cargo test --lib util::librarian_guard::tests::no_heading_information_keeps_the_whole_file_refusal 2>&1`

Expected: FAIL with `an empty heading list must not be read as permission`. Restore.

Then temporarily change the same function's final `prefixes.iter().any(...)` expression to `false`, and run:

Run: `cargo test --lib util::librarian_guard::tests::a_ledger_refuses_entry_headings_and_permits_every_other_heading 2>&1`

Expected: FAIL with `must refuse an entry heading: ## R-105 — a new lesson`. Restore and re-run to confirm PASS.

- [ ] **Step 8: Verify on the wire**

Run: `cargo rb`, then `/mcp` to reconnect, then probe the real ledger — `docs/trackers/reconnaissance-patterns.md` is augmented, so it must stay wholly refused:

```
edit_markdown(path="docs/trackers/reconnaissance-patterns.md",
              heading="## The seven laws (distilled 2026-08-16)",
              action="edit", old_string="zzz-absent", new_string="x")
```

Expected: refused with `(augmented — ...)`. A permit here means Step 3's arm ordering regressed the augmented case.

Then build the ledger-only case in the scratchpad (`entry_prefix: ZZ`, no `id:`, not augmented) and confirm both halves: an `## ZZ-N` heading refused with the `append_entry` hint, a `## Prose` heading permitted. Delete the probe file afterwards.

- [ ] **Step 9: Run the full gate**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings 2>&1 && cargo test --lib 2>&1`

Expected: all green. `guard_not_librarian_managed` is public, so confirm no caller was missed: `grep -rn 'guard_not_librarian_managed' src/ tests/` should show only the sites this task edited.

- [ ] **Step 10: Commit, then close the bug**

```bash
git status --short
git commit --only src/util/librarian_guard.rs src/tools/markdown/edit_markdown.rs src/tools/markdown/read_markdown.rs src/tools/edit_file/mod.rs -m "fix(librarian-guard): scope a ledger's refusal to its PREFIX-N entry headings

Task 1 left a declared ledger wholly refused for edit_markdown. That is right
for an augmented file -- the file is a rendered snapshot, so any edit
desynchronises it -- but overshoots for a merely-declared ledger, where the
file IS where its state lives. Refusing a typo fix in a 3,000-line ledger is
the ceremony objection the bug raises against a whole-file guard, and it is
what trains authors to route around the guard.

edit_markdown is heading-addressed, so the ledger arm now decides
mechanically: a target heading matching ^#{1,6} PREFIX-N is refused with a
hint naming append_entry; every other heading is directly editable. The
augmented and stamped-id arms keep whole-file semantics -- neither claim is
about the section being edited. Callers with no heading information pass &[],
which keeps the refusal rather than failing open."
```

Then update and archive the bug file. Its `## Status` section already carries one retraction; add what this plan established rather than rewriting it, and note that spec pieces 1 and 2 were found already shipped:

```
artifact(action="update", id="88129ecc9c4c87a2",
         patch={status: "fixed", body_edits: [...]})
artifact(action="move", id="88129ecc9c4c87a2",
         new_rel_path="docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md")
```

Read the returned new id from the `move` response and re-point every citation of the old id or old path in the same commit — `grep -rn '88129ecc9c4c87a2\|librarian-guard-blind-to-artifacts' . --include='*.md' --include='*.rs'`. Leave `docs/issues/archive/**` hits alone; those are historical snapshots.

---

## Self-Review

**1. Spec coverage.** The spec's piece 3 ("Guard structurally, not per-file") is Tasks 1 and 2. Pieces 1 and 2 are covered by being explicitly ruled already-shipped in Global Constraints, with the file:line evidence — the spec's "NOT yet called from any MCP tool" claim is stale as of `append_entry.rs:91`. The spec's "or the declared index heading" clause is explicitly dropped in Global Constraints with the grep that shows no such concept exists. The spec's `## Fix options` 2 and 3 are marked withdrawn by the spec itself; this plan does not revive them.

**2. Placeholder scan.** No TBD/TODO. Every code step carries the literal code it needs.

**3. Type consistency.** `declared_entry_prefixes(text: &str) -> Vec<String>` is defined in Task 1 Step 3 and consumed in Task 2 Step 3 with that exact signature. `clean_prefix(raw: &str) -> Option<String>` is used by `declared_entry_prefixes` via `filter_map(clean_prefix)` (flow branch) and `clean_prefix(val).into_iter()` (scalar branch) — both valid for `Option<String>`. `targets_a_ledger_entry(text: &str, headings: &[&str]) -> bool` is defined and used consistently. `guard_with_oracle`'s arity goes 4→5 in Task 2, and Step 5 explicitly updates every Task 1 test and the three pre-existing ones — the arity change is the plan's one cross-task breakage and it is handled in its own step.

**One risk worth naming for the executor:** Task 1 ships a guard that refuses whole ledger files, and Task 2 relaxes it. If Task 2 is abandoned mid-plan, the tree is left stricter than it started for any ledger not carrying a stamped `id:` or an augmentation — which today is none of the five, so the blast radius is zero. Shipping Task 1 alone is therefore safe; shipping Task 2 alone is not, since it is a pure loosening of the arm Task 1 adds.
