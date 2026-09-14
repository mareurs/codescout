---
id: 7cfeab41eec4d813
kind: bug
status: fixed
title: 'BUG: the fmt refusal''s owner list names a session that holds none of the bytes it asks them to format'
tags:
- cluster/gate-keyed-on-unobservable-event
- hooks
- shared-checkout
- provenance
---

# BUG: the fmt refusal's owner list names a session that holds none of the bytes it asks them to format

## Summary

`scripts/fmt-mine.sh` refuses to format files it cannot attribute to this session, and prints
an owner list so the reader can ask whoever holds them. The remedy text is explicit that the
named party **can act**:

> `[LIVE] peer` — ask them to format their own file … **They can perform it;** you cannot
> perform it for them without writing their bytes.

Measured 2026-09-14: the list named a session holding **zero** unformatted bytes in that
file. Every uncommitted line was a third party's. The message arrived at the right party and
asked them for something they had no way to do.

## Symptom (Effect)

The scan, run directly:

```
SHARED    src/librarian/tools/audit_doc_refs/parser.rs
          window: writes at or after 2026-09-13T09:25:37+00:00
          written by THIS session (f3c594ce)
          written by 9403d62d-…  [LIVE]
```

`git diff` on that path at the same instant: **148 added lines, all of them
`is_artifact_id` / `RefKind::ArtifactId` and four tests around them** — sessionId
`9403d62d`'s in-flight work, described by them to a third session before either scan ran.
`f3c594ce` held nothing there, and nothing uncommitted anywhere under `src/`.

**The cost landed on a live reader within four minutes**, which is what makes this more than
a cosmetic imprecision. A third session (`8bd791df`) hit the same refusal from the outside,
read the two-name list as *"these two people can each fix their part"*, and messaged
`f3c594ce` asking them to format their file. Their own words, reported back: *"I read the
list as 'these two people can each fix their part' and told you to format your file, which
you could not do because you hold none of it."* Under their reading the gate's step 1 stays
blocked for **every** session in the checkout until one of two named parties acts, and one of
them cannot.

## Reproduction

```
./scripts/file-provenance.py <a path whose uncommitted delta is entirely a peer's>
git diff -- <same path>          # the delta is someone else's
```

The two disagree about who holds it. `fmt-mine.sh` partitions on the first
(`awk '$1=="SHARED"||$1=="PEER"||$1=="UNKNOWN"'`) and prints the row verbatim, so the
disagreement reaches the reader as a list of people to ask.

## Environment

`experiments`, 2026-09-14. `scripts/fmt-mine.sh` (the gate's mandated step 1),
`scripts/file-provenance.py`.

## Root cause

**Not established, and deliberately left open.** What is measured is the *outcome*: the owner
list and the current dirty bytes disagree. Two candidates, neither confirmed:

1. **An inclusive window boundary.** `file-provenance.py` carries
   `last_commit_time()` — *"When this path was last committed. Writes older than that are
   baked into HEAD."* — and the printed window reads `writes at or after
   2026-09-13T09:25:37+00:00`, which is **exactly** `5d6b87b4`'s commit timestamp, the commit
   that shipped `f3c594ce`'s only work in that file. If the boundary is inclusive, the write
   that *produced* the last commit sits inside the window that exists to exclude it.
2. **A post-commit write since overwritten.** `f3c594ce` may have touched the file after
   `5d6b87b4` and had it replaced by the peer's rewrite, leaving a transcript record with no
   surviving bytes.

Separating them needs the per-write timestamps, which the scan summarises rather than prints.
**Naming one without that would close the inquiry** — the failure mode this corpus already
paid for on 2026-09-13, when a timezone offset was published as a clock disagreement and a
real filed bug nominated as its cause.

## Evidence

### E1 — the instrument answers a different question than the refusal needs

`file-provenance.py` answers *"who wrote this file recently"*. Step 1's refusal needs *"who
holds the unformatted bytes"*. Those diverge exactly when a writer's contribution has been
committed and someone else's has not — and the divergence is **invisible in the output**,
because both produce the same row shape.

The script already reasons about overlap: `SHARED` means *"this session wrote it AND so did a
peer"*, distinct from `PEER`. So the vocabulary for a two-party file exists. What has no
spelling is *wrote-and-committed* versus *holds-uncommitted*, which is the distinction that
would have let the row say **nothing to do** instead of naming an addressee.

### E2 — the guard is RIGHT about who and WRONG about what they can do

`f3c594ce`'s row was `SHARED`, so the script put it in `NOT_MINE` and refused. That is the
correct, safe direction — it did not let a session rewrite a peer's uncommitted Rust. Nothing
about the *refusal* is wrong. The defect is entirely in the **remedy text**, which asserts
performability of every named party.

This is `OB-20`, and specifically its measured ceiling rather than its opening claim:
*"the shape test buys ARRIVAL and never ANSWERABILITY — ask what the addressee can reply, not
only who they are."* That was earned on the pre-push foreign-session guard, which asked an
author a binary question whose true answer was neither branch. Same shape, different verb:
here the addressee arrives correctly and the **action** asked of them is not one they can
perform. Framing credited to `8bd791df`, who proposed it as an `OB-20` instance rather than a
new class — cheaper to file and easier to find later, and right.

### E3 — two independent parties, opposite sides of the same refusal

`8bd791df` hit it as a reader of the list and drew the wrong conclusion from it.
`f3c594ce` hit it as a named addressee and could not act. Neither could see the other's half:
the reader cannot tell which names are actionable, and the addressee does not see the message
that was composed from their row. That is the two-party structure `OB-1` § *the third
position* asks for — the party who can see it is not the party at risk.

## Hypotheses tried

**"The reader should just check `git diff` themselves."** True and insufficient. The refusal
exists precisely so a reader does not have to re-derive ownership, and `git diff` names no
session — that is why `file-provenance.py` was written. Pushing the check back to the reader
restores the gap the script closed.

**"`SHARED` already warns them."** The `SHARED` legend reads *"this session wrote it AND so
did a peer. Their bytes are still in there."* — addressed to the session whose row it is, and
silent on whether **this** session's bytes are still in there. That is the half that was
false here.

## Fix

Not implemented. This lands in `scripts/fmt-mine.sh` or `scripts/file-provenance.py`, both of
which every session's gate runs through, and a defect there blocks peers mid-work. Held for
an operator's go-ahead, and the shape should be argued before it is built:

- **Cheapest honest change, no new mechanism:** soften the remedy text from *"They can
  perform it"* to name the check — *"a named session may hold none of these bytes; `git diff
  -- <path>` says whose they are now"*. Costs nothing and cannot over-fire.
- **The mechanism, if it is worth it:** intersect the provenance verdict with the paths that
  actually differ from HEAD, and mark rows whose writer holds nothing. That is the
  distinction `SHARED` vs `PEER` already gestures at. Whether it earns the complexity is a
  judgement for whoever owns the script.

Note the ordering: the text repair is strictly safe, and the mechanism could over-fire on an
untracked file (no HEAD copy to diff against), which is the case the script is most careful
about elsewhere.

**Implemented 2026-09-14 — option 1 only (the cheapest, safe one).** The `[LIVE] peer`
remedy line no longer asserts the named session "can perform it"; it points the reader at
`git diff -- <path>` to check whose bytes are actually dirty before acting on the row. The
second option (intersecting provenance with a HEAD diff inside the script itself) is not
built — still a judgement call for whoever owns the script, per the note above about
untracked files.

## Tests added

One assertion, added to the existing PEER case in `tests/fmt-mine.sh` (the same block the
file's own note anticipated): `has "PEER remedy does not overclaim the peer can perform it"
"$OUT" "does not guarantee they hold anything to format"`. This is exactly the cheap
shape-assertion this section originally said a test here *could* buy — it reds on deletion
of the corrected wording, and does not attempt to assert performability, which stays
unverifiable per `OB-20`. `bash tests/fmt-mine.sh`: 43 passed, 0 failed.
## Workarounds

Read `git diff -- <path>` before believing an owner list is a list of people who can act. If
you are named and hold nothing, say so rather than staying silent: the reader has no way to
tell your row from an actionable one.

## Resume

`cluster/gate-keyed-on-unobservable-event` (`IC-2`), **retagged from `cluster/unclassified`
on 2026-09-14 after a sibling instance surfaced**, and the retag is a fit test rather than a
tidy-up.

`IC-2`'s claim is *"a gate whose condition is an event outside its observation boundary
substitutes a proxy … and the substitution fails silently, because a proxy returns a plausible
answer rather than an error."* That is this bug exactly: the condition the remedy needs is
**who holds the unformatted bytes**, the proxy is **who wrote this file since its last
commit**, and the failure returned a *name* rather than an error. **Crucially it commits to
nothing § Root cause declines to establish** — it is a claim about what the gate's inputs can
observe, not about where a window boundary was drawn, which is precisely why `IC-6` was
refused: *a selector matching too much* would assert the window is the defect.

**The sibling is what moved it.** `9d1e8696` filed the pre-edit `[cs-hint]` advisory firing on
a file the session itself had just written via `edit_code(action="rename")` — whose LSP rename
touches every referencing file — and tagged it `IC-2` with the framing *"dirtiness is the
proxy, authorship is the event, and nothing in the hook's inputs can answer the latter."*
Two hooks, two proxies, one unobservable event, filed the same day. Leaving this one in
`cluster/unclassified` would have split the pair across two classes and made the query that
finds them impossible.

**What the retag does NOT settle, and the reason the original filing is not simply wrong.**
This bug is still the third instance of a *different* shape with no class — a diagnostic
collapsing two states that take OPPOSITE repairs into one message, after
`120e3207d427ea14` and `a807b70cb7ee7340`. That reading is not refuted by `IC-2` fitting;
the two pick out different halves of one defect, and only one half has a class to go to. The
count argument is preserved on `cluster/unclassified`'s `**Members:**` line so that whoever
opens that class inherits three instances across three subsystems rather than re-deriving
them. A bug carries exactly one `cluster/` tag, so the choice was forced; it was not a
judgement that the other reading is wrong.
## References

- `scripts/fmt-mine.sh` — `refuse_not_mine`, and the `[LIVE] peer` / `SHARED` legend
- `scripts/file-provenance.py` — `last_commit_time`, and the window line it prints
- `docs/trackers/observer-blindness.md` — `OB-20`, and its measured answerability ceiling
- `docs/issues/archive/2026-09-09-the-documented-gates-first-command-rewrites-every-peers-uncommitted-rust.md` — why `fmt-mine.sh` exists at all

## Fix provenance

- **SHA:** `a427e90c` (experiments) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `e77c8b0ffa12f59dea75074270082b5724aa970a` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id.
