---
id: f361f362d77940bb
kind: bug
status: open
title: 'BUG: a conflicted merge commit does yield a patch-id, and it is the wrong one'
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-09-10
severity: med
---

# BUG: a CONFLICTED merge commit does yield a patch-id, and it is the wrong one

## Summary
Two surfaces state, without qualification, that a merge commit has no patch-id and that the
prescribed pipeline therefore returns empty:

- `CLAUDE.md` § *Bug Tracking*: *"A merge commit has no patch-id — `git show <merge>` emits
  no diff, so the pipeline returns empty and exits `0`, giving you no error and no value."*
- `get_guide("tracker-conventions")` § *Bug files*: *"`git show <merge>` emits the message
  with no diff … `git patch-id` given no patch prints **nothing and exits 0**."*

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
*Not fixed by this bug file.* The remedy is a wording change on two surfaces, and the
useful form is narrower than "add the caveat":

- The **operative instruction is unchanged and still right** — never record a merge's
  patch-id; cite the constituent commits. Do not weaken that.
- What needs correcting is the stated **reason**, because the reason is what a reader uses
  to decide whether the rule applies to the commit in front of them. "It returns empty" is
  a falsifiable premise, and a reader who observes a value concludes the rule does not
  apply to their case.

So: state that a clean merge returns empty and a conflicted merge returns a value over the
combined diff, and that **both** mean "cite the constituents". That makes the rule
independent of what the reader observes.

## Tests added
None, and this is a wording defect in prose that no test in this repo is positioned to
gate — `audit_doc_refs` checks citations against the filesystem, not claims against tool
behaviour. Noted rather than excused: the honest gate would be a fixture repo with one
clean and one conflicted merge asserting the two outcomes, which is more machinery than
the claim is worth. The reproduction above is the durable artifact.

## Workarounds
Never record a merge commit's patch-id, whatever the pipeline prints. If you need a
durable pointer to work delivered by a merge, cite the merged branch's constituent commits
by SHA + patch-id — which is what the rule already says.

## Resume
Correct the reason clause on both surfaces (`CLAUDE.md` § *Bug Tracking* and § *Git
Workflow*; `get_guide("tracker-conventions")` § *Bug files*, whose text is
`src/prompts/guides/` or the guide source). Keep the instruction, replace the premise.

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
