---
id: 85982e8f57d2dd90
kind: bug
status: fixed
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
`docs/issues/archive/2026-08-17-edit-markdown-edit-action-deletes-when-new-string-is-omitted.md`,
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

Implemented — options 1 and 2 from the original plan. Option 3 (converging the
vocabulary repo-wide) is deliberately not taken: it would churn every prompt surface
and guide documenting these names, against a character budget that is already the
binding constraint on `server_instructions`.

The alias infrastructure already existed and was already tested —
`require_str_param_or_hint(input, name, aliases, hint)` in
`src/tools/core/params.rs`, with canonical-wins-over-alias precedence covered by
`require_str_param_or_hint_prefers_canonical_over_alias`. The fix is to use it.

In `src/tools/symbol/edit_code.rs`:

- **`name_path` is accepted for `symbol`.** `require_str_param(&input, "symbol")`
  becomes `require_str_param_or_hint(&input, "symbol", &["name_path"], …)`. This is
  the more valuable of the two: the address is what gets copied out of the preceding
  `symbols` call.
- **`content` is accepted for `body`,** via a small `body_param(input)` helper shared
  by the `replace` and `insert` arms so they cannot drift.
- **Both refusals name their alias**, through a shared `BODY_PARAM_HINT` and the
  `symbol` hint text. An alias that only helps callers who already know it exists
  helps nobody — the mistake is made by callers reasoning from the sibling tool.
- **The schema advertises both**, so they are discoverable without first triggering a
  refusal.

The error *messages* are held byte-stable — `"action 'replace' requires 'body'"` and
`"action 'insert' requires 'body'"`. `src/usage/db.rs` classifies error families by
these strings (`normalize_err_family_maps_the_unclassified_head`), and the archived
bug `2026-08-15-conditionally-required-params-advertised-optional.md` measured this
class against them. All new text goes in the *hint*, which is additive.

Not done, and deliberately: `edit_markdown` was NOT taught to accept `body`. Its
sibling defect (`docs/issues/archive/2026-08-17-edit-markdown-edit-action-deletes-when-new-string-is-omitted.md`,
fixed in the same commit) hinges on `action="edit"` distinguishing a missing
replacement from an intentional empty one; adding more accepted spellings of the
replacement there widens exactly the surface that had to be narrowed. Aliasing is
safe where an absent value is refused outright, which is the case for both
`edit_code` arms and not for `edit_markdown`'s scoped swap.

Fix SHA: this commit, on `experiments`. `master` is a strict ancestor at fix time,
so the promotion path is fast-forward and this SHA is already the master SHA.
## Tests added

Two, in `src/tools/symbol/tests.rs`.

`edit_code_accepts_the_sibling_tools_names_for_symbol_and_body` asserts the aliases
resolve — **without needing a live language-server edit**. The trick is to assert on
*which* precondition fails: a call carrying `name_path` and no body must get past
address resolution and complain about the body instead. That is positive evidence the
alias took effect, and it costs no LSP round-trip:

| Call | Must NOT say | Must say |
|---|---|---|
| `name_path` + no body | `missing 'symbol'` | `requires 'body'` |
| `symbol` + `content` | `requires 'body'` | — |

`edit_code_refusals_name_the_accepted_aliases` covers the discoverability half:
the missing-symbol refusal names `name_path`, the missing-body refusal names
`content`, and the schema advertises both. Without these the aliases would work while
remaining invisible, which fixes the second attempt and not the first — and the first
attempt is the one that costs the round-trip.

Modelled on `edit_code_advertises_every_conditionally_required_param`, which
established the pattern of asserting runtime refusal and schema text together.

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean,
`cargo test` 4017 passed / 0 failed / 45 ignored.
## Workarounds

`edit_code` takes `symbol` and `body`. When carrying a symbol address over from a
`symbols` call, rename `name_path` → `symbol`; when carrying edit text over from
`edit_markdown`, rename `content` → `body`.

## Resume

N/A — both aliases accepted, advertised in the schema, named in the refusals,
tested, and confirmed on the wire after `cargo rb` + `/mcp`:

```
edit_code(path=…, action="replace", name_path="body_param")
→ ok: false
  error: action 'replace' requires 'body'
  hint:  Pass the code as body="..." — … `content` is accepted as an alias
         (that is edit_markdown's name for the same argument).
```

The address resolved from `name_path` — the call reached the *body* precondition
instead of `missing 'symbol'`, which is the same positive signal the unit test
asserts — and the refusal names the other alias unprompted.

Option 3 from § Fix (one name per concept across all four tools) stays unactioned by
choice, not by omission. Revisit only if usage data shows the mismatch still costing
refused calls after the aliases have shipped — the aliases cover the two directions
actually observed, and a rename would spend prompt-surface characters that Iron Law 1
currently needs.
## References

- `docs/issues/archive/2026-08-17-edit-markdown-edit-action-deletes-when-new-string-is-omitted.md` — the severe, silent instance of the same key-mix-up class
- `docs/PROGRESSIVE_DISCOVERABILITY.md` — tool-contract and guidance conventions
