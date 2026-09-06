---
id: d2ba6a00f3fb0061
kind: bug
status: fixed
title: 'BUG: the four-command gate never compiles server-stack, so it certifies a feature set the shipped binary does not use'
tags:
- cluster/repro-env-diverges-from-gate-env
closed: 2026-09-06
opened: 2026-09-06
owner: marius
related: []
severity: high
unverified: No count of how much code this hides. `qdrant-client`-gated modules were not enumerated; the one measured consequence is a single four-day-red CI job.
---

# BUG: the documented gate never compiles the feature set that ships

## Summary

`CLAUDE.md` § *Development Commands* prescribes four commands and mentions
`server-stack` **zero times**. `server-stack` is not in `default`, so none of the
four compiles the Qdrant backend. The live-MCP binary is built by `cargo rb`,
which *does* enable it. A session that follows the gate exactly gets a green that
structurally cannot see any defect in the code its own MCP server runs.

## Symptom (Effect)

CI stayed red for **four days** on a defect no local gate could reach. The CI job
is named, in the workflow, *"Test (server-stack — the build `cargo rb` ships)"* —
the divergence is stated in the job's own title and nowhere a local session
reads.

There is no error string for this bug. **The symptom is a green gate**, which is
what makes it expensive: the gate's four commands are followed carefully,
documented with a load-bearing ordering rationale, and the compliance is real.

## Reproduction

```
grep -c 'server-stack' CLAUDE.md          # 0
grep 'rb = ' .cargo/config.toml           # build --release --features server-stack,local-embed
grep '^default' Cargo.toml                # default = ["remote-embed", "http", "librarian"]
```

Then: introduce any error in a `#[cfg(feature = "server-stack")]` block and run
the full four-command gate. It passes.

## Environment

`experiments`, all platforms. Independent of machine state.

## Root cause

Three facts that are individually correct and compose into a hole:

| where | fact |
|---|---|
| `Cargo.toml` | `default = ["remote-embed", "http", "librarian"]` — no `server-stack` |
| `.cargo/config.toml` | `rb = "build --release --features server-stack,local-embed"` |
| `CLAUDE.md` § *Development Commands* | four commands, `--features local-embed` on clippy only |

`server-stack` gates `dep:qdrant-client` plus the hybrid sparse + reranker query
path. With it off, `QdrantArtifactStore` and its callers are not compiled at all
— so the gate's silence about them is **absence, not a thinner sample**, exactly
as the lean lane's silence about the librarian is.

This is the lean-lane vacuity law with the polarity reversed, and that is why it
is easy to miss. `CLAUDE.md` already teaches *"the lean lane is VACUOUS for
librarian code — `--no-default-features` switches the librarian off"*, and a
reader who has internalised that will still not notice that the DEFAULT lane is
vacuous for `server-stack` code, because nothing in the gate names a feature it
does not pass.

*Measured 2026-09-06:* the greps above; one confirmed consequence
(`42769b490e11f106`, four days red).

*Sized 2026-09-06, paying the debt this paragraph used to declare* (it read *"the
exposure is demonstrated, not sized"*). `feature = "server-stack"` appears **96
times across 30 files**; stripping docs, the code behind it is **11 source files
and 2 test files**:

    src/retrieval/{memory_payload,client,memory,payload,code_store,mod}.rs
    src/librarian/{artifact_store,mod}.rs, src/librarian/catalog/chunk.rs
    src/agent/mod.rs, src/memory/semantic_store.rs
    tests/retrieval_unit.rs, tests/feature_lanes.rs

So the hole is a subsystem, not a corner — which argues for the bound being
*published*, and against it being re-derived by each reader.

## Hypotheses tried

1. **Hypothesis:** the gate is fine and CI simply tests more, as CI should.
   **Verdict:** **rejected**, on the gate's own stated purpose. The gate exists
   so a session can know its work is sound before committing, and `cargo rb` +
   `/mcp` is the documented dev loop — this repo's own MCP server runs the
   `server-stack` build. A gate that cannot see the binary the developer then
   *runs* is not a narrower gate, it is one whose green means something other
   than what its readers take it to mean.

2. **Hypothesis:** `--all-targets` on the clippy line already covers it.
   **Verdict:** **rejected.** `--all-targets` widens *targets* (tests, benches,
   examples); it does not enable *features*. Different axis, and the similarity
   of the words is part of why this survives review.

## Fix

**Option 3, and the option list needed correcting before it could be picked.** Fixed
2026-09-06 by adding a bullet to `CLAUDE.md` § *Development Commands*, beside the
lean-lane vacuity law it is the mirror of.

**What changed the decision: a corpus check the original three options were written
without.** The framing above assumed nothing covers `server-stack`. Something does,
and has since 2026-08-08:

- `.github/workflows/ci.yml` has a dedicated `test-server-stack` job — `clippy
  --features server-stack --all-targets -- -D warnings` plus `cargo test --features
  server-stack`, Linux-only and hermetic (no Qdrant service container; the tests
  return false when the stack is unreachable). Shipped in `ecf3e461` for
  `docs/issues/archive/2026-08-08-server-stack-gated-tests-never-compiled-by-any-lane.md`,
  which is **this same defect one layer out** — then, no lane anywhere compiled it.
- `tests/feature_lanes.rs` guards that the lane keeps existing:
  `every_declared_feature_has_a_lane_or_a_reason` reds the build if a declared
  feature has neither a CI lane nor an explicit `EXEMPT` entry, and
  `the_guard_is_not_vacuous` asserts the guard's own inputs are non-empty —
  including, by name, that `server-stack` appears in a workflow — so it cannot pass
  by finding nothing.

So `server-stack` **is** compiled and tested, with a guard on the guard. The defect
is narrower than this file originally read: not that nothing covers it, but that the
**local** gate does not and `CLAUDE.md` never said so, leaving every session to read
local green as full coverage.

That collapses the three options rather than leaving a judgement call:

- **Option 1 (fifth command)** — now clearly wrong. It duplicates a guarded CI lane
  and pulls tonic/prost/gRPC into every session's compile budget, on a gate whose
  own ordering rationale is about not wasting shared `target/` rebuilds.
- **Option 2 (fold into clippy)** — same objection, smaller. Also catches no runtime
  failure, so it would not have caught `42769b490e11f106`.
- **Option 3** — and its costing here was wrong. It was written as *"costs nothing,
  catches nothing"*. CI catches it and a test guards the catcher; the only thing
  missing was the reader's expectation. That is not the weakest option, it is the
  whole remaining gap.

**Why a sentence is the right shape, and not a cop-out.** § *Observer Blindness*
position 3 already names this exact repair: *a bound that lives in the enforcement
layer — a test module header, a gate script, a hook — is correctly published to an
audience that never reads it, and the fix is to move the scope to the READ surface,
not to record the lesson.* The bound lived in `tests/feature_lanes.rs`'s module
header and in the CI yaml. A session running the four commands opens neither. So
`CLAUDE.md` is not a fallback here — it is the surface the reader is actually on.

**The change**: one bullet in `CLAUDE.md` § *Development Commands*, immediately after
the lean-lane bullet, stating that the default lane is vacuous for `server-stack`,
naming the CI job and the guard that keeps it alive, saying explicitly not to add a
fifth command and why, and ending with the operative instruction — *never report
"gate green" as coverage for `server-stack` work; read the CI job*. The pinned
`claude_md_gate_lists_its_four_commands_in_the_load_bearing_order` scopes to the
directive sentence only, which is untouched.

**Fixed on `experiments`, in two commits — the sentence and the guard that keeps it.**

| what | SHA (`experiments`) | patch-id |
|---|---|---|
| the CLAUDE.md bullet | `29f1940c` | `7722deef3bcee1d70818477ad138e83eb955133e` |
| the test pinning it | `c0cfa326` | `6e0447b4504e0c2d173112d0eede0c4e38e13bf6` |

Both recorded now rather than owed later: the SHA is positional and dies when
`experiments` is rebased, which happens after every ship; the patch-id is a content
hash of the diff and survives rebase and cherry-pick alike. There is no
pending-master line to reconcile.
## Tests added

`claude_md_gate_section_names_the_server_stack_blind_spot_and_its_live_guard`
(`src/prompts/mod.rs`, beside the existing gate-order test), asserting in **both**
directions — which is the whole design, because a one-way check rots in whichever
direction it is not looking:

- **CLAUDE.md must cite the guard.** Its gate section must name `server-stack`, the
  `test-server-stack` CI job, and `every_declared_feature_has_a_lane_or_a_reason`.
  Deleting or softening the bullet reds the build.
- **The guard must still exist under that name.** `tests/feature_lanes.rs` must
  define that function, so renaming or removing it also reds the build instead of
  leaving `CLAUDE.md` pointing confidently at nothing. An unresolvable citation is
  indistinguishable from a live one — `cluster/doc-contradicted-by-code`, and this
  is the cheap way to be immune to it.

Scoped to the gate section rather than the whole file, for the reason the sibling
gate-order test documents: `server-stack` is discussed elsewhere in this repo, so a
file-wide `contains()` would pass on a mention with nothing to do with the gate.

**Mutation-verified in both directions, rather than trusted for existing:**

| mutation | result |
|---|---|
| renamed `every_declared_feature_has_a_lane_or_a_reason` in `tests/feature_lanes.rs` | RED — *"tests/feature_lanes.rs no longer defines it"* |
| replaced the CI job name in the `CLAUDE.md` bullet | RED — *"no longer names `test-server-stack`"* |

**What it does NOT do, stated so nobody credits it with more:** it does not compile
`server-stack` and cannot. It guards the *sentence* and the *citation*. The code is
covered by CI's `test-server-stack` job, and that job's continued existence is
covered by `every_declared_feature_has_a_lane_or_a_reason` — which this test now
keeps CLAUDE.md honest about. Three links, each guarding the next.
## Workarounds

Run `cargo test --workspace --features server-stack` by hand before committing
anything touching `src/retrieval/`, `src/librarian/artifact_store.rs`, or any
`#[cfg(feature = "server-stack")]` block. Note this currently fails on
`42769b490e11f106` unless a Qdrant is reachable.

## Resume

Decide between the three options above with the repo owner. If option 1: fix
`42769b490e11f106` first (done, `a_failing_vector_store_does_not_refuse_the_move`),
then audit the remaining `server-stack` tests for daemon dependence before adding
the command — otherwise the new lane is red by default and gets ignored, which is
worse than absent.

## References

- `docs/issues/archive/2026-09-06-a-move-is-refused-when-the-vector-store-is-reachable-but-failing.md`
  (`42769b490e11f106`) — the one measured consequence. Sibling, not duplicate:
  that is the defect, this is the blindness.
- `docs/conventions/gate-ordering.md` — why the four commands are those four, in
  that order. Any change lands there too.
- `CLAUDE.md` § *Development Commands* — the executable copy, pinned by
  `claude_md_gate_lists_its_four_commands_in_the_load_bearing_order`, which
  would need updating with any option that changes the command list.

### Cluster adjudication

`cluster/repro-env-diverges-from-gate-env`. The gate environment and the shipped
environment differ by a feature flag, and every consequence is a defect that is
green locally and red where it matters. The rival was
`cluster/guard-narrower-than-its-name` — rejected because the gate is not
narrower than its *name*; it is narrower than its **readers' belief**, and its
name says nothing false. That distinction is the reason this needs its own
record rather than folding into `42769b490e11f106`.
