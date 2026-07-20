---
id: e000f27a6dd6c0a0
kind: bug
status: draft
title: 'BUG: append_entry assigns next monotonic ID from params only, colliding when body already has a newer entry (params/body ID drift)'
tags:
- append_entry
- monotonic-id
- tracker
- librarian
closed: 2026-07-20
opened: 2026-07-20
owner: marius
related: []
severity: medium
---

## Summary
`artifact(action="append_entry", ...)` computes the next monotonic ID as `max(ids present in the augmentation's params entry_collection array) + 1`, considering ONLY the params array. For the common tracker shape where the markdown body (index table + `## PREFIX-N` sections) is the canonical human-readable surface and params is a parallel structured index, a prior session that appends a body section + index row but skips the `append_entry` params step leaves params behind the body. The next `append_entry` call then silently reissues an ID the body already uses — a duplicate ID in an append-only registry whose entire value proposition is stable, citable IDs. No warning, no error.

## Symptom (Effect)
`append_entry` returns a normal-looking success envelope with an `id` that collides with an ID already present in the artifact body, e.g.:
```json
{"id": "OTK-33"}
```
even though the body already contains a `## OTK-33` section and index-table row for a different fact. The call gives no indication that a collision occurred.

## Reproduction
1. Take any augmented monotonic-ID tracker that follows the documented 3-step update flow (append `## PREFIX-N` body section → add index-table row → `append_entry` into params).
2. Have a prior session complete steps 1–2 for `PREFIX-N` but skip step 3 (params append) — leaving params' max id at `N-1` while the body already documents through `PREFIX-N`.
3. Call `artifact(action="append_entry", id="<artifact>", id_prefix="PREFIX", entry_collection="<array>", entry={...})` for a genuinely new fact.
4. Observe the returned id is `PREFIX-N`, colliding with the body's existing `PREFIX-N` section.

Concrete occurrence (2026-07-20, in the `backend-kotlin` project, not this repo):
- Artifact `docs/trackers/or-tools-knowledge.md` (id `401adef2be55c330`), entry_collection `facts`, id_prefix `OTK`.
- params held OTK-1..OTK-32. The body held through **OTK-33** (`num_workers` decides whether a PROOF lands) — index row and full body section both present; params append for OTK-33 was never done.
- `append_entry` for a genuinely new fact returned **OTK-33**, colliding with the body's existing OTK-33.
- Caught only because the calling agent happened to have read the body's index table minutes earlier and recognized the number already in use. With any other ordering the collision would have landed silently.

## Environment
codescout MCP `artifact` tool, `append_entry` action, any project with augmented monotonic-ID trackers (SI-N, OTK-N, F-N/W-N, R-N, T-N, HY-N, ...). Not specific to a language/runtime — this is librarian/catalog-layer behavior.

## Root cause
`append_entry`'s next-id computation reads only the augmentation's `params` entry_collection array and takes `max(existing ids) + 1` from it. It never inspects the artifact body for `PREFIX-<N>` occurrences. For this whole class of tracker the body is the canonical surface (per the documented 3-step flow: body section → index row → `append_entry`), so params can legitimately lag the body whenever a prior session completes steps 1–2 and skips step 3. `append_entry` has no way to detect that skew and silently reissues an ID the body has already claimed.

## Evidence
Traced from the `append_entry` tool description itself (`"append_entry: id_prefix — the assigned id is <id_prefix>-<next integer>, computed from the live max across existing entries"` — "existing entries" here means the params array, not the body) and from the concrete OTK-33 occurrence described above (backend-kotlin session, 2026-07-20): params array's max entry was OTK-32; body already had a full `## OTK-33` section (`num_workers` decides whether a PROOF lands) plus an index-table row; `append_entry` for a new, unrelated fact returned `OTK-33`.

## Hypotheses tried
1. **Hypothesis**: params is always the source of truth and the body is purely derived/rendered from it, so params-only max is correct by design.
   **Test**: checked the documented update flow for this tracker class (body section → index row → `append_entry`), which treats the body as canonical and params as a parallel index that can fall behind.
   **Verdict**: rejected — the documented flow explicitly allows params to lag the body (step 3 is separable from steps 1–2), so params-only max is not safe as the sole source for next-id computation.
   **Evidence link**: see Root cause above; also the prior-art note below.

## Fix

Implemented remedy (1) — reconcile against the body — plus a non-fatal form of remedy (2).

`augmentation::append_entry` now reads the artifact's `abs_path` inside the same `IMMEDIATE` transaction, scans the file with a new `body_max_index(body, id_prefix)` helper, and assigns `max(params_next, body_max + 1)`. A missing or unreadable file degrades silently to params-only behaviour, so the append never fails on a body it cannot see.

`body_max_index` matches only **line-anchored** occurrences — a markdown heading (`## F-12`) or the leading cell of an index-table row (`| F-12 |`), optionally wrapped in backticks / bold / link brackets. Those are exactly the two surfaces the documented 3-step flow writes. Prose mentions are deliberately ignored: an aside like "we should file F-999" must not blow a hole in the numbering, and over-allocating is only safe when the trigger is precise.

Because (1) alone leaves params still missing the skipped rows, `append_entry` also returns a new `AppendOutcome { id, warning }`. When the body claims ids params does not carry, the tool response gains a `warning` field naming both maxima and telling the agent to backfill. The append itself still succeeds.

Remedy (3) (a `librarian(action="doctor")` skew check for proactive discovery) was **not** implemented — the per-call warning covers the surface where the drift actually bites.
## Tests added

`src/librarian/catalog/augmentation.rs`:
- `append_entry_skips_ids_already_claimed_by_the_body` — the regression proper. params max `F-32`, body claims `F-33` in both an index row and a heading; asserts `F-34`. Confirmed failing first (`left: "F-33", right: "F-34"`) before the fix.
- `append_entry_ignores_body_when_params_is_ahead` — a stale body must not pull the next id backwards.
- `append_entry_tolerates_a_body_missing_from_disk` — missing file degrades to params-only, never errors.
- `body_max_index_reads_headings_and_index_rows` / `_ignores_prose_mentions` / `_respects_prefix_boundaries` (`F` must not match `FX-900` or `F-12x`) / `_returns_none_when_body_claims_nothing`.

`src/librarian/tools/append_entry.rs`:
- `call_warns_when_params_lags_the_body` — asserts the `warning` field is present and names the body's max.
- `call_omits_warning_when_params_is_current` — no warning on the clean path.

Full suite: 3395 passed, 43 ignored; `cargo fmt` + `cargo clippy --all-targets -- -D warnings` clean.
## Workarounds
Before calling `append_entry` on a tracker of this class, read the body's index table (or grep for `PREFIX-<N>` headings) and manually compare its max against the params array's max. If the body is ahead, reconcile params first (backfill the missing params row from the body section, e.g. via `artifact_augment(merge=true, params_path=...)` for large arrays) before trusting `append_entry`'s returned id.

## Resume

Fixed on `experiments`, not yet cherry-picked to `master`. Archive this file only after the fix ships to `master` (`git branch --contains <fix-sha>`).

Optional follow-up, deliberately deferred: remedy (3), a `librarian(action="doctor")` check for params-vs-body id skew across monotonic-id trackers, so existing drift is discoverable without waiting for the next append. Also unaddressed: the pre-existing OTK-33 drift in `backend-kotlin`'s `or-tools-knowledge.md` still needs a manual params backfill — this fix prevents future collisions there but does not repair the historical gap.
## References
- Prior art: the `backend-kotlin` project's `solver-invariants` codescout memory already carries a standing warning against blind `append_entry` on that tracker, referred to there as "F-19 id-drift" — i.e. this has bitten before and was being worked around by human/agent vigilance rather than fixed in the tool.
- Concrete occurrence: `backend-kotlin` repo, `docs/trackers/or-tools-knowledge.md` (artifact id `401adef2be55c330`), OTK-33 collision, 2026-07-20.
- Similar-shaped prior bug in this repo: `docs/issues/2026-07-17-worktree-cites-refusal-materializes-shadow-fork.md` (append_entry contract violation under a different trigger).
