---
id: '790e34f82ec93c1b'
kind: bug
status: fixed
title: 'BUG: grep''s zero/absent result is silent about the hidden-path skip, so `.github/` reads as "not present anywhere"'
tags:
- grep
- progressive-disclosure
- false-negative
- completeness-warning
- tooling
closed: 2026-08-07
opened: 2026-08-07
owner: marius
related:
- docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md
- docs/issues/2026-08-07-windows-ci-timing-flakes-block-the-gate.md
severity: low
---

# BUG: grep's zero/absent result is silent about the hidden-path skip

## Summary

`grep` defaults to `include_hidden: false`, so every dot-prefixed file and directory is
excluded from the walk. The response says nothing about it. In this repo that silently hides
the **entire CI configuration** (`.github/workflows/`), `.pre-commit-config.yaml`, and
`.codescout/` — so "does this identifier appear anywhere in the repo?" is exactly the question
the default answers wrongly, with no signal that it did.

Not a defect in the walk: the default is deliberate, documented in the tool schema, and pinned
by a test. The defect is that the **result does not say it can't be trusted** — the same class
as the `symbols` 0-match flake fixed in `3bfa4025`.

## Symptom (Effect)

Asking whether a test name appears in the workflow file, during work on WIN-30 (2026-08-07):

```
grep(pattern="background_command_with_quotes_captures_output",
     glob="**/*.{yml,yaml,rs,md}")
→ 15 matches in 5 files      # .github/workflows/ci.yml NOT among them
```

The name is on `.github/workflows/ci.yml:117`. Three follow-up probes all returned `0 matches`
— including one naming the file's exact path as the glob:

```
grep(pattern="...", glob="**/*.yml")                      → 0 matches
grep(pattern="...", glob=".github/**/*.yml")              → 0 matches
grep(pattern="...", glob=".github/workflows/ci.yml")      → 0 matches
grep(pattern="runs-on", mode="files")   # no glob at all  → 2 matches, both docs/*.md
```

Nothing in any response mentions hidden-path exclusion. `0 matches` is the same string the
tool returns when the pattern genuinely does not occur.

## Reproduction

At `ea0340b0` on `experiments`:

```
grep(pattern="runs-on", mode="files")                     → 2 matches, both docs/
grep(pattern="runs-on", mode="files", include_hidden=true) → also .github/workflows/*.yml
```

## Environment

codescout `0.15.0`, branch `experiments` at `ea0340b0`, Linux, MCP stdio transport, project
`codescout`. Not platform-specific — the exclusion is in the walk configuration.

## Root cause

`src/tools/grep.rs:106` — `wb.hidden(!include_hidden).git_ignore(true);`, where
`include_hidden` is read at `src/tools/grep.rs:93` and declared `"default": false` at
`src/tools/grep.rs:41`. `ignore::WalkBuilder::hidden(true)` is the `ignore` crate's default
standard filter: it skips any entry whose file name begins with `.`, and skipping a *directory*
prunes the whole subtree. So `.github/` is never descended into.

This is intended behaviour, not drift: the flag's own description says "Also search hidden
files/dirs (dotfiles, .github/)", and `include_hidden_searches_dotfiles`
(`src/tools/grep.rs:1152`) asserts the default skips them.

The bug is on the reporting side. The response carries no `completeness_warning`, so a caller
cannot distinguish these three states:

1. the pattern does not occur anywhere;
2. the pattern occurs only under paths the walk pruned;
3. the pattern occurs in a file the caller named explicitly but the walk still pruned (the
   `glob=".github/workflows/ci.yml"` case above — a glob cannot re-admit a pruned subtree,
   because `overrides` are applied *inside* a walk that already skipped the parent).

State 3 is the sharpest form: the caller has demonstrated exact intent and still gets a silent
zero.


**A second instance of the same defect, found while implementing the fix.** Both `walker.flatten()`
sites in `Grep::call` discarded `ignore::Walk`'s `Err` arm, and both `std::fs::read` calls dropped
their failure with a bare `continue`. So a walk truncated by a permission error or
file-descriptor exhaustion produced the same `0 matches` as a complete one — the identical bug
fixed in `symbols` at `3bfa4025`, in a tool where it bites harder. Four dropped-error sites in
one function, none of them reported. Both halves are addressed together in the Fix below, since
they are one question from the caller's side: *can this zero be trusted?*
## Evidence

### The extension and the glob both work — control case

A non-dot path with the same extension is found normally, so neither `.yml` nor the glob
mechanism is at fault:

```
grep(pattern="services|image:|version:", glob="docker-compose.yml", mode="files")
→ 5 matches in 1 files:  docker-compose.yml
```

### Explicit `path=` bypasses the walk

`path=` reads the file directly rather than walking, so it sees what the walk cannot:

```
grep(pattern="--skip", path=".github/workflows/ci.yml")
→ 117: run: scripts/build-windows.sh test --lib -- --skip symbols_path_type … 
```

This asymmetry between `path=` (works) and `glob=` (silently empty) for the *same file* is the
most confusing surface for a caller.

### A second, independent case in the same repo

`.pre-commit-config.yaml` — a dot-prefixed *file* at the repo root, not a directory:

```
grep(pattern="repos:|hooks:|rev:", mode="files")
→ 12 matches in 12 files    # .pre-commit-config.yaml absent, though it contains `repos:`
```

### Confirmation

```
grep(pattern="background_command_with_quotes_captures_output", glob="**/*.yml",
     include_hidden=true)
→ .github/workflows/ci.yml:117
```

## Hypotheses tried

1. **Hypothesis:** the brace-alternation glob `**/*.{yml,yaml,rs,md}` isn't expanded.
   **Test:** re-ran with the plain `**/*.yml`. **Verdict:** rejected — still 0 matches, and the
   brace form works for non-hidden paths.
2. **Hypothesis:** `.github/` is still gitignored, and `git_ignore(true)` excludes it.
   **Test:** read `.gitignore`. **Verdict:** rejected — narrowed to
   `/.github/copilot-instructions.md` in `6ca02767`; the broad `/.github/` rule is gone.
3. **Hypothesis:** `.yml` is not in a searchable-extension allow-list.
   **Test:** grepped `docker-compose.yml` (non-hidden, same extension).
   **Verdict:** rejected — 5 matches.
4. **Hypothesis:** the walk prunes dot-prefixed entries.
   **Test:** re-ran with `include_hidden=true`; read `src/tools/grep.rs:106`.
   **Verdict:** confirmed.

## Fix

Implemented on `experiments` in **`624f7f05`**, refined in **`cdfbbe0f`** — both
`experiments`-side SHAs; see Resume for the master-side rule.

**`cdfbbe0f` — the ordering fix, and the reason it is here at all.** Live verification against the
real tree caught what none of the seven tests could: each test tree had exactly one hidden entry,
so the truncation path never ran. The repo has 16 after the exclusions, and pure alphabetical
ordering put `.github/` twelfth — behind five `.env*` files — so a cap of 5 cut the one entry the
warning existed to surface. The rendered message read *".buddy/, .cargo/, .claude/, .env,
.env.amd and 11 more"*, as useless as the bare zero it replaced. Directories now sort before
files (alphabetical within each group) and the cap is 8. The ordering is the substantive half: a
pruned directory hides an unbounded subtree, a pruned dotfile hides exactly one file, so
directories carry far more information per character of a truncated list. Raising the cap alone
would have papered over the ordering and broken again on the next repo with more dotfiles.

`src/tools/grep.rs` now carries a `WalkAudit` (sibling of the one in
`src/tools/symbol/symbols.rs`) providing:

- **error counting** at all four dropped-error sites, each with a `tracing::warn!`;
- **`hidden_at_root`** — one `read_dir` of the search root, no recursion, naming the dot-prefixed
  entries `hidden(true)` pruned;
- **`completeness_warning`** — attached to `result["completeness_warning"]` on a zero-match result
  in *both* the `mode="files"` early return and the main tail, and rendered by `format_grep` in
  both its zero-match early return and its normal path.

**What changed from this file's original plan.** It proposed inverting the filter: build with
`hidden(false)` and apply the dot-prefix rule in a counting `filter_entry` closure. Rejected on
reading `ignore` — its `hidden` filter is not merely a dot-prefix test; on Windows it also honours
`FILE_ATTRIBUTE_HIDDEN`. Reimplementing it would have silently changed *which files are searched*
on the one platform this repo cannot test locally, purchased with an exact count. Inspecting the
search root with a single `read_dir` leaves the walk's behaviour untouched and produces the more
useful output anyway: entry **names**, from which a reader can judge relevance, rather than a
number that cannot tell `.github/` from `.git/`.

**`.git` and `.codescout` are excluded, and that exclusion is the whole difference between a
warning and noise.** The test helper `rooted_ctx` creates `.codescout/` in every test root, which
made the problem concrete: both directories exist by construction in every project codescout
touches, so counting them would attach a warning to essentially every zero-match grep everywhere
— precisely the failure mode that teaches readers to skip warnings. Content inside them stays
reachable via `include_hidden=true`, or `memory()` for memories.

**The explicit-glob question is deliberately left open.** `glob=".github/workflows/ci.yml"` still
returns empty, because overrides are applied inside a walk that has already pruned the parent. The
warning now says so in as many words, which is the reporting fix. Making a literal-prefixed glob
re-admit its subtree is a behaviour change and wants its own decision, not a ride-along.
## Tests added

Seven, in `src/tools/grep.rs` `mod tests`. The negative cases are the ones that keep the warning
meaningful:

- `zero_match_over_a_tree_with_a_hidden_dir_says_it_was_not_searched` — the warning must name both
  the pruned entry (`.github/`) and the remedy (`include_hidden`).
- `zero_match_over_a_clean_tree_returns_a_bare_zero` — the `None` branch. Searches a subdirectory
  so `rooted_ctx`'s `.codescout/` is out of scope.
- `metadata_dirs_alone_do_not_trigger_the_warning` — pins the exclusion list.
- `include_hidden_suppresses_the_warning_even_on_a_zero` — nothing pruned, so the zero is
  trustworthy.
- `files_mode_zero_match_carries_the_completeness_warning` — `mode="files"` returns from its own
  branch and needed wiring separately.
- `format_grep_surfaces_the_completeness_warning_on_zero_matches` and
  `format_grep_leaves_a_trustworthy_zero_bare` — the renderer.

Verified by mutation, each killing exactly one test and nothing else:

1. removing the warning append from `format_grep`'s zero-match early return killed only
   `format_grep_surfaces_the_completeness_warning_on_zero_matches` — the R-60 trap, confirmed live;
2. emptying the `uninformative` exclusion list killed only
   `metadata_dirs_alone_do_not_trigger_the_warning`.

`include_hidden_searches_dotfiles`, which pins the filtering behaviour itself, passes unchanged —
confirming the walk's semantics did not move.

An eighth test came from live verification, `a_pruned_directory_survives_truncation_of_the_hidden_list`
— nine dotfiles that all sort before a directory, asserting both that the directory survives
truncation and that the remainder is still reported. Mutation-verified: reverting to plain
`names.sort()` kills only that test, and its failure output reproduces the live symptom
(`.aaa0 … .aaa7 and 2 more`). Its existence is the argument for live verification as a distinct
step — seven passing tests, a green gate, and the shipped output was still unhelpful, because
every fixture had one hidden entry and the real tree has sixteen.

Gate: fmt, `clippy --all-targets -D warnings`, **3523 passed / 0 failed** (3515 + 8).
## Workarounds

No longer needed for diagnosis — a zero-match now says whether it can be trusted. The remedy it
names is still the right one: pass `include_hidden=true` when the question is about **absence**
rather than presence, or `path=` for a single known file, which bypasses the walk entirely.
## Resume

N/A for the reporting fix.

**Live-surface caveat:** inert in a running MCP session until `cargo rb` plus a `/mcp` reconnect —
the served binary is a separate artifact from the test build.

**Master-side SHA still needs recording after cherry-pick.** The SHA below is `experiments`-side
and orphans on rebase; run `git rev-parse HEAD` on `master` after the promotion and update it.

One deliberate non-goal is left open and is **not** tracked here: a glob whose literal prefix names
a hidden directory still returns empty. If that becomes worth changing it is a behaviour change to
`Grep::call`'s override handling, and wants a fresh bug file.
## References

- `src/tools/grep.rs:41` — `include_hidden` schema declaration, `"default": false`
- `src/tools/grep.rs:106` — `wb.hidden(!include_hidden).git_ignore(true);`
- `src/tools/grep.rs:1152` — `include_hidden_searches_dotfiles`, pins the default
- `src/tools/symbol/symbols.rs` — `WalkAudit` / `completeness_warning`, the pattern to copy
- `docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md` — the
  sibling false-negative, same defect class
- `docs/issues/2026-08-07-windows-ci-timing-flakes-block-the-gate.md` — the session this
  surfaced in

## Fix provenance

- **SHA:** `3bfa4025` (experiments-only) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `d8394178dbc6fb41f641a2cccef987304b183baf` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes —
codescout's Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep d8394178dbc6 /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several
branches (cherry-pick) and any of them is the fix. Recorded 2026-08-19.
