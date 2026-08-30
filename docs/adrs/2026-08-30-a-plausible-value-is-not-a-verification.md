---
id: b034816e36339b28
kind: adr
status: active
title: ADR-2026-08-30 — A plausible value is not a verification
owners:
- marius
tags:
- verification
- tool-contracts
- instruments
- multi-session
- false-confidence
topic: tool-contracts
time_scope: invariant
---

# ADR-2026-08-30 — A plausible value is not a verification

## Status

Accepted — active. Distilled 2026-08-30 from **nine instances observed in a single
day across four concurrent sessions**, none of which were looking for the pattern and
three of which found it while investigating each other's mistakes.

The sibling of `docs/adrs/2026-08-27-negative-results-name-their-scope.md`, and a
generalisation of it. That ADR governs a **zero**. This one governs the case its
Decision does not reach: a **confident, plausible, non-empty value**. The distinction
is the whole contribution — a zero invites a second look, and a plausible value
suppresses one.

## Context

`CLAUDE.md` says *"Do not hypothesise but ALWAYS VERIFY."* That instruction is correct
and it is not sufficient, because it says nothing about **what you verify with**. Every
instance below is someone following it.

Nine instruments, one shape. Each returned a plausible value where an error would have
been safe, and each was consulted **precisely because** its user was being careful.

| Instrument | The question it was asked | The question it actually answered |
|---|---|---|
| `git diff --cached --stat` | *whose* lines am I about to commit? | how many lines changed |
| file mtimes | which session last wrote these files? | when the prober's own `touch` ran |
| cached clippy `Finished in 0.48s` | does my change pass the lint gate? | did some earlier build pass it |
| green `cargo test` | is the tree gate-clean? | do the tests pass — a different gate |
| a mid-run test log tail | how many tests passed? | how many had passed *so far* |
| `cargo test --lib <bad filter>` | does this code have coverage? | did this filter match any test name |
| a 200-row page cap | how many results are there? | how many fit on a page |
| a passing unit test | does the policy take effect? | is the policy function correct |
| `ListAgents` *"Peer sessions (2)"* | who else is writing this checkout? | who shares my config profile |
| wall-clock around `cargo test` | how long does this suite take? | suite time **plus** any rebuild the run triggered |
| a stale semantic index | where is this concept in the code? | where it was, N commits ago |
| `indexed_with_model` | what embedded these vectors? | what some past writer *labelled* them |

**The costs were real and are all from one day:**

- `git diff --cached --stat` printed `bug-fix-session-log.md | 61 ++++++++++++++-`. Two
  of those 61 lines belonged to a peer; they went into `7930e0b7` under the wrong
  author. The check *ran and passed*.
- `ListAgents` reported `Peer sessions (2)` with four sessions live in one checkout.
  Six misattributions of authorship followed, across three sessions, **every one a
  correct elimination over an incorrect population**. One session held a commit rather
  than commit lines it believed were a peer's; another nearly filed a false confession
  of its own authorship.
- A peer ran `touch` on three files to bust a cached clippy fingerprint, then reached
  for mtimes as corroboration. Their own diagnostic had destroyed their own evidence,
  and all three files were stamped the same second.
- A green `cargo test` (4819 passed, 0 failed) sat on a tree where
  `clippy --workspace --all-targets --features local-embed -D warnings` exits 101. An
  unused import is a warning; only `-D warnings` promotes it. The green is real and it
  is not the gate.
- A mid-run test log showed `passed: 4686` with no `test exit=` line. The complete run
  was 4819. A partial total is indistinguishable from a total.
- A pure unit test for a policy resolver passed while its caller ignored the policy
  entirely — proven by mutation. Testing a function in isolation establishes the
  function is correct and says **nothing** about whether anything calls it. The same
  gap was measured independently the same day on an installed hook whose four-link
  chain had three links covered.
- **Wall clock reported a coupling that did not exist, and the number was published.**
  A probe timed `tools::memory::tests` across four ambient configurations to measure
  environment coupling. Changing any `CODESCOUT_*` variable invalidates cargo's
  fingerprint, so *switching conditions* triggered a rebuild and inflated three of the
  four arms by 15–27s. The verdict `*** STILL COUPLED ***` was reported to the operator
  before the artefact was caught. libtest's own `finished in` line — which excludes
  compilation — read `0.04s` in **all four** arms simultaneously. The instrument was
  measuring the cost of asking the question.
- **A stale index answers competently.** After a rebuild, `semantic_search` for a
  symbol committed hours earlier returned three plausible, well-ranked, entirely
  unrelated chunks. Nothing failed; `git_sync.behind_commits: 10` was the only tell,
  and it lives in `index(action="status")`, not in the search response. A search over
  a stale index is not a degraded search — it is a correct answer to *"where was this
  concept N commits ago"*, a question nobody asked.
- **A metadata label impersonated a measurement.** The same status call reported
  `model_mismatch: indexed_with all-minilm / configured CodeRankEmbed`, which reads as
  two embedding spaces and argues for a full 52k-chunk force re-embed. `GET /v1/models`
  returned `CodeRankEmbed-Q4_K_M, n_embd: 768` and a live embed returned dim 768: the
  vectors were fine and the *label* was stale, written by a process whose executable had
  since been deleted. This one is the counter-example that proves the remedy works — the
  field's own hint says to go ask the endpoint, and doing so cost one `curl` and saved a
  pointless rebuild.

### The property that makes this class distinct

**The failure is proportional to diligence.** Someone who skips the check stays
appropriately uncertain. Someone who runs `--stat` acquires *false confidence* — which
is strictly worse, because it terminates the search. That inverts the usual assumption
that more checking is safer, and it is why "always verify" cannot be the whole rule.

**And two instruments can agree while both being wrong.** `ListAgents` is the
demonstration: two sessions in different profiles both report `Peer sessions (2)` over
**disjoint** sets. Agents comparing notes find their numbers match and read the
agreement as corroboration. An incomplete count *known* to be incomplete is merely weak
evidence; two matching counts over disjoint populations manufacture confidence out of
the very discrepancy that should have exposed the gap.

## Decision

**Before an irreversible or outward-facing step, verify with an instrument that can
distinguish "checked and clean" from "did not check that."**

Three clauses, all load-bearing:

1. **The instrument must answer the question you actually have, not a neighbouring
   one.** State the question first, then check the instrument's output could differ
   between the two answers you care about. `--stat` cannot vary with authorship, so no
   `--stat` output is evidence about authorship — not a weak one, *none*.

2. **Prefer the instrument that reads the artifact you are about to act on.** Proxies
   and earlier observations decay between the reading and the act. `git diff --cached`
   reads the index that is about to become the commit and cannot be raced;
   `git diff -- <path>` reads a working tree a peer can change in the gap.
   `git show HEAD:<path>` compared against the worktree survives where mtimes do not,
   because it does not depend on anything you did to the files.

3. **A result indistinguishable from "the work did not run" needs a completion marker
   before it counts.** A cached green is byte-identical to a validating green; a
   mid-flight total is byte-identical to a final one; a filter that matched no test is
   byte-identical to a suite with no coverage. Require the marker — `test exit=`, a
   forced recheck, a non-zero match count — and treat its absence as *unknown*, never
   as *clean*.

### How to satisfy it — the remedies that survived

| Broken instrument | The one that holds | Why it holds |
|---|---|---|
| `git diff --cached --stat` | `git diff --cached` (content) | authorship is *in* the content and in nothing else |
| `git diff -- <path>` alone | it, **plus** the post-stage content diff | the index cannot be raced; the tree can |
| file mtimes | `git show HEAD:<path>` vs worktree | independent of anything the prober did |
| cached clippy | the long form, after forcing a real recheck | CLAUDE.md already calls it *"the gate, not garnish"* |
| green `cargo test` | it is not a substitute for clippy or the lean check | three gates, three questions |
| a log tail | wait for the process's own completion marker | partial and final are otherwise identical |
| a passing unit test | a mutation of the **caller** | isolation proves correctness, not reach |
| `ListAgents` | `ls /run/user/$(id -u)/cc-socks/` + `/proc/<pid>/cwd` | enumerates the machine, not the profile |

Note that every remedy is *cheaper* than the investigation it replaces. None of these
are expensive; they are simply not the first thing to hand.

### The corollary that decides ties

**Prefer the instrument that can return an error over the one that returns a value.**
Where both are available at similar cost, the one that can fail loudly is worth more
than the one that is usually right, because its silence is informative and the other's
is not. This is `negative-results-name-their-scope` clause 3 read from the producer
side: *claim only what is proven.* An instrument that cannot express "I did not measure
that" will assert something instead.

## Consequences

### Now easier

- The class has a name, so a session can say *"that's a `--stat`-shaped check"* and be
  understood, instead of re-deriving the lesson from its own incident.
- The remedy table is directly reusable; it is eight worked instrument→remedy pairs,
  not a principle needing translation.
- Review has a question to ask: *could this instrument's output differ between the two
  answers we care about?*

### Now harder / lost

- Slightly slower verification, and a judgement call per check. That cost is the point:
  clause 1 makes the question explicit where it was previously assumed.
- Clause 3 will occasionally flag a result that was fine. `unknown` is the correct
  reading of an unmarked result even when it later proves clean.

### Change scenarios absorbed

- A new tool grows a paginated or capped output → clause 3 covers it without a bespoke
  rule, because a cap and a completion are the same question.
- A new gate is added to the pre-commit sequence → clause 1 covers "does a green from
  gate A speak for gate B" without enumerating gates.

### Deliberately out of scope

**Reasoned claims are not instruments.** On the same day, a peer argued that splitting a
commit would fail `dead_code` on an uncalled `pub` item; two commands later it exited 0,
because `pub` items in a library crate are not dead-code-flagged. That is a plausible
inference asserted where a cheap measurement was available — a real error, and a
different one. This ADR is about instruments that *answer*; that failure is about not
consulting one. Conflating them would dilute both.

Nor does this ADR govern instrument *design* inside codescout — that is
`negative-results-name-their-scope`'s territory, and this one is about the caller's
choice among instruments that already exist, most of them not ours.

### Revisit-when

- An instance appears where clause 3 produced meaningful drag — a marker that was
  expensive or impossible to obtain. Today every marker was free.
- `ListAgents` gains machine-wide enumeration or a scope note
  (`docs/issues/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md`,
  `open-issue-work-queue:BL-58`). That would retire the worst member but not the class.

**Confidence: high.** Nine instances in one day, found independently by four sessions,
three of them by sessions investigating a *different* session's error. The pattern was
not sought; it was noticed because the same shape kept producing the same kind of
confident mistake. What is *not* established is the base rate — these nine come from one
unusually concurrent day, and a quieter session may meet the class far less often.

## Alternatives considered

1. **Fold it into the existing negative-results ADR.** Rejected. That ADR's Decision is
   scoped to negative results and its clause 2 (*stay silent on a trustworthy negative*)
   has no analogue here — there is no "trustworthy plausible value" to stay silent
   about. Widening it would blur a clause that is doing real work.

2. **Add it to `CLAUDE.md` beside "ALWAYS VERIFY".** Considered seriously, and still
   worth a one-line pointer. Rejected as the primary home because the value is in the
   nine worked pairs, and `CLAUDE.md` is a routing document under continuous size
   pressure — the table would be the first thing cut.

3. **Leave it in `bug-fix-session-log:W-69`'s amendment.** Rejected — that entry is
   scoped to *staging*, and five of the nine instances have nothing to do with git. The
   session log is where it was found, not where it belongs.

4. **Write a lint or a wrapper that refuses `--stat` in a verification context.**
   Rejected on the same grounds the sibling ADR rejects its clause-1 lint: no mechanical
   check can tell a verification context from a reporting one, and `--stat` is the right
   tool for reporting. The judgement is the work.

## Related

- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the sibling; this
  generalises it from zeros to any plausible value, and clause 3 there is this ADR's
  corollary read from the producer side.
- `bug-fix-session-log:W-69` — explicit-path staging, and its 2026-08-30 amendment
  adding the content-diff clause. `05ecf04a`.
- `bug-fix-session-log:W-73` — "compile-error → green" as the trigger for spending a
  mutation; the passing-unit-test member of this census is its caller-side twin.
- `bug-fix-session-log:W-74` — when the closure step *is* the broken operation, run it.
- `docs/issues/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md`
  (`open-issue-work-queue:BL-58`) — the worst member, filed separately.
- `docs/issues/2026-08-30-buddy-compact-banner-names-a-peers-session-as-your-own.md`
  (`BL-59`) — adjacent: it overstates what *you* wrote rather than misreporting an
  artifact, and cannot be refuted from inside the session.
- `7930e0b7` — the commit that carried a peer's two lines past a `--stat` check.
- `0c4931ef` — BL-60, where "there is nothing to run" was itself run, and returned the
  fact struct-reading could not supply.
