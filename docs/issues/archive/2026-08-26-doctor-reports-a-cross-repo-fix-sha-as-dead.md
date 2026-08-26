---
id: b1884398637c28c2
kind: bug
status: fixed
title: doctor reports a cross-repo `<repo>:<sha>` fix pointer as a dead SHA
tags:
- librarian
- doctor
- cross-repo
closed: 2026-08-26
opened: 2026-08-26
owner: marius
related:
- docs/issues/2026-08-18-companion-test-suite-stamped-live-rendezvous-slots.md
severity: low
---

# BUG: doctor reports a cross-repo `<repo>:<sha>` fix pointer as a dead SHA

## Summary

`scan_archived_fix_sha_unresolvable` resolves every declared fix SHA against the
active project's git root. CLAUDE.md documents a `<repo>:<sha>` prefix for citing a
fix that lives in a sibling repo, and the check does not recognise it — so a pointer
that is perfectly good, in the repo its own prefix names, is reported as an object
that "no longer names an object in this repo".

Found while working the doctor's own worklist: it was the single
`archived_fix_sha_unresolvable` finding in the 2026-08-26 run, and it is a false
positive.

## Symptom (Effect)

```
[archived_fix_sha_unresolvable] docs/issues/2026-08-18-companion-test-suite-stamped-live-rendezvous-slots.md
  declared fix SHA `codescout-companion:b8ffa8b` no longer names an object in this repo…
  No patch-id was recorded next to it, so there is no content-addressed way back…
```

Both sentences are wrong about this record. The commit resolves:

```
git -C ../claude-plugins/codescout-companion cat-file -t b8ffa8b   → commit
b8ffa8b 2026-08-18 fix(session-start): isolate rendezvous tests from real state, close review gaps
```

And the remedy it offers is unactionable here by construction — a patch-id computed
in this repo cannot address a commit in another one. The file already says so on the
line directly below the SHA: *"patch-id: not computable from this repo (fix lives in
codescout-companion)"*.

The cost is not the one finding. It is that a false positive in a rot-detector
teaches the reader to discount it, and this check's whole value is that nothing else
re-reads `archive/`.

## Reproduction

Measured 2026-08-26 on `experiments`:

```
librarian(action="doctor")
  → violations[] contains one archived_fix_sha_unresolvable naming
    `codescout-companion:b8ffa8b`
```

The declaration it reads (`docs/issues/2026-08-18-…:149`):

```
- **SHA:** `codescout-companion:b8ffa8b`
- **patch-id:** not computable from this repo (fix lives in codescout-companion)
```

## Root cause

`structured_fix_pointers` returns whatever sits inside the first backticks after
`- **SHA:**`, verbatim — the `<repo>:` prefix included. `scan_archived_fix_sha_unresolvable`
then passes that string straight to `repo.revparse_single`.

Git reads `a:b` as *"the object at path `b` in tree-ish `a`"*, so
`codescout-companion:b8ffa8b` is parsed as a path lookup inside a tree-ish named
`codescout-companion`, fails, and is reported as a dead object. Nothing in the path
is aware that the token names another repository.

The check's doc comment already states the governing principle — *"a SHA only means
anything inside the repo that minted it"* — and enforces it for rows whose **file**
sits outside the git root. The gap is the mirror case: a file inside this repo
declaring a SHA that belongs to another one.

## Fix

Skip pointers carrying a `<repo>:` prefix rather than resolving them, and count them
separately in `catalog_health` so a clean result still cannot be read as "every
declared fix resolves".

Skipping, not resolving-elsewhere: the check is deliberately scoped to one repo, has
no map from prefix to checkout path, and a sibling repo may not be present on the
machine at all. Reporting honestly that it was not checked is the accurate answer;
guessing a path would trade a false positive for an unbounded one.

## Tests added

`a_cross_repo_prefixed_pointer_is_skipped_rather_than_reported_dead`
(`src/librarian/tools/doctor.rs`), verified red first — it reproduced the real finding
verbatim, including the patch-id remedy that cannot work across repos.

It carries a genuinely dead LOCAL pointer as a control, so "skip everything" cannot
pass and silently disable the check, and asserts `skipped_cross_repo_pointer` is
counted rather than swallowed.

## Fix provenance

- **SHA:** `7b5325a9`
- **patch-id:** `26ecd2ee0c70fbdef6b4a44bfc10d0c3ebc41714`

Verified on the real catalog through the CLI doctor, not only in unit tests: the
finding is gone, `unresolvable` 1 → 0, and total violations 69 → 68.
## Workarounds

Read the pointer. The prefix names the repo, and `git -C <that repo> cat-file -t <sha>`
settles it in one command.

## References

- `docs/issues/2026-08-18-companion-test-suite-stamped-live-rendezvous-slots.md` — the record mis-flagged
- CLAUDE.md § Git Workflow — the `<repo>:<sha>` citation convention this check does not know about
