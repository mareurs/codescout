---
kind: plan
status: draft
title: A committed sequence for shared-checkout git operations
owners: [marius]
tags: [shared-checkout, guides, hooks, multi-session]
topic: multi-session commit coordination
---

# A committed sequence for shared-checkout git operations

**Status:** draft proposal · **Opened:** 2026-09-01 · **Rewritten:** 2026-09-01, direction
reversed after peer refutation · **Author:** session `codescout-b7`

## The friction, measured rather than asserted

One two-author commit on this checkout, 2026-09-01, cost **nine cross-session messages,
two windows where the shared working-tree gate was red, one refused commit, and roughly
two hours of two sessions' wall time.** Both sessions knew the rules. Both had read the
relevant CLAUDE.md sections. Neither could have predicted the sequence in advance, because
the sequence is not written anywhere — it is distributed across four hook refusal messages,
each of which fires *after* you have done the work it invalidates.

The individual defects were all caught. What was expensive was that they were caught
**one collision at a time**, and each collision was recoverable only by a round-trip to
the other session.

## What already exists

Two halves are built, and the gap is between them.

**Injection — `src/prompts/guides/*.md`.** Ten topics, each auto-injected on the first tool
call in a session that triggers it, via `Tool::relevant_guide_topic`. Delivery, dedup per
topic per session, and a ledger cleared on `workspace(activate)` all work today.

| existing topic | what it governs |
|---|---|
| `progressive-disclosure` | `@ref` buffers, output budgets |
| `tracker-conventions` | bug files, trackers, entry ids |
| `workspace-state` | activation, home/foreign |
| `iron-laws-detail`, `error-handling`, `symbol-navigation`, … | per-surface discipline |

**Enforcement — `scripts/`.** Four hooks already encode shared-checkout rules:

| hook | refuses |
|---|---|
| `pre-commit-foreign-index.sh` | a **bare** commit whose index holds another session's paths |
| `pre-commit-unreviewed-content.sh` | a **pathspec** commit carrying unstaged content |
| `pre-commit-ledger-counts.py` | a commit whose ledger counts disagree with its own index's corpus |
| `prepare-commit-msg-session-id.sh` | (stamps `Session-Id:`, enabling positive authorship) |

Plus `scripts/peer-sessions.sh`, which lists live sessions including the ones `ListAgents`
hides.

**The gap is not teaching quality — it is forward reach.** The hooks teach very well; read
`pre-commit-foreign-index.sh`'s refusal text. What none of them does is tell you what comes
*after* the rule that fired. Each fires at its own collision, in isolation, so the sequence
is learned one collision at a time — and each collision costs a round-trip to another
session.

## The proposal

**Put the sequence in the hook refusal texts. Do not build a guide topic.**

This reverses the draft's original direction, which proposed a new
`shared-checkout-commits` guide topic injected on the first git write command of a
session. That direction was refuted by `compact-root-claude-md` and then verified against
the code below. It is recorded rather than deleted because the *reason* it fails is the
reusable part: **the failure mode is silence — the topic simply never arrives, and nothing
reports that it did not.**

### Why guide injection cannot carry this — three independent blockers

Each read in the bytes at `30b6fc41`, not inferred.

1. **`relevant_guide_topic` never sees the input.** Its signature is
   `fn relevant_guide_topic(&self, _result: &Value) -> Option<&str>`
   (`src/tools/core/types.rs:1453`). It is handed the *result*. No command string is in
   scope, so no routing decision keyed on `git commit` can be made there.

2. **The selector path cannot introduce a topic on its own.** `topic_declaring` is not a
   second route — it sits *inside* `} else if let Some(content_topic) =
   self.relevant_guide_topic(&val) {` (`types.rs:1195`) and only appends a second candidate
   to a list `relevant_guide_topic` must first open (`types.rs:1255-1262`). It also selects
   only topics that **declare** `serves:` sections. So "non-declaring topic, routed from a
   projection of the command string" is not a configuration that exists: a non-declaring
   topic is reachable *only* from `relevant_guide_topic`, which is blocker 1.

3. **`run_command`'s selector carries no command string, and can no longer cheaply be made
   to.** Post-`30b6fc41` every tool's selector is `action_selector_key(self.name(), input)`
   (`types.rs:1459`), which reads the `action` field and falls back to the bare tool name.
   `run_command` has no `action` param, so its selector is the constant `"run_command"` —
   identical for `git commit` and for `ls`. Adding a command-string projection is not a
   local change either: the inversion put the default on the **hot path for all 21
   registered tools**, so a scan distinguishing git commands would be paid by every
   `symbols` call to benefit the few that shell out.

### What to build instead

The refusal texts are already the right surface, and already unusually good —
`pre-commit-foreign-index.sh` names the peer, resolves their pid, prints the `uds:` socket,
distinguishes a dead incarnation from an abandoned file, and says *"ASK before assuming."*
Nothing about the teaching quality needs fixing.

**What is missing is forward reach.** Each hook teaches its own rule at its own collision
and stops there, so a session pays for the sequence one collision at a time — which is
exactly what the nine-message, two-hour choreography above was.

The change is therefore small and needs no trigger at all:

> **Give each refusal text a "what comes next" tail carrying the whole sequence, not just
> the rule that fired.**

Three properties make this strictly better than the guide route for this content:

- **The trigger already exists and is free.** A hook fires when it fires. There is nothing
  for the model to notice — which matters here, because `skill-frictions:SKF-22` is
  precisely a trigger the model must notice going unnoticed for a whole session while its
  condition was stated out loud.
- **It costs nothing when it does not fire.** An injected guide is paid by every session
  that triggers it, whether or not that session was ever going to commit. A refusal text is
  paid only by the session already in trouble.
- **It reaches every session on the checkout, not just this profile's.** Hooks are
  repo-scoped; guide delivery is per MCP session.

The honest bound: a refusal text still fires *after* the work it invalidates. That is
unchanged, and it is why this is a mitigation of the friction rather than a fix.

### The sequence

Derived from what this session actually paid for. Each step exists because skipping it cost
something measured.

1. **Enumerate before you assume you are alone.** `scripts/peer-sessions.sh`, not
   `ListAgents` — the latter is per-profile and reported 2 peers where 16 sessions existed.
2. **Identify your own commits positively, by `Session-Id:` trailer** — never by a commit
   range. A range is a proxy for authorship and stops being one the moment anyone else
   commits. *(Measured: a review package scoped by recorded BASE collected 14 commits, 1
   of them mine.)*
3. **Before reverting anything you believe is yours, check `git show HEAD:<path>`.** An
   edit you made and an edit still uncommitted are different things and memory does not
   distinguish them — on this checkout your own work routinely lands in someone else's
   commit.
4. **Coupled changes commit together or not at all.** A count and its member, a citation
   and its target. Determine the coupling *before* staging, because every partial state is
   a red tree for every other session.
5. **`git commit -m "msg" -- <paths>`** — `-m` before `--`, or git reads `-m` as a
   pathspec.
6. **Verify the index, not the command's exit code.** `git show :<path>` and
   `git diff -- <paths> | wc -l` = 0. `git add` fails on a contended lock, and a retry loop
   that exhausts looks identical to one that succeeded.
7. **`git show --stat HEAD` after.** Before is not enough; the index moves between check
   and commit.
8. **If a commit captures another session's file: stop. Do not `reset` or `amend`.** On a
   shared tree the repair destroys work the defect only mislabels.

### Where a two-author commit is genuinely unrepresentable

This is the part a guide cannot fix, and the proposal should say so rather than imply
otherwise.

`foreign-index` accepts **only** a pathspec commit. `ledger-counts` accepts only a commit
containing every count and its member — which, on an entangled index, is **only the bare
one**. The intersection is empty. Measured by simulation against the real hooks
(`GIT_INDEX_FILE` + `git read-tree HEAD`, with a HEAD-only control row):

```
b7 pair (bug file + ledger)          FAIL  IC-17 17-vs-16, IC-12 1-vs-0
b7 bug file only                     FAIL  IC-3  22-vs-23
3e pair (2 bug files + ledger + OB)  FAIL  IC-3  23-vs-22
3e bug files only, no ledger         FAIL  IC-17 16-vs-17, IC-12 0-vs-1
ALL FIVE                             PASS
HEAD only (control)                  PASS
```

Every proper subset ships a count without its member, or a member without its count. Both
guards are correct; the intersection is still empty.

**Consent is not representable, and should not be** — a guard that read an asserted consent
would be a guard you could talk out of refusing. What is missing is not a consent channel
but **a way for two authors to build one commit deliberately**. Two sessions did it by hand
here, across nine messages and a five-step choreography. That is the thing worth
mechanising, and it is a separate piece of work from the guide.

## What this does NOT solve

- **The empty intersection above.** A guide teaches the choreography; it does not remove
  the need for one.
- **Capture generally.** `pre-commit-unreviewed-content.sh`'s own header is the honest
  bound: *"it narrows the window, it does not close it — only a per-session worktree does
  that."*
- **The after-the-fact problem.** A refusal text fires only once the index is already
  wrong. It shortens the recovery from N collisions to one; it does not move the teaching
  before the work. Only a per-session worktree does that.

## Open questions

The draft's three open questions are closed. Recorded with their answers, because two of
them dissolved rather than resolved and that is the informative part.

1. **~~Guide or hook-message improvement?~~ Answered: hook messages.** Not on the balance
   of the for/against originally listed, but because the guide route turned out to be
   *unbuildable* — see the three blockers above. The original "against" (they fire after
   the fact) survives as the honest bound; the original "for" (they fire exactly when
   relevant and cost nothing otherwise) is now the whole argument.

2. **~~Does it earn its slot?~~ Dissolved.** There is no slot. A refusal text is paid only
   by the session that already tripped the guard, so the base-arm measurement the promotion
   rule asks for is not owed here. It would still be owed by any future guide topic.

3. **~~Section-grain?~~ Dissolved with the guide.** Retained as a finding about the guide
   machinery rather than about this proposal: `Shape::matches` rejects a `None` selector,
   so a declaring topic delivered from a selector-less call silently downgrades to
   preamble-only, and Gate 7 (`b769277b`) refuses that configuration. Post-`30b6fc41` no
   tool returns `None` — but the downgrade remains the failure mode if one ever opts out.

**Newly open, and the real remaining question:** which of the four hooks carries the tail?
Repeating the full sequence in all four is the obvious wrong answer — four copies drift, and
`link_scan` cannot see shell strings. The likely shape is one shared text emitted by a
common helper, with each hook prepending its own rule. Not designed here.

## Evidence

- This session's `docs/trackers/response-envelope-session-log.md` — `F-2` (bare commit
  takes the whole index), `F-3` (`-m` after `--` is invalid), `F-4` (a range is a proxy for
  authorship), `F-5` (a contended `git add` fails silently).
- `docs/issues/2026-09-01-peer-commit-captures-another-sessions-working-tree.md` and the
  two bug files landed by session `codescout-3e` at `1fc91b93`.
- Simulation method and the six-row table: session `codescout-3e`, 2026-09-01.
- A bug file for the empty-intersection guard tension is **owed** and will be filed under
  `IC-17` once this session's own commit lands — filing it now would re-enter the very
  count-coupling dance this document describes.
- The refutation of this document's original direction is `compact-root-claude-md`'s,
  delivered 2026-09-01 and verified in the bytes before rewriting. The near-miss that
  preceded it — a token grep used to overrule a correct subagent signal — is
  `reconnaissance-patterns:R-162`.
