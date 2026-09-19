---
id: c222737eedb69850
kind: bug
status: fixed
title: 'BUG: three config tests red on any machine whose environment configures codescout — the derive-the-list fix landed at one site, three siblings still hand-type it'
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
topic: test isolation and the embedding env alias surface
---

## Summary

Four tests fail on any machine whose ambient environment configures codescout — which is
**every MCP-spawned shell on this machine**, because the codescout server inherits the live
embedder config. They are not broken code. They are under-isolated tests: each neutralises a
**hand-typed** list of env var names, while `EmbedEnv::from_real_env` reads a canonical+deprecated
**alias chain** that the hand-typed list does not cover.

The sibling test that does NOT fail derives its list from `codescout::config::embedding_env::all_names()`.
That fix landed on 2026-09-17 at **one** site. Three siblings were left hand-typing, and this is
what that costs.

## Symptom (Effect)

`./scripts/gate.sh` reports `FMT=0 CLIPPY=0 LEAN=101 DEFAULT=101` on a **clean worktree at a green
commit**. Failing:

- `config_from_env_reads_overrides` (`tests/retrieval_unit.rs`)
- `config_from_env_and_project_env_wins_over_project_toml` (`tests/retrieval_unit.rs`)
- `config_from_env_and_project_prefers_project_toml_when_env_silent` (`tests/retrieval_unit.rs`)
- `tools::memory::tests::resolved_chunk_budget_reflects_the_projects_configured_model` (lib)

**The failure is indistinguishable from a real regression**, and it arrives at the moment a session
is checking whether its own change is safe to ship. The natural next move is to go debug working
code. That is the cost, and it is why this is filed rather than worked around.

## Reproduction

At `HEAD 78b65f4e`, clean tree, from a shell with the codescout env loaded:

```
./scripts/gate.sh          # LEAN=101 DEFAULT=101
```

Then, same tree, same commit, with the 15 `CODESCOUT_*` vars cleared:

```
env -u CODESCOUT_BM25_BOOST -u CODESCOUT_EMBED_URL -u CODESCOUT_EMBED_MODEL \
    -u CODESCOUT_RERANKER_URL -u CODESCOUT_RERANKER_PROTOCOL -u CODESCOUT_EMBEDDER_MODEL_NAME \
    -u CODESCOUT_QDRANT_URL -u CODESCOUT_EMBEDDER_URL -u CODESCOUT_EMBEDDING_URL \
    -u CODESCOUT_QUERY_PREFIX -u CODESCOUT_MODEL_DIM -u CODESCOUT_SPARSE_EMBEDDER_URL \
    -u CODESCOUT_EMBEDDING_DIM -u CODESCOUT_EMBEDDER_PROTOCOL -u CODESCOUT_RETRIEVAL_PROFILE \
    cargo test --test retrieval_unit
```

→ `test result: ok. 9 passed; 0 failed`.

## Environment

Observed 2026-09-19 from an MCP `run_command` shell (`CLAUDE_CONFIG_DIR=/home/marius/.claude-kat`).
15 `CODESCOUT_*` variables are set: `BM25_BOOST`, `EMBED_URL`, `EMBED_MODEL`, `RERANKER_URL`,
`RERANKER_PROTOCOL`, `EMBEDDER_MODEL_NAME`, `QDRANT_URL`, `EMBEDDER_URL`, `EMBEDDING_URL`,
`QUERY_PREFIX`, `MODEL_DIM`, `SPARSE_EMBEDDER_URL`, `EMBEDDING_DIM`, `EMBEDDER_PROTOCOL`,
`RETRIEVAL_PROFILE`.

**CI does not see this**, which is exactly why it survived: CI runs with a bare environment, so the
hand-typed list is sufficient there and the gate is green. The divergence is local-only and
therefore invisible to the surface that would normally catch it.

## Root cause

`temp_env::with_vars` **sets** the names it is given. It does not **unset** the ones it is not
given. So every ambient `CODESCOUT_*` name outside the test's hand-typed list stays live during the
test body, and the resolver reads it.

The embedding env surface is an alias chain — canonical plus deprecated — so a single logical
setting has several spellings. `CODESCOUT_EMBEDDER_URL`, `CODESCOUT_EMBED_URL` and
`CODESCOUT_EMBEDDING_URL` all exist. A test that sets the canonical name and leaves an unlisted
alias ambient gets the **alias's** value.

`config_from_env_uses_defaults_when_unset` is immune because it builds its unset-list from
`embedding_env::all_names()` — the declaration — plus five hand-named non-embedding extras. Its own
comment records this exact failure happening once before:

> This list used to be written out by hand, and when the deprecated-alias chain was introduced on
> 2026-09-17 it silently stopped covering what the resolver reads: a machine exporting
> `CODESCOUT_EMBED_URL` failed this test, whose subject had not changed.

And `all_names()`'s doc comment (`src/config/embedding_env.rs:124`) states the general form:

> A list that must be kept in step by hand is the same shape as the eight scattered `env::var` calls
> this module exists to replace. Deriving it here means the ninth name cannot silently un-isolate a
> test.

**The remedy was written, published, and applied to one call site.** The class was known and named
in-tree; knowing it did not propagate it. That is `CLAUDE.md` § *Testing Discipline*'s
"mutate once per guarded SITE, not once per feature" holding on the repo that wrote it.

## Evidence

Bisecting the leak by unsetting only part of the surface, all at `HEAD 78b65f4e`:

| vars cleared | `retrieval_unit` result |
|---|---|
| none (ambient MCP shell) | 3 failed |
| `CODESCOUT_EMBED_MODEL`, `CODESCOUT_EMBED_URL` only | **still 3 failed** |
| all 15 | 9 passed, 0 failed |

The middle row is the load-bearing one. With the two obvious deprecated aliases cleared, the
assertion still reads:

```
thread 'config_from_env_reads_overrides' panicked at tests/retrieval_unit.rs:71:13:
assertion `left == right` failed
  left: Some("http://127.0.0.1:48081")
 right: Some("http://eb:2")
```

`http://eb:2` is what the test set on the canonical name. `http://127.0.0.1:48081` is ambient — so a
**third** alias was still winning. Clearing two of three aliases produces a failure
byte-identical to clearing none, which is precisely why a partial hand-written list reads as
adequate.

## Hypotheses tried

- **"The tree is red / someone broke config resolution."** FALSIFIED — all four tests pass at the
  same commit with the environment cleared. Nothing in the source is wrong.
- **"It is the two deprecated aliases named in the 2026-09-17 comment."** FALSIFIED by the middle
  row above. Naming the two aliases a previous incident happened to involve is the same
  hand-enumeration error one level up.

## Fix

**FIXED 2026-09-19** — `e635d4dab44a4b434a300ceab2b8402088d43a4d`, patch-id `89bf0a9bbd81be5a32efa790eb8dbfc912e5b9ca`. All five env-touching tests in `tests/retrieval_unit.rs` now derive their isolation list from `embedding_env::all_names()` via one `with_isolated_retrieval_env` helper, which also pins `XDG_CONFIG_HOME`. Verified green in BOTH directions — ambient vars set and cleared. The guard this file asked for ships as `the_isolation_helper_clears_every_declared_embedding_name`, mutation-verified KILLED.

Make each failing test neutralise the **whole** embedding env surface before setting its own vars —
derive from `embedding_env::all_names()`, as `config_from_env_uses_defaults_when_unset` already
does. Do not re-type names; the re-typing is the defect.

**Verify in both directions.** A fix confirmed only in a cleared environment has not been confirmed:
the test must pass with the ambient vars **set** (that is the world the bug lives in) and with them
**cleared** (that is CI). Checking only the second reproduces the blind spot that let this ship.

Not proposed: changing `gate.sh` to clear the vars. That would hide the defect from the gate while
leaving every other runner — a direct `cargo test`, an IDE, a peer's shell — exposed, and it would
make the tests permanently dependent on a wrapper to be correct.

## Tests added

Pending. The regression assertion worth having is not another config case but a **guard**: that no
test in this family hand-enumerates embedding env names. That reds on the next re-typed list rather
than on the next new alias, which is the direction that actually recurs.

## Workarounds

Prefix the cargo invocation with the 15 `env -u` flags shown under *Reproduction*. This is a
workaround, not a fix — it makes one runner correct and leaves the rest exposed.

## Resume

Open. Locus: `tests/retrieval_unit.rs` (three tests) and the failing test in `src/tools/memory/mod.rs`.
The correct shape to copy is `config_from_env_uses_defaults_when_unset` in the same file.

## References

- `src/config/embedding_env.rs:124` — `all_names()` and its doc comment, which predicted this.
- `tests/retrieval_unit.rs` — `config_from_env_uses_defaults_when_unset`, the immune sibling.
- `CLAUDE.md` § *Testing Discipline* — "Mutate once per guarded SITE, not once per feature."
- `docs/conventions/test-env-isolation.md` — the ruling against env-mutating tests; this is its
  read-side twin.
