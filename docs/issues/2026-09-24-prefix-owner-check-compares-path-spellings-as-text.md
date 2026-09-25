---
id: c6cff39df3eed0c6
kind: bug
status: fixed
title: 'BUG: the prefix-owner check compares path spellings as text, so on Windows a ledger re-declaring its own prefix is refused as a conflict'
tags:
- cluster/unclassified
closed: 2026-09-25
opened: 2026-09-24
owner: marius
severity: medium
unverified: The original red was on the Windows-gnu wine lane, which has not yet run 1df22edc; the Linux-reproducible test and its mutation are the evidence so far. Archive once a CI run on a commit containing 1df22edc shows a_ledger_re_declaring_its_own_prefix_is_not_a_conflict green on that lane.
---

# BUG: the prefix-owner check compares path spellings as text, so on Windows a ledger re-declaring its own prefix is refused as a conflict

## Summary

`refuse_taken_prefixes` (`src/librarian/catalog/augmentation.rs`) decides whether the declaring ledger IS the owner with a raw string comparison: `o.as_str() != me`, where `me = declaring.to_string_lossy()`. The owners come from the catalog's `abs_path`, which is stored forward-slash-normalized. On Windows, `declaring` holds the native spelling. The same file then fails to equal itself, and the owner is refused as a conflict with its own declaration.

## Symptom (Effect)

On Windows, any `doc(update)` to a ledger that declares an `entry_prefix` — or `rekey_prefix` back onto one — is refused with *"entry prefix already taken in this repository — `R` is owned by <the ledger itself>"*. `doc(update)` re-sends the whole `extra` value, so it is refused even when the edit is to an unrelated key. That is exactly the case the test names.


**Correction 2026-09-25, same session: the paragraph above overstates the production reach.** Each of the three production callers was traced to what it passes as `declaring`:

- `doc(update)` passes `row.abs_path` (`src/librarian/tools/update.rs:520`), which is the stored string itself.
- `rekey_prefix` passes the stored `abs_path` (`src/librarian/catalog/rekey.rs:568-580`).
- `doc(create)` passes the native-joined `full` (`src/librarian/tools/create.rs:373`). A create has no row of its own yet, so no comparison against itself can happen.

So as far as the code shows, no production call is refused on Windows today. What was red is the test, which passes a native-joined path the way `create` does. The defect stands: the function takes a `&Path`, answers wrongly for any spelling other than the stored one, and the next caller to pass a native path inherits it. For the same reason, case folding and `strip_verbatim` are NOT warranted here: every caller's two spellings derive from one source, so only the separator can differ.

## Reproduction

CI run `36054775540`, job `107818878761` (Windows-gnu cross, MinGW + wine), head `51edbf86`. Log read directly through `gh api …/actions/jobs/107818878761/logs`:

```
panicked at src/librarian/catalog/augmentation.rs:2932:10:
the owner re-declaring its own prefix must pass: entry prefix already taken in this repository —
`R` is owned by C:/users/runner/AppData/Local/Temp/.tmpoJpiDU/docs/trackers/recon.md.
```

The test joins `tmp.path()` (native, backslashes) with `"docs/trackers/recon.md"`, so `declaring` reads `C:\users\…\.tmpoJpiDU\docs/trackers/recon.md` while the catalog row reads all-forward-slash. The inner spelling comes from the path construction, not from the log, and has not been printed. That is inferred, not measured.

Reported by session `ebf651ec` (sender verified through its socket's registry row). It is the first wine run to reach the test step since 2026-09-19, after its apt fix `87d90d8c`. So this is new *visibility*, not a new regression: the test has failed on this lane since it landed in `45d49a10`.

## Environment

Windows only. Linux and macOS spell both paths with `/`, so every local lane and the Linux/macOS CI jobs pass. `windows-latest / no-features` compiles no librarian and runs none of these tests. The wine lane is the only Windows lane that reaches this code.

## Root cause

A second, private reimplementation of path identity. The canonical one already exists: `comparable_path` (`src/librarian/tools/mod.rs:306`) was written for this exact pair of spellings. Its doc says the catalog stores `//?/C:/…` forward-slash, while the adapter boundary holds `\\?\C:\…`, and `Path::starts_with` can never match them. It is private to `librarian::tools`, and `catalog` is the lower layer, so `augmentation.rs` could not reach it and compared text instead.

## Evidence

The log excerpt above. The raw comparison is at `refuse_taken_prefixes`, in the `taken` filter.

## Hypotheses tried

N/A. The spelling mismatch is read off the code and the message. The exact bytes of `declaring` on the runner are the one inferred link.

## Fix

Move `comparable_path` down to `src/librarian/util.rs` (both layers can import it; no `catalog → tools` edge) and compare `comparable_path(Path::new(o))` against `comparable_path(declaring)`. **Check the sibling in `prefix_owners_under`:** its `Path::new(&path).starts_with(root)` is the component-wise comparison `comparable_path`'s doc says fails across the two spellings. It passed in this run (the owner WAS found), so it is not the failing half here. It is named for whoever fixes this, not asserted broken.


**Correction 2026-09-25, same session: do NOT move `comparable_path`. The crate already has the right pieces.**

- `crate::util::fs::to_forward_slash` (`src/util/fs.rs:186`) is the normalizer the catalog itself uses when storing `abs_path`; its doc cites `artifact::upsert` and `artifact_id_from_abs`. It is crate-level, so it is not librarian-gated.
- `crate::util::fs::strip_verbatim` (added by session `ebf651ec` in `0ac67b35` for the sibling bug `868e689c`) removes the `\\?\` marker at the `Path` level.
- Moving `comparable_path` would have made a third normalizer beside two that already fit.

**The likely fix:** compare `to_forward_slash(&strip_verbatim(declaring))` against the stored owner string.

**Case is still open and must be decided in the fix, not assumed.** Neither helper folds case, and in production the two spellings come from different sources: the doc tool's resolved path against the catalog's `abs_path`. The failing test cannot show this. Both of its spellings derive from one `tmp.path()`, and the lowercase `users` in the log is wine's real directory name, not a case fold.


**Done 2026-09-25: `1df22edc` · patch-id `42564a7813d7a7d4565ceacb44c69384487a940b`.** `me` is now `crate::util::fs::RepoPath::from(declaring)`, the catalog's own write normalization, so the comparison is between two strings in the same form. This is smaller than the correction above anticipated: once the production callers were traced, neither `strip_verbatim` nor case folding had a caller that needed it (see § *Symptom*'s second correction).

## Tests added

`the_owner_is_recognized_under_a_backslash_spelling_of_its_own_path` (`src/librarian/catalog/augmentation.rs`) passes the owner's path with its last separator spelled `\`. `to_forward_slash` rewrites `\` on EVERY platform, so this test reds on Linux too. The original `a_ledger_re_declaring_its_own_prefix_is_not_a_conflict` can only red on Windows.

- **RED observed on Linux before the fix**, with the CI message's exact shape (`` `R` is owned by `` the file itself). GREEN after.
- **Mutation** restoring `declaring.to_string_lossy()`: KILLED by that test (`scripts/mutation-probe.sh`, isolated, 104 tests ran).
- **Gate green:** FMT 0, CLIPPY 0, LEAN 0, DEFAULT 0. The new test runs in the default lane only, because librarian code is not compiled in the lean lane.

## Workarounds

None on Windows short of editing the ledger's frontmatter by hand, which bypasses the check (and `doctor` then reports nothing wrong, because the file owns its own prefix).

## Resume

Filed by session `09093108`, which authored `45d49a10`, on notice, while fixing `bfdfeebd`. Queued behind that fix in the same session. The same wine run also failed `tools::symbol::tests::references_on_an_unused_file_local_symbol_that_returns_its_declaration_stays_bare` (`src/tools/symbol/tests.rs:9484`, a `ParseError` from a URL parse). That failure is unrelated to this file and not investigated here.

## References

- `45d49a10`: the commit that added the check and the test.
- `src/librarian/tools/mod.rs:306` `comparable_path`, and `docs/issues/archive/2026-08-07-artifact-move-cannot-resolve-source-in-subroot-workspace.md`, the earlier instance of the same two spellings.
