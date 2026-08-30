---
kind: bug
status: fixed
tags:
- conventions
- citation
- git
- silent-absence
- claude-md-drift
closed: 2026-08-30
opened: 2026-08-30
owner: marius
related: []
severity: medium
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

## Fix provenance

**Applied 2026-08-30 on `experiments`.**

- SHA: `c8b952e7` (`experiments` — orphans on the next rebase)
- patch-id: `89419e042f043d8072772d3e3e0b13e6e606f1ab`

This file argued the fix was on a user-owned surface and raised it rather than
editing. The operator authorised all three, so all three carry it, sized to each:
`src/prompts/guides/tracker-conventions.md` has the full rule (the other two defer to
it), `docs/RELEASE.md` has it with the corpus measurement, `CLAUDE.md` has one clause
and a pointer.

**Two things were added beyond the proposed wording**, both from the audit above:

- **The `git diff <first-parent>..<merge>` trap is named explicitly.** This file's
  § *Workarounds* already knew it produces a hash that is not the object a
  cherry-pick reproduces — but that knowledge sat in a workarounds section, which is
  where a reader goes *after* deciding what to do. Moved to the rule itself, because
  a plausible value in a field that means something else is worse than an empty one.
- **The worked example is cited.**
  `docs/issues/archive/2026-08-13-url-silently-overrides-local-dir-model.md` wrote the
  recommended form before the rule existed. That reframes the change from inventing a
  convention to codifying one, which is the easier thing to ask a reader to accept.

**Gate:** the *new* four commands from `4c88e129` — `cargo test --workspace` replaced
`cargo test` while this bug was open, so the run that gated it is not the one this
file was filed under. fmt, clippy `--workspace --all-targets --features local-embed`,
`cargo test --workspace` **4930/0**, lean **3385/0**.

**One thing this does not do.** `src/prompts/guides/tracker-conventions.md` is
`include_str!`'d into the binary, so `get_guide("tracker-conventions")` serves the old
text until a release rebuild. The file on disk is correct now; the *tool* is correct
after `cargo rb`.

**No lint enforces this.** § *Tests added* explains why — nothing in the repo parses
patch-id citations. `doctor`'s `archived_fix_sha_unresolvable` verifies SHA/patch-id
pairs that exist; it cannot see a merge cited without one. If a lint over
`docs/issues/` frontmatter ever lands, an empty-patch-id assertion belongs in it, and
so does a `git cat-file -t` check on the patch-id field — the audit above shows that
second one is decisive and cheap.

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

## Audit of the existing corpus — done 2026-08-30, nothing to clean up

The Resume's second item ran. **No existing citation is corrupted**, so the remaining
work is purely the prose addition.

**1 — Does any archived bug file cite a merge commit as its fix?** No. The repo has
**19** merge commits; searched all of `docs/` (hidden included) for each at 7-char
prefix, which matches any longer form. Three appear under `docs/issues/archive/`, and
all three are sound:

| file | how the merge SHA is used |
|---|---|
| `2026-08-08-edit-file-out-of-project-ack-handle-unresolvable.md` | environment marker — *"`experiments` @ `8f724171`"*; separately names `9cbe4002` as the binary's build commit |
| `2026-08-08-doctor-outside-roots-sample-is-unranked-and-unreachable.md` | environment marker — *"branch `experiments` at `9be2ede4`"* |
| `2026-08-13-url-silently-overrides-local-dir-model.md` | **already the recommended pattern**: *"Fixed at `38e0980b`, merged to `experiments` at `e6484b16`"* |

The third is worth noting for the fix: **the repo already contains a worked instance
of the proposed rule**, written before the rule existed. The wording below codifies
existing good practice rather than inventing a convention.

**2 — Any patch-id that is really a SHA?** No. Extracted **163** distinct cited
patch-id values from `docs/issues/` and tested each with `git cat-file -t`. **Zero
resolve as a git object** — which is the decisive form of this check, since a patch-id
is a content hash and must *not* be an object, while a duplicated SHA would be.

**3 — Any empty patch-id passing as compliance?** No. Every absence in the corpus
**states its reason**: `patch-id: N/A (not fixed)`, `not computable from this repo
(fix lives in codescout-companion)`, `SHA / patch-id: N/A — .env is gitignored`,
`no_fix_commit: Machine-local config change`. That is the *opposite* of the failure
this file predicted — "recording an empty patch-id … looks like compliance" — and it
means the convention's authors have consistently written the reason rather than the
blank.

**Scope of these negatives**, since two of the three are zeros: search 1 covers all of
`docs/**` including hidden files; searches 2 and 3 cover `docs/issues/**` only, which
is where fix citations live. Not covered: citations in commit messages, and any file
outside `docs/`.

**One badly-built instrument, recorded because it failed the good way.** The first
attempt at check 3 used `patch-id[^0-9a-f]{0,12}$`, which matched the boilerplate
recovery recipe `git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt` present
in ~40 archived files — 160 hits, all false. It failed *loudly*, which is the good
direction: an over-broad predicate floods, an over-narrow one returns a clean zero and
is believed. See `docs/PROBES.md` rule 4.

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
