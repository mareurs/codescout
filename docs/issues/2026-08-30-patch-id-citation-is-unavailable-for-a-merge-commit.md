---
status: open
opened: 2026-08-30
closed:
severity: medium
owner: marius
related: []
tags:
  - conventions
  - citation
  - git
  - silent-absence
  - claude-md-drift
kind: bug
---

# BUG: the SHA + patch-id citation rule has no answer for a merge commit, and `git patch-id` reports that by printing nothing

## Summary

CLAUDE.md, `docs/RELEASE.md` and `get_guide("tracker-conventions")` all require a
fix to be cited by **SHA *and* patch-id**, because `experiments` is rebased after
every ship and a bare SHA dies with the rebase. The patch-id is the durable half.

For a **merge commit** that half does not exist. `git show <merge>` emits the
commit message with no diff, so the prescribed pipeline
`git show <sha> | git patch-id --stable` produces **empty output and exit 0**.
The rule's own remedy is unavailable exactly where the rule is most needed, and
the tool reports it by staying silent rather than by failing.

## Symptom (Effect)

```
$ git show 03b3fb5c > /tmp/m.patch; wc -c /tmp/m.patch
2511 /tmp/m.patch                       # message only — no diff

$ git patch-id --stable < /tmp/m.patch
                                        # nothing. exit 0.

$ git show 304d530f > /tmp/o.patch; git patch-id --stable < /tmp/o.patch
1e4b50f70dc3d81ca79cdf2d8941bc76104ac96c 304d530f06613e795eecca2dc5cee84871ae037a
```

`03b3fb5c` is a real merge commit on `experiments` (operator-rules Phase 2, 17
files, 1750 insertions). `304d530f` is an ordinary commit, shown as the control.

**Exit 0 is the defect's shape.** A session following the documented recipe gets
no error and no value. The plausible outcomes are recording an empty patch-id,
recording the SHA twice, or silently dropping the field — none of which announce
themselves, and all of which look like compliance.

## Reproduction

```
git show <any-merge-commit> | git patch-id --stable
```

Empty. Exit 0.

## Environment

Any git. `git show` on a merge commit suppresses the diff by default (combined
diffs need `-m`, `-c`, or `--first-parent`), so `patch-id` receives no patch to
hash. Not version- or platform-specific.

## Root cause

Two correct behaviours composing into a gap:

1. `git show <merge>` prints no diff by default — deliberate, since a merge has
   more than one parent and "the diff" is ambiguous.
2. `git patch-id` hashes a patch. Given none, it emits nothing and exits 0 rather
   than erroring, because an empty stream is not malformed input.

Neither is a bug. The **convention** is what has the gap: it prescribes a single
pipeline as though it were total, and it is not.

**measured 2026-08-30** on `03b3fb5c` with `304d530f` as a control — see § Symptom.

## Evidence

### Why this is not theoretical for this repo

`experiments` is rebased after every ship. That is the stated reason the patch-id
requirement exists at all — a positional SHA is orphaned and eventually
garbage-collected, while a content hash survives rebase *and* cherry-pick.

A fix that arrives via a merge commit therefore has **only** the perishable half
of the citation. `03b3fb5c` is such a case today: it is how operator-rules Phase 2
entered the branch, and no patch-id can be recorded for it.

### The gap is narrower than it first looks, which is also the fix

The merge's *constituent* commits each have ordinary patch-ids that survive
normally. Nothing is unrecoverable — what is missing is a citation for the merge
**as an event**. So the remedy is a rule, not a tool change: cite the commits, not
the merge.

## Hypotheses tried

1. **Hypothesis:** `git patch-id` failed or was mis-invoked.
   **Test:** ran the identical pipeline on an ordinary commit in the same repo,
   same session.
   **Verdict:** rejected — the control returns a patch-id. The input is the
   difference, not the invocation.

## Fix

Not applied — this is a documentation/convention change on a surface owned by the
user (CLAUDE.md, `docs/RELEASE.md`, `get_guide("tracker-conventions")`), so it is
raised rather than edited.

Proposed wording, one sentence at each of the three sites:

> A **merge commit has no patch-id** — `git show <merge>` emits no diff, so the
> pipeline returns empty and exits 0. Cite the merged branch's constituent
> commits by SHA + patch-id instead; those survive rebase normally. Never record
> an empty patch-id field.

Worth considering alongside it: `docs/RELEASE.md`'s ship sequence could name the
check, since an empty patch-id is invisible at review time and looks exactly like
a filled one that nobody read.

## Tests added

None — the defect is in prose, not in code, and nothing in the repo parses
patch-id citations today. If that changes (a lint over `docs/issues/` frontmatter,
say), an empty-patch-id assertion belongs in it.

## Workarounds

Cite the constituent commits. For `03b3fb5c` that means the ten commits of
`sdd/operator-rules-phase-2`, each of which has an ordinary patch-id. Where a
single anchor is wanted, `git patch-id` over `git diff <first-parent>..<merge>`
produces a hash — but it is not the same object a cherry-pick would reproduce, so
it should not be recorded in the field the convention means.

## Resume

Decide whether the one-sentence addition goes in all three surfaces or only in
`get_guide("tracker-conventions")` (the one the other two defer to). Then check
whether any already-archived bug file cites a merge commit and carries an empty or
duplicated patch-id — `grep -rn 'patch-id' docs/issues/archive/` and compare each
against `git cat-file -t`.

## References

- `03b3fb5c` — the merge commit this was found on.
- `304d530f` — the control.
- `get_guide("tracker-conventions")` § Bug files — the rule.
- `docs/RELEASE.md` — SHA + patch-id citation rule.
