---
id: 76ef2915909f94a9
kind: bug
status: fixed
title: 'BUG: experiments HEAD fails `cargo check --no-default-features`, and the documented local gate runs only with default features — fourth instance of this class in one day'
tags:
- build
- feature-gates
- no-default-features
- pre-commit-gate
- recurrence
closed: 2026-08-27
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

## Fix applied

Fixed on `experiments` — `12f21926`, patch-id `9e75640a0e5e0a5cc131d4c5f35ad6b81996405c`.

**Instance (fix idea 1).** `names_path_containing` moved out of the gated `librarian::adapter`
into the unconditional `src/util/librarian_response.rs`. The function is pure `serde_json`
— `&Value` + `&str` -> `bool` — and carried no librarian dependency at all; it lived in
`adapter.rs` only because that is where the tracker-path check it was generalised from lives.

This file's own instruction — check for an unconditional twin before adding a `cfg` — was
the right question, and the answer was that no twin existed. But `src/util/librarian_guard.rs`
had already solved the identical problem structurally, and says so in its doc comment:
*"Left unset (tests, `--no-default-features`) the guard degrades to its frontmatter check
rather than failing open loudly."* The established shape is that the half with no librarian
dependency lives in `util`, and where a genuine dependency exists a runtime-installed oracle
trait bridges the gate rather than a `#[cfg]`. Only the first half was needed here.

**Gating the call was considered and rejected.** With `#[cfg]` at the call site, a `path~`
shape in the lean build would match on tool+action alone — the exact silent mismatch the
predicate exists to prevent, warned about in its own doc comment at `guide_index.rs:177`.
A compile error beats a build-dependent semantic fork.

**Class (fix idea 2).** `cargo check --no-default-features` added to the documented gate in
`CLAUDE.md` § Development Commands, with the reasoning recorded inline. ~10s incremental.

**Fix idea 3 not done** — no test asserting that nothing outside a `librarian` cfg names
`crate::librarian`. Idea 2 subsumes it for anything that reaches a commit; idea 3 would only
localise the error message. Not carried as open work.

### Verification

- `cargo check --no-default-features` — exit **101** before, exit **0** after.
- `cargo test --no-default-features --lib librarian_response` — 2 tests, **observed running**
  in the lean build rather than assumed to. The new module's doc comment claims they do; this
  is that claim checked rather than asserted.
- Caller side was already covered: `guide_index.rs:720` exercises `path~` through
  `Shape::matches` with both a matching and a non-matching result.
- Full gate: fmt clean; clippy `--workspace --all-targets --features local-embed -D warnings`
  exit 0; `cargo test` 4732 passed / 0 failed / 46 ignored.

## Status notes

Filed from the operator-rules Phase 1 merge session, which verified the break was
pre-existing on `experiments` and merged anyway on that basis — the branch's own lean build
was clean, and refusing to merge would not have repaired the base.

Fixed in the following session, from a plain "run the tests" request. The lean build was run
only because this file said to, and it failed exactly as recorded here — which is the whole
return on having written it down.

**One correction the reproduction forced.** This file was written believing the repo had no
lean guard. It has one, but only inside the `test` job's 3-OS matrix; the fast `clippy` job
runs two passes, **neither** with `--no-default-features`. So the class is caught late and
expensively rather than not at all — which is precisely why an instance could sit on
`experiments` long enough for merging sessions to inherit it. The gate addition moves
detection to author time; it does not replace the CI lane.
