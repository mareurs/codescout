# Ledger-Aware Librarian Guard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Teach `librarian_guard` the `entry_prefix` ledger declaration, so a prose ledger's `PREFIX-N` entry headings can only be written through the server's id allocator — while every other heading in that same file stays directly editable.

**Architecture:** Task 1 adds a third predicate to the guard's existing union in `src/util/librarian_guard.rs` (stamped `id:` OR augmented OR **declares `entry_prefix`**), which closes the hole for any newly-created prose ledger. Task 2 supplies the mechanism that keeps the guard's hand-rolled `entry_prefix` reader in agreement with the librarian's `serde_yml` one — a parity test over one corpus — because the feature boundary forces two readers and a doc comment is not a mechanism.

The spec's heading-scoped guard is deliberately **not** built; Task 2's Alternatives section records why, with the measurement that killed it. Read that before proposing it again.

**Tech Stack:** Rust, `regex`, `cargo test --lib`. No new dependencies.

**Status: EXECUTED 2026-08-18** — Task 1 in `f4db4e9c`, Task 2 in `9ac00440`, both **experiments**. The spec's heading-scoped guard was cut, not built; Task 2's Alternatives section records the measurement. Kept for the record.

**Spec:** `docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md` (artifact `388290ad0f86fe03`, archived on fix) — specifically its `## Status` → *"The shape the fix should take"* section. That bug file already carries one retraction; read the whole `## Status` section before starting, not just the summary.

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
| `src/util/librarian_guard.rs` | The guard predicate and its decision. Owns `is_librarian_artifact`, `guard_with_oracle`, the oracle plumbing, and all guard tests. Compiles under `--no-default-features`. | Modify — add ledger parsing + a third union arm (Task 1); widen the ledger refusal hint (Task 2 Step 6). |
| `src/librarian/catalog/augmentation.rs` | The allocator. Owns `ENTRY_PREFIX_KEY`, `allocate_entry_id`, `body_claimed_indices`. Feature-gated behind `librarian`. | Modify in Task 2 only — extract the inline `entry_prefix` match into a callable pure function, and add the parity test beside it. |

No new files, and **no signature changes**. `guard_not_librarian_managed` keeps its three-argument shape, so its three production call sites (`edit_markdown.rs:1271`, `read_markdown.rs:507`, `edit_file/mod.rs:692` — verified as the complete set; `grep` over `tests/` and `src/bin/` returns none) are untouched. That is a direct consequence of cutting the heading-scoped guard: the version of this plan that built it had to thread a fourth parameter through all three.

The guard is ~400 lines including tests and stays one focused unit; splitting it would separate the predicate from the tests that pin its rationale.

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
    /// docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md
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

### Task 2: Hold the two `entry_prefix` readers in agreement by test, not by comment

Task 1 creates a **second reader** of `entry_prefix`. The librarian already has one — `allocate_entry_id` parses it out of `fm.extra` via `serde_yml` (`src/librarian/catalog/augmentation.rs:798-802`). Task 1's `declared_entry_prefixes` hand-parses the same key from raw text, because it must compile under `--no-default-features` where `serde_yml` is absent.

Two readers, one contract. That duplication is **unavoidable** — see Alternatives below — but the way Task 1 states it is not acceptable: its doc comment says *"Accepts all three YAML forms `allocate_entry_id` honours (…:798-802)"*, which is a co-change contract enforced by **prose**. A comment that says "mirrors X" or "kept in sync with X" is strictly worse than compiler-visible duplication: it proves someone knew and supplies no mechanism. This project has already paid for that exact shape four times (`docs/adrs/2026-07-25-embedding-transport-boundary.md`, where a root `reqwest` copy carried the comment *"Mirrors the codescout-embed RemoteEmbedder guard"* and cost 48 needlessly-compiled crates).

This task supplies the mechanism: one corpus, both readers, asserted equal.

**Alternatives considered and rejected** — record these so a later session does not re-litigate:

- *Have the guard call `frontmatter::parse`.* Impossible: `librarian` is a Cargo feature and the guard compiles without it. This is a real boundary, not an oversight.
- *Have `allocate_entry_id` call `declared_entry_prefixes` instead, leaving one reader.* Rejected. `allocate_entry_id` already parses `fm` because it needs `entry_high_water_<PREFIX>` from `fm.extra`. Reading prefixes via raw text while reading the mark via YAML makes one function read one frontmatter block two ways — a new inconsistency, not a removed one.
- *A heading-scoped guard (the original Task 2).* **Cut, premise measured false.** It existed to answer the spec's objection that a whole-file guard makes a typo fix in a 3,000-line ledger a ceremony. Measured 2026-08-18: `artifact(action="update", id=…, patch={body_edits: [{heading, action: "edit", old_string, new_string}]})` is a section-scoped text swap that works on `docs/trackers/skill-frictions.md` — a catalog row with no `id:` and no augmentation — returning `old_string not found`, i.e. reaching the swap logic. Augmentation is not required. So the ergonomic gap the heading-scoping bought does not exist, and it would have cost a new parameter on a public function, three threaded call sites, a regex, and four tests, for a class of file with **zero current members**.

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs:798-802` (extract the inline `match` into a named pure function so it is callable from a test)
- Test: `src/librarian/catalog/augmentation.rs` `mod tests` (the parity test — lives here because this is the side where both readers exist)

**Interfaces:**
- Consumes from Task 1: `crate::util::librarian_guard::declared_entry_prefixes(text: &str) -> Vec<String>`.
- Produces: `pub(crate) fn declared_prefixes_from_frontmatter(fm: Option<&Frontmatter>) -> Vec<String>` — the librarian-side reader, extracted verbatim from the existing inline `match` and returning owned `String`s so the two readers' return types compare directly.

- [ ] **Step 1: Write the failing parity test**

Add to `mod tests` in `src/librarian/catalog/augmentation.rs`:

```rust
    /// The guard and the allocator each read `entry_prefix`, by different mechanisms,
    /// and they must agree — a disagreement is silent in the dangerous direction: the
    /// allocator honours a form the guard is blind to, so entries in that ledger can
    /// be hand-written past the allocator with no error anywhere.
    ///
    /// Two readers is forced, not sloppy. `src/util/librarian_guard.rs` compiles under
    /// `--no-default-features` where `serde_yml` does not exist, so it hand-parses;
    /// this side already parses `fm` for `entry_high_water_<PREFIX>` and would be made
    /// worse, not better, by reading one frontmatter block two ways. What is NOT
    /// acceptable is holding the agreement in a doc comment — this project has paid
    /// for prose-enforced co-change contracts before
    /// (`docs/adrs/2026-07-25-embedding-transport-boundary.md`). This test is the
    /// mechanism that comment stood in for.
    #[test]
    fn both_entry_prefix_readers_agree_on_every_yaml_form() {
        for (label, doc) in [
            ("scalar", "---\nkind: tracker\nentry_prefix: R\n---\n\n# L\n"),
            (
                "quoted scalar",
                "---\nkind: tracker\nentry_prefix: 'HY'\n---\n\n# L\n",
            ),
            (
                "double-quoted scalar",
                "---\nkind: tracker\nentry_prefix: \"HY\"\n---\n\n# L\n",
            ),
            (
                "inline flow",
                "---\nkind: tracker\nentry_prefix: [F, W]\n---\n\n# L\n",
            ),
            (
                "block sequence",
                "---\nkind: tracker\nentry_prefix:\n  - F\n  - W\n---\n\n# L\n",
            ),
            (
                "sequence then sibling key",
                "---\nkind: tracker\nentry_prefix:\n  - F\n  - W\nentry_high_water_F: 3\n---\n\n# L\n",
            ),
            ("absent", "---\nkind: tracker\n---\n\n# L\n"),
            ("bare key", "---\nkind: tracker\nentry_prefix:\n---\n\n# L\n"),
            (
                "empty string",
                "---\nkind: tracker\nentry_prefix: ''\n---\n\n# L\n",
            ),
            (
                "empty flow list",
                "---\nkind: tracker\nentry_prefix: []\n---\n\n# L\n",
            ),
            ("no frontmatter at all", "# L\n\nentry_prefix: R\n"),
        ] {
            let (fm, _body) = crate::librarian::frontmatter::parse(doc).unwrap();
            let librarian_side = declared_prefixes_from_frontmatter(fm.as_ref());
            let guard_side = crate::util::librarian_guard::declared_entry_prefixes(doc);
            assert_eq!(
                librarian_side, guard_side,
                "{label}: the allocator and the guard must read entry_prefix identically — \
                 a form only one of them honours is a silent hole in the guard"
            );
        }
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib librarian::catalog::augmentation::tests::both_entry_prefix_readers_agree 2>&1`

Expected: FAIL to compile — `cannot find function 'declared_prefixes_from_frontmatter' in this scope`.

- [ ] **Step 3: Extract the librarian-side reader**

In `src/librarian/catalog/augmentation.rs`, insert this immediately above `allocate_entry_id`:

```rust
/// The id namespaces a parsed frontmatter block declares via `entry_prefix`.
///
/// Extracted from `allocate_entry_id`'s body so it can be driven by
/// `both_entry_prefix_readers_agree_on_every_yaml_form` alongside the guard's
/// independent text-level reader. Returns owned `String`s rather than borrowed
/// `&str` so the two readers' outputs compare directly in that test.
///
/// Scalar or sequence: a session log legitimately owns two namespaces (F-N
/// frictions and W-N wins), so `entry_prefix: [F, W]` must be as valid as
/// `entry_prefix: R`. Reservations are keyed per (artifact, prefix), so the
/// counters stay independent either way.
pub(crate) fn declared_prefixes_from_frontmatter(
    fm: Option<&crate::librarian::frontmatter::Frontmatter>,
) -> Vec<String> {
    match fm.and_then(|f| f.extra.get(ENTRY_PREFIX_KEY)) {
        Some(Value::String(s)) if !s.trim().is_empty() => vec![s.trim().to_string()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}
```

Then replace the inline `match` inside `allocate_entry_id` (currently at :798-802) with a call to it, keeping the surrounding comment:

```rust
    // Scalar or sequence: a session log legitimately owns two namespaces (F-N
    // frictions and W-N wins), so `entry_prefix: [F, W]` must be as valid as
    // `entry_prefix: R`. Reservations are keyed per (artifact, prefix), so the
    // counters stay independent either way.
    let declared = declared_prefixes_from_frontmatter(fm.as_ref());
```

The two uses below this line — `declared.is_empty()` and `declared.contains(&id_prefix)` — need adjusting for `Vec<String>` rather than `Vec<&str>`: `declared.contains(&id_prefix)` becomes `declared.iter().any(|d| d == id_prefix)`, and the error hint's `declared.join(", ")` works unchanged.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --lib librarian::catalog::augmentation::tests::both_entry_prefix_readers_agree 2>&1`

Expected: PASS. If a case fails, the failing label names which YAML form the two readers disagree on — fix `declared_entry_prefixes` (Task 1's hand parser) to match the `serde_yml` reading, not the other way round: `serde_yml` defines what the file means.

- [ ] **Step 5: Mutation-test the parity test**

Temporarily delete the `("block sequence", …)` arm's handling from Task 1's `declared_entry_prefixes` — replace its block-sequence branch with `return Vec::new();` — then run:

Run: `cargo test --lib librarian::catalog::augmentation::tests::both_entry_prefix_readers_agree 2>&1`

Expected: FAIL naming `block sequence`. This is the exact hole the test exists to catch: the allocator would honour `entry_prefix:\n  - F` while the guard read the file as not-a-ledger. Restore and confirm PASS.

- [ ] **Step 6: Improve Task 1's refusal hint**

Task 1's ledger arm currently refuses with the generic artifact-tools hint. Since Task 2 is cut, that hint is the only thing routing an author who wanted to edit ledger prose. Replace the ledger branch's hint in `guard_with_oracle` so it names both cases:

```rust
    let hint = if ledger && !augmented && !is_librarian_artifact(text) {
        "This file is a ledger — it owns a PREFIX-N id namespace.\n\
         • Add an entry:  artifact(action=\"append_entry\", id=\"<id>\", id_prefix=\"<PREFIX>\")\n\
           then write the section yourself with the id it returns.\n\
         • Edit anything else (prose, a heading, a typo):\n\
           artifact(action=\"update\", id=\"<id>\", patch={body_edits: [{heading: \"## X\", \
         action: \"edit\", old_string: \"...\", new_string: \"...\"}]})"
            .to_string()
    } else {
        "Use artifact tools instead:\n\
         • Read:   artifact(action=\"get\", id=\"<id>\")\n\
         • Find:   artifact(action=\"find\", semantic=\"<topic>\")\n\
         • Edit:   artifact(action=\"update\", id=\"<id>\", patch={...})\n\
         Full guide: resources/read doc://librarian-guide"
            .to_string()
    };
```

Change `RecoverableError::with_hint`'s second argument to `hint`. Add a test asserting the ledger refusal names `append_entry` **and** `body_edits` — a single-branch hint is the failure mode being avoided, not a smaller version of it:

```rust
    /// A ledger's refusal is the only thing routing an author who wanted to edit its
    /// prose, since the heading-scoped guard was cut as unnecessary. Both routes must
    /// appear: `append_entry` for a new entry, `body_edits` for everything else. A
    /// hint naming only one of them sends prose edits to the wrong tool.
    #[test]
    fn a_ledger_refusal_names_both_the_entry_route_and_the_prose_route() {
        struct NothingIsAugmented;
        impl AugmentedArtifactOracle for NothingIsAugmented {
            fn is_augmented(&self, _: &std::path::Path) -> bool {
                false
            }
        }

        let text = "---\nkind: tracker\nentry_prefix: R\n---\n\n## Prose\n";
        let abs = std::path::Path::new("/repo/docs/trackers/recon.md");
        let err = guard_with_oracle("docs/trackers/recon.md", text, Some(abs), Some(&NothingIsAugmented))
            .expect_err("a ledger must be guarded");
        let re = err.downcast_ref::<RecoverableError>().unwrap();
        let hint = re.hint().unwrap_or_default();
        assert!(
            hint.contains("append_entry"),
            "the entry route must be named: {hint}"
        );
        assert!(
            hint.contains("body_edits"),
            "the prose route must be named: {hint}"
        );
    }
```

`RecoverableError`'s hint accessor returns text only for the `Guidance::Hint` variant — read `src/tools/core/types.rs`'s `hint()` before asserting on it, and fall back to `err.to_string().contains(...)` if the accessor shape differs; the `Display` impl renders `"{message} — Hint: {text}"` and is the documented stable test contract.

- [ ] **Step 7: Run the full gate**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings 2>&1 && cargo test --lib 2>&1`

Expected: all green. Also confirm the lean build still compiles, since Task 1's whole constraint was the feature boundary:

Run: `cargo check --no-default-features 2>&1`

Expected: clean. This is the check that would have caught a guard reaching into `crate::librarian`.

- [ ] **Step 8: Commit, then close the bug**

```bash
git status --short
git commit --only src/librarian/catalog/augmentation.rs src/util/librarian_guard.rs -m "test(librarian-guard): hold both entry_prefix readers in agreement

Task 1 added a second reader of entry_prefix -- hand-parsed in the guard,
because src/util/ compiles under --no-default-features where serde_yml does
not exist. Two readers is forced: allocate_entry_id already parses fm for
entry_high_water_<PREFIX>, and reading one frontmatter block two ways would be
a new inconsistency rather than a removed one.

What was not acceptable was holding the agreement in a doc comment. A
mirrors-X comment is a co-change contract enforced by prose -- it proves
someone knew and supplies no mechanism, which this project has already paid
for (2026-07-25-embedding-transport-boundary.md). One corpus, both readers,
asserted equal; mutation-verified by blinding the guard to block sequences and
watching the parity test name that form.

Also widens the ledger refusal hint to name both routes -- append_entry for a
new entry, body_edits for prose -- since the heading-scoped guard was cut."
```

Then update and archive the bug file. Its `## Status` section already carries one retraction; add what this plan established rather than rewriting it:

- spec pieces 1 and 2 were found already shipped (`append_entry.rs:91`);
- the hole was verified end-to-end, not inferred (the `entry_prefix: ZZ` probe);
- the heading-scoped guard the spec's piece 3 asked for was **cut**, because `body_edits` already provides section-scoped editing on any catalog row — measured on `skill-frictions.md`, a row with no `id:` and no augmentation.

```
artifact(action="update", id="88129ecc9c4c87a2",
         patch={status: "fixed", body_edits: [...]})
artifact(action="move", id="88129ecc9c4c87a2",
         new_rel_path="docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md")
# DONE — minted 388290ad0f86fe03; citations re-pointed in the same commit.
```

Read the returned new id from the `move` response and re-point every citation of the old id or old path in the same commit — `grep -rn '88129ecc9c4c87a2\|librarian-guard-blind-to-artifacts' . --include='*.md' --include='*.rs'`. Leave `docs/issues/archive/**` hits alone; those are historical snapshots.

- [ ] **Step 9: File the guide's stale advice as its own bug**

Do not fold this into the fix — it is a separate defect on a different surface, and it will mislead the next agent regardless of this plan.

`get_guide("tracker-conventions")` § *Make the tracker guarded* currently says: *"Stamp the catalog id into the file's frontmatter as `id: <16-hex>`… a fully registered tracker with no `id:` line is completely unguarded."* That advice was tried and reverted in `bb9a94d7`: stamping an id silently disabled `docs/TAXONOMY.md`'s documented `edit_markdown` append path for R-N. The bug file this plan implements retracts the same advice in its own `## Status` section, but the guide — which is auto-injected on the first `artifact` call of every session — still recommends it.

Open `docs/issues/<today>-tracker-conventions-guide-recommends-reverted-id-stamping.md` from `docs/issues/_TEMPLATE.md`, citing `bb9a94d7`, the guide section, and this plan. After Task 1 ships, the correct advice is *declare `entry_prefix`*, which guards the ledger without disabling anything.
---

## Self-Review

**1. Spec coverage.** The spec's pieces 1 and 2 are ruled already-shipped in Global Constraints, with file:line evidence — its "NOT yet called from any MCP tool" claim is stale as of `append_entry.rs:91`. Piece 3 splits: its *goal* (a ledger's ids can only come from the allocator) is Task 1; its *proposed mechanism* (heading-scoped refusal) is **cut**, with the measurement in Task 2's Alternatives. The spec's "or the declared index heading" clause is dropped in Global Constraints with the grep showing no such concept exists. The spec's `## Fix options` 2 and 3 are marked withdrawn by the spec itself; this plan does not revive them.

**2. Placeholder scan.** No TBD/TODO. Every code step carries the literal code it needs. Two steps deliberately end in a verification instruction rather than a code block (Task 2 Step 6's `hint()` accessor check, Step 8's citation re-point) — both name the exact file to read and the exact grep to run, which is the content an engineer needs there.

**3. Type consistency.** `declared_entry_prefixes(text: &str) -> Vec<String>` (Task 1 Step 3) and `declared_prefixes_from_frontmatter(fm: Option<&Frontmatter>) -> Vec<String>` (Task 2 Step 3) return the same type, which is what lets the parity test `assert_eq!` them directly — that is the reason for the owned `String`, and Task 2 Step 3 says so. `clean_prefix(raw: &str) -> Option<String>` is used via `filter_map(clean_prefix)` and `clean_prefix(val).into_iter()`, both valid for `Option<String>`. Task 2 Step 3 also names the one knock-on the extraction causes: `declared.contains(&id_prefix)` must become `declared.iter().any(|d| d == id_prefix)` because the element type changes from `&str` to `String`.

**What this plan gained by being reviewed.** The first draft built the spec's heading-scoped guard. Reviewing it as architecture rather than as a task list killed that task on a measurement — `body_edits` already provides section-scoped editing on any catalog row, so the ergonomic gap it existed to close was not real — and surfaced a defect the draft had *introduced*: a doc comment asserting the guard "accepts all three YAML forms `allocate_entry_id` honours", which is a co-change contract with no enforcement. The plan is now one task smaller, changes no public signature, and the agreement that comment claimed is a test.

**One risk worth naming for the executor.** Task 1 makes a future prose ledger wholly refused for `edit_markdown`, including its prose. Affected files today: zero — all five current ledgers are already guarded by a stamped `id:` or an augmentation. Task 2 Step 6 is what makes that refusal survivable, by naming the `body_edits` route in the hint. If you ship Task 1 without Step 6, an author who hits the refusal gets the generic artifact-tools hint and has to work out the prose route themselves. Ship Step 6 with Task 1 if you ship nothing else.
