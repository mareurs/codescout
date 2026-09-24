---
id: c6cff39df3eed0c6
kind: bug
status: taken
title: 'BUG: the prefix-owner check compares path spellings as text, so on Windows a ledger re-declaring its own prefix is refused as a conflict'
tags:
- cluster/unclassified
claimed_at: 2026-09-24
claimed_by: 09093108-1425-4f6d-9695-a9e3bb98ea0d
opened: 2026-09-24
owner: marius
severity: medium
---

# BUG: the prefix-owner check compares path spellings as text, so on Windows a ledger re-declaring its own prefix is refused as a conflict

## Summary

`refuse_taken_prefixes` (`src/librarian/catalog/augmentation.rs`) decides whether the declaring ledger IS the owner with a raw string comparison: `o.as_str() != me`, where `me = declaring.to_string_lossy()`. The owners come from the catalog's `abs_path`, which is stored forward-slash-normalized. On Windows, `declaring` holds the native spelling. The same file then fails to equal itself, and the owner is refused as a conflict with its own declaration.

## Symptom (Effect)

On Windows, any `doc(update)` to a ledger that declares an `entry_prefix` — or `rekey_prefix` back onto one — is refused with *"entry prefix already taken in this repository — `R` is owned by <the ledger itself>"*. `doc(update)` re-sends the whole `extra` value, so it is refused even when the edit is to an unrelated key. That is exactly the case the test names.

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

## Tests added

None yet. The test that fails is the regression test; it needs a Windows lane to be red. A Linux-reachable test should compare a backslash spelling against the forward-slash one through `comparable_path`, under `cfg(windows)`, where `\` is a separator.

## Workarounds

None on Windows short of editing the ledger's frontmatter by hand, which bypasses the check (and `doctor` then reports nothing wrong, because the file owns its own prefix).

## Resume

Filed by session `09093108`, which authored `45d49a10`, on notice, while fixing `bfdfeebd`. Queued behind that fix in the same session. The same wine run also failed `tools::symbol::tests::references_on_an_unused_file_local_symbol_that_returns_its_declaration_stays_bare` (`src/tools/symbol/tests.rs:9484`, a `ParseError` from a URL parse). That failure is unrelated to this file and not investigated here.

## References

- `45d49a10`: the commit that added the check and the test.
- `src/librarian/tools/mod.rs:306` `comparable_path`, and `docs/issues/archive/2026-08-07-artifact-move-cannot-resolve-source-in-subroot-workspace.md`, the earlier instance of the same two spellings.
