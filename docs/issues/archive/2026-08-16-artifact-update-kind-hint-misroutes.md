---
id: '6c0952e8908fbf00'
kind: bug
status: fixed
title: artifact(update) rejects extra.kind with a hint naming a parameter update does not accept
tags:
- librarian
- artifact
- misleading-error
- tool-quirk
---

## Summary

There is no way to write `kind:` into an artifact's frontmatter through the librarian.
`artifact(update)` rejects it via `extra`, and the rejection's hint points at a parameter
`update` does not have — so following the hint verbatim produces a second error.

## Symptom (Effect)

```
artifact(action="update", id="3505e9242548bda4", patch={"extra": {"kind": "tracker"}})
→ {"ok": false,
   "error": "extra must not contain frontmatter field(s) the schema already models: kind",
   "hint": "pass `kind=` as its own parameter instead. Reserved: id, kind, status,
            title, owners, tags, topic, time_scope. `extra` is for keys outside
            the schema (opened, closed, severity, owner, related, …)."}

artifact(action="update", id="3505e9242548bda4", kind="tracker")
→ missing field `patch`
```

Adding `patch` alongside `kind` does not help: `kind` is not among the keys `patch`
accepts (`status, title, owners, tags, topic, time_scope, extra, body, body_edits, params`).

## Reproduction

Any artifact whose file lacks a `kind:` key. Observed 2026-08-16 on
`docs/trackers/structural-edit-gate-session-log.md` (id `3505e9242548bda4`), which had no
YAML frontmatter at all. `artifact(update, patch={status, title})` synthesized a frontmatter
block containing exactly `status:` and `title:` — no `kind:` — and no subsequent call could
add one.

## Environment

codescout `experiments`, Linux, Claude Code stdio, 2026-08-16, during a tracker-hygiene sweep.

## Root cause

Read from source (was inferred when filed; the inference was right, and narrow).

`reject_reserved_extra_keys` (`src/librarian/tools/create.rs`) is called from
`create.rs` and from `update.rs`. It detects one defect — a reserved key in `extra` —
and carried one fixed hint, written looking at `create`, where every reserved key but
`id` really is a top-level parameter (`create::Args` declares `kind`, `title`, `owners`,
`tags`, `status`, `topic`, `time_scope`).

What the filing missed: `update` has **no** top-level reserved-key parameters at all.
Its accepted `patch` keys are `status, title, owners, tags, topic, time_scope, extra,
body, body_edits, params` — the six settable reserved keys travel inside `patch`, and
`kind` and `id` are accepted through no channel. So the hint misrouted *every* reserved
key on `update`, not only `kind`.

The underlying gap is real and deliberate: `kind` drives catalog classification and is
fixed once the artifact exists. The defect was that nothing said so — the caller was
sent looking for a parameter instead.
## Evidence

The catalog and the file disagree and cannot be reconciled through the API: the catalog row
for `3505e9242548bda4` carries `kind: tracker` (it is returned by
`artifact(find, kind="tracker")`), while the file on disk has no `kind:` key. Any consumer
reading the file rather than the catalog — a git-mv, a fresh clone's reindex, a non-librarian
reader — sees an unclassified document.

## Hypotheses tried

1. **Hypothesis:** `kind` is accepted as a top-level param on `update`, as the hint says.
   **Test:** `artifact(action="update", id=..., kind="tracker")`.
   **Verdict:** rejected — `missing field 'patch'`; `kind` is not read on this action.

## Fix

Fixed on `experiments` in `99fa967f` — option 1 from the original filing (action-aware
hint), widened by what the scout found.

Detection stays shared, so `RESERVED_KEYS` keeps a single place to stay in sync with;
only the *sentence* branches. A new `ExtraKeySurface` enum is passed by each caller and
owns two per-surface facts:

- `unsettable_reason(key)` — why a key is reachable through no channel here (`id` on
  both surfaces; `kind` additionally on `update`), returning `None` when it is settable.
  Per-key rather than a flat list, so a caller learns whether the refusal is a
  wrong-channel mistake or a permanent property of the field.
- `channel(key)` — how a settable reserved key is passed here: `` `kind=` as its own
  parameter`` on `create`, `` `patch={kind: …}` `` on `update`.

The hint is then assembled from the clashes actually present, naming only remedies that
exist on the surface that refused.

Option 2 (accept `kind` in `patch`, re-classifying the catalog row) was **not** taken —
it needs thought about reclassification side effects, and is not required to stop the
misrouting. The gap is now stated in the error rather than hidden behind wrong advice.
## Tests added

`reserved_key_hint_names_a_remedy_that_exists_on_the_calling_surface`
(`src/librarian/tools/create.rs`) pins the remedy, not just the detection. It asserts
the `create` hint still routes to the top-level parameter; that the `update` hint does
**not** name one and instead explains that `kind` is fixed at create time; that `update`
routes settable keys through `patch={…}`; and that `id` is caller-settable on neither.

Mutation-verified: reverting `Update` to the `Create` wording reproduces the filed
message verbatim — `pass \`kind=\` as its own parameter instead` on an `update` — and
fails the new test.

Neither pre-existing test could have caught this. `create_rejects_an_extra_key_…` and
`update_rejects_an_extra_key_…` both assert only that the message *names the clashing
key*, which the wrong hint also did. That is the reusable lesson: a test that pins the
detection while leaving the guidance unasserted cannot see a misrouting defect.

Gate: `cargo fmt` + `cargo clippy --lib -D warnings` clean, `cargo test --lib
librarian::tools` 634 passed.
## Workarounds

None through the librarian. The frontmatter must be written by another route, or the
artifact left classified in the catalog only.

## Resume

Find the `extra` reserved-key validation that emits "pass `kind=` as its own parameter
instead" and check whether it can see which action invoked it. If it can, branch the hint on
create-vs-update. Then decide separately whether `patch` should accept `kind`.

## References

- Surfaced by the tracker-hygiene sweep 2026-08-16 (`docs/trackers/tracker-hygiene-log.md`),
  detector D4 (frontmatter-catalog-mismatch) — this bug is why D4's "no `kind:`" sub-class
  could not be fully fixed in that sweep.
