---
id: 53e35aaefb9f7c71
kind: bug
status: fixed
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
closed: 2026-08-19
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

**Nine live terminal records declare no anchor, and seven of them read as though they do.**
The figures below are the shipped check's, not a hand count — an earlier revision of this
file said *three*, from inspecting three files and reporting the sample's count as the
population's.

| file | status | commit-like hashes in prose |
|---|---|---:|
| `2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md` | mitigated | 6 |
| `2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md` | mitigated | 6 |
| `2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md` | mitigated | 4 |
| `2026-08-08-workspace-toml-mis-rooted-declared-sibling-repos-as-projects.md` | mitigated | 3 |
| `2026-08-18-companion-test-suite-stamped-live-rendezvous-slots.md` | fixed | 2 |
| `2026-08-18-spawned-binary-test-points-guide-gc-at-real-state-dir.md` | fixed | 2 |
| `2026-08-07-edit-code-remove-ast-repair-over-deletes.md` | fixed | 1 |
| `2026-08-18-no-check-detects-a-params-row-stale-relative-to-its-body.md` | mitigated | 0 |
| `2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md` | fixed | 0 |

The hashes are real commits — environments, siblings, quoted work — just not the fix. Only
two records are plainly hash-free, so the *misleading* shape is the rule and the honest
blank is the exception. That inverts the fix: the finding's main job is not to report
absence but to say **the hashes you can see are not the anchor**.

One entry is worth naming on its own. `2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md`
is the record that established patch-id as the durable anchor, and it carries six loose
hashes and no anchor of its own.
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

## Fix

**Shipped** as `terminal_status_without_fix_anchor` in `src/librarian/tools/doctor.rs`,
plus a parser change the check depended on.

**The check.** Live bug rows with status `fixed` or `mitigated`, scoped to the active
project's git root, that declare no `## Fix provenance` pointer. Live-only is a measurement,
not a preference: 297 of 355 archived files declare none, and reporting those would bury the
nine live records where the remedy is still owed under a pile the guide itself calls stale
instructions. `wontfix` is excluded — nothing was fixed, so no commit exists to point at.

**A way to say nothing is owed.** `no_fix_commit:` in frontmatter discharges a record whose
mitigation was a doc note or a workaround. Without it the check would nag those forever and
get silenced wholesale. Same shape as `unverified:`: absence means owed, presence discharges,
empty counts as absent because presence is what a query reads.

**The decoy heuristic**, which is what makes the finding say more than *"no SHA found"*:
backtick-delimited tokens of 7–12 hex characters containing at least one letter. Narrow on
purpose — 16 hex is a catalog artifact id and 40 is a patch-id, so a file's own id would
otherwise be reported as a stray commit reference. It asserts only that hashes are present
and none is declared as the fix; it never names one as the fix, which is the confident wrong
answer `archived_fix_sha_unresolvable` documents at length.

**`structured_fix_pointer` → `structured_fix_pointers`.** Not tidying — a prerequisite. The
singular parser could hold one `(sha, patch_id)` pair, so the author of a genuinely
two-commit fix (`2026-08-19-entry-without-definition-…`, archived hours earlier) wrote the
anchors as a **table**, which reads well and is invisible to a parser keyed on
`- **SHA:**` lines. Both anchors it recorded were therefore verified by nothing. Each
patch-id now binds to the SHA above it, `archived_fix_sha_unresolvable` verifies every pair
rather than the first (`scanned` 54 → 63), and that file is converted to the bullet form.

A shape that makes the honest case unrepresentable gets worked around, and the workaround is
the defect.

### Options considered and not taken

- **Corpus-wide scope.** Rejected on the 297/355 measurement above.
- **A general hex sweep to identify the fix.** Rejected for the reason
  `archived_fix_sha_unresolvable` already documents: sampled prose includes a *reproduction*
  commit and a suspect the file explicitly exonerates. Reporting either as the fix is worse
  than reporting nothing.
- **A `fix=` autorepair.** Rejected — recovering a fix SHA is research, and a wrong anchor
  is worse than an absent one.
## References

- `get_guide("tracker-conventions")` § *Bug files* — the SHA + patch-id rule and its measurements
- `src/librarian/tools/doctor.rs` — `archived_fix_sha_unresolvable`, `terminal_status_with_caveat`
- `docs/issues/2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md` — the sibling
  that established patch-id as the durable anchor; this file is its unmeasured half
- `docs/issues/2026-08-18-no-check-detects-a-params-row-stale-relative-to-its-body.md` — same
  shape (a real drift with no detector), and the precedent for filing one as a bug


## Tests added

Eight, and five applied mutations with the observed result recorded rather than a coverage
argument:

| # | Mutation | Observed |
|---|---|---|
| M1 | archive-component skip disabled | archived-scope test FAILS, alone |
| M2 | hash length bound widened to `7..=40` | heuristic test FAILS — the artifact id and the patch-id are both swept in |
| M3 | patch-id binds to the first SHA, not the nearest | 2 tests FAIL |
| M4 | verify only the first pointer | multi-pointer test FAILS |
| M5 | empty `no_fix_commit` also discharges | discharge test FAILS |

Zero survivors. **M3 is the one the pairing test exists for**: a mis-bound pair does not
merely lose information, it makes the finding report *"No patch-id was recorded"* about a
commit that has one — a confidently wrong remedy, which is strictly worse than a missing one.

Two controls are load-bearing. `terminal_status_without_fix_anchor_fires_only_where_no_pointer_is_declared`
seeds an anchored file alongside the bare one, so a check that fired on every terminal record
would not pass. And the decoy test asserts the hash-free finding does **not** borrow the
stronger "READS as anchored" wording, because the two findings ask a reader to do different
things.

## Fix provenance

- **SHA:** `375225cc` (`experiments`) — *feat(doctor): terminal_status_without_fix_anchor,
  and a plural fix-pointer parser*. Positional; does not survive a rebase of `experiments`.
- **patch-id:** `ca1fca68b7c3be51d51d816716d4243079773e90` — content hash of the diff;
  survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes — Iron
Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep ca1fca68b7c3 /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several branches
(cherry-pick) and any of them is the fix.

**The nine records this check reports are NOT repaired by it, and that is deliberate.**
Recovering each one's fix SHA is research per record. The check makes the work visible and
queryable; doing it is a separate, per-record task.
