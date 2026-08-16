---
status: open
opened: 2026-08-16
closed:
severity: high
owner: marius
related:
  - docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md
  - docs/trackers/open-issue-work-queue.md
tags:
  - librarian
  - guard-gap
  - edit_file
  - twin-tool-defect
  - catalog-drift
kind: bug
---

# BUG: edit_file's replace_all path writes librarian-managed artifacts with no guard — and the markdown refusal hint is what sends you there

## Summary

`edit_file` guards exactly one of its three write paths against
librarian-managed artifacts. The two unguarded paths are precisely the ones the
tool's own markdown refusal hint tells you to use. A file-wide replace on an
augmented tracker therefore succeeds silently: no `field_patch` event, no
body-shrink guard, no `replaced_subsections` report, and the catalog's
`updated_at` never advances.

Found by using it, not by reading it — while re-pointing citations during the
BL-1 archive, an `edit_file(replace_all=true)` on `docs/trackers/tool-usage-patterns.md`
(augmented, `entry_collection: observations`) returned `"ok"` where it should
have been refused.

## Symptom (Effect)

`edit_markdown` correctly refuses a managed artifact. Reach for `edit_file`
instead and it refuses too — but its hint advertises the way around:

```
{"ok": false,
 "error": "Use edit_markdown for markdown files",
 "hint": "edit_markdown provides heading-based editing for .md files. edit_file is
          still allowed with insert='prepend'/'append' or replace_all=true
          (file-wide find/replace)."}
```

Take the advertised route on a managed artifact and it goes through:

```
edit_file(path="docs/trackers/tool-usage-patterns.md",
          old_string="docs/issues/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md",
          new_string="docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md",
          replace_all=true)
-> "ok"
```

No `librarian_guard` error. Compare `edit_markdown` on the same file, which
returns a `librarian_guard` error pointing at `artifact(action="update")`.

## Reproduction

Commit `a9a397a9` (`experiments`), build `7c91cdf7`.

1. Pick any augmented artifact — e.g. `docs/trackers/tool-usage-patterns.md`,
   id `f2ecdd76a6189efb`, `entry_collection: observations`.
2. `edit_file(path=<that file>, old_string=<any present string>, new_string=…, replace_all=true)`
3. Observe `"ok"`.
4. `artifact(action="get", id="f2ecdd76a6189efb")` → `updated_at` is unchanged.
5. `artifact_event(action="list", artifact_id="f2ecdd76a6189efb")` → no
   `field_patch` for the write.

Batch form reproduces identically when every element carries `replace_all: true`.

## Environment

Linux, codescout `0.15.0`, branch `experiments`, MCP stdio transport, project
`codescout`. Build `7c91cdf7`; the guard call site is unchanged since well before it.

## Root cause

**`edit_file` has three write paths and only the `insert` branch guards.**

*Read from `src/tools/edit_file/mod.rs` at commit `a9a397a9`; the empirical half
measured 2026-08-16 by making the call and reading the catalog back.*

| Path | Reads target | Writes | `guard_not_librarian_managed`? |
|---|---|---|---|
| batch `edits[]` | `mod.rs:472` | `mod.rs:563` | **no** |
| `insert` prepend/append | `mod.rs:591` | `mod.rs:605` | **yes — `mod.rs:593`** |
| single `old_string`/`new_string` (`perform_edit`, `mod.rs:678`) | `mod.rs:692` | via `mod.rs:256` | **no** |

`guard_not_librarian_managed` is called exactly once in the whole file
(`src/tools/edit_file/mod.rs:593`), inside the `insert` branch.

**The markdown gate then steers callers into the unguarded paths.** At
`src/tools/edit_file/mod.rs:436-438`, a `.md` target is permitted when *any* of
these hold:

```rust
let allowed = matches!(insert_mode, Some("prepend") | Some("append"))
    || single_replace_all
    || batch_all_replace_all.unwrap_or(false);
```

Of those three escapes, only the first lands on the guarded path. The refusal
hint at `mod.rs:441-442` names all three. So the sequence a caller actually
walks — `edit_markdown` refuses (managed) → `edit_file` refuses (markdown) →
hint says `replace_all=true` → write succeeds unguarded — is not an unlikely
path through the code. It is the path the error messages compose into.

This is the **twin-tool defect class** again, in its sharpest form: three
sibling paths in one file share a failure mode, and only one got the arm. Three
prior instances were catalogued during the 2026-08-15 tool-usage investigation
(`docs/trackers/2026-08-15-tool-usage-investigation.md`); each time the guard
landed on the first-written path and the later siblings were left bare.

## Evidence

### The write landed and the catalog never noticed

Measured 2026-08-16, immediately after the `replace_all=true` write above:

```
catalog updated_at:  2026-08-15T15:02:07+00:00
file mtime:          2026-08-16T08:53:01+00:00
now:                 2026-08-16T08:58:07+00:00
```

The body on disk moved; the catalog row is **21 hours stale**. Any consumer
reading `updated_at` — `artifact_refresh(action="list_stale")`, freshness
reporting, `librarian(action="context")` recency ranking — sees a file that has
not changed since yesterday afternoon.

### Only one guard call site exists

```
grep librarian_guard --glob '*.rs'  ->
  src/util/librarian_guard.rs        4
  src/tools/edit_file/mod.rs         2   <- 1 call (mod.rs:593) + 1 comment (mod.rs:592)
  src/tools/markdown/edit_markdown.rs 2
  src/tools/markdown/read_markdown.rs 2
  src/usage/db.rs                    2
  src/librarian/tools/update.rs      1
  src/tools/markdown/frontmatter.rs  1
  src/util/mod.rs                    1
```

`edit_markdown` and `read_markdown` each guard; `edit_file` guards one branch of three.

### The four protections that go missing

Identical in shape to the params/body asymmetry filed in
`docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md`:

| | via `artifact(update)` | via `edit_file(replace_all)` |
|---|---|---|
| 50% body-shrink refusal | yes (`src/librarian/tools/update.rs:430`) | **no** |
| `force=true` opt-in to destroy | yes | **not required** |
| `replaced_subsections` report | yes | **no** |
| `field_patch` forensic event | yes | **no** |

Unlike the params bug, git *does* back this one up — the file is tracked. That
is the only reason this is high and not critical.

## Hypotheses tried

1. **Hypothesis:** the guard lives in a shared helper both paths call
   (`read_edit_target`).
   **Test:** read `read_edit_target` (`src/tools/edit_file/mod.rs:662-676`).
   **Verdict:** rejected — it is a plain `read_to_string` with a friendlier
   ENOENT message. No guard.

2. **Hypothesis:** the librarian guard fires on write via `atomic_write` or the
   catalog watcher rather than in the tool.
   **Test:** made the call on a known-managed artifact and read `updated_at` back.
   **Verdict:** rejected — the write succeeded and the catalog row is 21 hours stale.

3. **Hypothesis:** `tool-usage-patterns.md` is not actually managed, so the
   guard was right to pass it.
   **Test:** `artifact(action="get", id="f2ecdd76a6189efb")` →
   `augmentation.entry_collection == "observations"`.
   **Verdict:** rejected — it is augmented, and `edit_markdown` refuses it.

## Fix

Not yet implemented. Proposed, cheapest first:

1. **Hoist the guard above the branch.** All three paths already resolve the
   target and read its content; move `guard_not_librarian_managed` to fire once
   after `read_edit_target` in each, or better, into a small wrapper both
   `perform_edit` and the batch path call. One call site becoming three (or one
   shared) closes the whole class here rather than patching the branch that got
   noticed.

2. **Stop advertising an escape that does not hold.** The hint at
   `src/tools/edit_file/mod.rs:441-442` should not name `replace_all=true` as a
   markdown route without saying it is still refused on managed artifacts —
   otherwise fixing (1) turns a helpful hint into a hint that reliably produces
   an error, which is the same defect the json_path hint bug was about
   (`docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md`).

3. **Consider a regression test per write path, not per tool.** The reason this
   survived is that "does edit_file guard managed artifacts?" has three answers
   and the suite only ever asked once. A table-driven test over
   {batch, insert, single} × {managed, unmanaged} would have caught it and will
   catch the next sibling added.

## Tests added

None yet — bug is `open`. The regression test must assert refusal on **all three**
write paths; a test that only covers `insert` reproduces the blind spot that
caused this.

## Workarounds

Use `artifact(action="update", id=…, patch={body_edits: [...]})` for any file
under `docs/trackers/` or any artifact with a non-null `augmentation`. When
unsure whether a markdown file is managed, `artifact(action="find",
filter={"rel_path": {"contains": "<name>"}})` and check for `augmentation`
before editing — `edit_file` will not tell you.

If a `replace_all` write has already landed on a managed artifact, the body on
disk is correct but the catalog is stale; `librarian(action="reindex")` re-syncs
it. The `field_patch` event is not recoverable.

## Resume

Start at `src/tools/edit_file/mod.rs:436-444` (the markdown gate) and
`mod.rs:593` (the sole guard call). Decide between hoisting the guard into a
shared helper vs. three call sites — the batch path at `mod.rs:472` and
`perform_edit` at `mod.rs:678` are the two that need it. Write the table-driven
test first: `{batch, insert, single} × {managed, unmanaged}`, asserting a
`librarian_guard` error for all three managed cases. `insert` should already
pass, which is the point — it proves the test discriminates.

Then fix the hint at `mod.rs:441-442` in the same commit, or it will
start recommending a call that now always fails.

## References

- `src/tools/edit_file/mod.rs:436-444` — the markdown gate and its hint
- `src/tools/edit_file/mod.rs:593` — the sole `guard_not_librarian_managed` call
- `src/tools/edit_file/mod.rs:678` — `perform_edit`, the unguarded single path
- `src/util/librarian_guard.rs` — the guard itself
- `docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md` — the same
  four-protection asymmetry on the params surface
- `docs/trackers/2026-08-15-tool-usage-investigation.md` — the twin-tool defect class
- `docs/trackers/open-issue-work-queue.md` — BL-21
