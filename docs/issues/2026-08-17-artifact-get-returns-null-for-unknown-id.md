---
id: be15087051e40e3d
kind: bug
status: open
title: 'BUG: artifact(get) returns bare `null` for an unknown or moved id — an Ok result that cannot be told from "exists but empty", while the guide promises `unknown id`'
tags:
- librarian
- artifact
- error-handling
- silent-failure
- doc-drift
---

## Summary

`artifact(action="get", id=<unknown>)` returns the JSON value `null` on a
**successful** call. It does not name the id, does not say the id is unknown, and
is indistinguishable from an artifact that exists with no content. The caller
cannot tell "no such id", "catalog is stale", and "moved, so the id was re-keyed"
apart — three states with three different repairs.

`get_guide("tracker-conventions")` states the opposite, twice:

> the old one stops resolving immediately, and a later call with it returns `unknown id`

No path emits that string.

## Symptom (Effect)

Cost one real turn this session. `artifact(find, kind="bug", status open/investigating)`
returned `8a374ba3510d5020`; the immediately following
`artifact(get, id="8a374ba3510d5020", full=true)` returned `null`. Reading the
response alone, the plausible causes were a mistyped id, catalog lag, or a
genuinely empty artifact — and the true cause was none of them: a concurrent
session had archived the file ~90s earlier, which mints a new id
(`id = sha256(abs_path)`). Recovering it took a second `find` with
`include_archived=true`.

## Reproduction

Two calls, no setup:

```
artifact(action="get", id="deadbeefdeadbeef")
→ null
```

An id that never existed and an id that was valid 90 seconds ago produce the
byte-identical response.

## Environment

codescout `experiments` @ `c9c5ade9`, 2026-08-17. Reproduced twice.

## Root cause

`src/librarian/tools/get.rs:127`, inside `call`:

```rust
let row = match artifact::get(&cat, &a.id)? {
    Some(r) => r,
    None => return Ok(Value::Null),
};
```

The `None` arm returns `Ok` — a success carrying a null payload — instead of a
`RecoverableError`. It is the only `return Ok(Value::Null)` under
`src/librarian/`, so this is a single-site defect, not a house style.

## Evidence

**The same codebase already does this correctly one layer down.** From the
`artifact` tool schema, on `update_entry`:

> Unknown ids are refused with the list of ids that do exist — never a silent no-op.

So the good shape is established, documented, and shipped for *entry* ids. It was
never applied to *artifact* ids, where the failure is more likely — an artifact id
is `sha256(abs_path)` and therefore changes on every `move`, and archiving is a bug
file's normal end state.

**The doc-vs-code drift is load-bearing, not cosmetic.** `tracker-conventions` tells
readers to expect `unknown id`, which is exactly the signal that would have made
this diagnosable. A reader who trusts the guide concludes "no error, so the id is
fine, so the artifact must be empty" — the one wrong conclusion of the three.

## Hypotheses tried

1. **Hypothesis:** the catalog was stale and a reindex would restore the row.
   **Test:** `librarian(action="reindex")` had already run in this session
   (`updated: 1`, `unchanged: 1042`), and `find` had returned the id *after* it.
   **Verdict:** rejected — the catalog was current; the artifact had moved.

2. **Hypothesis:** `null` means "artifact exists, body empty".
   **Test:** called `get` with `deadbeefdeadbeef`, an id that has never existed.
   **Verdict:** rejected — identical `null`. The response carries no information
   distinguishing the two, which is the defect.

## Fix

Return a `RecoverableError` from the `None` arm naming the id, and — this is the
half that turns it from an error into a repair — say what else to try:

```
unknown artifact id 'deadbeefdeadbeef'. If this id came from an earlier call, an
artifact(action="move") since then will have re-keyed it (id = sha256(abs_path));
find it by path with artifact(action="find", filter={"rel_path": {"contains": …}},
include_archived=true). If it was never seen before, run librarian(action="reindex").
```

Both branches matter: a moved artifact and an unindexed one need opposite repairs,
and the caller cannot pick without being told both exist.

Match the `update_entry` precedent in spirit — that call lists the ids that *do*
exist. Listing every artifact id is not useful here, so the actionable equivalent
is naming the two recovery paths above.

**Check `full=true`/`heading=` do not mask it.** The `None` arm is hit before any
body selector is read, so every selector shape returns the same `null`; the fix
should sit at the same point and cover all of them at once.

## Tests added

None yet. Wanted, and each must fail against `return Ok(Value::Null)`:

- `get` with a never-seen id → `RecoverableError`, message contains the id verbatim.
- `get` with an id that was valid, then `move`d → error names the move/re-key path,
  not just "unknown". This is the case that actually bit; an error that says only
  "unknown id" still sends the reader to `reindex`, which will not help.
- The error mentions **both** `reindex` and the `find`-by-rel_path recovery — a
  single-branch hint is the failure mode being fixed, not a smaller version of it.
- An artifact that genuinely exists with an empty body still returns an object, and
  is therefore distinguishable from the unknown-id case. This is the discriminating
  assertion: without it the suite passes for a fix that errors on *both*.

## Workarounds

Treat `null` as "unknown, cause undetermined" and disambiguate by hand:

```
artifact(action="find", filter={"rel_path": {"contains": "<slug>"}},
         include_archived=true)
```

`include_archived=true` is the part that matters — the most likely cause of a
freshly-dead id is an archive move, and the default scope hides archived rows, so
the obvious follow-up query reproduces the same empty answer for a second reason.

## Resume

Single-site change at `src/librarian/tools/get.rs:127`. Check first whether any
caller or test asserts on the `null` return — `get` is on the hot path for every
tracker read, and something may already depend on the soft failure.

## References

- `src/librarian/tools/get.rs:127` — the `None` arm.
- `get_guide("tracker-conventions")` § *Archiving / Moving Trackers* — the
  `unknown id` promise the code does not keep. Fix both together; the guide is
  currently describing the desired behaviour, so it becomes correct rather than
  needing a rewrite.
- `docs/issues/archive/2026-08-17-artifact-find-is-silent-about-files-the-catalog-has-never-seen.md`
  — the sibling defect on `find`, same shape: a zero/absent answer that does not
  distinguish "nothing there" from "catalog never looked", fixed by making the
  response describe its own limits rather than by making the tool do more.
- `docs/issues/archive/2026-08-07-grep-zero-match-silent-about-hidden-skip.md` —
  the same class again, on `grep`.

