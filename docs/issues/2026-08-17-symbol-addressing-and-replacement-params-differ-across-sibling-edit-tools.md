---
id: '3a678913656b3c38'
kind: bug
status: open
title: 'BUG: sibling tools name the same two concepts differently — symbols uses name_path but edit_code wants symbol, edit_markdown uses content but edit_code wants body — two failed calls to insert one function'
---

## Summary

Adding one test function cost two failed `edit_code` calls, both from parameter
names that differ from the sibling tool used to locate the target. `symbols`
addresses a symbol as `name_path`; `edit_code` refuses that and wants `symbol`.
`edit_markdown` and `artifact(update)` name replacement text `content`;
`edit_code` refuses that and wants `body`. Both refusals are loud and carry good
hints, so the cost is bounded — but it is paid on the normal path of the
find-then-edit workflow the Iron Laws prescribe.

Low severity by itself. Filed because it is the benign half of a pattern whose
severe half is
`docs/issues/2026-08-17-edit-markdown-edit-action-deletes-when-new-string-is-omitted.md`,
where the same class of key mix-up is silent and destructive.

## Symptom (Effect)

Locating the insertion point, using the name the search tool expects:

```
symbols(name_path="call", path="src/librarian/tools/find.rs", include_body=true)
→ (works)
```

Editing at that same address, carrying the same argument name over:

```
edit_code(path="src/prompts/mod.rs", action="insert",
          name_path="redesign_invariants/server_instructions_mentions_get_guide",
          position="after", content="…")
→ {"ok": false,
   "error": "missing 'symbol' parameter",
   "hint": "Name the symbol, e.g. symbol=\"MyStruct/my_method\" for a method
            or symbol=\"my_fn\" for a free function."}
```

Renaming `name_path` → `symbol` and retrying:

```
→ {"ok": false, "error": "action 'insert' requires 'body'"}
```

Renaming `content` → `body` succeeds. Two round-trips, no diagnostic naming both
problems at once — the first refusal validates the address before looking at the
payload, so the second key error is only reachable after fixing the first.

## Reproduction

`git rev-parse HEAD` → `66487591`, branch `experiments`.

1. `symbols(name_path="<Container>/<member>", include_body=true)` — succeeds.
2. `edit_code(path=…, action="insert", name_path=<same value>, position="after", body="…")`
   → `missing 'symbol' parameter`.
3. `edit_code(path=…, action="insert", symbol=<same value>, position="after", content="…")`
   → `action 'insert' requires 'body'`.

## Environment

Linux, codescout `experiments` @ `66487591`, stdio MCP.

## Root cause

Unknown — not traced. This is a naming/contract observation, not a logic defect:
each tool validates its own declared parameters correctly and refuses clearly.
The cost comes from two concepts each having two names across tools that are
designed to be used together:

| Concept | `symbols` | `edit_code` | `edit_markdown` | `artifact(update)` |
|---|---|---|---|---|
| symbol address | `name_path` | `symbol` | — | — |
| replacement text | — | `body` | `content` (`new_string` for `action="edit"`) | `body`, `body_edits[].content` |

Iron Law 1 routes discovery through `symbols` and Iron Law 2 routes the edit
through `edit_code`, so the handoff between these two vocabularies is the
prescribed path, not an unusual one.

## Evidence

Both refusals, verbatim, are in § Symptom — captured from the session that hit
them while inserting
`prompts::redesign_invariants::il1_states_the_overlap_condition_not_just_the_permission`.

Worth noting what worked well: each error named the correct key and, for
`symbol`, showed the exact syntax. Loud failure with a usable hint is why this is
low severity rather than a real hazard — contrast the silent-deletion sibling bug
referenced above, where the same shape of mistake returned `{"status": "ok"}`.

## Hypotheses tried

None — the behaviour is fully explained by the declared schemas. No investigation
needed to reproduce; the open question is what to do about it.

## Fix

Not implemented, and the right move is a judgment call rather than a defect fix.
Three options, cheapest first:

1. **Accept `name_path` as an alias for `symbol` on `edit_code`** (and keep
   `symbol` canonical). One-line param aliasing, no breaking change, removes the
   more likely of the two slips — the address is what gets copied from the
   preceding `symbols` call.
2. **Name both errors at once.** Validate the payload key alongside the address
   so a caller with both keys wrong learns both in one refusal. Turns two
   round-trips into one whenever a caller carries a whole call shape over.
3. **Converge the vocabulary** — pick one name per concept across all four
   tools, with aliases for the old ones. Widest fix, and the only one that stops
   this recurring; also the one most likely to churn every prompt surface and
   guide that documents these params.

Recommend 1 + 2. Option 3 should be weighed against the prompt-surface character
budget, since several of these names appear in `server_instructions` and in the
guides.

## Tests added

None yet. For option 1, an alias test asserting `edit_code` accepts `name_path`
and `symbol` interchangeably and errors identically when neither is present. For
option 2, a test that a call missing both keys names both in one error.

## Workarounds

`edit_code` takes `symbol` and `body`. When carrying a symbol address over from a
`symbols` call, rename `name_path` → `symbol`; when carrying edit text over from
`edit_markdown`, rename `content` → `body`.

## Resume

Decide between options 1–3 above. If option 1, find `edit_code`'s argument
deserialization and add a serde alias for `symbol`, then the alias test under
**Tests added**.

## References

- `docs/issues/2026-08-17-edit-markdown-edit-action-deletes-when-new-string-is-omitted.md` — the severe, silent instance of the same key-mix-up class
- `docs/PROGRESSIVE_DISCOVERABILITY.md` — tool-contract and guidance conventions

