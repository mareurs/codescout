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
