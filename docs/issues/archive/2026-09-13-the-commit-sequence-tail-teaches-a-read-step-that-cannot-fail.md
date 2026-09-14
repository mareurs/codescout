---
id: a29b6c1857744da4
kind: bug
status: fixed
title: 'BUG: the shared commit-sequence tail teaches a read step that cannot fail'
tags:
- cluster/assertion-that-cannot-fail
- commit-sequence
- hooks
- shared-checkout
closed: 2026-09-14
---

# BUG: the shared commit-sequence tail teaches a read step that cannot fail, and three of four hooks print only that

## Summary

`scripts/commit-sequence-tail.txt` is the single shared refusal tail emitted by every
refusing commit hook in this repo. Its step 4 reads:

```
4. Stage, then read the diff, then commit by pathspec.
     git add <paths> && git diff --cached && git commit -m "..." -- <paths>
```

The prose is right. **The command under it is the exact batched form that
`scripts/pre-commit-unreviewed-content.sh` documents as a measured capture vector**, and
whose own refusal body says, verbatim: *"Do this instead — FOUR SEPARATE calls. Not one
batched command."*

Two defects, and the second is what makes the first expensive.

## Symptom (Effect)

**1. The middle step cannot fail.** `&&` chains on exit status, and the check being
performed is a human or a model *reading content*. `git diff --cached` exits `0` whether
the staged content is yours or a peer's, so the chain always proceeds to the commit. The
step reads as a gate — `&&` means *stop if this fails* — and is a print.

There is no flag that repairs it. `--exit-code` makes the step exit `1` on *any* staged
diff, which would block every legitimate commit. The check is inherently a read, and no
shell chaining can express a read. That is precisely why the unreviewed-content hook asks
for four separate calls rather than a better one-liner.

**2. Three of the four emitting hooks print ONLY the tail.** The
*"FOUR SEPARATE calls"* correction exists at exactly one surface —
`scripts/pre-commit-unreviewed-content.sh:126` — and that hook fires on one narrow shape.
A session refused by the ledger hook, the cargo-fmt hook or the foreign-index hook sees
the `&&` form and never sees the correction. A session refused by unreviewed-content sees
both **in one message, contradicting each other**, with the `&&` form printed *last*
because the tail is appended after the body.

## Reproduction

```
grep -rl commit-sequence-tail scripts/     # 3 shell hooks; the ledger hook adds a 4th
                                           # via its own _emit_sequence_tail()
grep -r "FOUR SEPARATE" scripts/           # exactly 1 hit, in one of the four
```

And the exit-status half, in a throwaway repo:

```
git add f.txt
git diff --cached              ; echo $?   # 0  -- with a staged diff present
git diff --cached --exit-code  ; echo $?   # 1
```

## Environment

`experiments`, 2026-09-13. `scripts/commit-sequence-tail.txt` (25 lines), emitted by
`pre-commit-cargo-fmt.sh`, `pre-commit-foreign-index.sh`,
`pre-commit-unreviewed-content.sh` and `pre-commit-ledger-counts.py`.

## Root cause

The tail was extracted into one shared file so the refusing hooks could not drift apart —
a correct decision, and the reason this defect is uniform rather than occasional. The
extraction copied step 4 in its `&&` form. The *"not one batched command"* finding lives
in the hook body that was **not** extracted, because it is specific to that hook's own
refusal. So the shared surface carries the vector and the narrow surface carries the
antidote, which is the wrong way round: the shared surface is the one every refusal
reaches.

## Evidence

### E1 — a live capture downstream of it, the same evening

`cd138c30` captured two paragraphs belonging to sessionId `f3c594ce` into a commit about
the roster's mechanism column. The capturing session (`f0b1a4c7`) reported, unprompted,
that `pre-commit-run.sh`'s refusal tail had printed the sequence **at least three times**
that session while they were building an unrelated check, that they had quoted steps 3
and 6 to a peer from memory minutes earlier — and that they then staged, read
`git diff --cached --stat`, and committed off it.

The refusals they hit were the **ledger** hook's, which prints the tail and not the
*"FOUR SEPARATE calls"* body. So the only form they were shown, repeatedly, at the moment
of need, was the one this file is about.

### E2 — the batching mechanism already has its own measurement

`pre-commit-unreviewed-content.sh`'s header records `21258b4b`, 2026-09-02: a session ran
`git add -- <paths>` and a bare `git commit` **in the same invocation** that printed the
full `--name-status`, and captured four files. Its conclusion is the general one —
*"a read step placed in the same command as the write it gates is not a read step"* — and
it notes the sequence *"is invisible in a transcript"* and *"is caused by batching for
efficiency, which is otherwise the right instinct here."*

### E3 — it is the class's own signature shape

`IC-16`'s claim closes: *"it is added most often in the very commit that closes a
missing-guard finding."* The commit-sequence tail exists **because** of the capture
findings. A check with no failing input shipped inside the guidance written to stop them.

### E4 — both arms, same day, from a third session: the split form caught what the batched form could not

Reported unprompted by sessionId `9403d62d` while this file was being written, which makes it a
confirming instance rather than a reconstruction. Both arms in one evening, one session, same
checkout:

| form | commits | caught |
|---|---|---|
| **batched** — the form step 4 prints | 4 | nothing, *and could not have*: the staged list printed above the commit result in one blob, read after the fact |
| **split** — `git add` + read as one call, `git commit` as a **separate** call | 7 | nothing for six; on the seventh, another session's staged pair |

The seventh is this bug file. At 19:40 their `git diff --cached` showed three paths — their
own, plus **this file** (then still unarchived) and its class file
`docs/trackers/issue-clusters/IC-16-assertion-that-cannot-fail.md`, staged by `f3c594ce`.
They committed by pathspec (`4fdf51c7`, one file, verified) and left both. A bare `git commit`
would have taken the pair under a message about a hooks tracker **and split the coupling the
ledger gate requires** — a bug file and the `+1:` naming it on its class's `**Members:**` line
must land in one commit, so the capture would have redded the shared gate for every session in
the checkout, in the direction step 3 of the very same tail warns about.

They were reading in the split form only because a fourth session had corrected them into it
earlier that evening. Following the printed guidance would have left them in the arm that caught
nothing.

**And the guard did not cover this.** `refuse an index commit carrying another session's staged
paths` **Passed** on their pathspec commit — it cannot fire there, because a pathspec commit does
not commit the index. So the two mechanisms cover different halves and neither subsumes the
other: the guard catches a bare commit *after* the decision is made, and the read step is the
only thing that tells you the situation exists at all. Their words, and worth keeping because the
tail presents step 4 as the redundant half of a belt-and-braces pair: *"neither alone would have
told me your pair was there AND that I was safe."*

### E5 — the corrected form, observed catching a live index shift, inside the hour it shipped

Every datapoint above is evidence *against* the batched form. This one is the other
polarity, and it is the only one: the repaired step 4 working, in the wild, reported by
sessionId `f0b1a4c7` — who read `scripts/commit-sequence-tail.txt` while it was still
**dirty in the worktree**, so what they followed was the uncommitted fix rather than the
committed `&&` one-liner.

2026-09-14, in the ~90 seconds before `3835dd18` landed:

| | |
|---|---|
| ~06:03 | reads the dirty tail; follows the separate-call form |
| 06:03:5x | `git add <3 paths>`, then `git diff --cached --name-only` **as its own call** → 3 files, all theirs |
| next call | `git diff --cached --numstat` → **7** files. Four of mine had entered the shared index between their two calls |
| 06:04:31 | `3835dd18` — 4 files, exactly mine |
| 06:05:10 | `b4e2206b` — 3 files, exactly theirs, by pathspec |

No capture in either direction. The index shifted *between two consecutive reads by one
session*, which is the condition a chained form cannot surface in time: `&&` gates on exit
status, and nothing about a peer's four files arriving produces a non-zero one.

**The limit of the claim, stated by the reporter and preserved because it is the honest
shape:** *"I don't know whether I'd have noticed the four extra files in a chained diff's
output before typing the commit. The counterfactual is that the check would have been
structurally too late, not that I'd certainly have been captured."* That is the right
reading. This entry establishes that the corrected form **put the information in front of a
reader while it could still change the outcome** — which is the whole of what a read step
can offer, and precisely what the chained form removes. It does not establish that a capture
was averted, and it is not recorded as if it did.

## Hypotheses tried

**"The `&&` is harmless because a human sees all three outputs anyway."** False for the
population this tail is written for. For an agent the tool result arrives only after the
whole chain completes, so the diff is readable strictly after the commit already ran; for
a human the same holds whenever the commit is fast. Either way the output is a record of
what happened rather than a check on whether it should.

**"Just add `--exit-code`."** Falsified above — it exits `1` on any staged diff and would
refuse every legitimate commit.

## Fix

**Fixed in `3835dd18`** (patch-id `e086a01511c9f5577667c2f53aad473264ed0e54`). Both halves
landed in one edit, which is why the second was cheap: the tail now shows the four-line
block **and** carries the reason, so the hooks that previously printed only the vector print
the correction too.

**The fix was to match the source, not to invent guidance.** `docs/conventions/shared-checkout-commit-sequence.md`
§ 4 had the separate-line block correct throughout — and cites `1b40dabd` for precisely this
failure. So the drift ran source-correct / summary-wrong, which is the dangerous direction:
the summary is what a session reads mid-refusal, the source is what someone reads when
deciding whether to change a step. That made the change low-risk in a way § *Fix* did not
anticipate when it held both halves for a go-ahead.

**Verified end to end, not by grep.** `--fixture-growth-refusal` drives a real refusal
through the ledger hook; the emitted text now carries four separate command lines and zero
`&& git commit`. The pre-existing `every_refusing_hook_emits_the_shared_tail` greps for the
call site and deliberately does not execute, so the greps prove reachability and this run
proves the bytes.

**Two stale counts repaired in passing**, both reading *"three refusing hooks"* where
`every_refusing_hook_emits_the_shared_tail` enumerates four. Replaced with a pointer to that
test rather than the number — the set has already grown once, when the cargo-fmt hook
replaced a framework entry that could not emit the tail at all.
## Tests added

`the_tail_teaches_separate_calls_never_a_chained_commit` in `tests/hook_config.rs`, beside
the two tests that already guard the tail's existence and its emission.

**Four mutations on the production path — the tail itself, never the test's inputs.**

| mutation | result |
|---|---|
| rejoin the commit onto a `&&` chain | RED — and the message reproduced the defect line verbatim |
| delete step 4's command block | RED — this is the half that keeps the first honest |
| drop the `--name-only` read | RED |
| chain `git add` to the READ | **RED, and it was predicted to SURVIVE** |

The first three are ordinary. **The fourth existed to check OVER-firing and falsified the
test's own doc comment rather than the test.** That comment claimed the gate was scoped to
the commit alone and that chaining add-to-read would pass; it does not, because the joined
line no longer *starts with* the read. The test is right and the comment was wrong — this
gate is about the TAIL'S TEXT, which must display each command on its own line so the
separateness it teaches is legible. It says nothing about what a session may type: `git add`
plus the read in one call is fine and is the form `9403d62d` measured working (§ *E4*).

Recorded because a kill count never reports over-firing. Three kills read as full coverage;
only the mutation written to survive showed the scope was wrong, and what it corrected was
prose a later reader would have trusted.
## Workarounds

Four separate calls, as `scripts/pre-commit-unreviewed-content.sh` says. Read the diff's
**magnitude** against what you know you wrote — `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`
§ *Instance 11* and § *Instance 13* are two independent sessions who had that number on
screen and read it as confirmation that staging worked.

## Resume

Found while writing Instance 13 of the capture ledger, which is downstream of it. The sharp
thing was never that guidance was wrong — it is that this guidance was **printed unprompted
at the moment of need, repeatedly, to a reader who had quoted its neighbouring steps aloud
minutes earlier**, and the failure survived all of that. If a documentary remedy were going
to work, those are the conditions under which it would.

**So the fix here does not close the mechanism, and should not be read as closing it.** A
correct tail makes the right thing easier to copy; it cannot make a chained one-liner
impossible, and the next session to batch for efficiency will get no warning. The remedy
that would is still unbuilt and still designed, in `scripts/pre-commit-foreign-index.sh`
§ *the remedy for that half*: record each path's blob at `git add`, re-hash at pre-commit,
refuse if it moved. This bug's whole evidential contribution is **against the alternative**
rather than for that remedy — which is the more useful direction, and the reason § *E1* and
§ *E4* are kept in full.

**One live datapoint from the fix's own commit, worth more than the fix.** Staging
`3835dd18` and then reading `git diff --cached --name-only` as a separate call surfaced
three foreign paths a peer had staged in the interval — including
`docs/trackers/issue-clusters.md`, the known hot file. A bare `git commit` would have taken
all three. The step this bug is about caught something on its first use after being
repaired, in the arm the old text would not have produced.
## References

- `scripts/commit-sequence-tail.txt` — step 4
- `scripts/pre-commit-unreviewed-content.sh` — § *AND THE READ CAN BE DEFEATED BY BATCHING*, and the `FOUR SEPARATE calls` refusal body
- `scripts/pre-commit-foreign-index.sh` — CROSS-path vs INTRA-path, and the unbuilt re-hash remedy
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` — Instances 11 and 13
- `docs/conventions/shared-checkout-commit-sequence.md` — the full sequence the tail summarises
