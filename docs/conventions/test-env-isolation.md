---
kind: convention
status: active
title: Test environment isolation
owners: []
tags:
  - testing
  - concurrency
  - librarian
---

# Test environment isolation

Any test helper that constructs an object resolving configuration from
process-global env vars MUST isolate that state from concurrent tests.
"Object" here primarily means the librarian `Agent` (reads
`LIBRARIAN_DB`, `LIBRARIAN_WORKSPACE`, `LIBRARIAN_CWD`,
`LIBRARIAN_EMBED_*`), but the convention applies to any future builder
with the same shape.

Promoted from U-20 in `docs/trackers/codescout-usage-frictions.md`
after the #68 stability work surfaced a months-long latent SQLite race:
`make_server()` in `src/server.rs::guide_hint_tests` returned
`(TempDir, CodeScoutServer)` — looked self-contained — but built an
Agent that read `LIBRARIAN_DB` from process env, falling back to a
shared default `dirs::data_local_dir().join("librarian/catalog.db")`
when unset. Every parallel test calling that helper raced on the same
DB file. POSIX advisory locks usually hid the race on Linux; Windows
mandatory locks deadlocked routinely.

## The rule

A test helper that constructs an Agent (or any object that reads config
from process-global env) MUST resolve that config **before** the object
is built, and take it as an argument.

A. **Accept env values as explicit arguments — this is the rule.** The
   helper's signature names what it depends on; tests pass per-test
   values; no env mutation happens at all, so there is nothing to race.
   Where an existing constructor hard-codes env reads, add a `from_env()`
   wrapper that does the reading once at the edge and hand the resulting
   struct inward. `LibrarianEnv::from_env` and `ServerEnv::from_env` are
   that shape.

B. ~~Return an `EnvGuard` (RAII) plus `#[serial_test::serial]`.~~
   **NOT VIABLE.** `a656f8cec220d347` established this empirically and
   fixed the class project-wide. `#[serial]` coordinates only among
   *annotated* tests: it takes a lock that non-annotated tests never ask
   for, so any untagged test elsewhere in the suite that reads or writes
   the same var still races, and `std::env::set_var` is process-global
   with no way to scope it. The guard restores faithfully and the race
   happens anyway. Do not reintroduce this pattern; do not "copy the
   EnvGuard pattern locally" into a new module.

C. **Document a `#[serial]` requirement on the helper's docstring.**
   Weaker than A and subject to the same limitation as B — it narrows the
   window rather than closing it. Acceptable only when the helper does not
   itself set env and the object's behaviour merely *depends* on ambient
   env, and only as a stopgap with a link to the work that will remove it.

Since Rust 2024, `std::env::set_var` is `unsafe` precisely because of
this: it mutates process-global state that other threads may be reading.
The compiler now says out loud what B tried to work around.
## Established exemplars

Resolve env at the edge, pass the result inward. Both live examples
follow the same shape — a plain struct of resolved values plus a
`from_env()` that is the *only* thing touching the environment:

| Helper | Location | Pattern |
|---|---|---|
| `LibrarianEnv::from_env` | `src/librarian/mod.rs` | struct of resolved values; env read once, at the edge |
| `ServerEnv::from_env` | `src/server.rs` | same shape, same boundary |
| `EmbedderHttp` `api_key` | `src/retrieval/embedder.rs` | reads `EMBED_API_KEY` in `new()`, stores `Option<String>`; a test injects the value directly |

Tests construct the struct literally with per-test values and never call
`from_env()`, so they need no guard, no `#[serial]`, and no cleanup — and
they can run in parallel, which the B pattern explicitly could not.

**The two `EnvGuard` exemplars this section used to list are gone.** They
lived in `src/librarian/mod.rs::tests` and `src/server.rs::guide_hint_tests`;
`a656f8cec220d347` removed them along with the rest of the class. If you
arrived here from an older link expecting to copy one, that is the bug this
rewrite closes.

One `EnvGuard` use remains in the tree, and it is not a counter-example:

- `src/agent/mod.rs` — server-stack gated, exempt.

The `src/librarian/indexer.rs` instance this section used to list as outstanding
debt is gone. It was added in `109c1ead` and removed in `45669701`, which split
the env read out into `write_embeddings_with` (taking `allow_dim_migration` as a
parameter) plus a pure `migrate_opt_in` predicate — so those tests now set no
environment at all. That is the shape to copy when you meet an `EnvGuard`: push
the env read up to the caller and unit-test the decision as a pure function.
## Diagnostic shape

The race is detectable in production CI as one of:

- Intermittent `"tool '<name>' not registered"` panics in tests that
  call `tool_by_name(...).unwrap()` — the librarian feature failed to
  register because catalog open / Agent init lost its env race.
- Intermittent `LIBRARIAN_DB` resolution to a path that was a previous
  test's tempdir (now dropped) — manifests as "no such file or
  directory" on first catalog op.
- On Windows: deadlocks instead of failures, because mandatory locks
  on the shared default DB file cause both readers to block forever
  rather than fail.

If you see this shape, suspect missing isolation in the test's helper
chain. The fix is option A: find what the helper reads from env, resolve
it at the call site, and pass it in. Reaching for a guard plus `#[serial]`
recreates the bug — see option B.

## Known gaps (open)

**Closed 2026-07-27 by `a656f8cec220d347`.** The deferred option 2 —
"move resolution off process-global env and onto explicit arguments" —
shipped, and it is now the rule above rather than a proposal.

The gap this section described was real and is worth keeping as the reason
the rule is shaped the way it is: `#[serial]` + `EnvGuard` was robust
*within* a module and did not coordinate *across* modules, so a
non-annotated test in module X racing an in-flight construction in module
Y's `#[serial]` block was always possible. That is not a hole in the
discipline — it is what the discipline could never cover, because
`#[serial]` only ever locks against tests that opt in.

Measured effect of the fix: `set_var` / `remove_env` occurrences in the
default `cargo test` build went **119 → 0**.

Nothing remains in this class. The last outstanding instance —
`src/librarian/indexer.rs`'s `EnvGuard` — was removed in `45669701`, and the bug
that tracked it is archived at
`docs/issues/archive/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md`.
The structural `#[serial]` limitation described above is permanent, not debt.
