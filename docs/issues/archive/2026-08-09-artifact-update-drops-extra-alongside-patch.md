---
id: '973f0c0f6721ea83'
kind: bug
status: fixed
title: artifact(update) silently drops top-level `extra` when `patch` is also passed
owners:
- marius
tags:
- librarian
- artifact
- silent-data-loss
- tool-quirk
closed: 2026-08-14
opened: 2026-08-09
related:
- docs/issues/2026-08-09-path-strip-corrupts-file-content-and-root-fields.md
severity: medium
---

# BUG: artifact(update) silently drops top-level `extra` when `patch` is also passed

## Summary
Calling `artifact(action="update", …)` with top-level `status` **and** `extra` alongside a
`patch` object applies the `status` (auto-lifted, with a warning) but **silently discards
`extra`**. No error, no warning naming `extra`. The caller believes the frontmatter key was
written; it was not.

## Symptom (Effect)
Observed 2026-08-09 while closing a bug file during Task 5 of the field-aware-path-strip
plan. The call passed `status="fixed"`, `extra={"closed": "2026-08-09"}` and a
`patch={body_edits: [...]}`. Result: `status` became `fixed`, body edits applied,
`closed` absent from frontmatter. Re-issuing as a `patch`-only call
(`patch={"status": …, "extra": {…}}`) wrote both.

## Reproduction
```
git rev-parse HEAD   # 2aecc0bf (experiments)
```
1. `artifact(action="update", id=<any>, status="fixed", extra={"closed":"2026-08-09"}, patch={body_edits:[…]})`
2. `artifact(action="get", id=<same>)` → `status` is `fixed`, `extra.closed` is missing.
3. Re-issue as `artifact(action="update", id=<same>, patch={"status":"fixed","extra":{"closed":"2026-08-09"}})` → both land.

## Environment
codescout MCP server (release binary), project codescout, branch `experiments`.

## Root cause
Unknown — not investigated. Best lead: the auto-lift path that promotes top-level `status`
into `patch` appears to handle `status` only, so a sibling top-level `extra` is neither
lifted nor rejected. The warning emitted mentions the lift but not the dropped key.

## Evidence
Reported by the Task 5 implementer subagent; full transcript in
`.superpowers/sdd/2026-08-09-field-aware-path-strip/task-5-report.md`. The workaround
(patch-only form) was confirmed to work in the same session.

## Hypotheses tried
1. **Hypothesis:** `extra` requires the `patch` form. **Test:** re-issued patch-only.
   **Verdict:** confirmed workaround; does not explain why the mixed form is silent
   rather than rejected. The tool's own contract says unknown `patch` keys return a
   `RecoverableError` — the mixed form should be at least as loud.

## Fix

Fixed 2026-08-14 on `experiments`, in `src/librarian/tools/update.rs`.

**Reproduced live on HEAD before touching anything.** Not inferred:

```
artifact(action="update", id=f6dd06e3388e5465,
         extra={"probe_top_level": "should-land"}, patch={"status": "open"})
  -> {"id": "...", "updated": true}
```

`probe_top_level` never appeared in the frontmatter. No error, no warning,
`updated: true`.

### The bug is five params, not one

The mechanism is not specific to `extra`. `UpdatePatch` carries
`#[serde(deny_unknown_fields)]`, so an unknown key *inside* `patch` errors loudly.
`Args` **cannot** carry it — the dispatcher passes `action` through and the shared
artifact schema carries create-only keys — so any advertised top-level param that
`Args` does not declare is discarded by serde while the write still reports success.

`Args` declared exactly one such field: `status`. So every *other* param the schema
advertises as `create/update` was silently dropped. Confirmed live in a second call:

```
artifact(action="update", id=…, tags=["probe-tag-should-land"],
         topic="probe-topic", time_scope="2026-W33", patch={"status": "open"})
  -> {"updated": true}
```

Original tags unchanged; no `topic:` key; no `time_scope:` key.

| Top-level param | Schema says | Before |
|---|---|---|
| `status` | create/update | honored (fixed 2026-07-20) |
| `extra` | create/update | **silently dropped** |
| `owners` | create/update | **silently dropped** |
| `tags` | create/update | **silently dropped** |
| `topic` | create/update | **silently dropped** |
| `time_scope` | create/update | **silently dropped** |

### The shape of the fix

The 2026-07-20 fix for `status` established the right behaviour and encoded it as a
bespoke block for one field. That is why this bug exists: the repair was
field-shaped, so it could not cover its five siblings.

Replaced with one `lift_top_level_param!` macro applied to all of them, following the
same Repair-and-Continue semantics the `status` block had:

- top-level present, patch absent → lift into `patch`, add a `corrections` note
- both present and equal → repair silently, no note (agreement is not ambiguity)
- both present and different → **refuse**; a wrong guess on a write is unrecoverable

`title` is lifted too. The schema documents it as create-only, so top-level `title`
on update is off-schema rather than advertised — but `UpdatePatch` has the field, it
is the same class of mistake, and repairing a rename with a note beats discarding it
in silence.

One behaviour worth knowing: the reserved-key check
(`create::reject_reserved_extra_keys`) runs on `patch.extra` *after* the lift, so a
lifted top-level `extra` is validated identically to one passed canonically. That
ordering is load-bearing.
## Tests added

Three, in `src/librarian/tools/update.rs`:

- **`update_lifts_every_advertised_top_level_param`** — table-driven over all seven
  params. Each case creates a fresh artifact, passes the param top-level alongside an
  *unrelated non-empty* `patch`, and asserts the value reaches the frontmatter on
  disk plus a `corrections` note. The unrelated patch matters: it reproduces the real
  shape of the bug, where a succeeding patch masked the dropped param.
- **`update_conflicting_non_status_sources_are_refused`** — the refuse arm holds for
  non-`status` fields, and asserts *neither* reading was written.
- **`update_agreeing_non_status_sources_are_not_flagged`** — agreement produces no
  correction note but still writes.

**Table-driven deliberately.** A test covering one param would have passed through
this entire second bug — that is exactly what the existing `status` tests did. Three
of them (`update_lifts_top_level_status_into_the_patch`,
`update_top_level_status_reaches_the_frontmatter`,
`update_top_level_status_agreeing_with_patch_is_not_flagged`) were green for the
whole life of this defect, because each asserted on the one field that worked.

**Verified the test can fail, not just that it passes.** Removed the `tags` lift and
re-ran:

```
top-level `tags` was accepted (`updated: true`) but never reached the frontmatter
— the silent-drop bug. File:
---
id: '106b420f7d4afd17'
kind: spec
status: draft
title: T
---
```

Caught, with the diagnostic naming the defect. The mutation also produced
`warning: field \`tags\` is never read`, so under `-D warnings` a deleted lift fails
two independent ways. Note what that does **not** cover: the original bug was a field
that never existed, and no warning catches an absent field — which is why `Args` now
carries an explicit comment against "cleaning up" a field that looks unused.

Gate: **3710 passed / 0 failed / 44 ignored** (3707 + these 3, reconciling exactly),
`clippy --all-targets -D warnings` clean.

### Live-surface verification, 2026-08-14 (post `cargo rb` + `/mcp` reconnect)

Re-run against the running server with a top-level param, on the same artifact used to
reproduce the drop:

```
artifact(update, id=f6dd06e3388e5465, tags=[… 5 tags …], patch={"status": "open"})

  before  {"updated": true}                      tags silently discarded
  after   {"updated": true, "corrections": [
            "lifted top-level `tags` into `patch.tags` — …" ]}
```

And the values reached the frontmatter on disk — all five tags present, where the
pre-fix call left the original four untouched. The `corrections` note is the part worth
having: the fix does not just work, it *tells the caller* it repaired something, so the
canonical form gets learned instead of the repair being relied on silently.
## Workarounds
Pass everything inside `patch`: `artifact(action="update", id=…, patch={"status": …,
"extra": {…}, "body_edits": [...]})`. Verify with `artifact(action="get", id=…)` that
`extra` actually landed — the write reports success either way.

## Resume

N/A — fixed and verified, for all five dropped params rather than only the `extra`
this bug was filed about.

If a new top-level param is ever added to the artifact schema for `update`: add it to
`Args` **and** to the `lift_top_level_param!` list, or it will silently no-op exactly
as these did. The table in `update_lifts_every_advertised_top_level_param` is the
place to add its case.
## References
- Discovered during: `docs/superpowers/plans/2026-08-09-field-aware-path-strip.md` Task 5.
- Handler: `src/librarian/tools/update.rs`.
