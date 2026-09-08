---
id: '547725aa19652455'
kind: bug
status: taken
title: The p50 emission byte ceiling measures the fixture's tempdir path length, so it reds by platform
tags:
- cluster/repro-env-diverges-from-gate-env
topic: guide-hint emission budget
claimed_at: 2026-09-08
claimed_by: 5399543d-22d6-4ed9-9ebb-876be459989f
---

# BUG: a margin-0 byte ceiling measures the fixture's absolute root path, so it reds by platform

## Summary

**WIDENED 2026-09-08, after the fix, by `c9ab2c8d` reading the run I had stopped reading.** This
file was written as a macOS finding. It is **three lanes across two platforms**, and every one of
them failed on *this* test — verified in run `34222332438`, one FAILED test per job:

| lane | total | over |
|---|---|---|
| `macos-latest / default` | 12275 B | +31 |
| `windows-latest / default` | 12262 B | +18 |
| `Windows-gnu cross (MinGW + wine)` | 12257 B | +13 |

Windows temp paths are longer than `/tmp/.tmpXXXXXX` too. The `local-embed` and `no-features`
siblings went green **not because they were unaffected but because this test only exists under
`default`** — which is why the platform split read as macOS-only: the two lanes that would have
corroborated it were being read as the separator bug's, and the separator bug really did own the
other two.

**And it masked a peer's verification.** `windows-latest / default` and `Windows-gnu cross` never
ran `59112612`'s separator regression test at all: `cargo test` aborts after the `--lib` binary
fails, so the `tests/` integration binaries never started. Measured, with a control, because the
first version of this check used a `^test ` anchor that matches nothing in a timestamped CI log and
returned a vacuous zero:

```
CONTROL  lines matching 'test .+ \.\.\. ok'   5157 / 5148    <- the grep works
         92's separator regression test          0 /    0
         'test result:' lines                    1 /    1    <- only --lib ran
```

So for two lanes, **this defect was the reason a different session's fix could not be verified**,
and nothing in either job's output said so — the report named my test and stopped.

`server::guide_hint_tests::a_p50_session_stays_under_the_committed_emission_byte_ceiling`
(`src/server.rs:10249`) exists to catch **guides growing**. It also measures the **length of the
fixture's temp-directory path**, at exactly 1 byte of budget per character, against a ceiling whose
real margin is **21 B**.

So the same commit, with byte-identical guide content, is green on Linux and red on macOS.
Observed 2026-09-08 on CI run `34222332438`: `ubuntu-latest / default` **success**,
`macos-latest / default` **failure** at `12275 B` against `CEILING = 12244`.

## Symptom (Effect)

```
p50 session emitted 12275 B after the primary (whole librarian topic is 23923 B,
ceiling 12244 B, margin 0 B). Raising CEILING is a spec amendment, not a fix ...
The standing remedy is decomposing the Body Editing Surfaces section in the librarian guide ...
```

**The message is emphatic and, for this failure, wrong.** Nobody grew a guide. A reader following
it would compress a guide section by 31 bytes, watch macOS go green, and ship a gate that still
measures its own environment -- with 31 B of real headroom spent buying nothing. That is this
repo's remedy-text law one turn on: the predicate is right, the addressee is right, and the
**instruction** sends you to the wrong file.

## Reproduction -- on Linux, no macOS required

`tempfile` honours `TMPDIR`, so lengthening it reproduces the macOS geometry directly:

```
$ cargo test --workspace a_p50_session_stays_under                      # /tmp
   ok
$ TMPDIR=/tmp/var-folders-k1-2m3n4p5q6r7s8t9u0v-wxyz0000gn-T \
    cargo test --workspace a_p50_session_stays_under                    # 51 chars
   FAILED -- p50 session emitted 12270 B
```

## Root cause

`shape_total` sums `guide_blocks(out)`, which is **every content block after the primary** -- a
deliberate widening, recorded on `CEILING`, from an earlier guide-only filter. One of those blocks
is `post_process`'s once-per-activation banner, built at `src/server.rs:835`:

```rust
"\n[codescout] paths are relative to {root}"
```

`{root}` is the fixture's `tempdir()` absolute path. It lands inside the counted bytes, so the
budget carries a term that is a property of the machine rather than of the guides.

**The `CEILING` comment already tracks environment-dependent terms and missed this one.** It says
`_workspace_notice` is *"0 B here (a bare tempdir has no linked worktrees) but ~600 B/call in a
real repo"* -- correct, and about the sibling block. The banner is named in the same list as
*"244 B in this fixture"*, a constant, which it is not.

## Derivation -- measured, not reasoned

| `TMPDIR` length | total |
|---|---|
| 40 | 12259 B |
| 60 | 12279 B |
| 80 | 12299 B |

Exactly **1 B per character**, so the root appears **once**: `total = 12219 + len(prefix)`.

- Linux `/tmp` (4) -> **12223 B**, margin **21 B** -- not the 0 B the message prints, which is the
  post-failure margin.
- The model predicts CI: macOS `12275` implies a 56-char prefix, which is what
  `/var/folders/xy/<43-char hash>/T` measures. Derived before that path was known, then matched.

**So the gate's true headroom is 21 B, and any root 21 characters longer than `/tmp` reds it** --
independent of guide content. A developer on a deep checkout path can hit this with no CI involved.

## Fix

**Committed, not yet closed — and the reason is this bug's own subject.** The fix is in and the
four-command gate is green (FMT 0, CLIPPY 0, LEAN 0, DEFAULT 0), with both mutations red first.
But the claim being made is *"macOS now agrees with Linux"*, and no macOS lane has said so yet.
Closing on a Linux gate would be the same move that shipped the defect: treating one platform's
silence as the population. Held `taken` until a `macos-latest / default` verdict lands.

**`LEAN exit=0` is vacuous here, derived rather than assumed.** `guide_hint_tests::` runs **0**
times in `--no-default-features` and **46** in the default lane; control, so the zero is a
measurement and not a broken grep: `prompts::` returns **101 in both**. Only the default lane
exercises this change.

Normalise the root out of the counted bytes at the measurement site, keeping the banner **in** the
population -- the widening to "every block after the primary" was deliberate and is not being
reverted. What is removed is only the part that varies by machine.

## Tests added

A paired assertion inside the same test, monotone in opposite directions:

- **control** -- the fixture root DOES appear in the raw blocks. Without it, the property assertion
  below passes trivially against a build where the banner has vanished, which is the same shape as
  the `total <= CEILING` weakness the `CEILING` comment already documents.
- **property** -- the root does NOT appear in the normalised blocks that feed `total`.

Neither is a re-implementation of the production path: both read the same `guide_blocks` output the
budget is computed from.

## References

- `src/server.rs:835` -- the banner that interpolates `{root}`.
- `src/server.rs:10112-10130` -- the `CEILING` population note that tracks `_workspace_notice`'s
  environment dependence and treats the banner as a constant.
- `docs/issues/archive/2026-08-28-capped-get-body-round-trips-into-truncating-write.md` -- the
  precedent the message cites, where compressing the section rather than raising the ceiling was
  the right call. It was; this is a different failure wearing the same message.

## Attribution

Found by reading a CI red rather than by auditing: `macos-latest / default` failed alone in run
`34222332438` while `ubuntu-latest / default` passed the same commit, and a deterministic byte
count over `include_str!`'d markdown cannot legitimately do that.
