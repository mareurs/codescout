---
id: '1701c47a7b15b93d'
kind: bug
status: open
title: 'BUG: no check detects a terminal bug file with no recoverable fix anchor, and an observation SHA reads as one'
owners:
- marius
tags:
- librarian
- doctor
- trackers
- provenance
- record-legibility
topic: record-legibility
closed: ''
opened: 2026-08-19
owner: marius
related:
- '9e649674c95cd7bd'
- '403e3fad0356f171'
severity: med
---

> **Status: open, severity med.** Nothing breaks at runtime. What degrades is the ability
> to find the change that closed a bug — and the degradation is invisible, because a file
> with no fix anchor usually still contains a hex string.

## Summary

`get_guide("tracker-conventions")` requires an archived bug file to carry its fix **SHA and
patch-id**, on measured grounds: 10 of 63 archived files had already lost their SHA to a
rebase of `experiments`, and subject-keyword recovery returned 2–153 ambiguous candidates.

Nothing checks it. `doctor` gained `archived_fix_sha_unresolvable` (CAP-7), which verifies a
SHA that **is** recorded still resolves. It cannot fire on the worse case: a terminal record
with **no** fix SHA at all, or one whose only hex string is the commit the bug was *observed*
at.

The observation-SHA case is the dangerous one. A reader scanning for provenance sees a
plausible short hash in an `Environment` line and stops looking. The record reads as
anchored and is not.

## Symptom (Effect)

Three live `fixed` records, none of which names the commit that fixed it:

| file | only hex present | what it actually is |
|---|---|---|
| `2026-08-07-edit-code-remove-ast-repair-over-deletes.md` | `6ce49487` | *"codescout at `6ce49487` (experiments), rust-analyzer from pinned 1.97.1"* — the environment |
| `2026-08-18-companion-test-suite-stamped-live-rendezvous-slots.md` | `feb845aa` | *"Companion repo at `95e8c85`, codescout at `feb845aa`"* — the environment. The real fix is cross-repo, in the companion |
| `2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md` | `c7bdfd22` | *"codescout `experiments` @ `c7bdfd22`. Measured 2026-08-18"* — the environment, though the same commit did adjacent work, which is what makes this one genuinely ambiguous rather than merely absent |

None has a `## Fix provenance` section. The verified claim is narrow and checkable: **every
hex string in these files appears in observation context, and none is labelled as the fix.**

## Reproduction

Measured 2026-08-19 over the 25 live files in `docs/issues/` (excluding `archive/`):

```
for f in 2*.md; do
  st=$(awk -F': ' '/^status:/{print $2; exit}' "$f")
  printf '%-12s pid=%s %s\n' "$st" "$(grep -c 'patch-id' "$f")" "$f"
done
```

| bucket | n |
|---|---:|
| `open` | 7 |
| `zombie` (no fix, none owed) | 4 |
| terminal (`fixed` / `mitigated` / `wontfix`) | 14 |
| — of those, carrying a patch-id | 6 (two added 2026-08-19) |
| — of those, carrying none | 8 |
| — of those 8, `wontfix` and correctly anchor-free | 1 |

**7 live terminal records have no durable fix anchor.**

## Root cause

Two separate gaps, and neither is covered:

1. **No check.** `archived_fix_sha_unresolvable` validates a recorded SHA. There is no
   check for *absence*, and absence is the more common failure — the rule requiring the
   pair landed 2026-08-19, so every record predating it is unanchored by default.
2. **No way to declare "correctly anchor-free."** A `mitigated` record whose mitigation was
   a doc note or a workaround has no commit and owes no anchor. A check that cannot tell
   that case from a genuine omission would emit noise on every one of them — which is what
   `unverified:` solved for the analogous status question, and the same shape applies here.

## Evidence

The class was found by repairing one instance and then enumerating, per the
reconnaissance rule that a defect *class* must be swept before the fix is called done.
The triggering instance is instructive: `2026-07-28-edit-code-target-base-from-stale-lsp-range.md`
carried, in its `## Resume`, *"Also outstanding: master-side SHA after cherry-pick. The SHA
here is an `experiments` SHA and orphans on rebase."*

That sentence was wrong twice. The practice it named had been retired in favour of a
patch-id recorded once — and **no fix SHA was ever written into the file at all**, so it
worried about the durability of a pointer that did not exist. The real fix (`1e7722a0`,
patch-id `06f6ef543adeec3b67370c8a83e945034e8121fb`) was recovered with
`git log -S 'anchor_indent' -- src/symbol/edit.rs` and confirmed against the commit's stat.

Recovery cost scales badly. That one was cheap because the fix introduced a distinctively
named symbol. Where the fix is a behaviour change with no new identifier, `git log -S` has
nothing to search for, and the guide's own measurement of the fallback — subject-keyword
search — is 2–153 candidates.

## Fix ideas

1. **A `doctor` check: `terminal_status_without_fix_anchor`.** Scope: `kind=bug`, terminal
   status, no `patch-id` in the body. Report-only, no `fix=` — recovering a fix SHA is
   research, not repair, and a wrong anchor is worse than none.
2. **Pair it with a declaration, so the report is actionable rather than chronic.** Reuse
   the `unverified:` precedent: a frontmatter key (`no_fix_commit: <why>`) that says the
   mitigation had no commit. Absence means an anchor is owed; presence discharges it. As
   with `unverified:`, **never add it empty** — presence is what a query reads.
3. **Warn on observation-SHA-only records.** A hex string present with no `## Fix
   provenance` section is exactly the case a reader mis-reads. Naming it explicitly is the
   difference between this check and a plain "no SHA" grep.

Sequenced after `terminal_status_with_caveat` and `archived_fix_sha_unresolvable`, this
completes the Layer-2 set: those two make a *stated* caveat and a *broken* anchor findable;
this one makes a *missing* anchor findable.

## References

- `get_guide("tracker-conventions")` § *Bug files* — the SHA + patch-id rule and its measurements
- `src/librarian/tools/doctor.rs` — `archived_fix_sha_unresolvable`, `terminal_status_with_caveat`
- `docs/issues/2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md` — the sibling
  that established patch-id as the durable anchor; this file is its unmeasured half
- `docs/issues/2026-08-18-no-check-detects-a-params-row-stale-relative-to-its-body.md` — same
  shape (a real drift with no detector), and the precedent for filing one as a bug

