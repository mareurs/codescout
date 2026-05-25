---
status: fixed
opened: 2026-05-25
closed: 2026-05-25
severity: high
owner: marius
related: []
tags: [librarian, artifact, body-edit, data-loss]
kind: bug
---

# BUG: `artifact(update, patch={body: ...})` silently overwrites entire body with partial content

## Summary

`artifact(update)` exposed only a total-overwrite `patch={body}` surface for
augmented tracker bodies. When an LLM `get`s one section, then `update`s the
body with that section as the value, the rest of the document is wiped
without warning. A real ~600-line `retrieval-experiments.md` body was lost
this way on 2026-05-25 in the MRV-poc project; the user caught it via
`git diff --cached --stat` showing "646 lines changed, 46 insertions(+),
600 deletions(-)" and reverted with `git checkout HEAD --`.

Defense in depth: four layers added — `body_edits[]` surgical surface,
50% shrink guard, `deny_unknown_fields` on `UpdatePatch`, and `field_patch`
event emission on every body mutation.

## Symptom (Effect)

LLM workflow that triggered the loss (reconstructed from MRV-poc's
`.codescout/usage.db`, call id 22795 at 13:24:04):

```text
1. artifact(action="get", id=X, heading="## Currently Shipped")
   → returns the body of that section only

2. artifact(action="update", id=X, patch={body: <the retrieved section>})
   → file shrinks from 642 → 88 lines (-554 lines, -86%)
   → outcome: "success" — no warning, no error
```

Earlier same day (07:27:57) the same operator hit the same trap via a
different mechanism (`cp /tmp/_retrieval_exp_new_body.md
docs/trackers/retrieval-experiments.md`) and recovered the same way.

## Reproduction

```text
# 1. Create a tracker with substantial body
artifact(action="create", repo=R, rel_path="t.md", kind="tracker",
         title="T", body="...600 lines of content...")

# 2. Get one section
artifact(action="get", id=<id>, heading="## SomeSection")
   → returns body string for that section, e.g. 80 lines

# 3. Update with that string as the body
artifact(action="update", id=<id>, patch={body: <80-line string>})
   → file is now 80 lines. Everything else gone. No error.
```

Repository: `code-explorer` (codescout). Fix branch: `experiments`.
Reproducer test: `body_shrink_guard_blocks_destructive_overwrite`
in `src/librarian/tools/update.rs`.

## Environment

- Project: codescout (this repo), affecting any project using the librarian
- Discovery context: MRV-poc retrieval-experiments tracker
- Workspace: parking_lot::Mutex<Catalog> + sqlite (in-memory + on-disk)
- MCP transport: irrelevant — fault is in the artifact `update` handler

## Root cause

`UpdatePatch.body: Option<String>` at `src/librarian/tools/update.rs:9`
accepted any string and the `call` function passed it through to
`std::fs::write` with no length-delta check. Three contributing factors:

1. **No surgical body edit surface.** `edit_markdown` refused to touch
   librarian-managed files (see
   `src/util/librarian_guard.rs::guard_not_librarian_managed`), pointing
   the caller at `artifact(update)` — but `update` had no per-section
   surface. The LLM's only option for "edit one section" was "rewrite the
   whole body," which it did with just the section in hand.
2. **No deserialiser strictness.** `UpdatePatch` lacked
   `#[serde(deny_unknown_fields)]`, so misspelled patch keys (e.g.
   `body_prepend_section`) returned `success` and silently no-opped —
   which encouraged the LLM to keep trying variations until it landed
   on `patch={body: <section>}`.
3. **No forensic trail.** `events::insert` was not called from
   `update.rs::call`, so a postmortem required scraping
   `.codescout/usage.db` SQL by hand. `artifact_event(list)` returned `[]`.

## Evidence

### Usage DB query identifying the destructive call

```text
sqlite3 .codescout/usage.db "SELECT id, called_at, tool_name, outcome,
  substr(input_json, 1, 300) FROM tool_calls WHERE called_at > '2026-05-25 06:00'
  AND (input_json LIKE '%retrieval-experiments%' OR input_json LIKE '%1e14da7042af28d1%')
  ORDER BY id ASC"

→ 22795 | 2026-05-25 13:24:04 | artifact | success |
  {"action":"update","id":"1e14da7042af28d1",
   "patch":{"body":"## Currently Shipped\n\n> **Scope note:** ..."}}
```

The `patch.body` string was the result of a prior
`artifact(get, heading="Currently Shipped")` call — a 6KB section — not
the intended full 63889-byte document.

### Fallback-cascade preceding the destructive call

```text
13:22:49 | artifact update {patch={body_prepend_section: null}} → success (no-op)
13:22:57 | edit_markdown insert_after → error
13:23:05 | artifact get start_line=1 end_line=60 → error (invalid params on get)
13:23:09 | artifact get heading="Currently Shipped" → ok
13:24:04 | artifact update {patch={body: <section just retrieved>}} → success, BUT WIPES FILE
```

The "success" outcome on the unknown patch key `body_prepend_section`
falsely signalled "the API accepts that shape." If it had returned
`RecoverableError("unknown field")`, the LLM would have stopped earlier.

## Hypotheses tried

1. **Hypothesis:** `artifact_augment(merge=false)` foot-gun cleared
   `render_template`, causing the body to re-render from params alone.
   **Test:** queried augmentation row — no `render_template`,
   `append_mode`, or `history_cap` fields present.
   **Verdict:** rejected.

2. **Hypothesis:** A refresh round-trip wrote a thin body.
   **Test:** `last_refreshed_at: null`, `refresh_count: 0` — the artifact
   has never been refreshed.
   **Verdict:** rejected.

3. **Hypothesis:** Direct `patch={body}` overwrite from
   `artifact(update)`.
   **Test:** ran the smoking-gun SQL above against MRV-poc's `usage.db`.
   **Verdict:** confirmed at call id 22795.

## Fix

Four-layer defense in depth on `experiments` branch. Cherry-pick to
`master` pending (run `git rev-parse HEAD` on master after cherry-pick
and update this Fix section with the master-side SHA).

**Layer 1 — body-shrink guard.** Refuses any body write that would reduce
the file by more than 50% unless `force=true`.
- `src/librarian/tools/update.rs::call` — guard before `std::fs::write`,
  using new `SHRINK_GUARD_MIN_BYTES = 200` constant.
- `src/tools/markdown/edit_markdown.rs::call` — parallel guard before
  `atomic_write`.
- Exemptions: files <200 bytes, augmentations with
  `append_mode + history_cap` (legitimate history trimming),
  `force=true` explicit opt-in.

**Layer 2 — strict patch deserializer.**
`src/librarian/tools/update.rs:8-36` adds `#[serde(deny_unknown_fields)]`
to `UpdatePatch` and `Args`. Misspelled keys now return
`RecoverableError`.

**Layer 3 — `body_edits[]` surgical surface.**
`patch={body_edits: [{heading, action, content?|old_string+new_string?,
at?, replace_all?, include_subsections?}]}` mirrors `edit_markdown`'s
batch shape; routed through `perform_section_edit_ext` / `perform_scoped_edit`
helpers (now `pub(crate)` via
`src/tools/markdown/mod.rs:3`). Mutual exclusion with bare `body`
enforced; combined with `params` updates atomically.

**Layer 4 — forensic events.** Every body mutation emits an `events` row
with `kind="field_patch"`, payload `{field: "body", prev_bytes, new_bytes,
edits_count, mode, forced}`. Reuses the existing `field_patch` event kind
(no schema migration needed).

**Prompt surfaces updated:**
- `src/prompts/source.md` — `server_instructions` librarian-topic line
  now mentions "body editing".
- `src/prompts/guides/librarian.md` — new "## Body Editing Surfaces"
  section explicitly diagrams the anti-pattern and the surgical fix.
- `src/librarian/tools/artifact.rs` — `patch` schema description now
  declares `body_edits`, `force`, and the mutual-exclusion rule.
- `docs/architecture/augmented-artifacts.md` — new "## Body editing
  surfaces — `body_edits` vs. `body`" section after the
  managed-markdown gate explanation.

## Tests added

In `src/librarian/tools/update.rs::tests`:

- `body_shrink_guard_blocks_destructive_overwrite` — 600-byte seed,
  overwrite with "tiny" → error mentions "body-shrink guard", "body_edits",
  "force".
- `body_shrink_guard_allows_with_force` — same setup + `force=true` →
  succeeds, file contains new short body.
- `body_shrink_guard_skips_tiny_files` — file <200 bytes shrinks freely.
- `unknown_patch_key_rejected` — `patch={body_prepend_section: null}` →
  RecoverableError listing the bad key or "unknown field".
- `body_edits_inserts_after_section` — `body_edits` insert_after preserves
  siblings + original section body.
- `body_and_body_edits_mutually_exclusive` — both set → error mentions
  "mutually exclusive".
- `body_patch_event_emitted_on_body_change` — body update emits a
  `field_patch` event with `payload.field="body"`; frontmatter-only update
  does NOT emit one.

All 23 tests in `librarian::tools::update::tests` green; full suite
2502/2502 passing.

The edit_markdown path's guard has no dedicated integration test (no
existing `EditMarkdown::call` tests; the structurally-identical logic in
update.rs IS covered). Acceptable: load-bearing site is covered.

## Workarounds

Pre-fix: catch via `git diff --cached --stat` before committing — look
for any file with `XX% reduction` shape in the diffstat. Recovery:
`git checkout HEAD -- <file>` then redo the edit surgically. This is
the path the user took on 2026-05-25 (twice: 07:27 and 13:24).

Post-fix: no workaround needed — the guard will refuse the destructive
call. If a legitimate large shrinkage is needed, pass `force=true`.

## Resume

N/A — fixed on `experiments`. Open follow-up if needed: the
`edit_markdown` shrink guard (Layer 1b) has no dedicated test because
`EditMarkdown::call` has no existing integration test harness. Adding one
(an `EditMarkdown` ToolContext fixture parallel to
`src/librarian/tools/update.rs::tests::mk_ctx`) would be the right shape
if a future bug surfaces on that code path.

## References

- Original incident transcript: MRV-poc session 2026-05-25 13:22-13:25.
- Smoking-gun call: MRV-poc `.codescout/usage.db` tool_calls id 22795.
- Earlier same-day occurrence (different mechanism): call id 22588 at
  07:27:57 (`cp /tmp/_retrieval_exp_new_body.md ...`).
- Conceptual docs: `docs/architecture/augmented-artifacts.md`
  § "Body editing surfaces — `body_edits` vs. `body`".
- Prompt surface: `src/prompts/guides/librarian.md`
  § "## Body Editing Surfaces".
