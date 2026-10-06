---
id: c269b760403ff5c9
kind: bug
status: fixed
title: 'RESIDUAL: Install Git for Windows in the wine CI image and drop the 22-test cfg(windows) skip block'
tags:
- cluster/unclassified
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-08-run-command-unusable-without-git-bash.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-08-run-command-unusable-without-git-bash.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Install Git for Windows in the wine CI image and drop the 22-test cfg(windows) skip block.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-08-run-command-unusable-without-git-bash.md` (status `mitigated`):

> Windows-only fix, never exercised on this host — its tests are cfg(windows), and the wine CI image still ships without Git for Windows, so the 22-test skip block stands. Root cause (install Git in the image, drop the skip block) is unaddressed, which is why this is mitigated rather than fixed.

## Fix

The root-cause step the parent named (give the wine lane a Git so the skip block can go) was done on 2026-08-26, a month BEFORE this residual was filed (2026-09-24), by three commits on `experiments`; the parent caveat was written at the parent's closing and never updated. Verified at the bytes on 2026-10-06 in `.github/workflows/ci.yml`:

- `ba046b9c` ("give the wine lane a Git Bash instead of skipping what needs one") added the `Cache PortableGit` and `Seed PortableGit (cache miss)` steps (line ~674: downloads `PortableGit-2.55.0.5-64-bit.7z.exe` and unpacks it) and exported `CODESCOUT_BASH=Z:<...>\bin\bash.exe` to `$GITHUB_ENV` (line ~701).
- `70f1a32d` ("drop 22 stale skips, and classify the 10 that remain") cut the `cargo test` line's `--skip` list from 32 entries to 10 and wrote the per-entry classification comment (the 22-test skip block of the parent's caveat).
- `d0aabbe3` ("WINEPATH retires 3 more skips; teach the last failure to explain itself") added `WINEPATH=Z:<...>\cmd` (line ~702) so tests that invoke `git` directly find `git.exe`, retiring 3 more skips. NOTE: the brief for this closure attributed `WINEPATH` to `ba046b9c`; `git show ba046b9c` adds `CODESCOUT_BASH` only.
- Later commits retired more (group 6 by the wine pin on 2026-09-02, the MAP-shape live test on 2026-10-04). At HEAD the line at ~840 is `scripts/build-windows.sh test --lib -- --skip …` with exactly 5 `--skip` entries (`symbols_path_type`, `symbols_name_path_pattern_in_directory`, `include_docs_attaches_docs_in_search_mode`, `background_command_with_quotes_captures_output`, `buffer_query_truncation_hint_shows_next_page`), each classified in the comment block above it.

**Nuance against the title:** the lane does NOT install Git for Windows with its installer. It unpacks the PortableGit self-extracting 7z with `7z x` (no installer executes) into a cached `portable-git` directory and points two env vars at it. That gives a real Git Bash, which is what the parent's "install Git in the image" was after. Also, the lane has run a pinned WineHQ devel build (`WINE_PIN: "11.17"` at line 605, whole `wine-devel` family held) since 2026-09-02, not the wine 9.0 that was current when most of the above was measured.

What I did NOT verify: any CI run. The workflow comments cite runs (e.g. 32961510592, "7 of the 8 pass"; 36054775540 for the pin on wine-11.17 in a later commit message); I read those, I did not open them. No code change in this sweep.

## Tests added

None: this is CI configuration, and the tests it un-skipped already existed. The verification the commits cite is the CI runs recorded in the `ci.yml` comments (not re-run here). The `Point codescout at Git Bash under wine` step fails loudly (`test -f` on `bash.exe` and `cmd/git.exe`) if the PortableGit layout changes.

## Fix provenance

- **SHA:** `ba046b9c` (`experiments`)
- **patch-id:** `6da1b8b22512932e135c2a82a4023afc92ad8876`
- **SHA:** `70f1a32d` (`experiments`)
- **patch-id:** `292ebfc10e99f54c537f7d5fa5112d9a6f7fdbef`
- **SHA:** `d0aabbe3` (`experiments`)
- **patch-id:** `360f32c468a0a754c3173b1420186a20a48a8f91`

## Resume

Closed on 2026-10-06; nothing to resume. Residual follow-ups, listed and not filed: (1) the 3-entry glob-walk skip (`symbols_path_type`, `symbols_name_path_pattern_in_directory`, `include_docs_attaches_docs_in_search_mode`; the comment's "group 1", WIN-27, `docs/issues/2026-07-02-windows-gnu-wine-20-test-failures.md`) is the optional "PASS 3" and is not part of this residual. (2) The parent `docs/issues/archive/2026-08-08-run-command-unusable-without-git-bash.md` is still `status: mitigated` with a body that says the root-cause close is owed; its caveat is updated, its status is the integrator's call.

## References

- `docs/issues/archive/2026-08-08-run-command-unusable-without-git-bash.md` — parent
