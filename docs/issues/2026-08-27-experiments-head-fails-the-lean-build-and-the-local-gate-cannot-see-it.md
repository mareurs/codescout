---
id: '89a7942d5e5f9152'
kind: bug
status: open
title: 'BUG: experiments HEAD fails `cargo check --no-default-features`, and the documented local gate runs only with default features — fourth instance of this class in one day'
tags:
- build
- feature-gates
- no-default-features
- pre-commit-gate
- recurrence
---

## Symptom

`experiments` HEAD does not compile with default features off:

```
$ cargo check --no-default-features
error[E0433]: cannot find `librarian` in `crate`
   --> src/prompts/guide_index.rs:194:24
    |
194 |             if !crate::librarian::adapter::names_path_containing(result, needle) {
    |                        ^^^^^^^^^ could not find `librarian` in the crate root
    |
note: found an item that was configured out
   --> src/lib.rs:33:9
 32 | #[cfg(feature = "librarian")]
 33 | pub mod librarian;
exit 101
```

`src/prompts/guide_index.rs` is unconditional; `crate::librarian` is behind
`#[cfg(feature = "librarian")]` at `src/lib.rs:33`.

## Reproduction

Verified 2026-08-27 in an isolated detached worktree at `experiments` alone, with no
operator-rules code present — so the break is not from the branch merged that evening:

```
git worktree add --detach /tmp/exp-probe experiments
cd /tmp/exp-probe && cargo check --no-default-features   # exit 101, same line
```

## Root cause — the gap is WHERE the guard lives, not whether one exists

CI **does** have a lean lane: `.github/workflows/ci.yml:75-76`
(`- name: no-features / flags: "--no-default-features"`). So this will go red on CI.

The project's documented pre-commit gate does not. `CLAUDE.md` § Development Commands
mandates `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` — **all three run with
default features, and `librarian` is in `default`**. So the break is invisible locally,
lands on `experiments`, and is inherited by every session that merges before CI reports.

This is a green result certifying only the configuration it ran in. Three local gates pass
in a world where the defect cannot appear.

## The class, not the instance — four occurrences in one day

All four are "unconditional module reaches a feature-gated one", all four invisible to the
local gate, across two independent sessions on 2026-08-27:

| # | Site | Gated dep | Outcome |
|---|---|---|---|
| 1 | operator-rules Task 1 (planned) | `librarian::frontmatter::parse` | caught in plan self-review, never shipped |
| 2 | `src/operator_rules/profiles.rs:26` | `dirs::home_dir()` | **shipped**, `E0433`; fixed by switching to unconditional `crate::platform::home_dir()` |
| 3 | operator-rules fix wave | `crate::util::fs` | enumerated and checked clean before use |
| 4 | `src/prompts/guide_index.rs:194` | `crate::librarian::adapter` | **this bug** |

Instance 2 is why `cargo check --no-default-features` was added to that plan's own gate —
a per-plan mitigation that no other session inherits.

## Prior art

`docs/issues/archive/2026-07-06-no-default-features-build-broken.md` is the same class,
already fixed once. Its `## Fix` closed two specific sites (`src/cli/mod.rs`'s
`pub mod doctor;`, and `heartbeat_dir` / `kotlin_lsp_home_root` splitting on the feature)
and added **no guard against the next instance**. That is the pattern: each occurrence is
fixed correctly and individually, and nothing afterwards prompts anyone to re-check the
class.

`docs/issues/archive/2026-08-11-project-status-backend-misreports-bare-model-and-lean-build.md`
is adjacent — the lean build misreporting rather than failing.

## Fix ideas

1. **Immediate:** gate the call at `guide_index.rs:194`, or move
   `names_path_containing` to an unconditional module. Note the operator-rules precedent:
   the fix that worked was not "make the dep unconditional" but "use the unconditional
   helper that already existed" (`crate::platform::home_dir()` over `dirs::home_dir()`).
   Check whether `librarian::adapter::names_path_containing` has an unconditional twin
   before adding a cfg.
2. **Structural, and the point of this file:** add `cargo check --no-default-features` to
   the documented pre-commit gate in `CLAUDE.md` § Development Commands. It is one command
   and a few seconds against an incremental target dir. Without it the local gate will keep
   certifying a configuration in which this class is undetectable, and CI will keep finding
   it after the fact.
3. **Cheap detection for the class rather than the instance:** a test or script asserting
   that no file outside `#[cfg(feature = "librarian")]` names `crate::librarian`. Would have
   caught instances 1, 2 and 4 at author time.

## Status notes

Not fixed here. Filed from the operator-rules Phase 1 merge session, which verified the
break is pre-existing on `experiments` and merged anyway on that basis — the branch's own
lean build was clean and refusing to merge would not have repaired the base.

