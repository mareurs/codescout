---
kind: tracker
status: active
title: SDD ruling log — decisions agents made autonomously, and how they turned out
tags:
  - sdd
  - autonomous-execution
  - rulings
  - process
topic: agent-process
---

# SDD ruling log

When a plan runs under `superpowers:subagent-driven-development`, the controller is
instructed to **rule rather than stall**: conflicts, plan defects, and scope calls get
decided mid-run and recorded, so the run does not park waiting on a human. Those rulings
are real delegated judgements — and they normally live in the run's progress ledger, which
the process **deletes at finish**.

This file is where they land instead.

**Not a ledger.** No `entry_prefix`, no ids, nothing to cite. The unit of value is the
**ruling line itself**, not an index. Rows are appended by whoever runs a plan; the shape
is fixed so the table can be mined across runs.

## What to record

One row per ruling the controller made without asking the human. Not every decision — only
the ones where a human might reasonably have chosen differently, which is the same test the
SDD skill uses for what must be surfaced at finish.

| column | meaning |
|---|---|
| `date` | when the run happened |
| `plan` | plan-file slug, so the diff is findable |
| `ruling` | **the line.** What was decided, terse but complete enough to judge without the plan open |
| `class` | `correctness` · `scope` · `safety` · `process` · `measurement` |
| `cost if wrong` | what the human eats if the call was bad — the SDD skill requires this |
| `verdict` | `held` · `WRONG` · `open` — fill in later if it changes |

**`verdict` is the column that makes this minable.** A ruling that held teaches little; a
ruling that was wrong, next to its class and its cost, is the signal. Update it whenever a
later run proves one wrong — a row whose verdict never changes is either correct or unread.

## Lessons extracted so far

These are the generalisable findings, promoted out of the rows below. Each names the run
that produced it and how many datapoints it rests on.

### "Already fails loudly" is a claim about a code path, not about a feature

*(get-guide-section-grain, 2026-08-27 — 3 datapoints in one run.)*

A deferral was justified on the grounds that a malformed input "already errors". It did — on
the branch that was checked. `parse_declarations` comma-splits both `serves:` and
`requires:`; the `serves:` path errors on an unterminated predicate, the `requires:` path
silently yields two plausible heading names that resolve to nothing. The same reasoning
error was then made a second time one task later, and a third instance (near-miss comment
prefixes like `<!-- serve: -->`) was found by the final review.

**Name the path you checked, and check every path that reaches the same symptom.**

### A plan's reference code is a sketch; expect the review loop to supply correctness

*(get-guide-section-grain — 5 of 10 tasks carried a plan-mandated defect.)*

The plan produced correct interfaces, correct ordering, correct test *structure* across ten
tasks. Its inline code was right in shape and under-specified at the edges: a `bool` where a
state machine was needed, an unvalidated identifier split, a test whose fixture could not
discriminate, a gate predicate that was unconditionally true. Every one was caught by a task
review, none by the author.

The `plan-mandated` label in the review rubric is what makes this work — it stops a reviewer
waving a defect through because the plan told the implementer to write it.

### Vacuous assertions cluster — hunt for the next one

*(get-guide-section-grain — 4 found, the fourth only because it was hunted for.)*

Four separate assertions could not fail: a fixture whose alphabetical order coincided with
document order; a `contains("get_guide")` guard defeated by the guide's own preamble text; a
success check blind to the `RecoverableError` class it was written for; and a gate clause
that was `true || …` for every section it governed. All four passed their suites and looked
rigorous.

Three of them were found one at a time by per-task reviewers who each saw an isolated
finding. The fourth was found because the final reviewer was explicitly told *"three of these
have been found — look for a fourth."* **Once you have two, name the pattern in the next
review brief.**

The discriminating question is never "does this test pass" but **"what mutation of the
production code would make this test fail?"**

### An assertion added to close a missing-guard finding can itself be unable to fire

*(get-guide-section-grain — 1 datapoint, but a sharp one.)*

A reviewer found that a failed call and legitimate dedup were indistinguishable. The fix
added a success assertion. That assertion checked `is_error`, which codescout deliberately
sets to `false` for `RecoverableError` — the exact class the failing call would have
returned. The guard was decoration, in the specific place a guard had just been demanded.

**Demand a deliberate break.** Not "add the check and re-run a green suite" — break the
input, watch the assertion fire, revert, paste the output.

### Scoping a gate and weakening a gate look identical in a diff

*(get-guide-section-grain — the Task 9 gates.)*

Restricting a new gate to the topics that opted in is correct. Exempting by size, or
accepting a laxer assertion, is a retreat. Both produce a green build and both read as
reasonable engineering. Only the author knows which they did.

**Name both in the dispatch brief, say which is which, and ask for BLOCKED rather than a
weakened assertion.**

### Fixing a broken gate can reveal the constraint it was hiding

*(get-guide-section-grain — the byte ceiling.)*

Repairing the reachability gate exposed two unreachable sections. Making either reachable
was *arithmetically impossible*: the byte ceiling's margin was 54 B and the smaller section
was 346 B. The broken gate had been concealing that the corpus was already at capacity.

The honest move was a waiver naming the byte constraint and the remedy — converting a silent
hole into a declared one — rather than narrowing more content or raising the ceiling. **A
ceiling tuned to whatever the code currently does is a description, not a gate.**

### Correct code in the wrong tree defeats every quality gate

*(get-guide-section-grain — the `edit_code` leak,
`docs/issues/2026-08-27-edit-code-writes-to-session-default-not-pinned-workspace.md`.)*

A structural write landed in a different checkout than the one it was pinned to, returning
`ok`. The leaked change was *semantically neutral* — an additive trait default plus a
behaviour-preserving refactor — so the contaminated tree still compiled and still passed its
tests. No compiler, lint, test or review can distinguish correct-code-nobody-asked-for from
intended work.

**The only detector is `git status` on the other tree, after every task.** Not a gate — a
habit.

### Ephemeral reports are functionally deleted

*(get-guide-section-grain — recognised in a review, then repeated by the controller.)*

The final review flagged that evidence recorded only in a run ledger would be destroyed at
finish, and asked for it to be folded into a committed bug file. The controller then deleted
that same ledger — including every per-task report — ten minutes later, following the
process's own cleanup step.

**Commit the residue before the finish step runs.** This file exists because of that.

---

## Rulings

Append below. Newest run first.

### get-guide-section-grain — 2026-08-27

Plan: `docs/superpowers/plans/2026-08-27-get-guide-section-grain.md` · 23 commits ·
final review clean · 5 of 31 rulings turned out wrong.

| ruling | class | cost if wrong | verdict |
|---|---|---|---|
| `selector_key` must fall back to `Some(tool_name)` when a call has no `action`, else tool-only declarations can never match | correctness | wider slice on action-less calls | held |
| A closure test must use a synthetic fixture — the live corpus had zero declarations, so it asserted on an empty vec and could not fail | correctness | one test-only constructor | held |
| Rename a test whose name contradicted its assertion | process | none | held |
| Subagents must pass per-call `workspace=` pins, never `workspace(activate)` — the active project is one server-side slot shared across sessions, and a foreign activation defaults to read-only | safety | verbosity in dispatch prompts | held |
| Fence tracking must be `(delimiter, run_len)`, not a bool — the spec says "any line inside a fence", and a bool desyncs on nested/mixed fences | correctness | a few lines of state | held |
| Fold a corpus-wide partition test into the fence fix as its regression guard | scope | one cheap test | held |
| Fix the identifier-validation *class* (`[A-Za-z0-9_]+`), not the two reported instances | correctness | a typo'd declaration fails the build instead of silently never matching | held |
| Defer comma-splitting as "already fails loudly" | correctness | **broke a real `requires:` edge; caught two tasks later by the gate written for it** | **WRONG** |
| The reported "silent no-op" writes were a leak into another checkout — file as a regression of an archived bug | safety | a duplicate bug file | held |
| Do NOT self-authorise reverting files in another live session's working tree — escalate | safety | a pause | held |
| Read "run the next task first" as continue-but-don't-clean, not as authorisation to revert | safety | two inert files persist briefly | held |
| Revert the leak on explicit authorisation, after proving it inert (0 references in committed code, 0 in the other session's in-flight work) | safety | verified before acting | held |
| Move a task's review BASE so an unrelated doc commit stays out of its review diff | process | none | held |
| An ordering assertion whose fixture sorted identically to document order cannot fail — rename the fixture | correctness | strictly stronger test | held |
| Judge duplicate headings "fail-safe over-delivery" from `match_sections` without checking the delivery path | correctness | **it is silent UNDER-delivery — the second section is dropped permanently. Same reasoning error as the comma-split ruling, one task later** | **WRONG** |
| Add the missing `link_scan` table row rather than dropping the shape from its `serves:` | correctness | one table row | held |
| Use a `requires:` edge for `artifact.get`'s missing call syntax rather than a second `serves:` | scope | 591 B extra on one shape | held |
| The `_guide_hint` field contradicted the fallback block in the same response — make the hint shape-aware | correctness | a longer hint string | held |
| The pointer's only guard could not fail (`contains("get_guide")` matched the preamble's own text) — assert the emitted sentence | correctness | none | held |
| Fold in a stale doc comment that documented a real fail-safe inversion | scope | one larger fix round | held |
| The ceiling task must MEASURE, not inherit an estimate — and narrow declarations rather than raise the ceiling | measurement | set up the capacity finding | held |
| "Gate 2 replaces the old gate" was true of one of its four assertions — restore the other three | scope | **my brief was wrong; the implementer declined to follow it and was right** | **WRONG** |
| Escalate an "Important, non-blocking" finding: a failed call reads identically to legitimate dedup, and it feeds the headline number | measurement | assertions in a passing test | held |
| Escalate: a follow-up recommendation existed only in a report the process deletes | process | three lines in a doc | held |
| Verify a 2-byte delta personally rather than accept the explanation — it was a stale mid-work reading, not a masked failure | measurement | checkable from git | held |
| A success assertion could not detect `RecoverableError`, the class it was written for — demand a deliberate break as proof | correctness | a stricter assertion | held |
| A gate clause was `true \|\| …` for every section it governed | correctness | **1,802 B of a guide was unreachable with no waiver; both existing waivers were decorative** | **WRONG** |
| Fixing that gate showed reachability was arithmetically impossible at a 54 B margin — waive with honest reasons rather than narrow content or raise the ceiling | scope | content stays undeliverable in Phase 1, but visibly and with a named remedy | held |
| The ceiling constant stays; the failure MESSAGE was the wrong instrument | process | four lines of prose | held |
| Preserve the run's deferred minors in a committed doc before the workspace is deleted | process | three lines in a doc | held |
| Delete the run workspace at finish, per the process | process | **destroyed every per-task report ten minutes after a review flagged this exact failure mode** | **WRONG** |
