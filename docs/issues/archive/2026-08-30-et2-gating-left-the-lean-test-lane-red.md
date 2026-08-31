---
kind: bug
status: fixed
tags:
- ci
- retrieval
- feature-gates
- lean-build
- cluster/repro-env-diverges-from-gate-env
closed: 2026-08-30
opened: 2026-08-30
owner: marius
related:
- docs/trackers/resume-embedding-transport-stages-1-3.md
severity: high
---

# BUG: ET-2's transport gating left CI's `no-features` test lane red, and no local gate command can see it

## Summary

`2c6f2677` (ET-2) made the HTTP embed transport optional behind `remote-embed`.
**Three** pre-existing, ungated tests assert that a call a lean build can no
longer satisfy **succeeds**, so `cargo test --workspace --no-default-features`
had failed since 2026-08-29 — exactly the command CI's `no-features` lane runs,
on three OSes (`.github/workflows/ci.yml:174`).

Every gate command in CLAUDE.md passed on the same tree throughout. The lean
gate is `cargo check --no-default-features` — a *check*. It compiles; it does
not run.

Fixed by `f3dbfdf4` (§ Fix). The fix is deliberately **not uniform** across the
three.

## Symptom (Effect)

At `16dc28a5`, `cargo test --workspace --no-default-features` → **3210 passed,
3 failed**:

```
retrieval::client::selection_tests::build_embedder_accepts_a_url_with_the_defaulted_local_model
retrieval::client::selection_tests::build_embedder_still_accepts_a_url_with_an_ordinary_model
agent::tests::memory_embedder_is_built_from_the_shared_code_embedder
```

The first two panic out of `src/retrieval/client.rs`:

```
thread '...' panicked at src/retrieval/client.rs:1034:9:
a url with the DEFAULT local: model is the ordinary remote deployment — rejecting it would break every setup that configures a url and no model

thread '...' panicked at src/retrieval/client.rs:1050:9:
a url with a non-local model is the ordinary remote setup and must build
```

Exit code 101.

## Reproduction

```
cargo test --workspace --no-default-features
```

**Run it unfiltered.** See § *A correction this file owes* — a scoped run of
this bug returns a count that reads as a total, and undercounts it.

## Environment

Linux (Arch, 7.1.11-arch1-1), rustc 1.97.1, branch `experiments`, project
codescout. Not OS-specific — the condition is a compile-time feature arm, so
all three CI OSes are affected identically.

## A correction this file owes

**This file originally reported 2 failures. There were 3.**

The original run was `cargo test --no-default-features --lib -- retrieval::client::selection_tests`
— scoped to 19 tests. It reported `17 passed; 2 failed`, and this file then
asserted:

> The full CI form (`cargo test --workspace --no-default-features`) fails the
> same way; the `--lib` filter above is just faster.

That sentence was never run. It was an inference written in the voice of a
measurement, and it was wrong: the filter never reached `agent::tests`, where
the third failure lives. The unfiltered run at `16dc28a5` gives 3210/3.

The generalisable trap: **a filtered test run returns a count that reads as a
total.** `17 passed; 2 failed` is a true sentence about 19 tests and says
nothing about the lane, and nothing in the output marks the difference — the
`N filtered out` line reads as bookkeeping rather than as the caveat it is.
Same shape CLAUDE.md already warns about for the prompt-eval harness
(`Summary: 1/1 passed` being a scenario count, not a run count); this is a
second instrument with the same failure mode.

Corrected 2026-08-30, after `fix-embedding-transport-stage-1` measured the
unfiltered lane in a detached worktree at clean HEAD.

## Root cause

`2c6f2677` added a lean arm to `RetrievalClient::build_embedder_for_url`
(`src/retrieval/client.rs`) that unconditionally bails:

> an embedder url is configured, but this build has no HTTP embed transport.
> Rebuild with --features remote-embed, or unset [embeddings].url …

The two `selection_tests` call `RetrievalClient::build_embedder(&c, /* lite */ true)`
with `embedder_url: Some(...)` and assert `.is_ok()`. The third,
`agent::tests::memory_embedder_is_built_from_the_shared_code_embedder`, asserts
`Arc::ptr_eq` on an `EmbedderHttp` instance. None carried
`#[cfg(feature = "remote-embed")]`, unlike the five sibling `selection_tests`
ET-2 did gate.

**measured 2026-08-30**, by running the suite at both commits rather than
reading the diff:

- detached worktree at `2c6f2677^` → `retrieval::client::selection_tests`
  **24 passed, 0 failed**.
- at `16dc28a5`, unfiltered → **3210 passed, 3 failed**.

So the regression is `2c6f2677`, not something older ET-2 merely exposed. (The
drop in collected `selection_tests` is ET-2 correctly gating five others; that
part is intended.)

**Ambient environment ruled out**, which mattered because this host carries 14
stale `CODESCOUT_*` vars in the MCP server's inherited env (including a
`CODESCOUT_RETRIEVAL_PROFILE=amd` that `.env` no longer sets). Re-running with
all 14 stripped via `env -u` reproduced the failures identically, so the cause
is the compile-time feature arm, not host config.

## Evidence

### Why no local gate command catches it

CLAUDE.md's four gate commands, run on the tree while the lane was red:

```
cargo fmt --check                                                            → OK
cargo clippy --workspace --all-targets --features local-embed -- -D warnings → OK
cargo test                                                                   → 4819 passed, 0 failed
cargo check --no-default-features --all-targets                              → OK
```

All four green. The first three build **with** default features, so they never
reach the bail arm. The fourth builds lean but is a `check` — it compiles the
test targets and never executes them. **No command in CLAUDE.md runs the lean
test suite.**

This is the third instance of one family, and the first the previous remedy
cannot reach:

| Instance | Failure kind | Reached by |
|---|---|---|
| `ET-7` — `rendezvous_poll_for_test` dead lean | compile-time warning | `--all-targets` (fixed, `141b69a3`, ET-9 T12) |
| ET-9 T12 generally | compile-time | `--all-targets` |
| **this one** | **runtime** | **only actually running the lean tests** |

Compiling is not running, so `--all-targets` was green and stays green whichever
way this is fixed.

**A fourth floor, found by the fix itself.** The first version of the repair
used `expect_err` in the lean arms. It compiled clean by default and failed to
compile **lean** — the `Ok` variant is `Arc<dyn CodeEmbedder>`, which is not
`Debug`, so the bound does not hold; `match` is required. A lean-only *compile*
failure can therefore be introduced by the very patch repairing a lean-only
*runtime* failure.

### The CI lane that does catch it

`.github/workflows/ci.yml:174`:

```yaml
- run: cargo test --workspace ${{ matrix.config.flags }}
```

with `matrix.config` including `{name: no-features, flags: "--no-default-features"}`
across `ubuntu-latest`, `macos-latest`, `windows-latest`.

## Hypotheses tried

1. **Hypothesis:** the failures were caused by an uncommitted T11 retrofit in
   `src/retrieval/{search,sync}.rs` in the same tree.
   **Test:** backed both files up, `git checkout --` them, re-ran on a pristine tree.
   **Verdict:** rejected — still fails at HEAD with no local modifications. The
   retrofit touches neither `client.rs` nor `build_embedder`.

2. **Hypothesis:** stale ambient `CODESCOUT_*` env on this host makes
   `build_embedder` take a different arm locally than it would in CI.
   **Test:** re-ran with all 14 ambient `CODESCOUT_*` vars stripped (`env -u` ×14).
   **Verdict:** rejected — identical failures, identical messages.

3. **Hypothesis:** long-standing breakage ET-2 merely surfaced.
   **Test:** ran the filtered suite in a detached worktree at `2c6f2677^`.
   **Verdict:** rejected — 24 passed / 0 failed there. `2c6f2677` is the
   regressing commit.

4. **Hypothesis (this file's own, and wrong):** the filtered count of 2
   characterises the lane.
   **Test:** unfiltered `cargo test --workspace --no-default-features` at clean
   `16dc28a5`.
   **Verdict:** rejected — 3 failures. See § *A correction this file owes*.

5. **Hypothesis:** all three should be gated, matching ET-2's five siblings.
   **Test:** determine empirically whether `guard_local_model_with_url` runs
   before or after the transport bail —
   `build_embedder_rejects_a_url_combined_with_a_local_dir_model` is among the
   tests that PASS in a lean build, which is impossible if the bail preempts the
   guard.
   **Verdict:** rejected. The guard runs first, so the two `selection_tests`
   retain a lean-meaningful claim and must NOT be gated. Only the third should be.

## Fix

**`f3dbfdf4` on `experiments`, patch-id `ff3ffccb37031041c8f9ed64cb2baaa35ef2004a`.**
Touches `src/agent/mod.rs` and `src/retrieval/client.rs`. Measured after:
`cargo test --workspace --no-default-features` → **3360 passed, 0 failed**.

**The fix is deliberately not uniform**, and that is its point: "gate all three"
looks correct for all three and is correct for exactly one.

- **The two `selection_tests` stay UNGATED**, branching only the assertion —
  `is_ok()` under `remote-embed`, and in a lean build the *specific* transport
  error. Their claim is "the guard must not over-fire", and that claim survives
  a lean build: the refusal must be the transport bail and must **not** be the
  guard. Gating them would delete the guard's non-over-firing proof from exactly
  the configuration where a mis-widened guard is hardest to see — there it hides
  behind a refusal that is legitimate anyway.
- **`memory_embedder_is_built_from_the_shared_code_embedder` IS gated.** Its
  subject is `Arc::ptr_eq` on an `EmbedderHttp` instance, and a build with no
  HTTP transport has no such instance to share. No lean-meaningful version of
  the claim exists, so gating removes nothing. (It mentions
  `guard_local_model_with_url` only to explain its model name; the assertion is
  pointer identity. Two readers misidentified its subject from that mention
  alone — only opening the test body separates them.)

The asymmetry is written into both files so a later reader does not "simplify"
the ungated pair into matching the gated one.

**Not fixed here, and larger than the bug:** no command in CLAUDE.md runs the
lean test suite. Adding `cargo test --workspace --no-default-features` to the
documented gate is the remedy. CLAUDE.md is the user's surface, so this is
raised, not edited.

## Tests added

None new — the three failing tests were themselves the regression detector,
working correctly and reporting a real configuration-dependent failure. The fix
repairs their assertions rather than adding coverage, and verifies by running
the previously-red lane green (3360/0).

## Workarounds

N/A — fixed.

## Resume

N/A — fixed and verified on `experiments`.

## References

- `docs/trackers/resume-embedding-transport-stages-1-3.md` — `ET-2` (the
  regressing commit), `ET-7` (the six design defects found executing it, plus
  the `rendezvous_poll_for_test` sibling), `ET-9` T12 (the `--all-targets`
  instance of this family).
- `.github/workflows/ci.yml:164-174` — the `no-features` test lane.
- `src/retrieval/client.rs` — the lean bail arm and the two ungated tests.
- `src/agent/mod.rs` — the third test, now gated.
