# Closing Advisory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `doc(action="update")` returns a `fix_anchor_advisory` string when the call closes a bug as `fixed` or `mitigated` and the file as written lacks the recoverable fix pair; the write always succeeds.

**Architecture:** `doctor.rs` already owns the anchor predicates (`declares_fix_pair`, `declares_fix_anchor`) and the archive guard `refuse_unanchored_archive`. Task 1 extracts the guard's classification and remedy text into shared helpers and adds `closing_advisory`, so the guard and the advisory cannot drift. Task 2 calls `closing_advisory` in `update::call` next to the archive guard, puts the string in the response, and records it in the guide and the bug record.

**Tech Stack:** Rust, `serde_json`, tokio tests in `src/librarian/tools/update.rs` (`mod tests`, helpers `mk_ctx`, `mk_bug`, `call`).

**Spec:** `docs/superpowers/specs/2026-11-03-closing-advisory-design.md`

## Global Constraints

- The advisory never makes a call fail. No `?`, no `Err`, on its path.
- One definition of "anchored": only `declares_fix_pair` / `declares_fix_anchor`. No new parser.
- Response field name: `fix_anchor_advisory`, a JSON string. Absent (not null) when silent.
- Fires only when THIS call's `patch.status` is `fixed` or `mitigated` and `row.kind == "bug"`. The top-level `status` shortcut is already lifted into `patch.status` (`update.rs:553`).
- Checked against `new_content`, the file as it will be written.
- The cited section heading is exactly `Closing a bug — patch-id and fix provenance` (em dash), a `###` heading in `src/prompts/guides/tracker-conventions.md:150`.
- Do not edit `src/prompts/guides/librarian.md`: its sections are served on `doc.update` and the p50 ceiling (`CEILING = 12_244`) has a recorded margin of 9 B. Ceiling constants must not change.
- Size: the spec said "about 300 B". Reusing the guard's remedy text makes it about 650–800 B. Accepted: it is shared text, fires only on a violation, and is outside every emission ceiling. Report the measured size in the final task.
- Shared checkout: commit by pathspec (`git add <paths>`, `git diff --cached --name-only`, `git commit -m … -- <paths>`). A peer's staged `.codescout/memories/bug-triage-2026-10-27.md` sits in the index: leave it. Never push.
- Before completion, `./scripts/gate.sh`. If peers' uncommitted Rust breaks the shared tree, run it in a detached worktree holding only these commits (`git worktree add --detach <scratchpad>/gate-wt HEAD`, `PYTHON_LIGHT=0 ./scripts/gate.sh`, then `git worktree remove --force`). Run `rustfmt --edition 2021 --check` on the changed `.rs` files first.

## Review Focus

1. **Close and add provenance in one call** — expected silent, because the check reads `new_content`. Test in Task 2.
2. **Patch-id on the same line as the SHA** — not parsed by `structured_fix_pointers`, so the record is SHA-only; expected the patch-id text, not the "nothing" text. Test in Task 1.
3. **`no_fix_commit: ""`** — empty counts as absent (matching `doctor`); expected the advisory to fire. Test in Task 2.
4. **Re-setting `fixed` on an already-fixed, unanchored bug** — this call sets the status, so expected to fire. Test in Task 2.
5. **Archive guard wording after the refactor** — its refusals must still contain `## Fix provenance`, `no_fix_commit`, `no patch-id` and `orphans` (asserted in `update.rs` and `mv.rs` tests). Covered by running those existing tests in Task 1.

---

### Task 1: Shared fix-anchor classification, remedy text and `closing_advisory` in `doctor.rs`

**Files:**
- Modify: `src/librarian/tools/doctor.rs` — `refuse_unanchored_archive` (`:7527-7569`) and new items beside it
- Test: `src/librarian/tools/doctor.rs` test module (same file)

**Interfaces:**
- Produces (all `pub(crate)` in `crate::librarian::tools::doctor`):
  - `enum FixAnchorGap { Nothing, PatchId }` with `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`
  - `fn fix_anchor_gap(content: &str) -> Option<FixAnchorGap>` — `None` iff `declares_fix_pair(content)`; `Some(PatchId)` iff `declares_fix_anchor(content)` but not the pair; else `Some(Nothing)`.
  - `const CLOSING_SECTION: &str = "Closing a bug — patch-id and fix provenance";`
  - `fn closing_advisory(kind: &str, new_status: &str, content: &str) -> Option<String>` — `None` unless `kind == "bug"`, `new_status` is `fixed` or `mitigated`, and `fix_anchor_gap` is `Some`.
- Private: `fn fix_anchor_remedy(gap: FixAnchorGap) -> &'static str`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn closing_advisory_names_what_is_missing_and_the_section() {
    let none = closing_advisory("bug", "fixed", "## Summary\n\nFixed.\n").expect("fires");
    assert!(none.contains("no fix anchor"), "{none}");
    assert!(none.contains("## Fix provenance") && none.contains("no_fix_commit"), "{none}");
    assert!(none.contains(CLOSING_SECTION) && none.contains("tracker-conventions"), "{none}");
    assert!(none.contains("Archiving it will be refused"), "{none}");

    let sha_only = "## Fix provenance\n\n- **SHA:** `5a72304c` (`experiments`)\n";
    let pid = closing_advisory("bug", "mitigated", sha_only).expect("fires");
    assert!(pid.contains("no patch-id") && pid.contains("orphans"), "{pid}");
    assert!(!pid.contains("no fix anchor"), "SHA-only must not be told it has nothing: {pid}");

    // Review Focus 2: a patch-id on the SAME line as the SHA is not parsed.
    let same_line = "## Fix provenance\n\n- **SHA:** `5a72304c` **patch-id:** `e9f8df63b911`\n";
    assert_eq!(fix_anchor_gap(same_line), Some(FixAnchorGap::PatchId));
}

#[test]
fn closing_advisory_is_silent_when_anchored_or_out_of_scope() {
    let pair = "## Fix provenance\n\n- **SHA:** `5a72304c` (`experiments`)\n- **patch-id:** `e9f8df63b911`\n";
    assert_eq!(fix_anchor_gap(pair), None);
    assert!(closing_advisory("bug", "fixed", pair).is_none());
    let nfc = "---\nkind: bug\nstatus: fixed\nno_fix_commit: doc note only\n---\n\nBody.\n";
    assert!(closing_advisory("bug", "fixed", nfc).is_none());
    for status in ["wontfix", "active", "open", "archived"] {
        assert!(closing_advisory("bug", status, "Body.\n").is_none(), "{status}");
    }
    assert!(closing_advisory("tracker", "fixed", "Body.\n").is_none());
}

#[test]
fn closing_section_is_a_real_heading_of_tracker_conventions() {
    let guide = include_str!("../../prompts/guides/tracker-conventions.md");
    assert!(guide.contains(&format!("\n### {CLOSING_SECTION}\n")));
}
```

Check while writing: if `declares_no_fix_commit` needs a different frontmatter shape than `nfc` above, copy the shape from `a_non_empty_no_fix_commit_discharges_the_archive_guard_and_an_empty_one_does_not` (`update.rs:4257`), not a new one.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/with-slot.sh cargo test --lib -- closing_`
Expected: compile error, `closing_advisory` / `fix_anchor_gap` / `CLOSING_SECTION` not found.

- [ ] **Step 3: Implement the four items and refactor the guard**

`fix_anchor_remedy` returns the guard's two current hint texts with the archive-only clauses removed (`"; one update may add it and archive in the same call"` and the closing `"Then archive."`). Copy, do not reword:
- `Nothing`: "Add a `## Fix provenance` section with two bullets, `- **SHA:** `<sha>` (`experiments`)` and `- **patch-id:** `<id>`` from `git show <sha> | git patch-id --stable`, via doc(action=\"update\", patch={body_edits: [...]}). If nothing was committed, declare `no_fix_commit: \"<reason>\"` via patch.extra instead."
- `PatchId`: "Add the patch-id as its own bullet under the SHA, `- **patch-id:** `<id>``, with the id from `git show <sha> | git patch-id --stable` (docs/RELEASE.md § Citing a fix), via doc(action=\"update\", patch={body_edits: [...]}). Both labels must be line-start bullets outside any fence: a patch-id written on the SAME line as the SHA is not parsed. If nothing was committed, declare `no_fix_commit: \"<reason>\"` via patch.extra instead."

`refuse_unanchored_archive` keeps its signature, its early return and both error headlines unchanged. Its hint becomes `format!("{} One update may add it and archive in the same call. Then archive.", fix_anchor_remedy(gap))`, with `gap` from `fix_anchor_gap`.

`closing_advisory` headlines (then a space, the remedy, a space, the pointer):
- `Nothing`: "This `{status}` bug declares no fix anchor. Archiving it will be refused until it does."
- `PatchId`: "This `{status}` bug declares a fix SHA but no patch-id, and a SHA alone orphans on the next rebase. Archiving it will be refused until it does."
- pointer: `See get_guide("tracker-conventions") § {CLOSING_SECTION}.`

Doc comment on `closing_advisory`: why it informs and does not refuse (operator decision 2026-11-03), and the bug path `docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md`.

- [ ] **Step 4: Run the new tests and the guard's existing tests**

Run: `scripts/with-slot.sh cargo test --lib -- closing_ unanchored no_fix_anchor no_patch_id fix_anchor`
Expected: PASS, including `archiving_a_fixed_or_mitigated_bug_with_no_fix_anchor_is_refused_and_writes_nothing` (update.rs) and the `mv.rs` refusal tests that assert `## Fix provenance`, `no_fix_commit`, `no patch-id`, `orphans`. Then `scripts/with-slot.sh cargo test --lib librarian::tools::mv` and `librarian::tools::update` in full: PASS.

- [ ] **Step 5: Commit**

```bash
rustfmt --edition 2021 --check src/librarian/tools/doctor.rs
git add src/librarian/tools/doctor.rs
git diff --cached --name-only
git commit -m "feat(librarian): closing_advisory shares the archive guard's fix-anchor remedy (b9eec23561e3299b)" -- src/librarian/tools/doctor.rs
```

---

### Task 2: Wire the advisory into `doc update`, document it, record it

**Files:**
- Modify: `src/librarian/tools/update.rs` — `call` (`:539`), beside the archive guard (`:813`) and the response build (`:941`)
- Test: `src/librarian/tools/update.rs` `mod tests` (`:1082`)
- Modify: `src/prompts/guides/tracker-conventions.md` — end of `### Closing a bug — patch-id and fix provenance`
- Modify (via `doc(update)`): `docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md`

**Interfaces:**
- Consumes: `super::doctor::closing_advisory(kind: &str, new_status: &str, content: &str) -> Option<String>` from Task 1.
- Produces: response key `fix_anchor_advisory: String`, present only when the advisory fires.

- [ ] **Step 1: Write the failing tests**

All use `mk_ctx`, `mk_bug(&ctx, "bug", "<status>", "<body>")` and `call(&ctx, json)`; each asserts the call is `Ok` and `v["updated"] == true`. The module writes `serde_json::json!` fully qualified; follow it (the sketch below abbreviates it as `json!`).

```rust
#[tokio::test]
async fn closing_an_unanchored_bug_returns_the_advisory_and_still_writes() {
    for status in ["fixed", "mitigated"] {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = mk_bug(&ctx, "bug", "open", "## Summary\n\nNo pointer.\n").await;
        let v = call(&ctx, json!({"id": id, "patch": {"status": status}})).await.unwrap();
        assert_eq!(v["updated"], json!(true));
        let adv = v["fix_anchor_advisory"].as_str().expect("advisory present");
        assert!(adv.contains("no fix anchor") && adv.contains("tracker-conventions"), "{adv}");
        // the status really changed on disk / in the catalog
        assert_eq!(artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap().status, status);
    }
}

#[tokio::test]
async fn the_top_level_status_spelling_also_gets_the_advisory() {
    // {"id": id, "status": "fixed"} — no patch object.
    // assert v["fix_anchor_advisory"].is_string()
}

#[tokio::test]
async fn closing_and_adding_provenance_in_one_call_is_silent() {
    // Review Focus 1: patch {status: "fixed", body_edits: [insert_after the "## Summary"
    // section, at "end-of-section", content = FIX_PROVENANCE]} (const at update.rs:4107).
    // assert v.get("fix_anchor_advisory").is_none()
}

#[tokio::test]
async fn an_empty_no_fix_commit_still_gets_the_advisory() {
    // Review Focus 3: patch {status: "fixed", extra: {no_fix_commit: ""}} -> advisory present.
    // A non-empty no_fix_commit in the same shape -> absent.
}

#[tokio::test]
async fn re_setting_fixed_on_an_unanchored_fixed_bug_fires_again() {
    // Review Focus 4: mk_bug(.., "fixed", ..), then patch {status: "fixed"} -> present.
}

#[tokio::test]
async fn updates_that_do_not_close_are_silent() {
    // mk_bug(.., "fixed", unanchored body): patch {title: "renamed"} -> absent.
    // mk_bug(.., "open", ..): patch {status: "wontfix"} -> absent; {status: "investigating"} -> absent.
    // mk_bug(&ctx, "tracker", "active", ..): patch {status: "fixed"} -> absent (non-bug kind).
}
```

Adapt `artifact::get(...)`'s exact call to the form `archiving_a_fixed_or_mitigated_bug_with_no_fix_anchor_is_refused_and_writes_nothing` (`update.rs:4127`) already uses. If `mk_bug` cannot create `kind: "tracker"` at its fixed `docs/issues/` path, create the tracker the way the nearest existing tracker test in this module does.

- [ ] **Step 2: Run to verify they fail**

Run: `scripts/with-slot.sh cargo test --lib -- update::tests::closing_ update::tests::the_top_level update::tests::an_empty_no_fix update::tests::re_setting update::tests::updates_that_do_not`
Expected: FAIL on `advisory present` (key absent); the silent tests may pass already.

- [ ] **Step 3: Implement in `update::call`**

Directly after the archive-guard `if` (`update.rs:813-815`), before any write:
`let fix_anchor_advisory = patch.status.as_deref().and_then(|s| super::doctor::closing_advisory(&row.kind, s, &new_content));`
After `let mut out = json!({"id": a.id, "updated": true});` (`:941`):
`if let Some(t) = fix_anchor_advisory { out["fix_anchor_advisory"] = json!(t); }`
One comment above the first line: inform, not refuse; same predicate as the guard; bug path.

- [ ] **Step 4: Run the new tests, the module, and the ceiling tests**

Run: `scripts/with-slot.sh cargo test --lib librarian::tools::update`
Expected: PASS.
Run: `scripts/with-slot.sh cargo test --lib -- a_p50_session_stays_under_the_committed_emission_byte_ceiling a_status_update_keeps_the_librarians_anti_wipe_sections_first`
Expected: PASS with no constant changed.

- [ ] **Step 5: Add one paragraph to the guide**

At the end of `### Closing a bug — patch-id and fix provenance` in `src/prompts/guides/tracker-conventions.md` (with `edit_file`, heading-addressed, `insert_after` at end of section):

> When `doc(action="update")` sets a bug to `fixed` or `mitigated` and the file as written has no recoverable pair, the response carries `fix_anchor_advisory` naming what is missing. The write still happens; the advisory informs, and archiving stays refused until the pair or `no_fix_commit:` is present.

Run: `scripts/with-slot.sh cargo test --lib prompts` — Expected: PASS (guide-index, heading-citation and waiver gates). If a section-size gate fails, report it; do not raise a cap.

- [ ] **Step 6: Measure the advisory size**

Add nothing to the code for this. In the Step 1 test for `fixed`, temporarily `eprintln!("{}", adv.len())`, run once, note both sizes (`Nothing`, `PatchId` via a SHA-only body), then remove the line.

- [ ] **Step 7: Gate**

`rustfmt --edition 2021 --check src/librarian/tools/update.rs src/librarian/tools/doctor.rs`, then `./scripts/gate.sh` (or the detached-worktree form in Global Constraints). Expected: exit 0.

- [ ] **Step 8: Commit the code and guide**

```bash
git add src/librarian/tools/update.rs src/prompts/guides/tracker-conventions.md
git diff --cached --name-only
git commit -m "feat(librarian): doc update names a missing fix anchor in fix_anchor_advisory when it closes a bug (b9eec23561e3299b)" -- src/librarian/tools/update.rs src/prompts/guides/tracker-conventions.md
```

- [ ] **Step 9: Record it in the bug**

`doc(action="update", id="b9eec23561e3299b", patch={body_edits: [{heading: "## Fix", action: "insert_after", at: "end-of-section", content: …}]})` with: the two commit SHAs, the measured advisory sizes from Step 6, that step 1's closing knowledge is now delivered as an advisory (approach B of the spec), that the status stays `investigating` because step 2 (`check~<name>`) is open, and the re-measure trigger: re-run the transcript scan two weeks after release; below 42% unanchored closings passes, otherwise approach C. Then commit that file by pathspec as in Step 8.
