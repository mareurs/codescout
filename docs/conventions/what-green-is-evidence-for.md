---
kind: convention
status: active
title: What a green suite is evidence for — the measurements behind the laws
owners: []
tags:
  - testing
  - mutation-testing
  - epistemics
---

# What a green suite is evidence for — the measurements behind the laws

**Audience:** anyone adding a test, trusting one, or deciding a property is covered. The **laws**
live in [`CLAUDE.md`](../../CLAUDE.md) § *Testing Discipline* and stay there because they change
what you do at the keyboard. This page holds the **derivations** — the mutation runs, the
counts, and the superseded formulations — so the executable copy stays readable.

Each law below was promoted from a measured failure, not from argument. There is deliberately **no
count** of the laws, here or in `CLAUDE.md`: a tally of the section's own contents is a premise that
every addition falsifies. That sentence read *"all four"* for an hour after a fifth law landed,
which is § *Observer Blindness*'s premise-moved-conclusion-didn't firing inside the section that
documents it. Removing the number removes the class; correcting it would only have reset the clock.

## Monotone assertions — the `e6414362` run

**Law:** a test cannot detect a change its assertion is MONOTONE under.

Absence assertions (`is_empty()`, `!exists()`) are monotone under **removal** — a dead mechanism
produces exactly the silence they assert. Existence assertions ("a region *containing* X is found")
are monotone under **widening** — over-reporting satisfies them. Both look like guards, and neither
fires in its own direction, so a property held by one of each is covered **zero** times, not
weakly. For each test ask which direction its assertion is monotone under, and mutate the *other*
way.

Measured (`e6414362`): a locator widened to swallow its whole section killed **none of six tests** —
the silence test and the positive test were both blind, and pairing them bought nothing.

**Superseded formulation, kept because it is falsified above:** an earlier version of this law
prescribed *"pair an absence test with a positive one."* That is exactly the configuration the
`e6414362` run killed six-for-zero. Full history in `reconnaissance-patterns` R-132 and R-133.

## Recording filters — the harder twin, and why "widen the sample" is a no-op

**Law:** a test cannot detect what its RECORDING filters out.

The monotone law is about *members*: a population selected so no member can falsify. Its fix is to
add one that could — the nine dead-code probes that all had visible callers become informative the
moment a tenth is drawn from the dispatch side. This one is about *observations*: the refuting
outcome leaves **no artifact**.

Measured 2026-09-01 — a ledger was about to publish *"of the near-miss numbers, four were caught by
re-derivation and zero by inspection"*, and that zero is unfalsifiable by construction, because a
reader who doubts a figure and re-counts it produces nothing. The wrong number never ships, so
nothing is recorded, so the population contains only the cases where doubt failed to occur.
**"Widen the sample" fixes member-selection and is a no-op here at any corpus size** — a bigger
corpus of ledger entries still contains zero catches-by-reading, forever. That is what makes it
worse than a small sample: the reflex answer looks responsive and changes nothing.

The remedy is to instrument the **doubt**, not the correction: when a re-derivation *confirms*,
publish the confirmation. Nobody is inclined to, because there is no finding to report — which is
exactly why the negative case has no record. One null result was published that day only because
someone said so out loud, and it is a **denominator**, not a catch: a confirming re-derivation never
entered the wrong-number population and bears on the base rate rather than on the hit rate.
Absorbing it as a near-catch would make the population look self-correcting.

## One mutation per guarded SITE — the `doc(action="augment")` run

**Law:** mutate once per guarded SITE, not once per feature.

A mutation run answers a question about one *line*. Where a law is implemented at N call sites, one
kill proves exactly one site is guarded and says nothing about the other N−1.

Measured: the augment path (then the standalone `artifact_augment` tool) had two shape-writing paths, and mutating each separately killed
**different** tests, neither failing under the other's mutation — so a single mutation would have
supported "covered" with the second site unguarded.

### The sharpening — a case guards an INPUT, not a guard

**Law:** a case only exercises the site it NAMES if every **other** guard admits its input.
Where several guards can refuse one input, the **first to refuse owns it**, and every other
guard's case is vacuous *for that guard* while reading as full coverage.

The section above is about N call **sites**. This is about N guards at **one** site, and the
laws are independent: you can mutate every site and still have this one, because the thing that
goes unexercised is a *branch of a predicate* rather than a location.

**Three instances, 2026-09-15/16, two subsystems, every one green until mutated.**

1. **Two bounds addressing one symptom** — `scripts/file-provenance.py`'s `RELOCATORS` gained
   `(?=\s)` (a verb boundary a filename cannot satisfy) and `\n` (an operand tail that stops at
   its own command). TDD was followed: a test pair written, observed RED, observed GREEN.
   Mutating each bound **separately** — both **SURVIVED**. Each bound *independently* prevents
   the composite failure the assertions describe, so the pair guarded the **conjunction** and
   neither site. **TDD cannot reach this by construction:** the red is observed in the pre-fix
   state, the one configuration where *both* bounds are absent. `b59a035d`.

2. **A NEW bound silently un-guarding an OLD one** — a third bound (command position) was added
   to the same regex hours later, and the case written that same morning *specifically to
   isolate* `(?=\s)` then **SURVIVED**. Both its inputs put the verb-shaped filename after a
   `/`, which the new anchor rejects **earlier, for its own reason**. Nothing announced the
   transfer of ownership. `2f32faa3`.

3. **A disjunction in a different language** — `empty_test_selection_diagnostic`'s
   `!saw_summary || filtered == 0 || passed > 0 || ignored > 0`. The case written to justify
   `filtered == 0` carries `14 passed`, so `passed > 0` refuses that input first and the
   filtered guard is never consulted; dropping it **SURVIVED**. Isolated by an empty
   **population** (`0 passed; 0 filtered out`, no filter in play), which only that guard can
   refuse. `cb397d1f`.

**Why it is invisible.** In all three the test name, its comment and the author's intent all
claim the guard is covered, and the suite agrees by staying green. Nothing in a diff, a review
or CI distinguishes "this case exercises this guard" from "this case is refused earlier by a
different one". Only a mutation asks the question the name is already answering.

**Operational form, both halves.** When writing a case for a bound, construct an input every
**other** bound admits — otherwise you are testing the other bound. And **after adding or
changing any bound in a predicate, re-run the mutation set for EVERY bound**, not a new case
for the new one.

**Corollary, measured the same day: a mutation verdict is a measurement of specific BYTES.**
Clippy's `question_mark` lint rewrote one line of (3) into `strip_prefix("ok.")?` — behaviour
identical, so the natural move is to keep the verdicts. They had already decayed: the anchor
string no longer existed, so a stale script either aborts on its occurs-exactly-once assertion
or, lacking one, matches nothing and reports SURVIVED — indistinguishable from a real
survival. **A refactor that preserves behaviour still invalidates the evidence that the
behaviour is guarded.** Green tests after a refactor say the behaviour survived; they say
nothing about whether the guards still discriminate.

Session-log entries: `bug-fix-session-log:W-141` (instance 1), `W-143` (instance 2).

## `contains` is monotone too — two assertions that survived removing the thing they guarded

**2026-09-13, two sessions, two subsystems, same day.** Both assertions read correctly, were
deliberately written to guard the property in question, and were worth nothing. Neither was
found by review; each was found by one mutation.

| | assertion | mutation applied | result |
|---|---|---|---|
| A | `err.contains("rename")` — *the refusal routes the caller to `action="rename"`* | delete the routing from the hint | **green** |
| B | integration: *the citation scan finds a dateless-slug citation* | `stem[11..]` → `stem[12..]`, one byte short | **green** |

**A** (`120e3207d427ea14`, `edit_code`): the *diagnosis* sentence already reads
*"`replace` cannot rename"*, so the needle is present whether or not the remedy is. The
assertion could not distinguish a message that routes the caller from one that merely
mentions the word.

**B** (`eba3d2c6`, `files_mentioning`): the mutation produced `"xample-bug"` — still a
**substring** of the citer's `"example-bug"` text, so the `grep -F` still matched. The
assertion could not distinguish the correct stem from a truncation of it.

**The unifying claim, and it is the FIRST law in this file arriving at string assertions.**
`haystack.contains(needle)` is **monotone in both arguments**: it is satisfied by a haystack
that grew *and* by a needle that shrank. So it cannot discriminate

- a needle present for the reason under test from one present for an unrelated reason (A), nor
- a correct needle from any substring of it (B).

Ask which direction the assertion is monotone under and mutate the other way — here, *both*
ways are the wrong way, which is what makes `contains` unusually weak for a property claim.

**The remedy is NOT "assert more".** Adding a second `contains` inherits the same monotonicity.
It is **choose a needle that can only appear when the property holds**, or leave containment
entirely:

- A: assert the **callable form** `action="rename"`, which occurs only where the caller is
  told what to run — not in the diagnosis.
- B: a direct unit test with `assert_eq!` on the exact output. Equality is monotone under
  nothing.

**Both repairs were verified against the same mutation that defeated the original**, which is
the only evidence that counts: the weaker form is what a later reader naturally writes, so each
site carries a comment saying why the obvious assertion was rejected.

**Why this is worth a section rather than a note.** A is `cluster/assertion-satisfiable-by-accident`
occurring *inside a test written to guard remedy text* — the instrument built to catch a class
of vagueness was vague in the same way. And in A the per-SITE discipline immediately above had
already been followed: mutating the *verdict* site killed both tests and reproduced the bug's
headline message verbatim, which read as full coverage. One site's kill said nothing about the
other's, exactly as that section claims — and the surviving site was the one the test existed
for.

## Loudness is a property of a PATH — the `BL-66` alarm nothing reached

**Law:** loudness is a property of a PATH, not of a failure.

An alarm nothing reaches is exactly as informative as no alarm: `BL-66` *aborts the process* —
maximally loud — and survived anyway, because every in-tree construction site installs the provider
first. When adding a guard, alarm, error return or `panic!`, name the concrete caller that reaches
it **and** the observer who acts on what it emits.

"An external consumer we do not have in-tree" is a legitimate reason to keep one and is **not**
coverage of our own risk — say so at the test, or the green tick gets read as protection. The tell:
ask what an observer would *see differently* if this were broken right now; if the answer is
"nothing", the guard is decoration however loudly it is written.

## The law reaches past guards, to features — the un-registered tools

`ListFunctions` and `ListDocs` each implemented the `Tool` trait and were **registered nowhere** —
`src/server.rs` is where a tool joins the registry (`Arc::new(Grep)`), and neither name ever
appeared in it. Their module carried a full test suite that passed for months while no agent could
reach a line of it. Both were **deleted 2026-09-01**; the tree-sitter layer the module wrapped
(`crate::ast`) was untouched and remains load-bearing:

```text
src/tools/ast.rs — deleted 2026-09-01, along with ListFunctions and ListDocs
```

Keep `BL-66` above rather than treating this as its replacement: that is the **alarm** shape, a
`panic!` nothing arrives at, and it is the harder half to see. This is the **feature** shape, and it
is cleaner in exactly one way — `BL-66` had an out-of-tree consumer to argue about, and these had
none, so the green tick protected precisely nothing. Note what the unit is: the defect was the
*tests*, not the tools.

### Derive the count, don't cite it — one population, four defensible numbers

One population yielded **three** defensible numbers inside an hour, and each was the right answer to
a different question.

| number | question it answers |
|---|---|
| **18** | tests living in `mod tests` (19 functions less the `project_ctx_with_file` fixture) |
| **13** | tests exercising the tools by name — 12 there, plus `tests/integration.rs::workflow_analyze_ast` |
| **17** | tests guarding unreachable code — four formatter tests covered `format_list_functions` / `format_list_docs`, whose only production caller was the tools' own `format_compact` |

A first pass published **15**: 13 with two wrong inclusions — a JSON fixture containing the *string*
`"ListFunctions"`, and an e2e helper (`run_list_functions`) mistaken for a test. None of these was a
mistake about the code; they were four different units.

**A count of a defect population must arrive with its unit or not at all** — and note that 13, 15,
17 and 18 are near enough to each other that no reader would query any of them, which is §
*Observer Blindness*'s closing rule firing on the example added to illustrate it.

The deletion removed 18 tests across two lanes, 3424 → 3406 lean and 4991 → 4973 default: 17 from
the module plus `workflow_analyze_ast`, with the one test that covered a *reachable* feature —
`symbols(include_docs)` — moved to `src/tools/symbol/tests.rs` rather than lost. The matching delta
on both lanes is what proves nothing else went with them.

#### An INSTANT needs its frame too — and the cost is exactly the one this section warns about

**2026-09-13.** Two sessions compared the last write to the same untracked files. One read
`06:34:50Z` from the catalog's audit trail; the other read `09:34:50` from `stat`, unstamped.
The first concluded *"the two clocks disagree by three hours and only one of them is about
the author"*, and pointed the second at a real open bug — mtime measuring other sessions'
git operations rather than the author's edits — as the likely cause.

**There was no disagreement.** The box is UTC+3. `09:34:50+03:00` is `06:34:50Z`, the same
event to the second. The offset WAS the difference, and it was read as evidence.

Two things this adds to the law above.

**The unit rule covers timestamps, and the unit is the FRAME.** `Z` on one reading and
nothing on the other is the same defect as a count published without saying what it counts —
and it is easier to miss, because a bare `HH:MM:SS` looks complete in a way a bare integer
does not. The instrument that stamps its frame cannot protect a comparison against the one
that does not.

**The cost is the one § *Reaching a Peer Session* already names, realised from the other
side.** That section says omitting the stamp makes *"ordinary churn present as a tooling
defect, and the natural next step is to go debug an instrument that is working."* Here the
reader went one step further and nominated a specific, real, filed bug as the explanation —
which is worse than a vague suspicion, because a named cause closes the inquiry. The mtime
bug is genuine; it simply was not what happened, and citing it lent a fabricated discrepancy
the credibility of the corpus.

**The tell, and it inverts the line-drift tell above.** There, drift between two otherwise
agreeing readings meant the CORPUS MOVED rather than a reader erring. Here, a clean constant
offset between two otherwise agreeing readings meant the FRAME DIFFERED rather than the event
being observed twice. Both are the same instruction: when two careful readings differ by
something *regular*, suspect the coordinate system before the observation. A three-hour gap
that is exactly a timezone is not a measurement.

Caught by the session holding the unstamped clock (`9403d62d`), who converted both to one
frame and pinned the offset from their own `date` output rather than asserting it.

## Annotate the fixture — both directions

**Law:** annotate a fixture's load-bearing detail, on the fixture line.

The assertion states what must be true; nothing states which part of the *setup* is what makes the
test able to tell. Say what breaks if the detail goes — not in the test name, not in the assertion
message, and never as a bare "do not edit". A tidy-up that removes it leaves the test passing and no
longer discriminating, which no assertion can catch because that change is monotone too.

**The inverse is also owed, and is the worse of the two.** Annotate a fixture as **inert** where it
provides no coverage, so nobody credits it with coverage it does not have. One guards against silent
**removal**, the other against silent **credit** — and false coverage actively stops the next person
looking, where silent removal does not. Worked example: the `output_id` probe in
`every_declaring_topic_has_a_live_route_to_a_declared_section` (`src/server.rs`), annotated at the
fixture line as changing no outcome today and explicitly not to be cited as overflow coverage.
Promoted from `reconnaissance-patterns:R-161`.

## Scope, not direction — the p50 byte-ceiling run

**Law:** an assertion computed over a population cannot verify a claim about a member.

This is the axis the monotone law does not cover. That one asks which *direction* an assertion is
blind to; this one asks what *scope* it is computed over. A per-member claim checked against an
aggregate is vacuous for every member, and it reads as coverage rather than as a gap.

`a_p50_session_stays_under_the_committed_emission_byte_ceiling` (`src/server.rs`) asserts
`total <= CEILING` and `total > 0` over six tool shapes. Guide bytes alone, measured
2026-09-02 per shape:

| shape | bytes |
|---|---|
| `create` | 2785 |
| `get` | 0 — legitimate; `create` already delivered its section and the ledger dedups per session |
| `update` | 3193 |
| `append_entry` | 1643 |
| `find` | 2233 |
| `move` | 2018 |

Guide bytes total **11,872**. The test itself was renamed and widened 2026-09-02 (Plan 3
Task 2) from `a_p50_session_stays_under_the_committed_guide_byte_ceiling`, which is the name
this section originally measured: `shape_total` now sums every block after the primary, not
only guide-marked ones, so the asserted total also folds in a fixed 244 B of `post_process`'s
onboarding hints present in this fixture — `total = 12,116` against `CEILING = 12_244` (see
`docs/PROBES.md` and the spec's § *Measurements this spec rests on* for the current
mechanism and why 12,116 B is itself a floor of a real session's total). `total > 0` is a sum
of six non-negative addends, so it fails only if **all six** are zero — insensitive to five of
them by construction. That much is a proof rather than a measurement.

**Both aggregates are blind in the same direction, which is worse than one blind spot.** A section
ceasing to be delivered *reduces* the total, so it makes `total <= CEILING` **more** comfortable.
The pair's failure modes agree, so content vanishing moves both the safe way.

**The test already computed the discriminating value and dropped it.** `shape_total` returned
`bytes`; all six call sites discarded the return, silently, because `usize` is not `#[must_use]`.
The author had even written the predicate in prose — *"`get` is the ONE shape expected to report 0 B
here … Any OTHER shape reporting 0 B is suspicious, not normal."* A guard that would have worked was
sitting in the function, unreferenced. (The adjacent mode *was* closed deliberately: every call goes
through `call_tool_checked` rather than `call_tool`, because a silently-failed call reports 0 B and
is character-identical to legitimate dedup.)

### Two remedies falsified, and that is the reusable part

Mutation used throughout: rename `serves: artifact.append_entry` in `src/prompts/guides/librarian.md`
so that shape's section no longer matches — a realistic serves-drift, not a synthetic break.

| remedy | result |
|---|---|
| `total > 0` (the existing assertion) | **GREEN** — absorbed |
| `bytes > 0` per shape, exempting `get` | **GREEN** |
| assert no block carries the declared failure marker | **RED**, naming serves-drift |

The second is the one worth knowing, because it is what the law's own diagnosis implies and two
readers agreed on it independently. It fails for a reason neither could reason to: a shape whose
section is gone does **not** report 0 B. It receives a 491-byte fallback whose own marker reads
`no section declares this call's shape`. Every shape therefore has a floor above zero, and **no
non-emptiness assertion at any grain can discriminate** — not the aggregate, not the per-member one.
Under mutation `append_entry` reads 491, not 0.

Hence two rules that generalise past this test:

- **A per-member assertion is only as good as the member's ability to reach the failing value.** A
  deliberate, self-describing fallback floor makes that value unreachable, so the assertion is
  vacuous for a second and independent reason.
- **Demand an observed RED, never an assertion's existence.** That acceptance bar is immune to the
  trap by construction, because it separates reachable from unreachable without anyone needing to
  know the system's floors in advance. It is what saved the sibling instance below: the same
  underspecified *"a per-tool expectation replacing the global sum"* was read as an exact-count
  table (which discriminates, 3 ≠ 0) rather than as `found > 0` (which would not have). The sound
  reading was chosen, not specified.

### What worked — cause-naming over magnitude-guessing

The injector announces the condition in the block it emits — `src/tools/core/guide_emit.rs:182`
writes `preamble — no section declares this call's shape` — so the discriminator was in the output
all along while both readers reached for a number. **Where a system already names its own failure
state, assert on the name rather than on a proxy for it.** It also needs no per-shape labels, no
call-site edits and no ordering assumption — all three of which the rejected shapes required.

**Annotating this fix's own weakness, per the fixture law above: it matches a STRING.** A reword of
that marker leaves the assertion passing and no longer discriminating, and no assertion can catch
that because the change is monotone. The robust form exists one layer down —
`GuideDeliveryShape::Preamble` (`src/tools/core/guide_emit.rs:41`) is the typed signal
`guide_blocks_for` already returns — but it is not reachable from this test's vantage, which sees
only the emitted `Content` blocks through `call_tool_checked`. So the string is the best available
discriminator here and the weakest link in the fix; a future change that surfaces the shape enum to
the test should replace it.

Derivation, reproducible by anyone who wants to disbelieve it, run in an isolated worktree with its
own `target/` so no shared build lock was involved:

```text
A. unmutated + marker guard                  -> GREEN
B. `serves: artifact.append_entry` renamed   -> RED, naming serves-drift as the cause
C. reverted                                  -> GREEN
```

Held on branch `p50-absorption-demo` (`13ee893b`, patch-id
`95fd1e5230f052448d99346801c659993d1941b9`) rather than landed: `src/server.rs` carried another
session's uncommitted instrumentation inside that same function at the time — an aggregate-only
`eprintln!` of `total`, which is the blindness this fix removes.

**Provenance, because the parts came from different sessions.** The class was named by a peer session
from a symptom in its own gate (`total_aliases_found`, a global accumulator summed across 26 tools,
fixed separately with both demonstration steps observed RED). The instance here, the two falsified
remedies and the marker fix were derived in this run. A candidate offered first —
`tool_surface_under_budget` — was **not** an instance: it asserts a population claim against a
population accumulator, which is honest, and per-tool is separately covered by
`every_tool_description_under_cap`. That correction is kept because a class shown once is an
incident, and a mis-assigned member is a different error from a short count.

### A symbol name is a claim scoped to a TREE — and three doubts that confirmed

**The correction above was itself published without its scope.** Both greps were right.
`every_tool_description_is_under_its_cap` exists on branch `tool-collapse`
(`src/server.rs:2445`, where a refactor replaced the `experiments` test) and **nowhere on
`experiments`**; `every_tool_description_under_cap` exists on `experiments` (`:3354`) and **nowhere
on that branch**. So "zero definitions" and "one definition" were both correct answers to different
questions, and `0d2ab2b1`'s commit message — which reads *"has ZERO definitions … one was wrong"* —
is true of this tree and uncharitable about the peer who cited it from theirs. **When citing a
symbol across sessions on a shared repo, name the branch.** On a checkout with live worktrees that
is not pedantry: it is the same shape as everything else on this page, a claim true inside a
boundary and published without it.

**And a LINE NUMBER is scoped harder than a symbol name.** A symbol survives until someone renames
it; a line number dies to any insertion above it — including one in a different function, by a
different session, in the same hour. So `path:line` across sessions needs the branch *and* is
short-lived even with it. Prefer citing the symbol and letting `symbols` / `get_guide` find the
line, which is what those tools are for.

Measured twice in one conversation, on one file. `fn input_schema` sits at
`src/tools/core/types.rs:714` on `experiments` and `:726` on `tool-collapse` — a delta of **exactly
12**, verified in both trees, being the doc comment plus body one task added *above* it. The drift
was produced by the very commit whose value was being described at the time, so neither citation was
careless and both were right. That is the tell for this whole family: when two parties disagree
about a fact and both verified it, the disagreement is about scope, not about the fact.

**Three doubts raised in that run all confirmed, and they are recorded because confirmations are
the denominator this page's recording-filter law asks for.** None is a catch; absorbing them as
catches is what makes a population look self-correcting.

| doubt raised | outcome |
|---|---|
| "the per-tool cap is 1800, but a commit message argues from 300 — is one wrong?" | **Both real, different populations.** `tool_descriptions_stay_under_budget` (`src/server.rs:2449`) asserts `d.len() <= 300` and skips the librarian family via `is_librarian_tool`; `every_tool_description_under_cap` (`:3354`) asserts `CAP = 1800` over everything. A non-librarian tool is bound by 300, so the 304-char breach that commit describes was exact. |
| "`over.is_empty()` in the per-tool cap test is an absence assertion, monotone under an empty population" | **Closed by a pair.** `server_registers_all_tools` (`:2228`) and `server_tool_count_is_l3_target` (`:2278`) pin the count independently, so a broken or empty `server.tools` reds there first. |
| "is the aggregate/member split covered elsewhere, or is this systemic?" | **Generally covered by a pair, where anyone has looked.** Both cases above are two-level. The p50 test is the one where the pair was missing — which is what makes it an instance rather than the norm. |

Note the shape of the second and third: *the aggregate/member split is usually guarded by two tests
at different levels.* That is the remedy pattern, and its absence — not the presence of an
aggregate — is what identifies an instance. An aggregate assertion is not a defect; an aggregate
assertion **standing alone** where a per-member claim is being made is.

**That inversion needed one more word, supplied by a second instance below: the absence of a second
level that REACHES THE PRODUCTION PATH.** A second level can be present, named for exactly the job,
and dead.

### A third cause of two correct readings — one symbol, two ROLES, no drift at all

The section above gives two ways two verified parties cite different lines for one fact: the symbol
moved (rename), or the tree differs (insertion above, another branch). Both are **scope**
disagreements, and both have a remedy — name the branch, or `git grep <pattern> HEAD`. Measured
2026-09-10: there is a third that presents identically and **defeats both remedies**, because
nothing moved and no scope differs.

Two sessions on `experiments`, `src/server.rs` clean in both (`git diff --stat` empty), disagreed by
99 lines about one table-driven gate:

| line | role |
|---|---|
| `:2847` | `async fn required_names_no_key_that_has_a_declared_alias` |
| `:2891` | `for (tool, expected) in EXPECTED_ALIAS_COUNTS_BY_TOOL` |
| `:2893` | the `assert_eq!` — **the panic site** |
| `:2992` | `const EXPECTED_ALIAS_COUNTS_BY_TOOL` — **the data site** |

Rust module items are order-independent, so the `const` legally sits 99 lines *below* the test that
reads it. One party held the panic line out of a failure log; the other had resolved the symbol and
cited its definition. Both were reading the same bytes of the same tree at the same instant, and
both were right about different things.

**Why this is worse than either listed cause, rather than another example of them.** Re-reading
returns the same two numbers; `git grep <pattern> HEAD` confirms the tree never moved; naming the
branch changes nothing. Every instrument this page offers for the first two causes returns
*agreement that the disagreement is real*, so the reflex "one of us must be wrong" is
unfalsifiable — and the three explanations actually offered in that exchange (stale tree, two
assertion sites, a mis-transcription) were all wrong, in a session that had this page's laws in
context.

**And it falsifies this page's own remedy for one case.** *"Prefer citing the symbol and letting
`symbols` find the line"* is what produced the mismatch: symbol resolution lands on the
**definition**, while a failing test hands the reader the **assertion**. For a table-driven gate
those are different lines by construction, and the more disciplined party is the one holding the
number that will never appear in any failure output.

So the convention, which costs nothing and is the whole finding: **a panic line is always the
assertion, never the data it reads.** When citing a gate whose data lives in a `const` table, cite
the `assert_eq!` — that is the number the next reader will be holding. Cite the table's definition
only as a second, labelled coordinate.

Raised as a discrepancy by sessionId `26cb9b5b-2c9c-489e-97d9-3a907c8b2941` against a citation of
mine, who then withdrew all three of their own explanations once the fourth was verified at the
bytes; the role-pair cause and the convention are this session's.

### A second subsystem — and a DIFFERENT way to be vacuous

**Scope first, per the rule immediately above.** Every symbol in this subsection lives on branch
`tool-collapse` **only** — `required_names_no_key_that_has_a_declared_alias`,
`alias_offender_detection_catches_a_synthetic_offender`, `parse_declared_aliases`,
`EXPECTED_ALIAS_COUNTS_BY_TOOL` and `every_property_has_a_description` each return **0 occurrences
on `experiments`**, verified. Line numbers are that branch's at `98e7c788`. Only
`tool_surface_under_budget` exists in both trees.

**The floor mechanism was ruled out cleanly, and that turned out to prove nothing.** The gate has no
491-byte analogue: `input_schema()` is a bare trait declaration with no default body (true on
`experiments` too — `src/tools/core/types.rs:714`), there is no server-side schema augmentation,
`parse_declared_aliases` `filter_map`s with `?` and synthesises nothing, and
`EXPECTED_ALIAS_COUNTS_BY_TOOL` is a genuine equality rather than a `> 0`. The two-part
demonstration is real, and step 2's `left: 0  right: 3` is itself the proof that the failing value
is *reachable* — the exact property the p50 case lacked.

**Then mutating the production predicate left the suite green with the detector dead.**
`if *target == req` → `if *target == req && false`, which makes the entire offender scan
unreachable: **24 passed, 0 failed.** Two independent causes, neither of which the floor analysis
would have found:

1. **The guard asserts about a copy of the thing it guards.** The synthetic fixture *re-types* the
   matching loop rather than calling the production one; only `parse_declared_aliases` is shared. So
   its own failure message — *"the offender-matching logic itself is broken"* — is **false of the
   production loop**, and the doc comment claiming the shared-function design prevents exactly this
   drift is true of half the logic and the wrong half.
2. **`offenders.is_empty()` over a population empty by design**, because the task that installed the
   guard fixed every offender. Today's correct answer and a dead detector's are byte-identical —
   the monotone-under-removal law, arriving underneath the scope law.

**Two subsystems, two DIFFERENT falsification mechanisms, which is worth more than two of the same.**
The p50 case: *the member cannot reach the failing value.* This one: *the guard asserts about a
re-implementation of what it guards, over a population the fix emptied.* A reader who has closed one
has not closed the other.

Hence the sharpened acceptance bar: **mutate the PRODUCTION path, not the test's inputs.** A second
level asserting about its own copy is indistinguishable from coverage at every point except a
mutation of the shipping predicate.

### A third instance of the p50 shape, in a third subsystem

`tool_surface_under_budget` is satisfied **better** by gutting a description — the same
wrong-direction blindness as `total <= CEILING`. On `tool-collapse` its only counterweight is
`every_property_has_a_description`, asserting `!d.trim().is_empty()`: the falsified shape again, a
non-emptiness claim that cannot distinguish an accurate description from a one-character one. At
budget headroom 0 the mechanical pressure is downward on characters, so the incentive points at the
blind spot. **Nothing is wrong today** — five were verified — and it is recorded as structural, not
as a defect. What makes it more than hypothetical is that the same task produced *three successive
wrong versions of one description*.

### The population can GROW after the assertion is written

Every instance above is a population **fixed when the assertion was authored** — six contributors,
six tool shapes, five property descriptions. The author could in principle have enumerated it and
seen the gap. This one cannot be caught that way, because the set the assertion quantifies over
did not exist yet.

Measured 2026-09-16 by sessionId `f0b1a4c7-e991-4478-bf22-b088483b6821`, in
`src/librarian/tools/doctor.rs`. `a_citation_still_dirty_in_the_working_tree_is_silent` asserted
`v.is_empty()` — a claim over **every check the scan emits**. When written it was adequate: one
check existed, and "the report is empty" and "my check is silent" picked out the same set. Adding
a second check silently widened the assertion's subject, and the two claims came apart. The
fixture was also the positive fixture for the new check, so the suppression could have gone green
by deleting the surfacing, and no assertion in the file would have noticed.

**Nothing was edited to cause this.** Not the assertion, not the code it guards, not the fixture.
The assertion was per-member-adequate on the day it was written and became an aggregate later, by
a change somewhere else that had no reason to look at it. So the usual tells are all absent: no
diff to review, no author to have been careless, and re-reading the test at any point returns a
sentence that is true.

**It sits at the intersection of two laws, and neither is the one it looks like.** The scope law
above — the subject is wider than the name — and the monotone law at the top of this page, since
`is_empty()` is an absence assertion and therefore monotone under removal, which is exactly why
deleting the surfacing would have passed. It is **not** an instance of the guard-ordering twin in
§ *One mutation per guarded SITE*: there is no ordering here and no second guard racing to refuse
an input, only one predicate quantifying over a set that grew.

**The remedy is the one this page already warns about, plus a second half.** Narrowing to
`v.iter().all(|x| x.check != "<mine>")` is correct and makes the fixture **inert** for the new
check — the per-member fix that is not automatically the remedy. Asserting that the complement
**fires** is what keeps it discriminating in both directions. The author's own summary is the
line worth keeping: the fix *strengthened* the test rather than narrowing it, and that only
became visible because the suppression and the surfacing were one decision made at two sites.

**So the question to ask of an absence assertion is not only what it is computed over, but
whether that set is CLOSED.** `is_empty()` over a growing collection is a claim whose meaning
changes without its text changing. Where the collection is open-ended, name the member.


## `SURVIVED` has a THIRD reading — the two-site `ack=all` run

Measured 2026-09-16 by sessionId `f5f48b42-6d84-482e-84a4-8eaebb0ce60f` while fixing
`8988bc7719b83b3b`
(`docs/issues/archive/2026-09-15-push-ack-all-publishes-every-foreign-sessions-work-and-tells-none-of-them.md`,
fix `fbddd86d`); the discriminator below by sessionId
`f0b1a4c7-e991-4478-bf22-b088483b6821`. Two defects in one bug, one mutation per site as
§ *One mutation per guarded SITE* requires:

| mutation | site 1 (`acked()` wildcard) | site 2 (`!= "all"` gate) | result |
|---|---|---|---|
| wildcard short-circuit restored | **broken** | fixed | **KILLED** — 3 rows |
| `!= "all"` gate restored | fixed | **broken** | **SURVIVED** — 0 rows |

**Neither published reading fits row 2.** The clause is not untested — the suite runs it.
It is not unreachable-by-any-test-you-could-write — no seam is missing. It is
**semantically inert**: once site 1's fix puts real sids in `ack_matched`, the condition
`ack_matched != "all"` cannot be false on any input the fixed code produces. Site 1's
repair removed the clause's entire domain.

So **the repair is neither of the published two.** Not *write the test* — no test can
distinguish a condition that is always true. Not *add a seam* — the code is reachable. The
correct action is to recognise the clause is dead and delete it, which is what `fbddd86d`
did. A `SURVIVED` read as "untested" here sends you to write a test that cannot exist, the
same terminal state the two-reading form already warns about, reached by a third route it
does not name.

### The discriminator — what makes this a category rather than a judgement about intent

Inertness is **checkable**: revert the sibling fix and re-run the SAME mutation. If it now
KILLs, the clause was inert rather than untested. Row 1 above *is* that check, run by
accident.

That is also why one-mutation-per-**SITE** is what surfaces this and a single run cannot:
**inertness is a property of the PAIR of sites, not of the line**, so an instrument asking
about one line at a time can only reach it by comparing two answers. The existing law was
written to stop a kill at one site being credited to the other N−1; this is the same law
paying out in the opposite direction, where a *survival* at one site is explained by the
fix at another.

### What the bug file got wrong, and the generalisation

The record asserted **"two independent failures, and either alone is sufficient"**. That is
correct about the **defects** and false about the **repairs**. Fixing `acked()` alone closes
the bug — real sids make the old gate's condition true, so the block runs. Fixing the gate
alone does not: it leaves the wildcard short-circuit in place and prints the literal token
`all` as a session to go and notify, a plausible line naming a party that does not exist.

**Independence of causes does not transfer to independence of repairs.** The author derived
the independence by reading, and reading cannot separate the two — § *Bug Tracking*'s *"run
the reproduction before reading the fix plan"* reaching the **fix** rather than the
diagnosis.

### A second result from the same run — prose is monotone under naming the wrong parties

Under the site-1 mutation, the assertion `says they have not been told` **passed**. The
sentence survived; only the list of sids was wrong. An assertion about a notification's
*wording* cannot detect that notification naming the wrong recipients — § *Monotone
assertions* applied to the half of a guard nobody writes assertions about (CLAUDE.md
§ *Testing Discipline*, the remedy-text ceiling, which had been measured on deletion and is
here measured on **substitution**). **Assert the sids, never the prose.**

The same run supplies the converse, and it is why the ordering matters: a `hasnt` row
asserting the literal token `all` never appears as a sid was **vacuous** before the fix —
nothing printed at all — and became load-bearing only once the `has` rows could red on
silence. A negative assertion is worth what the positive assertions beside it are worth.

## A red does not survive its assertion being edited

`CLAUDE.md` § *Testing Discipline* states the law; this is the run behind it.

**Setting.** `scripts/mutation-probe.sh`'s `ran_lines == 0` refusal named three causes, none of
which fits a non-cargo runner
(`docs/issues/archive/2026-09-16-mutation-probe-renders-no-verdict-for-a-non-cargo-runner.md`).
The fix adds a fourth cause plus a remedy sentence, and case 21 of `tests/mutation-probe.sh`
guards it with two needles: that the cause is named, and that the remedy says where the verdict
IS readable.

**The red that was observed.** Test written and run first: 2 of its 4 assertions red, on needles
`not CARGO` and `summary line`; the other two green — itself the control, since it shows the
fixture reaches the branch under test rather than failing for an unrelated reason.

**The edit that discarded it.** `has` is `grep -qF`, so `not CARGO` reds on any rewording that
keeps the meaning — while the comment beside it promised it survived one. The repair looked
obvious, and is what this document prescribes elsewhere: assert the ENTITY, case-insensitively
— *"does this message still say cargo at all"* — the form that reds on deletion and survives
rewording (§ *Loudness is a property of a PATH*, the two-addressee shape test).

**The measurement.** Arming the mutation the case exists to kill — delete the non-cargo clause,
`--replace` a cargo-free string, `-- bash tests/mutation-probe.sh`, against a known-clean
`50 passed, 0 failed` baseline:

| assertion form | mutation | result |
|---|---|---|
| entity, case-insensitive `cargo` | clause deleted | `50 passed, 0 failed` — **SURVIVED** |
| phrase, `not CARGO` | clause deleted | `49 passed, 1 failed` — KILLED |
| phrase, `summary line` | remedy reworded | `49 passed, 1 failed` — KILLED |

The entity needle was vacuous because the fix's own remedy paragraph, two lines below the
clause, says *"On the NON-CARGO cause"* — so deleting the clause left `cargo` in the output.
That is § *Scope, not direction* (an assertion computed over a POPULATION cannot verify a claim
about a MEMBER) arriving through a door nobody watches.

**What makes it a law of its own rather than an instance of that one.** The author held an
observed red and believed it covered the assertion. It did not — that red was produced by the
text then deleted. Re-reading the new assertion returns a true sentence, the suite is green, and
the red-then-green cycle has been performed in full. **Every signal TDD generates was present,
and none of them was about the assertion that shipped.** The scope law tells you an assertion
can be vacuous; this one tells you why your evidence that it is not has quietly expired.

**Why the trigger is counter-intuitive.** The dangerous edit is not a careless one — it is the
improvement. The rewrite above was made *toward* this document's own prescription, by an author
who had just read it, inside a commit fixing a bug about guards narrower than their names.
Confidence was highest exactly where the evidence had lapsed.

**The remedy is one line, and it is not "be careful":** treat any edit to an assertion as
invalidating its red, and re-run the mutation. On this checkout that is
`./scripts/mutation-probe.sh` at ~11 s per run (`docs/PROBES.md` carries the cost and the
caveats). What it cannot be answered by is re-reading at any level of attention — both forms
read as correct, which is the whole difficulty.

**Attribution:** measured by sessionId `a3bf229c-658b-42f9-8f4b-794fcf0d35c7`, 2026-09-16, while
fixing the bug above. The vacuity was found by the mutation and by nothing else — including the
author's own re-reading of the line they had written minutes earlier.

## Related

- The laws themselves: [`CLAUDE.md`](../../CLAUDE.md) § *Testing Discipline*.
- Superseded formulations and the route that got here: `reconnaissance-patterns` R-132, R-133,
  R-161.
- § *SDD Rulings*' "vacuous assertions cluster" and "demand a deliberate break" are instances of
  these laws, found before the general form was — `docs/trackers/sdd-ruling-log.md`.
- The epistemics of why care is the wrong instrument for these classes:
  `docs/trackers/observer-blindness.md`.
