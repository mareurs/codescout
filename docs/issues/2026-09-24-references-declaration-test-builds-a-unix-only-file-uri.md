---
id: '868e689cccfe84b3'
kind: bug
status: fixed
title: 'BUG: on Windows, references drops every project location (canonical \\?\ root vs URI path); its test also built a Unix-only file:// URI'
owners:
- marius
tags:
- cluster/unclassified
claimed_at: 2026-09-24
claimed_by: ebf651ec-5ab7-42d9-a526-dcf9758692e1
opened: 2026-09-24
severity: medium
---

# BUG: `references_on_an_unused_file_local_symbol_that_returns_its_declaration_stays_bare` fails on every Windows lane

## Summary

A test added in `1ea1d36b` (the references false-zero fix) passes on Linux and macOS and fails on every Windows lane. Its fixture builds the mock declaration's URI with `format!("file://{}", file.display())`. On Unix the path starts with `/`, so that yields a valid `file:///…`. On Windows it yields `file://C:\…`, which `lsp_types::Uri` refuses at index 7, where the drive letter starts. The test panics before the code under test runs, so on Windows it guards nothing.

Established: the parse failure. **Not yet established:** whether switching to `url::Url::from_file_path` is enough. See Root cause.

## Symptom (Effect)

Run `36054775540` (head `51edbf86`). The same panic appears in `Test (windows-latest / no-features)` (job `107818878922`, its only failure), `Test (windows-latest / default)`, `Test (windows-latest / local-embed)` and `Windows-gnu cross (MinGW + wine)` (job `107818878761`):

```
thread 'tools::symbol::tests::references_on_an_unused_file_local_symbol_that_returns_its_declaration_stays_bare' panicked at src\tools\symbol\tests.rs:9484:14:
called `Result::unwrap()` on an `Err` value: ParseError { index: 7, kind: UnexpectedChar }
```

It was already red at the previous tip, `16c7ed4f` (run `36046104949`, same line). So it has failed on Windows since `1ea1d36b` reached origin.

## Reproduction

Any Windows lane in CI. It does not reproduce on Linux, which is why the four-lane gate was green when it was committed.

## Environment

`windows-latest` (MSVC), and `x86_64-pc-windows-gnu` under wine on `ubuntu-latest`.

## Root cause

**Two defects, measured under wine-11.17 (`x86_64-pc-windows-gnu`, `scripts/build-windows.sh test --lib`, leased slot) on 2026-09-24.**

1. **The fixture (test-only).** `src/tools/symbol/tests.rs` built the declaration's URI with `format!("file://{}", file.display())`, which `lsp_types::Uri` refuses for a Windows path (`ParseError { index: 7 }`). The sibling fixtures at `:6177`/`:6422` already used `url::Url::from_file_path`.

2. **The references tool (production).** With only the URI fixed, the test still failed under wine: `total: Some(0)`, `right: Some(1)`, result `{"file_groups":[],"total":0,"files":0}`. There was no `excluded_from_build_dirs` key, so the build-directory filter did not drop it. Nor was there a zero-answer warning, so `total_raw` was 1 and the location did arrive. The project-scope filter dropped it. `classify_reference_path` (`src/fs/mod.rs`) used `path.starts_with(project_root)`. The root comes from `Agent::new`, which canonicalizes, so on Windows it is `\\?\C:\…`. A location path comes from `FileAddress::from_lsp_uri`, which cannot produce the marker, so it is `C:\…`. `Path::starts_with` compares components, and the prefixes parse as `VerbatimDisk('C')` and `Disk('C')`, so the check fails. **So on Windows, `references` in its default project scope dropped every location a language server reported.** The mock returns exactly what a server returns, so this is the production path. No Windows CI lane has a real language server, which is why nothing saw it. A second site, `References::call`'s outside-reference count (`p != full_path`), compared the same two spellings, so the declaration counted as an outside reference and the text-scan cross-check never ran.

**Not measured:** `relative_forward_slash` is shared with `list_overview`, `symbol_at` and the `agent` status (its own doc comment says so), and those may compare an LSP-derived path against a canonical root the same way. They are candidates, not findings.

## Fix

- `src/util/fs.rs`: new `strip_verbatim(&Path) -> Cow<Path>`. It rewrites `\\?\C:\…` to `C:\…` and `\\?\UNC\srv\share` to `\\srv\share`. Every other path, including every non-Windows path, is returned borrowed. It is a `Path` operation, not a string one, because the callers need component-aware containment. So it is not a second copy of the librarian's string-equality `comparable_path` (`c6cff39df3eed0c6`).
- `src/fs/mod.rs` `classify_reference_path`: compares against the project root and the library roots with the marker stripped. Only the roots are stripped, since `path` comes from a URI and stripping it would be inert.
- `src/tools/symbol/references.rs`: the outside-reference count compares against `full_path` with the marker stripped.
- Fixture: `url::Url::from_file_path`.

Library roots are stripped for the same reason as the project root, but no test exercises a Windows library root, so that site is **unmeasured**.

## Resume

**Verified 2026-09-24.**

- **Fixed state under wine-11.17** (`scripts/build-windows.sh test --lib`): 14 passed, 0 failed. That includes the Windows-only `strip_verbatim_makes_a_canonical_root_contain_its_uri_spelling`, whose precondition asserts that the raw spellings really do not match there.
- **One mutation per site through `scripts/mutation-probe.sh`**, each KILLED:
  - M1 (wine): project-root strip reverted. Killed by `…_stays_bare` (`total` 0).
  - M2 (wine): `full_path` strip reverted. Killed by `references_with_only_the_declaration_is_cross_checked_against_other_files`.
  - M3 (wine): the helper's `VerbatimDisk` arm reverted. Killed by the unit test and `…_stays_bare`.
  - M4 (Linux): the outside-reference test forced to `true`. Killed by the cross-check test, so it pins the branch on Linux too.
  - M1 leaves the cross-check test green, and that is correct: with the scope filter broken, zero locations count as outside and the text scan still warns. Each site has its own test.
- **Gate:** `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`. All three cross-platform tests ran in both test lanes.

**Not yet observed:** CI's Windows lanes on this commit, and library roots (unmeasured, see Fix).

## References

- `docs/issues/2026-09-24-prefix-owner-check-compares-path-spellings-as-text.md` (`c6cff39df3eed0c6`): the same shape, two spellings of one Windows path compared as different, in the librarian. Candidate shared class.
- `docs/issues/archive/2026-09-24-references-silent-false-zero-for-file-local-symbols-while-warming.md`: the fix whose test this is.

## Fix provenance

- **SHA:** `0ac67b35` (on `experiments`). Positional, so it does not survive a rebase of `experiments`.
- **patch-id:** `af669ed4c26caed2d439ee0b411006130e682a61`. A content hash of the diff, so it survives rebase and cherry-pick.

`fix(references): on Windows, compare locations against the root without its \\?\ marker, so project references are not all dropped`

**Archive when:** a CI Windows lane (`Test (windows-latest / …)` or the wine lane) runs `0ac67b35` or later with these tests green. Archiving mints a new id, so re-point the `868e689cccfe84b3` citations in `src/fs/mod.rs`, `src/tools/symbol/references.rs`, `src/tools/symbol/tests.rs`, `src/util/fs.rs` and the archived wine bug in the same commit.
