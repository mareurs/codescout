---
kind: bug
status: fixed
tags:
- xdg
- config
- path-resolution
closed: 2026-08-18
opened: 2026-08-18
owner: marius
related: []
severity: low
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

Shipped as `b17987b8` on `experiments` — the one-line gate that mirrors `state_dir_from`:

```rust
let base = xdg_config_home
    .map(PathBuf::from)
    .filter(|p| p.is_absolute())
    .or_else(|| home.map(|h| PathBuf::from(h).join(".config")))?;
```

The Resume's prerequisite was carried out first: `references(symbol="global_config_dir_from")`
returns one non-test caller (`global_config_dir`, same file, line 62) and five test call
sites, every one of them passing an absolute path. Nothing depended on CWD-relative
resolution, so the change is behaviour-preserving for every existing caller.

The two helpers' signatures still drift (`Option<&OsStr>` here vs `Option<OsString>` /
`Option<PathBuf>` in `state_dir_from`). Harmonising them remains optional and separate, as
filed — it was not done here.
## Tests added

Two, in `src/config/global.rs`'s `mod tests`. Each was watched fail before the fix, and
each guards a **distinct** mutation — measured, not assumed:

| Test | Mutation it is the sole guard against |
|---|---|
| `config_dir_ignores_relative_xdg_and_falls_back_to_home` | Deleting the `.filter` (i.e. the pre-fix code). Red with `left: "relative/state/codescout"`, `right: "/tmp/fake-home/.config/codescout"`. |
| `config_dir_none_when_xdg_is_relative_and_home_unset` | Keeping the relative value as a *last-resort* fallback (`.or_else(|| xdg_config_home.map(PathBuf::from))` appended). Leaves the first test **green** and reds only this one. |

The second test exists precisely because the first cannot distinguish "ignored" from
"deprioritised" — a fix that merely reordered the branches would pass it. The mutation run
above confirms it is not redundant.

Full gate on `c86c5a68` + this change, run in a clean worktree so a peer's in-flight edits
to `server.rs` / `guide_ledger.rs` could not colour the result: `cargo fmt --check` clean,
`cargo clippy --all-targets -- -D warnings` clean, `cargo test --workspace --no-fail-fast`
at **4191 passed / 0 failed / 50 ignored**.
## Workarounds

Set `XDG_CONFIG_HOME` to an absolute path, or leave it unset and let `$HOME/.config`
apply. Both are what conforming systems already do; a relative value is unusual.

## Resume

Nothing outstanding. Fixed and verified on `experiments` as `b17987b8`; archived on
2026-08-18.

Promotion to `master` is a **fast-forward** (`git rev-list --left-right --count
master...experiments` → `0 1043`), so the `experiments` SHA above already *is* the master
SHA — there is no second SHA to record, and deliberately no pending-master-SHA line here.

The one deliberately-unclosed thread, carried over from Fix rather than left implicit: the
two XDG helpers still disagree in signature shape. That is a tidiness item, not a defect,
and was out of scope by the original filing.
## References

- `src/config/global.rs:49-57` — the defective resolver
- `src/util/fs.rs:107-114` — the conforming sibling, added `d2d5686f`
- `docs/superpowers/plans/2026-08-18-guide-ledger-phase-a-storage.md` — the plan whose
  Task 1 review surfaced this
- `docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md`
  — why both helpers take values instead of reading env
