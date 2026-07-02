---
status: mitigated
opened: 2026-07-02
closed:
severity: low
owner: marius
related: []
tags: [artifact_augment, librarian, params-path, silent-noop]
kind: bug
---

# BUG: `artifact_augment(merge=true, params_path=...)` silently no-ops when the file's top-level JSON is a bare array instead of an object

## Summary
Passing `params_path` pointing at a JSON file whose top-level value is an
array (rather than an object) returns `"ok"` and appears to succeed, but
the augmentation's `params` are left completely unchanged — no error, no
warning, no indication the merge was skipped.

## Symptom (Effect)
```
artifact_augment(id="52451519052d207c", merge=true,
                  params_path="/tmp/.../win-issues-fix.json")
=> "ok"
```
where `win-issues-fix.json`'s top-level content was a bare `[ {...}, ... ]`
array (the intended replacement for `params.issues`). A subsequent
`artifact(action="get", id=..., entry_filter={...})` still returned the
OLD `params.issues` entry, byte-for-byte, proving the merge never applied.
Re-running with the array wrapped as `{"issues": [...]}` applied correctly
and was confirmed via the same `entry_filter` query.

## Reproduction
1. Fetch an augmented tracker's `params.issues` array (an array of objects
   keyed by `id`).
2. Write a corrected copy of that array to a scratch file as a **bare
   top-level array** (no wrapping object).
3. Call `artifact_augment(id=<tracker>, merge=true, params_path=<file>)`.
4. Re-fetch via `entry_filter` — the change is absent; tool returned `"ok"`
   with no diagnostic.
5. Rewrap the same array as `{"issues": [...]}` in the file, re-run step 3
   — the change now lands and is visible via `entry_filter`.

Observed on `experiments`, HEAD at the time `59df5c9e` / `90ffff4e`
(codescout-on-codescout self-hosted MCP session), tracker id
`52451519052d207c` (`docs/trackers/windows-platform-support.md`).

## Environment
codescout MCP server (self-hosted, running binary at session start),
librarian artifact backend, `experiments` branch.

## Root cause
Unknown — see Hypotheses tried. Likely: the merge-patch implementation
expects `params_path`'s content to be the full `params` object (RFC 7396
merge-patch target), and when handed a non-object JSON value, some guard
silently skips the merge rather than either (a) erroring with "params must
be a JSON object" or (b) following RFC 7396 §"if the patch is not an
object, params becomes the patch verbatim" (which would itself be
destructive/surprising here). Either documented behavior would be
preferable to a silent no-op.

## Evidence
- First `artifact_augment` call returned `"ok"`.
- Immediate `artifact(action="get", entry_filter={"id":{"eq":"WIN-27"}})`
  after that call still showed the pre-edit summary text
  (`"9 guide_hint ... 5 assorted"`).
- `artifact(action="get", id=..., full=true)` confirmed
  `$.augmentation.params.issues` was still the old 27-entry array,
  untouched — not replaced wholesale either (ruling out a destructive
  RFC-7396-literal interpretation).
- Second call with the array wrapped as `{"issues": [...]}` — same
  `entry_filter` query then returned the corrected summary
  (`"8 guide_hint ... 6 assorted"`).

## Hypotheses tried
1. **Hypothesis:** `params_path` requires the file's top-level value to be
   a JSON object matching the augmentation's `params` shape; a bare array
   is rejected/ignored rather than merge-patched.
   **Test:** re-ran the identical logical change wrapped in `{"issues": [...]}`.
   **Verdict:** confirmed as the workaround — wrapping fixed it. Whether
   the underlying no-op is an explicit type check or an unhandled branch
   in the merge code was not traced (no source access to the
   MCP-server-side implementation from this session).

## Fix
Not implemented — no source-side fix attempted this session (out of scope
for the delegated task; documenting for whoever owns the `artifact_augment`
implementation). Suggested direction: either reject a non-object
`params_path` payload with a `RecoverableError` naming the expected shape,
or document the object-payload requirement explicitly in the tool's
description (currently says "the params payload," which reads as "whatever
JSON you want stored," not "must be a top-level object").

## Tests added
N/A — no code fix in this session; this is an operational discovery
against a live MCP tool, not a change to codescout's own source tree.

## Workarounds
Always wrap the intended array under its `entry_collection` key name
(e.g. `{"issues": [...]}`) before passing `params_path`, even when only
one param field is being replaced. Verify the merge landed via
`artifact(action="get", entry_filter=...)` on the specific row you
changed — do not trust the bare `"ok"` return value as proof of a
successful write.

## Resume
N/A for this session. If picked up: trace the `artifact_augment` /
`params_path` merge-patch code path server-side to confirm whether the
no-op is a deliberate object-only guard or an unhandled type branch, then
either add an explicit error or update the tool description.

## References
- `docs/trackers/windows-platform-support.md` (tracker id `52451519052d207c`) — the artifact this was discovered against, WIN-27 row.
- Discovered during the 2026-07-02 final-review fix wave, Part 3 / M1 (WIN-27 count reconciliation).
