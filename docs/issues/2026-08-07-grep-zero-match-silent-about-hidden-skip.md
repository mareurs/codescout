---
id: c63b594c54199e81
kind: bug
status: open
title: 'BUG: grep''s zero/absent result is silent about the hidden-path skip, so `.github/` reads as "not present anywhere"'
tags:
- grep
- progressive-disclosure
- false-negative
- completeness-warning
- tooling
closed: ''
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

Not yet implemented. Two parts, and the second is the one that matters.

**Report the exclusion.** Mirror the `WalkAudit` pattern that `src/tools/symbol/symbols.rs`
gained in `3bfa4025`: count what the walk declined, and attach a `completeness_warning` naming
`include_hidden=true` as the remedy. To get an exact count rather than a guess, invert where the
filtering happens — build the walk with `hidden(false)` and apply the dot-prefix rule in a
`filter_entry` closure that increments a counter. That costs no extra walk and yields "skipped N
hidden entries" instead of a blanket caveat.

Warn on a **zero-match result** when `N > 0`, not on every zero. The `symbols` fix deliberately
left a trustworthy zero bare, on the grounds that warning unconditionally trains the reader to
skip the warning that matters; the same reasoning applies here, and it is why a static "hidden
paths were not searched" note appended to every empty result is the wrong shape.

**Consider making an explicit glob re-admit its subtree.** When the caller passes a `glob` whose
literal prefix names a hidden directory (`.github/**`), honouring it is almost certainly what
they meant — the current behaviour returns empty for a path the caller spelled out in full. This
is a behaviour change, so it wants its own decision rather than riding along with the warning.

## Tests added

None yet. The fix wants:

- zero-match grep over a tree containing a hidden directory → response carries
  `completeness_warning` naming `include_hidden`;
- zero-match grep over a tree with **no** hidden entries → no warning (guards against the
  warn-on-every-zero regression);
- a hit under a hidden path with `include_hidden=true` → no warning, results present.

`include_hidden_searches_dotfiles` (`src/tools/grep.rs:1152`) already pins the filtering
behaviour itself and must keep passing unchanged.

## Workarounds

Pass `include_hidden=true` whenever the question is about **absence** rather than presence — any
"does X appear anywhere", "who references X", or pre-edit blast-radius sweep. For a single known
file, `path=` bypasses the walk entirely and always sees it.

## Resume

Implement the `filter_entry`-based counter in `src/tools/grep.rs` around the `WalkBuilder` at
`src/tools/grep.rs:105-117`, following `WalkAudit` in `src/tools/symbol/symbols.rs` (added
`3bfa4025`) for both the counting shape and the warning wording. Decide the explicit-glob
question separately; it changes behaviour, not just reporting.

## References

- `src/tools/grep.rs:41` — `include_hidden` schema declaration, `"default": false`
- `src/tools/grep.rs:106` — `wb.hidden(!include_hidden).git_ignore(true);`
- `src/tools/grep.rs:1152` — `include_hidden_searches_dotfiles`, pins the default
- `src/tools/symbol/symbols.rs` — `WalkAudit` / `completeness_warning`, the pattern to copy
- `docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md` — the
  sibling false-negative, same defect class
- `docs/issues/2026-08-07-windows-ci-timing-flakes-block-the-gate.md` — the session this
  surfaced in

