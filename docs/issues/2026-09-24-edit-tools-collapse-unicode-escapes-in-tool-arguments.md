---
status: open
opened: 2026-09-24
closed:
severity: low
owner: marius
related: [docs/issues/2026-09-04-patch-must-be-a-json-object-refusal-is-unreproduced.md]
tags: [cluster/unclassified]
kind: bug
---

# BUG: a literal `\uXXXX` escape cannot be written through the edit tools — it arrives decoded, and the layer that decodes it is not identified

## Summary
Reported from another project's SDD run (`codescout-lessons.md` § 6.5, secondhand from a
subagent): writing curly quotes / en-dashes / NBSP through `edit_file` / `edit_code` was
unreliable — "a single `\u` escape auto-decoded to the literal character, while a doubled
backslash was preserved literally instead of collapsing". Worked around with a `python3` heredoc.
Reproduced the first half here; the second half reproduced **inverted**, and codescout's own
decoder does not touch `\u` at all, so this may be the tool-call encoding layer rather than
codescout.

## Symptom (Effect)
Scratch file, 2026-09-24, bytes checked with `od -c`:
```
edit_file(edits=[{new_string: "don’t – x y"}])  → don’t – x y      (342 200 231, 342 200 223)
edit_file(edits=[{new_string: "don\\u2019t"}])            → don’t            ← doubled backslash ALSO decoded
edit_file(insert="append", new_string: C = "don’t")   → don’t            ← single-string mode, same
```
Typed characters arrive correctly; a *literal six-character* `’` cannot be written by any
of these three forms. The report said a doubled backslash stayed literal; here it decoded too.

## Reproduction
Live binary at `506924f2`, Claude Code harness, `edit_file` on any scratch file; inspect with
`od -c`.

## Root cause
Unknown. Ruled out and ruled in, from the source:
- **Not codescout's escape repair.** `decode_literal_escapes_inner` (`src/tools/edit_repair.rs:28`)
  decodes `\n`, `\t`, `\r` and quotes only, never `\u`, and runs on the `old_string` repair path.
  Read 2026-09-24.
- **One codescout path decodes a second time:** `optional_array_param`
  (`src/tools/core/params.rs:317`) re-parses an `edits` array that arrives as a JSON *string*.
  If the client stringifies the array, `\\u2019` → (client) `’` → (codescout) `’` — which
  matches the doubled-backslash row above. Not confirmed: the client's wire form is not visible
  from the caller's side.
- The single-string row involves no array at all and still decodes, which points at the harness's
  own argument encoding for that case.

## Evidence
See Symptom. The adjacent open bug in `related:` needs the same missing observation — the bytes
the server received — and names the same instrument.

## Hypotheses tried
1. **codescout's escape-repair decodes `\u`.** Test: read `edit_repair.rs:28`. **Verdict:** rejected.
2. **The stringified-array fallback double-decodes.** Test: none possible from the caller.
   **Verdict:** deferred — consistent with row 2, cannot explain row 3.

## Fix
None proposed until the received bytes are logged (see `related:`'s Resume). If
`optional_array_param` is confirmed as a second decode, it is correct for clients that stringify
and the fix is on the caller side: document that a literal backslash-u needs one more level of
escaping in `edits[]` than in a top-level string.

## Tests added
None — nothing verified to regress against.

## Workarounds
Type the characters themselves (they arrive intact), or write the file with a `python3` heredoc
through `run_command` when a literal escape sequence is the content.

## Resume
Log the raw `edits` / `new_string` value at `edit_file`'s entry for one call carrying `\\u2019`,
and compare against what the caller typed.

## References
- `codescout-lessons.md` (repo root, untracked) § 6.5.
