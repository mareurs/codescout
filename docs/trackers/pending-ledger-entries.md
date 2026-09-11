---
id: '330f7d7f01f4ec23'
kind: tracker
status: active
title: Pending ledger entries — drafts blocked by the unpushed-ledger guard (delete on fold-in)
owners:
- marius
tags:
- pending
- ledger-blocked
- peer-sessions
topic: blocked ledger writes awaiting a push
---

> **This is not a ledger.** It defines no entry ids and declares no `entry_prefix`, on
> purpose — see *Why there are no `## PREFIX-N` headings below*. It is a holding pen with an
> owner and an exit condition, and it should be **deleted** once its contents land.

## Why this file exists

`append_entry` — the only write path into a declared ledger — refuses while that ledger's own
file has commits not on `@{upstream}`. On 2026-09-11 it refused an append to
`docs/trackers/bug-claim-liveness-session-log.md` because `15cbe5e6`, a commit *from the same
session that was appending*, had touched that ledger and was unpushed.

That is open bug `cd808780d9ea2db9`, and this is a live instance of it. The refusal's own
fallback reads:

> *"If you cannot push right now, do not write the entry by hand instead — a declared
> `entry_prefix` puts this file off-limits to direct `edit_file`. Note the entry somewhere
> worktree-local instead, and fold it into the ledger once these commits are pushed."*

**That fallback is a deferral, not a remedy, and this file exists to make the deferral
visible.** Left in a session scratchpad the drafts die with the session and nothing anywhere
records that they were owed — a blocked write quietly becomes an untracked one. Committing
them costs one small file and converts a silent loss into a visible obligation.

The push that would clear it is itself blocked, for an unrelated reason: `origin/experiments`
had diverged by 5 commits from a native-Windows VDI at 2026-09-11T09:42:08Z, so no rung of the
`docs/RELEASE.md` ladder can be taken until someone integrates. Two blockers stacked, neither
caused by the other.

## Exit condition

Once `docs/trackers/bug-claim-liveness-session-log.md` has no unpushed commits, append each
draft below **through the librarian**, then delete this file in the same commit:

```
doc(action="append_entry", id="6c66df6348aae62d", id_prefix="F"|"W",
    anchor_heading="## Template for new entries", title=…, body=…)
```

Let the server allocate the id. Do not compute one, do not hand-write the section, and do not
pre-write an index row — a pre-written row consumes the id it names.

## Why there are no `## PREFIX-N` headings below

`link_scan` binds a `PREFIX-N` token to a `## PREFIX-N — <title>` heading and to nothing else.
A draft written with its real heading would become a **second definer** of that token the
moment the entry lands in the ledger, and an ambiguous token resolves to nothing — which would
manufacture the citation break this file is trying to prevent. That is `IC-6`, *an addressing
scheme with no escape hatch*, and the escape here is simply not to spell the token. Titles
below are plain text; ids are assigned at fold-in.

## Draft 1 — append with `id_prefix="F"`

**Title:** I shipped the defect class I had filed an hour earlier, into the doc comment of its
own fix's test

**Body:**

**Valid:** invariant

**Category:** self-friction

**Severity:** med

**Status:** fixed-verified

**Observed.** Writing the fix for the params-array rule bug (`ea302e013cf7c85a`, archived), I
put a justification into a new test's doc comment that I had not checked:

> *"That matters here: the token appears in the § Development Commands region too, and a
> file-wide `contains` would pass with the carve-out deleted."*

`grep -c 'params_behind_body' CLAUDE.md` returns **1** — the sole occurrence being the
carve-out I had just written. The claim was false, and it was the *reason* given for the
test's design.

**Why this is worth an entry rather than a quiet correction.** One hour earlier I filed an
entry in this same ledger whose entire subject is `doctor` emitting a `detail` string that
*asserts a false cause* — prose that is confident, load-bearing, and untested by construction.
I then committed the same defect, in the same shape, into a test comment, while writing the fix
for the bug that entry supports. **Knowing the class prevented nothing.** That is `OB-1`'s
measured claim reproduced on myself: *four instances in one evening across three sessions,
every one committed by an author actively writing about that class.*

**What caught it was not care.** It was running `grep` before letting the sentence stand — the
same verify-don't-hypothesise step that had already, four times that session, inverted a
conclusion about to be published. The remedy is not "be careful with doc comments"; it is that
a causal claim in prose gets the same one-command check as a claim in code.

**The sharper half — the corrected version is a better argument than the false one.** The
scoping to the `>` blockquote is still right, for a reason reached only by being wrong: today
the scoping is *redundant*, and the redundancy is precisely what decays. The day anything else
in `CLAUDE.md` mentions that check, a file-wide assertion silently stops discriminating and
nothing reports that it has. The false version claimed present necessity; the true version
claims future necessity, which is the stronger reason to write it that way. **A fabricated
justification can defend a correct decision**, which is what makes this class survive review —
the reviewer checks the decision, agrees, and never audits the reason.

**Cost:** none shipped — caught pre-commit. `med` rather than `low` because the artifact was a
*test's rationale*, the surface a future session reads to decide whether the test still earns
its place; a false rationale there invites a correct test to be deleted as over-engineering.

**Rests on:** `grep -c 'params_behind_body' CLAUDE.md` → 1, run 2026-09-11 at HEAD `1fe07709`;
`observer-blindness:OB-1`.

## Draft 2 — append with `id_prefix="W"`

**Title:** three gate refusals in one session, and not one was findable in the refused author's
own diff

**Body:**

**Valid:** invariant

**Category:** process

**Status:** validated

**Observed.** Three separate guards refused work on this shared checkout in one session. All
three refusals were correct, and **none of the three causes was visible in the diff of the
session being refused**:

1. **Pre-commit `refuse a stored count, or a class gaining a member it does not name`.** A
   sweep commit added a bug carrying `cluster/doc-contradicted-by-code`, and `IC-11`'s
   `**Members:**` line did not name it. The coupling lives in a *different file* the commit did
   not touch. The refusal pointed at the exact path and warned that grepping the roster returns
   0 — *"that zero means WRONG FILE, not no such class"*.
2. **`./scripts/fmt-mine.sh` refusing to format `src/tools/guide_rearm.rs`.** A peer's
   uncommitted file, attributed by sessionId, with the socket to reach them printed. Not mine,
   and deliberately no `--force`.
3. **`every_open_bug_file_declares_one_known_defect_class` reddening both test lanes.** A
   peer's *committed* bug file (`1fe07709`) carrying no `cluster/` tag — so HEAD was red before
   the session began, and none of its four changed files feeds that test.

**Counterfactual.** Without (1) the class ledger ships a member it does not name, the exact
state that makes *"which architectural problem do these bugs share?"* stop being a query.
Without (2) a peer's in-flight Rust gets rewritten — itself a filed defect here. Without (3)'s
attribution a red suite reads as evidence against one's own change, and the next hour goes into
debugging a working diff — which is
`docs/issues/2026-09-08-the-author-of-a-tree-reddening-write-is-the-one-party-never-told.md`.

**The generalisable claim, and the reason it is one entry rather than three notes.** On a
checkout this shared, **the modal cause of a red is not in the diff of the session that meets
it.** Every one of these guards therefore spends its budget on *attribution* rather than
detection — naming the file, the sessionId, and the socket — and that is what converts each
refusal from an obstacle into a one-message action. A guard that had merely said "formatting
needed" or "test failed" would have produced three investigations into the wrong session's
work.

**Promote-when:** a fourth refusal in a later session whose cause is again outside the refused
diff. At n=4 the claim is worth stating in `CLAUDE.md` § *Observer Blindness* as a design rule
for new guards: **a refusal on a shared tree must name the author, not just the fault.**

**Rests on:** the three refusal texts as emitted 2026-09-11 between 09:30 and 12:30, HEADs
`15cbe5e6` → `1fe07709`; `scripts/file-provenance.py` output for (2) and (3).

## Draft 4 — append with `id_prefix="F"`

**Title:** a merge kills the RELEASE.md ladder, and the ladder's own precheck returns the green
answer for a rung that can never be pushed

**Body:**

**Valid:** invariant

**Category:** process

**Severity:** high

**Status:** open

**Observed.** `docs/RELEASE.md` § *Publishing a stack several sessions wrote* prescribes the
ladder: each author pushes their own commit by refspec, bottom-up, so *"no operator is ever
asked to authorise someone else's work"*. Its stated precheck is
`git rev-list --count origin/<branch>..<your-sha>` must be `1`, *"checked BEFORE the push"*.

After reconciling a 5-vs-9 divergence by merge, measured 2026-09-11 at HEAD `e4262c97`:

```
rev-list --count origin/experiments..15cbe5e6          = 1     <- precheck PASSES
merge-base --is-ancestor origin/experiments 15cbe5e6   = false <- cannot fast-forward

lowest fast-forwardable ref is the MERGE COMMIT 8c795a92, carrying 10 commits
across four sessions.
```

**Two separate defects, and the second is the one that bites.**

1. **The precheck is necessary and not sufficient**, and fails in the direction that reads as
   clearance. It counts *how many of mine are unpublished* and is silent on *whether the
   remote is still an ancestor*. A session that reads the ladder literally, gets `1`, and
   concludes it may push has verified a proposition that does not entail the one it needs.
   Remedy is one line beside the existing check: `git merge-base --is-ancestor
   origin/<branch> <your-sha>`.
2. **A merge collapses the ladder entirely, and merging is what you must do to push at all.**
   The ladder presumes a LINEAR stack: rung N becomes pushable once rung N-1 lands. A merge
   commit makes every commit beneath it unreachable as a fast-forward target, so the smallest
   publishable unit becomes the merge itself — all sessions' work, at once. Reconciling the
   divergence, which was the prerequisite for anyone publishing anything, is precisely the act
   that destroyed the mechanism designed so nobody publishes anyone else's work.

**Why this is not merely a documentation gap.** The ladder exists to keep an operator from
being asked to authorise another session's commits — RELEASE.md is explicit that the question
should never reach them. After a merge that question is unavoidable and returns in a *worse*
form than before: pre-merge it was 7 commits across 3 sessions, post-merge it is 10 across 4
and cannot be decomposed. **The remedy that unblocks the branch and the mechanism that makes
the branch publishable safely are in direct opposition**, and nothing in RELEASE.md says so.

**What a fix would have to decide** (not proposed here, because it is a real design question):
whether the ladder is abandoned once a branch diverges — in which case RELEASE.md should say
that a divergence escalates to a single operator authorisation by construction — or whether
reconciliation should be a REBASE after all, which preserves linearity at the cost of
re-keying peers' commits, the thing § *Concurrent-Work Rules* forbids for its own good
reasons. Both horns are real; this entry claims only that the corpus currently documents
neither.

**Rests on:** the four `merge-base --is-ancestor` probes above, run 2026-09-11 at HEAD
`e4262c97` against `origin/experiments = f50be810`; `docs/RELEASE.md` § *Publishing a stack
several sessions wrote — the ladder*. The insufficiency half (1) was raised to the session
implementing the pre-push divergent-push guard before this instance existed, as a predicted
gap; this is its first measured occurrence.

## Draft 5 — append with `id_prefix="F"`

**Title:** the gate selects by PATH and the task framing assumed STATUS, so "classify them
properly" would have put defect classes on six closed bugs

**Body:**

**Valid:** invariant

**Category:** self-friction

**Severity:** med

**Status:** fixed-verified

**Observed.** A merge landed 8 bug files under `docs/issues/` with no `cluster/` tag, reddening
`every_open_bug_file_declares_one_known_defect_class`. Three sessions independently described
the population as *"8 untagged open bugs"*, and the operator, given that framing, chose to
classify them properly. Reading the test rather than its failure message:

```rust
// tracked_open_bug_files()
p.strip_prefix("docs/issues/").is_some_and(|rest| {
    !rest.contains('/') && rest.ends_with(".md") && rest != "_TEMPLATE.md"
})
```

It selects by **path** — anything directly under `docs/issues/`, never reading `status:`. Six
of the eight were `status: fixed`, closed 2026-08-19, and simply never archived. Only two were
open.

**The cost of acting on the framing.** Tagging all eight would have assigned defect classes to
six closed bugs, inflating the counts that class promotion reads — the precise outcome the
test's own failure message warns against, reached by following an instruction that said
*properly*. The correct action for those six was ARCHIVING, which removes them from the gate's
population entirely and needs no class at all.

**The generalisable shape.** A gate's failure message names the members it rejected and not
the predicate that selected them. *"open bug files with a bad defect-class declaration"* is
prose; `tracked_open_bug_files()` is the definition, and the word `open` in the message means
something different from the word `open` in the frontmatter of the files it lists. Everyone
downstream — three sessions and one operator — inherited the message's vocabulary and none
checked the selector.

**What actually caught it** was not suspicion of the framing: it was that the ids would not
resolve for a catalog write, which forced reading the frontmatter, which showed `status:
fixed`. An accident of the write path, one step before six wrong tags.

**Cheap general remedy, offered not claimed:** when a gate names a population in prose, read
its selector before acting on the population — the same move § *Observer Blindness* already
prescribes for a coverage ratio, applied to a membership predicate instead of a count.

**Rests on:** `tests/issue_clusters.rs` `tracked_open_bug_files()`; the eight files'
frontmatter as merged; `e4262c97`.

## Draft 6 — append with `id_prefix="F"`

**Title:** the verify-before-asserting habit was scoped to my own artifacts, so a claim about a
peer's code bypassed it entirely — and "do not write their file" had quietly become "do not
read it"

**Body:**

**Valid:** invariant

**Category:** self-friction

**Severity:** med

**Status:** open — no mechanism; the remedy below is a policy and I do not have a gate for it.

**Observed.** Three instances of one class in a single session — *asserting a property of a
mechanism from its description rather than from reading it* — and the defence that caught the
second did not fire on the third.

| # | the claim | caught by |
|---|---|---|
| A | `doctor`'s `params_behind_body` `detail` asserts *"a move re-keys the row"* for an id that was worktree-minted | me, reading the filed bug — became the entry this ledger already holds |
| B | my own new test's doc comment: *"the token appears in the § Development Commands region too"* | me, `grep -c` before commit — returned **1**, the carve-out itself |
| C | told a peer their pre-push guard would see merges *"go from rare to universal"* and warned about its false-positive rate | **the peer**, by reading their own implementation |

**The asymmetry is the finding.** B and C are the same defect. B triggered a check because the
artifact was mine and opening it was reflex. C did not, and the reason is not carelessness: I
had the peer's one-line description of their check and treated it as sufficient, because the
artifact felt like theirs to inspect.

**It was fully readable.** Their implementation — `src/librarian/tools/append_entry.rs`, the
pre-push guard script and its test — was **dirty in the shared working tree at that moment**.
I had already enumerated those exact paths, by name, in a provenance run, in order to avoid
committing them. I could have opened any of them in one call. What stopped me was not access.

**The conflation, stated plainly.** This checkout's discipline is *do not WRITE a peer's
uncommitted file* — `fmt-mine.sh` refuses it, `git commit` by pathspec exists for it, and
`docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` measures the
cost of getting it wrong. Somewhere that became *do not TOUCH a peer's file*, and reading is
not touching. The write prohibition is load-bearing and correct; extending it to reads removes
the only thing that would have stopped me making a false public claim about their work.

**Why the cost is not hypothetical.** The claim was addressed TO the author, who could check
it — the best possible case. Sent to any third party, or recorded in a ledger, *"the pre-push
guard's false-positive rate goes up after a merge"* is a plausible, specific, wrong statement
about a subsystem under active development, and the party best placed to refute it would never
have seen it. Severity `med` rather than `high` only because the addressing happened to route
it to the one reader who could falsify it.

**Remedy, and it is narrow on purpose:** before asserting a property of code a peer holds,
read the code — the same standard applied to one's own. The shared-checkout rules restrict
writes and say nothing about reads, so no rule had to change; what needed changing is that I
had generalised one into the other. **The tell:** a sentence of the form *"if your X does Y
then Z"* where X is someone else's and you have not opened it. State the conditional
explicitly, or open it.

**Not generalisable to "ask fewer questions of peers."** The prediction was worth making and
the peer said so — it prompted a stress-test they had not run. What was wrong was the
grammar: I asserted a property where I held a hypothesis. *"Does your check key on merge
presence or on a verified duplicate? If the former, …"* costs one sentence and is true.

**Rests on:** the three instances above, 2026-09-11; the peer's reply confirming their refuse
fires on `git grep -c '^## PREFIX-N' >= 2` in the merge's own tree rather than on merge
presence; `observer-blindness:OB-1` for the *knowing the class prevents nothing* precedent,
of which this is a same-session n=3.

## Addendum — fold into Draft 2, or drop

**Three registry names for one sessionId inside one day, observed live.** sessionId
`f3c594ce-c424-40d3-a603-9693cfef3f63`, pid 703051, profile `.claude-kat`, socket
`/run/user/1000/cc-socks/703051.sock` — all four constant throughout. Its `name` was:

| observed | name | source |
|---|---|---|
| 11:09 | `codescout-53` | `scripts/peer-sessions.sh` socket walk + `file-provenance.py` |
| 11:29 | `fix-subagent-guide-starvation` | `fmt-mine.sh` refusal banner |
| ~12:45 | `append-entry-unpushed-guard-fix` | the `from-name=` on its own inbound message |

`CLAUDE.md` § *Observer Blindness* already holds the law — *attribute by sessionId, never by a
self-reported name* — citing a two-name instance. This is a **three**-name instance in a single
working day, and the aggravating detail is that each name is *descriptive of the session's
current task*, so every one reads like a durable identity rather than a label; the third would
look to any reader like a different session entirely from the first. Every message exchanged
with it was addressed by socket path, which is why none of the three renames cost anything.

**Recorded, not promoted.** The rule needs no change and the existing citation already carries
it. Fold this in only if a sharper instance is wanted.
