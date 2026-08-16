---
id: '02d2d9d8a7eeec2e'
kind: bug
status: fixed
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

Confirmed at the bytes, and **wider than filed**.

`required` in JSON Schema is a flat list. "Required when `action` is one of X" is expressible only
via `if/then` or `oneOf`, which many MCP clients do not surface to the model. So the honest options
are (a) mark it required always — wrong, it genuinely is optional for `rename`/`remove`; or (b) say
so in the description. Today neither happens: the description explains what the field *means* for
each action, which reads as documentation of an optional convenience rather than a precondition.

Measured 2026-08-15: 14 of 34 live schema errors are Class A; the two responsible parameters are
`edit_code.body` (`src/tools/symbol/edit_code.rs`) and `artifact.patch`
(`src/librarian/tools/artifact.rs`).

**What the filing missed (found 2026-08-16 while fixing).** `edit_code.new_name` has the same
defect and was not filed. Its description read `"rename only"` — and this same tool uses that
*identical phrasing* for two params that are genuinely optional: `attributes` ("replace only: …
Omit to keep the default") and `position` ("insert only, default 'after'").

So the underlying defect is not "two descriptions are incomplete". It is that **the `"<action>
only"` convention marks *scope*, never *obligation*** — the reader cannot tell a scoped-and-required
param from a scoped-and-optional one. Where a `default` is stated they can infer it; where none is
(`body`, `new_name`) they cannot. `body` is simply the highest-traffic instance.

The runtime, by contrast, is unambiguous: `edit_code`'s `call()` refuses three (action, param)
pairs — `rename`→`new_name`, `replace`→`body`, `insert`→`body`. The schema and the check were two
hand-maintained statements of one rule, and only one of them was true.
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

**Class A fixed on `experiments` in `1a54b5a6`.** Classes B and C deliberately not attempted — see
below.

1. **Class A, description fix — DONE, and extended to a third param.** `edit_code.body`,
   `edit_code.new_name` (added by the scout) and `artifact.patch` now open with an explicit
   requirement clause. In `edit_code` the clause is rendered from two consts —
   `BODY_REQUIRED_ACTIONS` and `NEW_NAME_REQUIRED_ACTIONS` — through one `required_for()` helper,
   so every conditionally-required param states the obligation in the same words and a test can
   assert on it without pattern-matching prose.

   **Runtime messages were deliberately left byte-identical.** `usage::db::normalize_err_family`
   classifies this family by the literal substring `requires '`; changing the wording would have
   broken the very measurement that found the bug. The new test asserts that substring for exactly
   this reason.

2. **Class B, hint fix — DONE in `c057ff5a`** (concurrent session, same day; this entry was
   written while it was still outstanding and is reconciled here rather than left stale).
   The generic *"Add the required 'X' parameter to the tool call"* — roughly 13 of the 34 — is
   replaced by hints that show the call: `grep` with no pattern now answers *"Pass the regex to
   search for, e.g. pattern=\"fn my_func\" — 'query' and 'regex' are also accepted."*

   Two mechanisms, because a shared table alone is provably insufficient: a name→hint table for
   the seven names whose meaning is identical everywhere, plus **per-site** hints for `path` and
   `action`, which mean different things in different tools. A shared entry for `path` would be
   confidently wrong at two sites out of three *and* read as authoritative — the same
   failure mode as the `kind=` hint fixed in `99fa967f`.

   **Class B is not entirely closed.** Its other emitter is serde's bare `missing field 'X'`
   — which is exactly how `artifact(update)` without `patch` fails. Class A covered that one by
   *description*; the error itself still names the field without the action that wanted it, and
   fixing it is error mapping in the librarian adapter rather than the shared helper.

3. **Class C, leniency — still not done.** Accepting a one-sided line range is a behaviour
   change, not a description change; lowest urgency (nothing since May) and lowest risk, per the
   original ranking. This is the only part of the original three-class plan still outstanding,
   alongside the serde half of Class B noted above.

The sweep the filing asked for was run, and it found a **fourth tool**.

**`artifact_event` — fixed in `6ba720bc`.** Its `payload` is the same defect nested one level
deeper: required keys depend on `kind`, and the schema said only "create: event payload (a JSON
object)". Nine required fields across seven kinds. This one was already on record as TU-9 in
`docs/trackers/2026-08-15-tool-usage-investigation.md`, which asked for exactly this — *"Add it to
TU-4's fix"* — and noted `artifact_event` ran a **50% error rate** in the 2026-07 window.

Why the original measurement missed it is the reusable part: **those errors carry no
`err_family`**, so the family-ranked analysis that surfaced `edit_code.body` and `artifact.patch`
was structurally unable to see them. It took a per-tool sweep.

Other `action`-dispatched tools do carry conditional requirements at runtime (`peer` needs
`peer`/`tool`/`handle` per action; `artifact`'s `move`, `delete`, `graft` need their ids), but
those surface as serde `missing field` errors rather than the Class A shape, which puts them in
Class B — the same fix, deferred with it, rather than a different one.
## Tests added

`edit_code_advertises_every_conditionally_required_param` (`src/tools/symbol/tests.rs`) asserts
both halves of the contract, separately, over all three (action, param) pairs.

The first half **executes** the runtime rather than reading a constant — it calls `edit_code` with
the action and no param and asserts the refusal. That is what stops the table drifting from
`call()`: delete a check there and the test fails, rather than the table quietly describing a rule
that no longer exists. The second half asserts the schema description carries `REQUIRED` and names
the action.

Mutation-verified: restoring `new_name`'s original `"rename only"` wording fails the test. A
second, independent tripwire fires at the same time — `NEW_NAME_REQUIRED_ACTIONS` becomes unused
and `-D warnings` rejects the build.

Gate: `cargo fmt` + `cargo clippy --all-targets -D warnings` clean, `cargo test --lib` 3756 passed
/ 0 failed / 7 ignored.
## Workarounds

Pass `body` whenever `action` is `replace` or `insert`; pass `patch` whenever `artifact` action is
`update`.

## Resume

Classes A and B are done — `1a54b5a6` + `6ba720bc` (A), `c057ff5a` + `ab94c33f` (B). The
previous text here (*"edit the two descriptions named in Fix (1) and re-measure"*) predated
the fix and is superseded.

**Two remainders, both real, neither blocking:**

1. **Class C — leniency on a one-sided line range.** `read_file` / `read_markdown` reject
   when exactly one of `start_line`/`end_line` is given (strict XOR,
   `src/tools/markdown/read_markdown.rs:531-537`). Both readings are unambiguous and both
   are reads, so this qualifies under the policy above. 11 historical occurrences, none
   since May — cheap insurance, not urgent.
2. **The serde half of Class B.** `require_param`'s emitter now teaches the call, but the
   *other* Class B emitter is serde's `missing field 'X'` — `artifact(action="update")`
   without `patch` being the case the librarian tool description already documents as a
   wart. That needs error mapping in the librarian adapter, not the shared params helper,
   which is why `c057ff5a` deliberately stopped short of it.

**Then re-measure**, which the original Resume was right about: Class A should fall from 14
per ~34 live schema errors toward zero, and the per-1,000-call rate from 1.36. Both queries
are in *Evidence* / *Scoping*. Do the measurement **before** attempting Class C — if Class A
and B took the rate to near zero, Class C's 11 stale occurrences are not worth a behaviour
change.

**One lesson from Class B worth carrying into the serde half:** its first commit passed its
own tests while the live server still emitted the generic hint, because
`require_topic_param` bypassed the shared helper the tests exercised. When the fix lands in
a shared helper, drive at least one real call site in the test — otherwise the test proves
the helper works, not that anything uses it.
## References

- `src/tools/symbol/edit_code.rs:44-53` — `edit_code` schema; `body` optional, description silent on requiredness
- `src/librarian/tools/artifact.rs:126-128` — `artifact` schema; same for `patch`
- `src/tools/markdown/read_markdown.rs:531-537` — the XOR that rejects a one-sided range
- `docs/issues/archive/2026-06-04-edit-file-old-string-miss-no-closest-match.md` — the precedent that a
  bare rejection is a defect when the tool holds what the caller needs
- `docs/issues/archive/2026-07-10-artifact-filter-inversion-misleading-hint.md` — fixed; why the `eq` rows are excluded
