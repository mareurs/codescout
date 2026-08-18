---
status: open
opened: 2026-08-18
closed:
severity: low
owner: marius
related: []
tags: [xdg, config, path-resolution]
kind: bug
---

# BUG: `global_config_dir_from` accepts a relative `XDG_CONFIG_HOME`, contrary to the XDG basedir spec

## Summary

`global_config_dir_from` uses whatever `XDG_CONFIG_HOME` contains, absolute or not.
The XDG Base Directory Specification requires that a relative value be treated as
invalid and ignored (falling back to `$HOME/.config`). With a relative value set,
codescout resolves its global config directory against the process CWD, so the same
user gets a different config directory depending on where they launched the server.

## Symptom (Effect)

Not observed in the wild — found by code inspection while reviewing a sibling helper.
With `XDG_CONFIG_HOME=relative/path`, `global_config_dir_from` returns
`Some("relative/path/codescout")` where the spec requires `Some("$HOME/.config/codescout")`.
The returned path is then joined against the CWD by every downstream filesystem call.

## Reproduction

Not yet reproducible as a user-visible failure — no runtime repro was run. The defect
is visible directly in the function body (see Root cause). A unit-level demonstration:

```rust
// src/config/global.rs, mod tests
assert_eq!(
    global_config_dir_from(Some(OsStr::new("relative/state")), Some(OsStr::new("/home/u"))),
    Some(PathBuf::from("/home/u/.config/codescout")),   // spec-required
);
// actual: Some(PathBuf::from("relative/state/codescout"))
```

Commit at time of filing: `d2d5686f` (branch `experiments`).

## Environment

Linux, Rust, branch `experiments`. Platform-independent — the logic has no `cfg` gates.

## Root cause

`global_config_dir_from` maps the env value straight into a `PathBuf` with no
absoluteness gate:

```rust
let base = xdg_config_home
    .map(PathBuf::from)
    .or_else(|| home.map(|h| PathBuf::from(h).join(".config")))?;
```

`src/config/global.rs:54-56`. There is no `is_absolute()` check on the `xdg_config_home`
branch, so a relative value short-circuits the `HOME` fallback.

*Inferred from `src/config/global.rs:54-56` — not measured. No runtime observation was
made; the mechanism is read directly out of the function body.*

## Evidence

### The sibling helper added the same day gets it right

`state_dir_from` (`src/util/fs.rs:107-114`, added in `d2d5686f` for the guide-ledger
Phase A work) resolves `XDG_STATE_HOME` with an explicit `is_absolute()` gate, treating
a relative value as unset. The two XDG resolvers in this codebase now disagree on XDG
semantics, which is how the older one's non-conformance was noticed.

Surfaced as Minor finding 3 in the Task 1 review of
`docs/superpowers/plans/2026-08-18-guide-ledger-phase-a-storage.md`; ledger entry in
`.superpowers/sdd/2026-08-18-guide-ledger-phase-a-storage/progress.md` (Ruling 6).

### Spec text

XDG Base Directory Specification: *"All paths set in these environment variables must be
absolute. If an implementation encounters a relative path in any of these variables it
should consider the path invalid and ignore it."*

## Hypotheses tried

1. **Hypothesis:** already filed under an existing bug file.
   **Test:** `grep("XDG_CONFIG_HOME|global_config_dir_from", glob="docs/issues/**/*.md", mode="files")`.
   **Verdict:** rejected — 26 matches across 6 files, all in `docs/issues/archive/` and all
   about unrelated subjects (Kotlin LSP disk use, guide-hint artifact registration, test
   mutex poisoning, a workspace-summary flake, the test-env UB race).

## Fix

Not planned for the guide-ledger Phase A work — explicitly ruled out of scope for that
plan's Task 1 (Ruling 6). The one-line fix mirrors `state_dir_from`:

```rust
let base = xdg_config_home
    .map(PathBuf::from)
    .filter(|p| p.is_absolute())
    .or_else(|| home.map(|h| PathBuf::from(h).join(".config")))?;
```

Before shipping it, check callers for anyone relying on the current relative behaviour —
`references(symbol="global_config_dir_from")`. Note the two helpers also drift in
signature (`Option<&OsStr>, Option<&OsStr>` here vs `Option<OsString>, Option<PathBuf>`
in `state_dir_from`); harmonising them is optional and separate.

## Tests added

None yet — bug is open, not fixed. A fix must add the relative-value assertion shown
under Reproduction to `src/config/global.rs`'s `mod tests`.

## Workarounds

Set `XDG_CONFIG_HOME` to an absolute path, or leave it unset and let `$HOME/.config`
apply. Both are what conforming systems already do; a relative value is unusual.

## Resume

Run `references(symbol="global_config_dir_from")` to enumerate callers and confirm none
depends on CWD-relative resolution. Then apply the `.filter(|p| p.is_absolute())` shown
under Fix, add the assertion from Reproduction to `src/config/global.rs`'s `mod tests`,
and run `cargo test --lib config::global`. Decide separately whether to harmonise the
two helpers' signatures.

## References

- `src/config/global.rs:49-57` — the defective resolver
- `src/util/fs.rs:107-114` — the conforming sibling, added `d2d5686f`
- `docs/superpowers/plans/2026-08-18-guide-ledger-phase-a-storage.md` — the plan whose
  Task 1 review surfaced this
- `docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md`
  — why both helpers take values instead of reading env
