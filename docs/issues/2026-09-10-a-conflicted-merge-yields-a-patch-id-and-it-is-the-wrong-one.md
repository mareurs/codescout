---
id: f361f362d77940bb
kind: bug
status: fixed
title: 'BUG: a conflicted merge commit does yield a patch-id, and it is the wrong one'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-09-10
opened: 2026-09-10
severity: med
---

# BUG: a CONFLICTED merge commit does yield a patch-id, and it is the wrong one

## Summary
Three surfaces state, without qualification, that a merge commit has no patch-id and that the
prescribed pipeline therefore returns empty:

- `CLAUDE.md` § *Bug Tracking*: *"A merge commit has no patch-id — `git show <merge>` emits
  no diff, so the pipeline returns empty and exits `0`, giving you no error and no value."*
- `get_guide("tracker-conventions")` § *Bug files* (source: `src/prompts/guides/tracker-conventions.md`): *"`git show <merge>` emits the message
  with no diff … `git patch-id` given no patch prints **nothing and exits 0**."*
- `docs/RELEASE.md` § *Citing a fix*: same claim again. **This third surface was missed when
  the bug was filed** — the file said "two surfaces" on the strength of a grep that did not
  reach it. Corrected at fix time, and worth noting as the counting law biting inside a bug
  file about a wrong claim: the count was asserted from a partial search rather than derived
  from a complete one.

That is true of a **clean** merge and false of a **conflicted** one. `git show` on a merge
whose resolution differs from both parents emits a *combined* diff, so the pipeline returns
a well-formed 40-hex value.

## Symptom (Effect)
The documented failure mode is **an empty field you notice**. The actual failure mode for a
conflicted merge is **a plausible value you record** — and it is wrong in a way nothing
downstream can detect: it hashes only the resolution hunks (the changes belonging to
neither parent), not the change the merge delivers. It matches no constituent commit's
patch-id and would not survive a different-but-equally-valid resolution of the same
conflict.

A reader following the documented rule expects the pipeline to tell them, by silence, to
fall back to citing constituent commits. On a conflicted merge it does not tell them
anything; it answers.

## Reproduction
Measured 2026-09-10 on branch `doctor-per-project-isolation`, using the redirect form the
guide itself prescribes (a pipe here is blocked by Iron Law 3, and the guide warns the
`@cmd_*` buffer is capped, which would silently hash a prefix):

```
git show <sha> > /tmp/p.patch
git patch-id --stable < /tmp/p.patch
```

| commit | kind | `git show` bytes | result |
|---|---|---|---|
| `4485eeb0` | clean merge (ort, no conflicts) | 457 — log message only | **empty**; the documented behaviour |
| `8cf67de0` | merge WITH conflict resolution in two files | 32,634 — combined diff | **`9817e7c60e5199bb4d36d23c6d573575e3366b0c`** |

Both are on `experiments`; re-runnable at any time.

## Environment
`git` on Linux; no codescout involvement. The defect is in this repo's documented
procedure, not in git — git's behaviour here is correct and documented upstream.

## Root cause
The rule was generalised to *all merge commits* from an observation that holds for merges
git can resolve mechanically. A clean merge's tree is fully derivable from its parents, so
`git show` has nothing to print; a conflicted merge's tree is not, so `git show` prints the
part that is not — which is exactly what `git patch-id` will consume.

**The guide already anticipates the shape and stops one step short.** It says: *"Do not
reach for `git diff <first-parent>..<merge>` to manufacture one: it hashes, but not to the
object a cherry-pick would reproduce, so it is a plausible value in a field that means
something else."* That is the correct diagnosis of this exact hazard — and what neither
surface says is that on a conflicted merge you do not have to reach for anything. The
prescribed command does it for you.

## Evidence
See the Reproduction table. The 32,634-byte figure is `wc -c` on the redirected `git show`
output; the empty result is `git patch-id --stable` printing nothing and exiting `0`.

## Hypotheses tried
1. **Hypothesis:** `git show` on `8cf67de0` was emitting the message plus some
   non-diff noise that `patch-id` happened to hash.
   **Test:** compared byte counts against the clean merge `4485eeb0` (457 bytes, no diff,
   empty patch-id) from the same command shape.
   **Verdict:** rejected — 32,634 bytes against 457 is a diff, and the two merges differ
   in exactly one property: whether the resolution was mechanical.

## Fix

**FIXED 2026-09-10.** The operative instruction is unchanged on all three surfaces — never
record a merge's patch-id, cite the constituent commits — and what changed is the stated
REASON, because the reason is what a reader uses to decide whether the rule covers the
commit in front of them.

Each surface now says: a **clean** merge emits no diff and the pipeline returns empty and
exits 0; a merge that **resolved a conflict** emits a *combined* diff and returns a
well-formed value hashing only the resolution hunks, which matches no constituent commit
and does not survive a different-but-valid resolution of the same conflict. Both outcomes
carry the same instruction, so the rule no longer depends on what the reader observes —
which was the defect: *"it returns empty"* is falsifiable, and a reader who saw a value
concluded the rule did not apply to their case.

Each carries the measurement rather than the assertion: `4485eeb0` clean → 457 bytes, empty;
`8cf67de0` conflicted → 32,634 bytes, `9817e7c6…`. And each keeps the pre-existing warning
against manufacturing one with `git diff <first-parent>..<merge>`, now sitting beside the
case where `git show` manufactures it for you unasked.

**Surfaces changed:** `CLAUDE.md` § *Bug Tracking*; `docs/RELEASE.md` § *Citing a fix*;
`src/prompts/guides/tracker-conventions.md` § *Bug files*.

**Deliberately NOT changed:**
`docs/issues/archive/2026-08-30-patch-id-citation-is-unavailable-for-a-merge-commit.md`,
which is where the claim originated. It is an archived historical snapshot, and
`get_guide("tracker-conventions")`'s own rule is to leave `docs/issues/archive/**` alone —
rewriting it would falsify the record of what was believed then to satisfy a linter that is
already ignoring it.
## Tests added

None, and the reason is structural rather than an omission. All three surfaces are prose,
and no test in this repo is positioned to gate a claim about tool behaviour —
`audit_doc_refs` checks citations against the filesystem, not assertions against `git`. The
honest gate would be a fixture repo carrying one clean and one conflicted merge, asserting
the two outcomes; that is more machinery than the claim is worth, and saying so is better
than implying coverage exists.

What the change IS gated by, incidentally: `CLAUDE.md` and the guide are both pinned
surfaces — `claude_md_contains_no_deprecated_tool_names` and the prompt-surface tests run
over them — so a malformed edit reds, even though a *false* one would not. The
reproduction table in this file is the durable artifact; it is re-runnable in two commands
against two commits that are both on `experiments`.
## Workarounds
Never record a merge commit's patch-id, whatever the pipeline prints. If you need a
durable pointer to work delivered by a merge, cite the merged branch's constituent commits
by SHA + patch-id — which is what the rule already says.

## Resume

Nothing owed. All three surfaces corrected 2026-09-10; the archived origin file left as the
historical record it is.
## Cluster — deliberately `unclassified`, with the reasoning
The two nearest classes were read in full and neither fits; forcing one would be the
mistake `docs/trackers/issue-clusters.md` warns about in its own Index preamble.

- **`IC-11 doc-contradicted-by-code`** requires that the statement was *true when written*
  and that the code later gained or lost the capability, and states *"unlike a wrong
  statement, this defect has no authoring error to find."* This one does have one: the
  claim was never true of conflicted merges. Git did not change.
- **`IC-18 selector-narrower-than-its-population`** is the mirror of what happened. Here
  the rule's selector ("is it a merge commit?") is **wider** than the population its
  reasoning holds for ("is it a clean merge?"). `IC-6`'s own ledger note rejects a tag on
  exactly that basis — that class is a guard covering LESS than its name, and the
  covering-MORE case does not belong there.

If a third instance of *"a universal generalised from one subcase, whose other subcase
returns a plausible value"* turns up, that is the class — not either of the above.

## References
- `CLAUDE.md` § *Bug Tracking*, § *Git Workflow* (the two statements)
- `get_guide("tracker-conventions")` § *Bug files* (the third, and the `git diff
  <first-parent>..<merge>` warning that gets it almost right)
- `docs/issues/archive/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md`
  § *Fix* — where the measurement was first recorded, in the archive commit that needed it


## Fix provenance

- **SHA:** `aeab4ee2` (`aeab4ee22a6ea6dde9584188e7bc8dfd4b5e078e`, on `origin/experiments`) —
  positional; does not survive a rebase of `experiments`.
- **patch-id:** `a383c873c73a6012482076d50ab381c68e39d23f` — content hash of the diff; survives
  rebase and cherry-pick.

Derived 2026-09-11, not recorded at fix time. `git log -S'8cf67de0' -- CLAUDE.md` identifies it
uniquely, and the commit touches exactly the three surfaces § *Fix* names — `CLAUDE.md`,
`docs/RELEASE.md`, `src/prompts/guides/tracker-conventions.md` — plus this bug file.

**Read the patch-id with one caveat: `aeab4ee2` carries TWO fixes.** Its subject is *fix(gates):
git ls-files counts index stages; and a merge patch-id that is not empty*, and the diff also
changes `tests/result_caps.rs` and a second bug file for the unrelated `ls-files` index-stage
defect. A patch-id is a content hash of the **whole** diff, so this one identifies the combined
commit and not this bug's half of it. It recovers the commit, which is what the anchor is for;
it is not evidence about which half shipped, and a future reader comparing it against a
cherry-pick of only one half will not match.

That is not a defect in the anchor — it is the ordinary consequence of citing a
multiple-purpose commit, and it is recorded here rather than left for someone to rediscover
when the hash fails to match. The alternative, splitting the citation, is unavailable after the
fact.
