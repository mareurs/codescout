---
id: '365b599f3573b1c0'
kind: bug
status: open
title: 'BUG: conditionally-required params are advertised as optional — 41% of live schema errors are a param the schema never said was required'
owners:
- marius
tags:
- tool-schema
- agent-guidance
- leniency-policy
- usage-db-evidence
topic: prompt-surface-consistency
---

## Summary

Several tools have parameters that are required only for **certain values of another parameter** —
`edit_code`'s `body` (required when `action` is `replace` or `insert`), `artifact`'s `patch`
(required when `action` is `update`). JSON Schema's `required` array cannot express that, so these
sit in the optional set, and their descriptions explain *purpose per action* without ever saying
**required**. The model reads "optional", omits it, and the server rejects the call.

This is **41% of all live schema-shaped parameter errors** (14 of 34, July 2026 onward) — the
largest single cause, and the only one where the fix is free of judgement: state the requirement in
the description, so the first call is correct.

The remaining cases split cleanly into "can never be inferred" (a target or payload — guessing
would edit the wrong thing) and "could safely be completed". That split is the policy this bug
proposes, below.

## Symptom (Effect)

    action 'insert' requires 'body'
    action 'replace' requires 'body'
    missing field `patch`

## Reproduction

Call `edit_code(symbol=…, path=…, action="insert")` with no `body`. The schema
(`src/tools/symbol/edit_code.rs:44-53`) lists `required: ["symbol", "path", "action"]` and describes
`body` as *"replace: new body; insert: code to inject"* — accurate about meaning, silent about
requiredness. Same shape for `artifact(action="update")` without `patch`
(`src/librarian/tools/artifact.rs:126-128`), whose ~900-character description covers every accepted
key and never says the field is required for `update`.

## Environment

codescout on `experiments` @ `9746a5f0`. Evidence from 13 `.codescout/usage.db` files, 53,916 calls,
read 2026-08-15. Counts below are **2026-07 onward** — see *Scoping* for why the lifetime figures
are not used.

## The live list (2026-07 onward, 34 occurrences)

| n | Tool | Error | Class |
|--:|---|---|---|
| 6 | `edit_code` | `action 'insert' requires 'body'` | **A — conditional** |
| 6 | `edit_code` | missing `'symbol'` | B — target |
| 5 | `artifact` | missing field `patch` | **A — conditional** |
| 4 | `edit_markdown` | missing `'heading'` | B — target |
| 3 | `edit_code` | `action 'replace' requires 'body'` | **A — conditional** |
| 3 | `edit_file` | missing `'old_string'` | B — payload |
| 2 | `edit_markdown` | missing `'old_string'` | B — payload |
| 2 | `read_markdown` | missing `'path'` | B — target |
| 1 | `artifact` | unknown field `repo` | C — wrong name |
| 1 | `edit_code` | missing `'action'` | B — required |
| 1 | `references` | missing `'path'` | B — target |

**Class A (conditional): 14 of 34 = 41%.**

## Root cause

`required` in JSON Schema is a flat list. "Required when `action` is one of X" is expressible only
via `if/then` or `oneOf`, which many MCP clients do not surface to the model. So the honest options
are (a) mark it required always — wrong, it genuinely is optional for `rename`/`remove`; or (b) say
so in the description. Today neither happens: the description explains what the field *means* for
each action, which reads as documentation of an optional convenience rather than a precondition.

Measured 2026-08-15: 14 of 34 live schema errors are Class A; the two responsible parameters are
`edit_code.body` (`src/tools/symbol/edit_code.rs:53`) and `artifact.patch`
(`src/librarian/tools/artifact.rs:126`).

## Proposed policy — lenient where unique and safe, confident everywhere else

The request that prompted this bug was to be *"lenient but confident at the same time"*. Those are
not in tension if leniency is scoped by **blast radius**:

**Be lenient — accept and proceed — only when both hold:**

1. the missing value has exactly **one** sensible completion, and
2. the operation **cannot destroy anything** (a read, or an already-idempotent write).

Qualifying cases:

- **Partial line range.** `read_file` / `read_markdown` reject when exactly one of
  `start_line`/`end_line` is given (`src/tools/markdown/read_markdown.rs:531-537`, a strict XOR).
  `start_line` alone means "from here to the end"; `end_line` alone means "from the start to here".
  Both are unambiguous, both are reads, and the existing overflow machinery already prevents an
  accidental whole-file dump by returning a handle. 11 historical occurrences, none since May — so
  this is cheap insurance, not an urgent fix.
- **Accepted aliases.** Already precedent: `references` and `read_markdown` accept `file_path` as
  well as `path`, and say so in the hint. That is leniency done right — the alternative name cannot
  mean anything else.

**Be confident — refuse — whenever the missing value is a target or a payload:**
`symbol`, `path`, `heading`, `old_string`, `command`, `action`, `body`, `patch`. Inferring any of
these means choosing *what to edit* or *what to write*, and a wrong guess edits the wrong thing
silently. Class B (17 of 34) must keep failing.

**But "confident" is not satisfied by merely refusing.** It has two obligations:

1. **Prevent, upstream.** For Class A the requirement is knowable before the call — put it in the
   description: *"Required when action is 'replace' or 'insert'."* This is the whole fix for 41% of
   live cases, costs nothing at runtime, and removes the round trip rather than improving it.
2. **Name the concrete shape, in the rejection.** Compare the two hint styles currently shipping:

       missing 'symbol' parameter — hint: Add the required 'symbol' parameter to the tool call.

       missing 'path' parameter — hint: Pass the file path, e.g. path="src/foo.rs" — 'file_path'
       is also accepted. There is no implicit current file; every call names its own.

   The first restates the error. The second teaches the call. Roughly 13 of the live 34 carry the
   generic template. The project already ruled on this principle in
   `docs/issues/archive/2026-06-04-edit-file-old-string-miss-no-closest-match.md` — a bare
   "not found" was a defect *because the tool was holding the content needed to help*. The same
   argument applies here: the server knows the parameter's name, type, and purpose at the moment it
   rejects.

## Scoping — what was excluded, and why

The lifetime corpus shows 90 such errors; this bug counts 34. Two exclusions, both checked rather
than assumed:

- **Inverted filter AST** (`unknown field 'eq'`, 2 occurrences) — both dated **2026-05-03**, before
  `docs/issues/archive/2026-07-10-artifact-filter-inversion-misleading-hint.md` fixed it. Already
  solved; counting it would have inflated the case.
- **Smoke-test noise** — `completely_invalid_field`, `bogus_action_for_smoke_test` are deliberate
  test inputs, not agent behaviour.

The class as a whole is live but not worsening: 1.76 → 2.04 → 1.36 schema errors per 1,000 calls
across May / July / August.

## Hypotheses tried

1. **Hypothesis:** the whole class is historical and needs no action.
   **Test:** normalise by call volume per month rather than counting raw occurrences.
   **Verdict:** rejected — 1.36 per 1,000 calls in August, 19 occurrences. Live, mildly declining.

2. **Hypothesis:** leniency is the right fix for the dominant case.
   **Test:** ask what a lenient `edit_code(action='insert')` with no `body` would insert.
   **Verdict:** rejected — there is no unique completion, and the operation writes code. For Class A
   the fix is upstream (description), not at the error site. Leniency is the wrong tool for the
   biggest bucket, which is why the policy above is scoped by blast radius rather than by frequency.

## Fix

Not implemented. In priority order:

1. **Class A, description fix** — append the requirement to `edit_code.body`
   (`src/tools/symbol/edit_code.rs:53`) and `artifact.patch`
   (`src/librarian/tools/artifact.rs:126`). Addresses 41% of live cases. No runtime change.
2. **Class B, hint fix** — replace the generic *"Add the required 'X' parameter to the tool call"*
   template with a shape-carrying example, following `references`'s existing wording.
3. **Class C, leniency** — accept a one-sided line range; complete the missing bound. Lowest
   urgency (nothing since May) and lowest risk.

Sweep for other conditionally-required params rather than fixing only the two measured: any
`action`-dispatched tool is a candidate (`edit_markdown`, `librarian`, `memory`, `workspace`).

## Tests added

None — filed on discovery.

## Workarounds

Pass `body` whenever `action` is `replace` or `insert`; pass `patch` whenever `artifact` action is
`update`.

## Resume

Edit the two descriptions named in Fix (1) and re-measure: Class A should fall from 14 per
~34 live schema errors toward zero, and the per-1,000-call rate from 1.36. Both queries are in
Evidence/Scoping. Then decide on Fix (2) — it is a wording sweep across the `missing 'X' parameter`
emitters, not a behaviour change.

## References

- `src/tools/symbol/edit_code.rs:44-53` — `edit_code` schema; `body` optional, description silent on requiredness
- `src/librarian/tools/artifact.rs:126-128` — `artifact` schema; same for `patch`
- `src/tools/markdown/read_markdown.rs:531-537` — the XOR that rejects a one-sided range
- `docs/issues/archive/2026-06-04-edit-file-old-string-miss-no-closest-match.md` — the precedent that a
  bare rejection is a defect when the tool holds what the caller needs
- `docs/issues/archive/2026-07-10-artifact-filter-inversion-misleading-hint.md` — fixed; why the `eq` rows are excluded

