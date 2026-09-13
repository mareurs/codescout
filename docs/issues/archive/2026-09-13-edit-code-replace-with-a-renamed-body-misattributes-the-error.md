---
id: 120e3207d427ea14
kind: bug
status: archived
title: 'BUG: edit_code(action="replace")''s error for a body whose fn name differs from `symbol` misattributes the cause, sending the caller to change the body rather than the action'
owners:
- marius
tags:
- cluster/unclassified
- codescout-tool
- edit_code
- error-messages
claimed_at: 2026-09-13
claimed_by: f3c594ce-c424-40d3-a603-9693cfef3f63
closed: null
opened: 2026-09-13
owner: marius
related: []
severity: low
---

## Summary

`edit_code(action="replace", symbol="OLD_NAME", body="fn NEW_NAME() { ... }", attributes=[...])`
— i.e. supplying a `body` whose function name differs from `symbol`, in an attempt to rename and
re-body a function in one call — is refused with:

> edit_code replace('OLD_NAME') dropped the symbol definition — body must be the complete
> declaration (attributes, doc comments, signature, and body), not just body statements. File
> restored.

The message's stated diagnosis (*"not just body statements"*) is not what happened: the supplied
`body` **was** a complete declaration — full signature, braces, everything — it just declared a
different name than `symbol`. The message describes the wrong defect and sends the reader to fix
the wrong thing.

## Symptom (Effect)

A caller reads *"you passed body statements, not a full declaration"*, goes back and re-checks
that their `body` opens with `fn` and closes its braces — which it already did — and has no route
from the message to the actual rule (renaming happens via `action="rename"`, not by changing the
name inside a `replace` body).

## Reproduction

Against any Rust file with a test function `foo`:

```
edit_code(action="replace", path="<file>", symbol="foo",
          body="fn bar() { /* same or different body */ }")
```

→ the "dropped the symbol definition" message above, naming `foo` (the old `symbol`) as the
subject, with no mention that `bar` (the name actually in `body`) is why.

Observed live 2026-09-13 while fixing `tests/result_caps.rs`
(`docs/issues/archive/2026-09-03-classify-conflates-two-malformed-reasons-under-one-message.md`): renaming
`unclassified_decls_reports_a_malformed_result_cap_id_under_the_not_a_cap_message` to
`..._with_its_own_message` via `replace` with a full attributes+body payload hit exactly this
message. The fix was to keep `symbol` and the body's fn name identical for the `replace` call, then
issue a separate `action="rename"` call — which worked immediately once tried.

## Environment

codescout MCP server, `edit_code` tool, `action="replace"`. Session date 2026-09-13.

## Root cause

Not read from `edit_code`'s implementation (out of scope for this fix session) — inferred from
behavior: `replace` appears to key its symbol-presence check on parsing the new `body` for a
declaration named `symbol`, so a body naming a *different* function does not "contain" the symbol
being replaced and trips the same guard a truncated/partial body would. The message was written
for the truncated-body case and is reused verbatim for this one, even though the two have
different causes and different remedies.

## Hypotheses tried

1. **Hypothesis:** the `attributes` array was malformed or too short, causing the tool to see an
   incomplete declaration.
   **Test:** re-read the call — `attributes` was a complete list ending in `#[test]`, and `body`
   opened with `fn ... () {` and closed correctly.
   **Verdict:** rejected — the declaration was complete by inspection.
2. **Hypothesis:** `replace` does not support changing the function name in the same call as a
   body edit, and the guard's message is just misattributed to a different (truncation) cause.
   **Test:** retried with the body's fn name matching `symbol` (no rename), which succeeded; then
   issued a separate `action="rename"` call, which also succeeded.
   **Verdict:** confirmed.

## Fix

**FIXED** on `experiments` — `cacbae14`, patch-id `3b9bab0771af214f8230b8da0a9560090b0e5dd4`.

`CorruptionVerdict` gains `TargetRenamed(String)`, split out of `TargetDropped`.

**The two were one verdict because the name-set test cannot tell them apart:** the
target's name is absent from the post-AST either way. What separates them is *what took
its place*. A complete declaration under a different name leaves a **1:1 substitution** —
nothing else lost, exactly one name appeared. Bare statements leave nothing. So the split
is a second observation on the same data, not a new scan:

```rust
if lost_others == 0 && appeared.len() == 1 {
    return CorruptionVerdict::TargetRenamed(appeared.remove(0));
}
```

**The predicate is unchanged and that is deliberate.** `replace` still refuses, still rolls
back. Only the diagnosis and the remedy moved. This bug is about where a correct refusal
sends the reader — widening what `replace` accepts would be a different change with a
different risk, and nothing here asked for it.

**The new arm is guarded against over-firing rather than trusted.** Damage that merely
resembles a rename — a sibling also gone, or two names appearing where one symbol stood —
fails the one-for-one test and keeps falling through to the blunter verdicts. Telling a
caller *"you meant to rename"* while their edit is destroying siblings would be a worse
wrong answer than the one being fixed.

**`do_remove` is unaffected by construction, not by care.** It passes `pre_count = 0`, and
the rename branch is nested inside the same `pre_count > 0` test as `TargetDropped`, so it
is unreachable there — a removal replaces the target with nothing, so no name can appear.
Adding the variant forced a compiler-checked arm at `do_remove`'s exhaustive match, which
is how that site got its explanatory comment.
## Tests added

Three, and the third exists because the second one **failed its own mutation**.

| test | site | what kills it |
|---|---|---|
| `a_body_declaring_a_different_name_is_a_rename_not_a_dropped_target` | the verdict | disabling the one-for-one branch |
| `a_rename_shaped_verdict_requires_a_clean_one_for_one_substitution` | the verdict's *guard* | widening it to fire on damage |
| `a_replace_whose_body_renames_is_told_to_use_rename` | the **remedy text**, end to end | deleting the routing from the hint |

**The remedy test is the one this class of bug needs, and it is the one nobody writes.**
`CLAUDE.md` § *Testing Discipline*: *"a suite tests a guard's PREDICATE and never its
REMEDY TEXT ... every assertion is about who is refused; nobody writes one about where the
refusal sends you."* Before this, **no test anywhere asserted on this message** — a single
grep for its text across `src/` returned the message site and nothing else. It runs end to
end through `EditCode.call` against a `MockLspProvider`, so it exercises the shipped
refusal rather than a re-implementation.

**MUTATION RESULTS — one per guarded SITE, and the second site's first attempt SURVIVED.**

- Verdict site, one-for-one branch disabled → **both** the unit test and the end-to-end test
  red, and the end-to-end failure printed this bug's headline message verbatim
  (*"dropped the symbol definition — body must be the complete declaration ... not just body
  statements"*). The regression reproduces on demand.
- Remedy site, routing deleted from the hint → **GREEN**, wrongly. The assertion was
  `err.contains("rename")`, and the *diagnosis* already says *"`replace` cannot rename"* —
  so the word is present whether or not the remedy is. **That is
  `cluster/assertion-satisfiable-by-accident` occurring inside the test written to guard
  remedy text**, which is the sharpest thing this fix produced. Repaired by asserting the
  **callable form** `action="rename"`, which appears only where the caller is told what to
  run; re-run against the same mutation, it reds. The comment on the line says why, because
  the weaker form is the one a later reader would naturally write.

This is § *Testing Discipline*'s *"demand an observed RED, never an assertion's existence"*
paying out directly: the assertion existed, read correctly, and was worth nothing until it
was mutated.
## Workarounds

Do a `replace` with the body's function name unchanged from `symbol`, then a separate
`action="rename"` call for the name change. Two calls, but each succeeds cleanly and the intent
(re-body vs rename) stays legible in the tool-call history.

## Resume

Filed 2026-09-13 by one session; hit independently the same day by `f3c594ce`, twice within
an hour, while renaming test functions in `parser.rs`. Both readers did what the message
said — re-checked that the body was a complete declaration, found it already was — and then
had to derive `action="rename"` from somewhere other than the error.

**That is the diagnostic-value test this repo asks for, answered in the negative by use
rather than by review:** ask what an observer would *see differently* if this were broken
right now. Two of them saw the same wrong thing and recovered by guessing.

**A CLASS WITH NO CLUSTER, and both instances sit in `cluster/unclassified` — flagged, not
opened.** This file's own § Reproduction cites
`docs/issues/archive/2026-09-03-classify-conflates-two-malformed-reasons-under-one-message.md`
(`a807b70cb7ee7340`, archived 2026-09-13), and the shape is identical: **a diagnostic
collapses two causes that take opposite repairs into one message, and the message names
one of them.** Neither is a guard whose coverage is too narrow (`IC-14`), nor an
addressing scheme without an escape (`IC-6`), nor a selector narrower than its population
(`IC-18`) — the predicate is right and the population is right in both; the *diagnosis* is
coarser than the causes.

That is `n=2` across two subsystems (a result-cap probe's `classify()`, and
`corruption_verdict`), which is better supported than `IC-20` was at its opening. It is
recorded here rather than opened because opening a defect class was not part of this fix,
and a class opened in passing is exactly the kind whose inclusion test nobody defends
later. The argument is written down so the next instance does not have to re-derive it —
which is the failure `docs/trackers/issue-clusters.md` exists to prevent, three open files
saying *"this is the third instance of one mechanism"* with nowhere to put the count.

**Related but distinct:** `OB-26` is the filing-side twin (a § Summary asserting the
mechanism its § Root cause hedges), and `IC-23` is the fixture-side one (a test that holds
the deciding variable constant). All three are about a **discriminator that exists in the
data and is not read** — here the post-AST already contained the replacement name, unused,
while the verdict reported only that the old one was gone.
## References

- Surfaced fixing `docs/issues/archive/2026-09-03-classify-conflates-two-malformed-reasons-under-one-message.md`
  in this same session (2026-09-13).
- `tests/result_caps.rs`'s
  `unclassified_decls_reports_a_malformed_result_cap_id_with_its_own_message` — the function whose
  rename tripped this.
