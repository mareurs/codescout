---
id: ec21d94e3f156b65
kind: bug
status: fixed
title: 'BUG: body_edits replace+include_subsections silently destroys prior section content when the whole-file write is net larger'
tags:
- librarian
- artifact-update
- data-loss
- trackers
closed: 2026-08-06
opened: 2026-08-06
owner: marius
related:
- '6b6e71ede6fff4c2'
severity: high
---

# BUG: body_edits replace+include_subsections silently destroys prior section content when the whole-file write is net larger

## Summary

`artifact(update, patch={body_edits:[...]})` with `action:"replace"` and
`include_subsections:true` can silently overwrite an existing child heading's
content — with no error, no warning, and a `field_patch` audit event that
looks healthy — whenever the replacement text is byte-for-byte larger than
what it removes. Both of the tool's existing protections are structurally
blind to this shape: the body-shrink guard compares whole-file aggregate
bytes (net growth never trips it), and the per-entry `include_subsections`
guard (BUG-043) is unconditionally skipped by the very flag needed to
legitimately touch a section that has children. Severity `high`: this is the
core surgical-edit surface the whole `body_edits[]` API (`docs/issues/archive/2026-05-25-augmented-artifact-body-overwrite.md`,
id `6b6e71ede6fff4c2`) was built to make safe for exactly this use case —
append-only ledgers (`F-N`/`W-N` session logs) — and the loss is
self-concealing: a reused entry number reads as "correctly numbered", not
"missing".

## Symptom (Effect)

Real incident (external repo, EDU-Planner `backend-kotlin` monorepo, tracker
`docs/trackers/its-integration-session-log.md`, artifact id
`051682efd7186dad`). To append a second win to a `## Wins` section that
already contained one entry (`### W-1 — <original content>`), this call was
made:

```
artifact(action="update", id="051682efd7186dad", patch={"body_edits": [{
    "heading": "## Wins",
    "action": "replace",
    "include_subsections": true,
    "content": "## Wins\n\n### W-1 — <new content>\n…\n### W-2 — <new content>\n…"
}]})
```

Result: `{"id": "051682efd7186dad", "updated": true}` — no error, no
warning, nothing else in the response.

Effect: the pre-existing `### W-1` entry (different text than what's in the
call above) was destroyed. The replacement's `### W-1` heading reused the
same number for unrelated content, so the section read as populated and
correctly numbered afterward — nothing about the resulting file looked
wrong. It was recovered only because an unrelated file copy happened to
preserve an older version, discovered by chance hours later while diffing
two copies' heading lists during a merge.

## Reproduction

Verified live against this repo at commit `553e618e` (branch `experiments`):
a scratch `#[tokio::test]` was inserted into
`src/librarian/tools/update.rs::tests` via `edit_code`, run with
`cargo test --lib`, observed to pass (i.e. reproduce the bug), then reverted
with `git checkout -- src/librarian/tools/update.rs` — `git status` confirms
the file carries no diff after the revert; no permanent test was left in
the tree. Paste this into `src/librarian/tools/update.rs`'s `tests` module
and run `cargo test --lib librarian::tools::update::tests::repro_body_edits_replace_growth_blind_spot`:

```rust
#[tokio::test]
async fn repro_body_edits_replace_growth_blind_spot() {
    let tmp = TempDir::new().unwrap();
    let ctx = mk_ctx(tmp.path().to_path_buf());
    let original_win = "Original content that must survive this call.";
    // Padding lives in a SIBLING section untouched by the edit, so the
    // whole-file byte count stays large regardless of what happens inside
    // "## Wins" — isolates net-larger-whole-file-write from an incidental
    // whole-file shrink.
    let padding = "padding ".repeat(30);
    let body = format!(
        "## Wins\n\n### W-1 — original entry\n\n{}\n\n## Padding\n\n{}",
        original_win, padding
    );
    let v = crate::librarian::tools::create::call(
        &ctx,
        serde_json::json!({
            "repo": "r", "rel_path": "wins.md",
            "kind": "tracker", "title": "T", "body": body,
        }),
    )
    .await
    .unwrap();
    let id = v["id"].as_str().unwrap().to_string();
    let original_file = std::fs::read_to_string(tmp.path().join("wins.md")).unwrap();

    // Replacement for "## Wins" alone is deliberately verbose so the WHOLE
    // FILE still grows overall, even though only this section changed —
    // mirrors the real incident (adding W-2 made the file net larger).
    let new_section = "## Wins\n\n### W-1 — different content\n\nNot the original — this replacement text is deliberately long-winded so the whole file still grows overall even though only this section changed.\n\n### W-2 — new entry\n\nSecond win.\n";
    let out = call(
        &ctx,
        serde_json::json!({
            "id": id,
            "patch": {"body_edits": [{
                "heading": "## Wins",
                "action": "replace",
                "include_subsections": true,
                "content": new_section,
            }]},
        }),
    )
    .await
    .expect("neither guard should have allowed this silently — but it does");
    assert_eq!(out["updated"], serde_json::json!(true));

    let content = std::fs::read_to_string(tmp.path().join("wins.md")).unwrap();
    assert!(content.len() > original_file.len(), "whole file grew");
    assert!(
        !content.contains(original_win),
        "original W-1 content was silently destroyed and the call reported success"
    );
}
```

Observed (this session): `test ... ok` — both assertions passed. The whole
file grew (394 → larger after fixing an earlier test-setup mistake where
padding was accidentally inside the edited section, which correctly
triggered the shrink guard for an unrelated reason — see *Hypotheses
tried*), and the original W-1 text was gone.

## Environment

codescout `v0.15.0`, branch `experiments` at `553e618e1528bd260bf136f4b1cdd3f2afa8d503`, Linux. Applies to any caller of `artifact(action="update")`
regardless of MCP client.

## Root cause

`artifact(update)`'s only size-based protection
(`src/librarian/tools/update.rs::call`) compares WHOLE-FILE byte length
before vs. after, identically regardless of which patch shape produced the
write:

- `original = std::fs::read_to_string(&full)?` — whole file, pre-patch
  (`update.rs:252`).
- `body_changing = patch.body.is_some() || patch.body_edits.is_some()`
  (`update.rs:262`) — the guard gate does not distinguish the two patch
  shapes.
- For `patch.body_edits`, `new_content` is built by `apply_body_edits(&working, edits)` (`update.rs:280-294`), which reconstructs and returns the
  **entire file**, not just the edited section.
- The guard: `if body_changing && !a.force && original.len() >= SHRINK_GUARD_MIN_BYTES` (`update.rs:342`, `SHRINK_GUARD_MIN_BYTES = 200` at
  `update.rs:70`) `{ ... if !allow_history_trim && new_content.len() * 2 < original.len() { return Err(...) } }` (`update.rs:347`).

This condition is satisfied only when the **whole file** roughly halves or
worse. It says nothing about whether any individual heading's content
survived. A `body_edits` `replace` that deletes an existing child's text
while adding a same-or-larger sibling nets a `new_content.len()` at or
above `original.len()` — the condition at line 347 is false by
construction, so the destructive edit is never evaluated, no matter how
much was lost within the replaced span.

The second protection, `find_consumed_subsections`
(`src/tools/markdown/edit_markdown.rs`, the BUG-043 guard), detects exactly
this heading-nesting shape (`## Wins` containing `### W-1`) — but
`apply_body_edits` only calls it here:

```
if action == "replace" && !edit["include_subsections"].as_bool().unwrap_or(false) {
```
(`update.rs:164`). The incident's call passed `include_subsections: true`,
so this check — the ONE guard purpose-built for "replace is about to wipe
a nested heading" — is skipped entirely; `find_consumed_subsections` is
never invoked for that call, and no list of about-to-be-removed headings is
ever computed.

There is no third check. `apply_body_edits` (`update.rs:124-198`) does no
content-preservation comparison between the old and new section body for
`action="replace"`. `call`'s success path
(`let mut out = json!({"id": a.id, "updated": true});`, `update.rs:454`)
returns nothing else — no removed-heading list, no diff, no per-section
size signal. The `field_patch` audit event (`kind` at `update.rs:415`,
`payload` incl. `prev_bytes`/`new_bytes` at `update.rs:418`) records the
SAME whole-file aggregate the shrink guard compares — since the file grew,
`prev_bytes < new_bytes`, so even after-the-fact audit of the event log
looks like a benign append.

Net effect: for `body_edits` + `replace` + `include_subsections:true`, the
tool has exactly one behavioral contract — "overwrite this section's body
with `content`, verbatim, full stop" — and zero content-aware signal on
either side of the call (pre-write refusal or post-write forensics) if that
overwrite happens to drop something the caller meant to keep.

## Evidence

### The real incident

`docs/trackers/its-integration-session-log.md` (external repo
`backend-kotlin`, artifact id `051682efd7186dad`) lost its original `### W-1`
entry to a `body_edits` `replace` + `include_subsections:true` call whose
`content` reconstructed both `W-1` and a new `W-2` from scratch, not
verbatim-preserving the prior `W-1` text. No error surfaced at call time;
the `updated:true` response and (per the code above) a growth-shaped
`field_patch` event gave no indication anything was wrong. Recovery was
incidental — an unrelated stale file copy happened to retain the original
text, noticed only while diffing heading lists during an unrelated merge.

### Live reproduction, this session

See *Reproduction* — a scratch test was added to
`src/librarian/tools/update.rs`, run via `cargo test --lib`, observed
`ok` (both the "call succeeds" and "original content gone" assertions
passed), then the file was reverted (`git checkout --`) with `git status`
confirming zero residual diff.

## Hypotheses tried

1. **Hypothesis** — the body-shrink guard simply doesn't run on the
   `body_edits` path at all (only on `patch.body` total-overwrite).
   **Test** — read `update.rs::call` end to end.
   **Verdict** — rejected. `body_changing` (`update.rs:262`) and the guard
   gate (`update.rs:342`) are common to both patch shapes; `new_content` for
   the `body_edits` branch is the fully-reconstructed file
   (`update.rs:280-294`), fed through the identical comparison at line 347.
   **Evidence link** — *Root cause*.

2. **Hypothesis** — the guard runs on both paths but is blind to a
   net-larger write because it compares whole-file aggregate bytes, not
   per-section content.
   **Test** — read the comparison at `update.rs:347`; confirmed live via
   the *Reproduction* test (first draft accidentally put filler padding
   inside the edited section itself, which shrank the whole file when the
   padding wasn't carried into the replacement — the guard correctly fired,
   `394 → 163 bytes, 59% reduction`. Moving the padding into an untouched
   sibling section and making the replacement content itself longer than
   what it replaced reproduced the silent-loss case: `test ... ok`).
   **Verdict** — confirmed.
   **Evidence link** — *Reproduction*, *Evidence → Live reproduction*.

3. **Hypothesis** — the BUG-043 `include_subsections` guard already covers
   this; it's a documented, working protection, not a gap.
   **Test** — read `apply_body_edits` (`update.rs:124-198`), specifically
   the conditional at line 164.
   **Verdict** — rejected as applied to this call shape. The guard exists
   and does fire when `include_subsections` is omitted/false — but the
   incident's call passed `include_subsections: true`, which is precisely
   the input that disables it. The guard protects against *accidentally
   forgetting* a section has children; it does not, and structurally
   cannot, protect a caller who has consciously opted in but whose
   replacement content fails to faithfully carry forward an existing
   child's text.
   **Evidence link** — *Root cause*.

4. **Hypothesis** — this failure mode is already documented as a known
   anti-pattern and was simply missed.
   **Test** — read `get_guide("librarian")` § "Body Editing Surfaces",
   `docs/architecture/augmented-artifacts.md` § "Body editing surfaces —
   `body_edits` vs. `body`" (incl. its "Exemptions to the shrink guard" and
   "The anti-pattern to remember" subsections), the `patch` field's own
   input-schema description in `src/librarian/tools/artifact.rs`, and
   grepped `include_subsections`/`shrink` across `docs/**/*.md` and
   `src/prompts/*.md`.
   **Verdict** — rejected. All three surfaces document the ORIGINAL
   2026-05-25 anti-pattern (`patch={body: <one section>}` wiping the whole
   file — id `6b6e71ede6fff4c2`, already fully mitigated by the shrink
   guard) and the `include_subsections` opt-in's basic behavior. None
   mentions that a `body_edits` `replace` + `include_subsections:true`
   whose replacement is net-larger evades both the shrink guard and the
   BUG-043 guard simultaneously. This is not a "warned about, ignored"
   case — it's a genuinely undocumented gap in the Layer-3 surgical
   surface that the 2026-05-25 fix introduced as the *safe alternative* to
   the failure mode it was fixing.
   **Evidence link** — quoted guide/doc text gathered this session (not
   reproduced here for length; see get_guide("librarian") and
   `docs/architecture/augmented-artifacts.md` directly).

## Fix

**IMPLEMENTED 2026-08-06 (experiments) — option 2 ("return what was removed") plus option 4 (document it). Options 1 and 3 deliberately NOT taken.**

The tension this filing named is real, so the fix is observability, not refusal. `include_subsections: true` still does exactly what it says; it simply no longer does it silently.

`apply_body_edits` now computes the victim list for **every** `replace`, not only the ones it is about to refuse:

```rust
if action == "replace" {
    if let Ok(victims) = find_consumed_subsections(&buf, heading) {
        if !victims.is_empty() {
            if edit["include_subsections"].as_bool().unwrap_or(false) {
                consumed.extend(victims);          // opted in — record, proceed
            } else {
                return Err(...);                    // unchanged refusal
            }
        }
    }
}
```

The old code gated the whole block on `!include_subsections`, so the one guard purpose-built for "replace is about to wipe a nested heading" never ran on the exact calls that do the wiping. It now runs on both paths and only the *response* differs.

Surfaced in two places, both named in this filing's option 2:

- `artifact(update)` response gains `replaced_subsections: [...]` when non-empty.
- The `field_patch` event payload gains the same field, so the forensic trail no longer reads as a benign append. (`prev_bytes`/`new_bytes` are whole-file aggregates and cannot express a section-level loss when the write grew.)

**Signature change:** `apply_body_edits(working, edits, consumed: &mut Vec<String>)`. An accumulator rather than a tuple return, so the existing `apply_body_edits(...).map_err(...)?` expression chain in `call` — which carries the augmented-artifact drift nudge — needed a one-token change instead of restructuring.

**Not implemented, with reasons:** the section-scoped shrink check (option 1) re-raises the "what counts as intentional shrinkage" design question at section grain, which the whole-file guard already had to answer once for history trimming and archiving; and the threshold-acknowledgement flag (option 3) adds a second opt-in on top of an opt-in the caller already had to pass deliberately. Both are refusal-shaped, and this filing's own reasoning is that refusal risks re-blocking the legitimate rewrites `body_edits[]` was added to enable.

### Tests added

In `src/librarian/tools/update.rs` tests:

- `body_edits_include_subsections_reports_what_it_destroyed` — two `### W-N` children under `## Wins`, replaced with one larger `### W-3`. Asserts the replace applied, that `W-1` is gone (this test exists *because* it is), that both destroyed headings are named in `consumed`, and — the load-bearing assertion — that `out.len() > body.len()`, pinning the precondition that the whole-file shrink guard could never have fired.
- `body_edits_replace_still_refuses_without_include_subsections` — the BUG-043 guard is unchanged by moving the victim computation out of its branch, and a refused edit reports an empty `consumed`.
- `body_edits_leaf_replace_reports_nothing_consumed` — the list means "content was destroyed", not "a replace happened".

37/37 in `librarian::tools::update`.

### Docs updated (option 4)

- `src/prompts/guides/librarian.md` § *Body Editing Surfaces* — second anti-pattern, plus `replaced_subsections` added to the documented `field_patch` payload with the whole-file-aggregate caveat spelled out.
- `docs/architecture/augmented-artifacts.md` § *Body editing surfaces* — same anti-pattern, and a **correction**: that page described `include_subsections` as a "Per-entry `include_subsections` guard for `action=\"replace\"`", i.e. as a guard. It is the guard's off switch. The table row now says so.

Not implemented — filing for the maintainer to weigh direction. Offered as
suggestions, not a prescription; note the inherent tension:
`include_subsections:true` doing exactly what it says (consume and replace
the whole section including children) is not itself wrong, so a fix that
simply *refuses* more often risks re-blocking legitimate rewrites the
2026-05-25 fix specifically added `body_edits[]` to enable. The gap feels
like it belongs in **observability**, not refusal:

- **Section-scoped shrink check.** In addition to the whole-file guard,
  compare the OLD section body (`heading_idx+1..end_idx`, already computed
  by `find_consumed_subsections`/`compute_section_end`) against the NEW
  section body for a `replace`, independent of whether the whole-file total
  grew. This is the most direct fix but re-raises the "what counts as
  intentional shrinkage" design question the original shrink guard already
  had to answer (history trimming, archiving) — now at section grain.
- **Return what was removed, even when the call proceeds.** When
  `include_subsections:true` causes `find_consumed_subsections` to find
  victims, don't skip the call at `update.rs:164` — still compute the
  victim list, and instead of refusing, attach it to the response (e.g.
  `{"id":..., "updated": true, "replaced_subsections": ["### W-1 — ..."]}`
  ). Cheap, backward-compatible, gives a caller (or a human skimming tool
  output) something to notice. Could extend the `field_patch` event payload
  the same way for the forensic trail.
- **Require explicit acknowledgement above a threshold.** If
  `include_subsections:true` would consume more than N existing
  subsections (or more than K bytes of pre-existing content), require a
  second explicit flag/confirmation — mirrors the `force=true` escape
  hatch pattern already used for the whole-body guard.
- **Cheapest: document it.** Add a second "anti-pattern to remember" beside
  the existing one in `docs/architecture/augmented-artifacts.md` §
  "Body editing surfaces" and `src/prompts/guides/librarian.md` §
  "Body Editing Surfaces" — spell out that `include_subsections:true`
  disables the ONLY per-entry guard `body_edits` has, that the whole-file
  shrink guard cannot compensate when the write is net-larger, and that the
  safe pattern for "add a sibling entry" is `action="insert_after"`
  targeting the last existing child heading (verified untouched — see
  *Workarounds*) rather than `action="replace"` reconstructing the whole
  section from memory.

## Tests added

None permanent — the *Reproduction* test was added, run (confirmed it
reproduces: `ok`), and reverted (`git checkout --
src/librarian/tools/update.rs`), per this task's brief to avoid leaving
side effects while only verifying a claim. Justification for `N/A`: no fix
direction is chosen yet (see *Fix*); a permanent regression test belongs
with whichever fix lands, most naturally named
`body_edits_replace_include_subsections_growth_blind_spot` alongside the
existing `body_shrink_guard_*` tests in
`src/librarian/tools/update.rs::tests` (same file, ~line 1367 area). The
*Reproduction* section's snippet is copy-pasteable as that test's starting
point.

## Workarounds

- **Prefer `action="insert_after"` over `action="replace"` for pure
  appends.** Per `edit_markdown`'s own tool description, `insert_after`
  "add[s] a new sibling section... (target body preserved)" — it never
  touches existing content, so it cannot exhibit this bug. Targeting the
  LAST existing child heading (e.g. `### W-1`) with `insert_after` and
  `content="### W-2 — ..."` achieves the incident's actual intent (append
  one entry) with no `include_subsections` needed at all.
- If `action="replace"` + `include_subsections:true` is genuinely required
  (rewriting a section's structure, not just appending), first
  `artifact(get, id=X, heading="## Foo")` the CURRENT full section, and
  build the new `content` as that verbatim text plus the addition — not
  from memory/regeneration — since regenerating existing entries' text is
  exactly what silently dropped `### W-1`'s original content in the
  incident.

## Resume

N/A — fixed and verified on `experiments` at **`45669701`** (label: `experiments`;
master-side SHA still needs recording after cherry-pick per CLAUDE.md § "After
cherry-pick").

One judgement was deliberately left to the maintainer rather than decided here:
whether `replaced_subsections` should eventually *gate* (refuse above N consumed
subsections) rather than only report. This filing argued against refusal and the
fix followed that argument; revisit only if a real loss recurs **despite** the new
signal being present in the response.
## References

- `src/librarian/tools/update.rs` — `call` (`update.rs:200-462`),
  `apply_body_edits` (`update.rs:124-198`), `SHRINK_GUARD_MIN_BYTES`
  (`update.rs:70`).
- `src/tools/markdown/edit_markdown.rs` — `find_consumed_subsections`,
  `subsection_guard_error` (the BUG-043 guard `apply_body_edits` reuses).
- `docs/architecture/augmented-artifacts.md` § "Body editing surfaces —
  `body_edits` vs. `body`".
- `src/prompts/guides/librarian.md` § "Body Editing Surfaces" (served via
  `get_guide("librarian")`).
- Related/origin bug: `docs/issues/archive/2026-05-25-augmented-artifact-body-overwrite.md`
  (id `6b6e71ede6fff4c2`) — introduced the shrink guard, `body_edits[]`
  surface, and `field_patch` events this bug finds a gap in.
- Real incident: EDU-Planner `backend-kotlin` monorepo (external repo,
  session dated 2026-08-04/05), tracker
  `docs/trackers/its-integration-session-log.md`, artifact id
  `051682efd7186dad`. Not in this repo's git history.
