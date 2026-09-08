---
id: '0ca13ad23c9c4bc3'
kind: bug
status: open
title: ResilientStdin's only test asserts about a copy of ResilientStdin, so the shipped type has zero coverage
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
topic: test coverage that reads as coverage
closed: ''
opened: 2026-09-08
severity: medium
unverified: The mutation that would OBSERVE the blindness was deliberately not run - src/server.rs is a shared checkout and an armed mutation is the hazard docs/issues/2026-09-08-an-armed-mutation-is-a-deliberate-red-no-observer-can-distinguish.md records. The claim rests on reachability (the test body never names ResilientStdin), which entails the blindness but does not observe it.
---

# BUG: `ResilientStdin`'s only test asserts about a copy of `ResilientStdin`

## Summary

`resilient_stdin_absorbs_would_block` (`src/server.rs:10545`) is the only test
for the EAGAIN backoff that keeps the MCP server alive on a briefly-`O_NONBLOCK`
stdin pipe (BUG-047). It never constructs, polls or names the shipped type. It
declares a **function-local** `ResilientReader<R>` (`src/server.rs:10549`) whose
`poll_read` is a hand copy of the production one, and asserts about that. Delete
the backoff from `ResilientStdin` and the test stays green.

## Symptom (Effect)

No runtime symptom. The defect is in the evidence, not the behaviour: production
is correct today, and the suite would report exactly the same green if it were not.

The test's own doc comment states the claim the code does not support:

```
/// Mirrors the production `ResilientStdin` backoff pattern (BUG-047): on
/// EAGAIN, arm a 1ms sleep, poll it to register the waker via the timer
/// reactor, return Pending. Production cannot be tested directly because
/// `ResilientStdin` is hard-coded to `tokio::io::Stdin`; this generic
/// version mirrors the state machine so regressions in the pattern are
/// caught by test.
```

"so regressions in the pattern are caught by test" is false of the shipped
pattern. Regressions in *the copy* are caught; nothing edits the copy.

## Reproduction

Tree: `beb33b3f` (`experiments`), worktree byte-identical to HEAD for
`src/server.rs` at 2026-09-08 17:40Z.

```
grep -n 'ResilientStdin' src/server.rs
```

Seven hits, none of them an assertion:

```
1832:struct ResilientStdin {
1838:impl ResilientStdin {
1847:impl tokio::io::AsyncRead for ResilientStdin {
2034:                .serve((ResilientStdin::new(stdin), stdout))
10508:// ── ResilientStdin ─────────────────────────────────────────
10538:/// Mirrors the production `ResilientStdin` backoff pattern (BUG-047): on
10541:/// `ResilientStdin` is hard-coded to `tokio::io::Stdin`; this generic
```

Definition, inherent impl, trait impl, one production call site, and three
occurrences inside comments in the test region. `ResilientStdin` is named in
zero test bodies, in this file and in the repo (`grep -rn 'ResilientStdin'
--include=*.rs` returns the same seven).

## Environment

Linux, `experiments` at `beb33b3f`. Reached in the `default` and `local-embed`
lanes only in the sense that the test runs there; the blindness is
platform-independent.

## Root cause

`ResilientStdin` (`src/server.rs:1832`) hard-codes `inner: tokio::io::Stdin`, a
concrete type with no constructor that yields a controllable reader. A test
cannot hand it an EAGAIN source, so the author wrote a generic
`ResilientReader<R>` inside the test function and exercised that instead.

The two `poll_read` bodies are currently equivalent — the copy at
`src/server.rs:10550` reproduces the production body at `src/server.rs:1848`
minus one `tracing::trace!` line. So there is **no drift today**; the defect is
that nothing detects drift tomorrow. Two independent bodies, one edited by
change, the other only by a reviewer who notices.

Measured 2026-09-08: read both bodies in full at `beb33b3f` and enumerated every
`ResilientStdin` reference in the tree (command and output above). The
blindness follows from reachability — a test whose code path never reaches a
symbol cannot observe a change to it — **not** from an observed mutation run;
see `unverified:` in frontmatter.

Two secondary consequences of the same hard-coding, both visible at the file tail:

- `WouldBlockThenData` (`src/server.rs:10512`) and its `AsyncRead` impl sit at
  column 0, **outside** either `#[cfg(test)]` block in the file (the only two are
  at `src/server.rs:2169` and `src/server.rs:7757`; `mod guide_hint_tests` closes
  at `src/server.rs:10506`). The `#[allow(dead_code)]` on it is the fossil: in a
  non-test build the `#[tokio::test]` fn expands away, the mock loses its only
  user, and the warning was silenced rather than the block gated.
- `mod guide_hint_tests` is gated `#[cfg(feature = "librarian")]` *and*
  `#[cfg(test)]` (`src/server.rs:7756-7757`), while this tail block is gated by
  neither — so it runs under different conditions from every other test in the
  file.

## Evidence

### The test body, in full, constructs no production type

`src/server.rs:10545-10596`. It builds `WouldBlockThenData`, wraps it in the
locally-declared `ResilientReader`, calls `read()`, asserts `b"hello"`. The
production `poll_read` at `src/server.rs:1848` is not on the path.

### The class this instantiates is already a written law

`CLAUDE.md` § *Testing Discipline*: *"mutate the PRODUCTION path, not the test's
inputs: a second level asserting about its own re-implementation is
indistinguishable from coverage until you break the thing that ships."* This is
that shape, and unusually it **announces itself in its own doc comment** — the
author named the limitation honestly, and the code then shipped as coverage.

## Hypotheses tried

1. **Hypothesis** — some other test covers the real `ResilientStdin`.
   **Test** — `grep -rn 'ResilientStdin' --include=*.rs .` across the tree.
   **Verdict** — rejected. Seven hits, all accounted for above; none in an
   assertion.

2. **Hypothesis** — the mirror has already drifted, making this a live defect
   rather than a latent one.
   **Test** — read both `poll_read` bodies at `beb33b3f`.
   **Verdict** — rejected. They are equivalent today (modulo one `tracing::trace!`).
   The bug is the absent guard, not a present divergence.

## Fix

Not fixed. The obstacle the doc comment names is removable in one line:

```rust
struct ResilientStdin<R = tokio::io::Stdin> {
    inner: R,
    backoff: Option<std::pin::Pin<Box<tokio::time::Sleep>>>,
}
```

The production call site (`src/server.rs:2034`) keeps working via the default
type parameter. The test then wraps the **real** type around the existing
`WouldBlockThenData`, the function-local `ResilientReader` is deleted, and the
mock moves inside a `#[cfg(test)]` block — which also retires the
`#[allow(dead_code)]` and the ungated tail.

## Tests added

None yet. The regression test is the fix: `resilient_stdin_absorbs_would_block`
rewritten to poll `ResilientStdin<WouldBlockThenData>`. Its acceptance criterion
is an **observed red** — deleting the backoff arm from `src/server.rs:1858-1885`
must fail it — which the current test does not satisfy.

## Workarounds

None needed; nothing is broken for users. For a reviewer touching
`ResilientStdin::poll_read`: the suite will not tell you if you break it. Read
`src/server.rs:10550` and keep the two bodies in step by hand until the fix
lands.

## Resume

Make `ResilientStdin` generic over `R: AsyncRead + Unpin` with
`R = tokio::io::Stdin` as the default (`src/server.rs:1832`), point the test at
the real type, and move `WouldBlockThenData` inside a `#[cfg(test)]` block.

**Before claiming it fixed, run the mutation that this file could not:** delete
the `WouldBlock` arm from `ResilientStdin::poll_read` (`src/server.rs:1858-1885`)
and confirm `cargo test --workspace resilient_stdin_absorbs_would_block` goes
RED. Restore under a `trap`, and check `git diff` is empty before doing anything
else — `src/server.rs` is on a shared checkout and several sessions build
against the same `target/`.

## References

- `docs/issues/archive/2026-04-22-resilient-stdin-spin-flood.md` — the original
  BUG-047 spin, which is what this backoff exists to prevent.
- `docs/trackers/issue-clusters/IC-14-guard-narrower-than-its-name.md` — the class.
- `docs/issues/2026-09-08-an-armed-mutation-is-a-deliberate-red-no-observer-can-distinguish.md`
  — why the confirming mutation was not run here.
- `CLAUDE.md` § *Testing Discipline* — the law, stated before this instance.
