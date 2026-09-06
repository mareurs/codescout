---
id: '4a9ffbd8789df751'
kind: bug
status: open
title: 'BUG: the four-command gate never compiles server-stack, so it certifies a feature set the shipped binary does not use'
tags:
- cluster/repro-env-diverges-from-gate-env
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
(`42769b490e11f106`, four days red). *Not measured:* how much code is hidden — no
enumeration of `server-stack`-gated modules was made, so the exposure is
demonstrated, not sized.

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

*Not implemented — this is a gate change and the tradeoff is a judgement, not a
defect fix.* Three options, with the cost each pays:

1. **Add a fifth command**, `cargo test --workspace --features server-stack`.
   Complete, and the most expensive: it pulls `qdrant-client` (tonic/prost/gRPC)
   into the gate's compile budget for every session, on a gate whose ordering
   rationale is already about not wasting shared `target/` rebuilds. It also
   **needs a reachable Qdrant or it reproduces `42769b490e11f106`'s failure on
   every run** — which is either a feature (it would have caught that bug) or a
   permanent local red, depending on whether the affected tests are made
   backend-independent first.
2. **Fold the feature into the existing clippy line** —
   `--features local-embed,server-stack`. Cheap, catches compile errors and lint
   failures, catches **no** runtime failure. Would not have caught
   `42769b490e11f106`.
3. **Leave the gate and fix the expectation** — state in `CLAUDE.md` that green
   does not cover `server-stack`, and that the CI lane is the only check. Costs
   nothing, catches nothing, and makes the limit legible instead of invisible.

Option 3 is not a null option here: this repo's own § *Observer Blindness*
argues that publishing a bound's **scope** at the read surface is the repair when
re-checking is impossible. But it is the weakest of the three, and picking
between them is a call for the repo owner.

**Whichever is chosen, the mechanism belongs in the gate, not in a resolution to
remember** — a session cannot notice a feature the gate does not name.

SHA: *(not fixed)*
patch-id: *(not fixed)*

## Tests added

None — nothing is fixed. When it is: the guard must fail when a
`server-stack`-gated compile error exists, which means the gate change itself is
the test. A test asserting `CLAUDE.md` contains the string `server-stack` would
be monotone under the gate being wrong in any other way, and would pass on option
3 while catching nothing.

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
