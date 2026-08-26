---
kind: bug
status: investigating
tags:
- test-isolation
- memory
- qdrant
- pollution
opened: 2026-08-26
owner: marius
related:
- docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md
severity: high
unverified: 'The writing lane is now established and fixed (part 1 of Fix, 2026-08-26). What''s still unverified: the ~1959 already-leaked fixture points have NOT been cleaned from the live Qdrant collection -- that is a deliberate, separate, not-yet-approved live-data operation (part 2 of Fix).'
---

# BUG: 91.5% of the live `memories` collection is test-fixture data

## Summary

The Qdrant `memories` collection on this host holds **2142 points**, of which
**1959 (91.5%) sit under 1909 distinct tempdir `project_id`s** (`.tmpcfUc6n`,
`.tmpBv3QkP`, …) and carry obvious fixture content (`hello`, `test-topic`,
`References \`src/a.rs\``, `We chose Rust for performance.`). Real projects
account for 183 points across 25 project_ids. Something in the test suite is
constructing a Qdrant-backed memory store against the live stack instead of
`InMemorySemanticMemoryStore`, and every run leaks a fresh tempdir project.

## Symptom (Effect)

Measured 2026-08-26 against `http://localhost:6333`:

```
total points          : 2142
distinct project_ids  : 1934
  tmp-named (fixtures): 1909 projects, 1959 points
  real-named          :   25 projects,  183 points
```

Sample fixture payloads:

```
 proj=.tmpcfUc6n  bucket=structured  title=architecture/decisions
     content: We chose Rust for performance.
 proj=.tmpBv3QkP  bucket=structured  title=test-topic
     content: References `src/a.rs`.
 proj=.tmpH8IdGd  bucket=structured  title=test/key
     content: hello
```

Two of the real-named points are themselves debris rather than memories:
`zz-probe-delete-me` (96 B) and a note titled *"Auto mode kept on — hard-deny
hooks neutralize the Bash-first nudge"*, neither of which has a file on disk.

## Reproduction

```
curl -s http://localhost:6333/collections/memories \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["result"]["points_count"])'
```

Then scroll with `with_payload: ["project_id"]` and count keys matching `^\.tmp`.
The full audit script used here is in this session's scratchpad
(`mem_audit.txt` holds its output); it is ~20 lines of paginated
`/points/scroll`.

## Environment

- Linux, branch `experiments`, HEAD `20d5d43f`
- Qdrant at `localhost:6333`, collection `memories`
- The retrieval stack is the `server-stack` default (Qdrant, not sqlite-vec)

## Root cause

**Established 2026-08-26.** `test_ctx_with_project()` (`src/tools/memory/tests.rs`) —
the shared context helper dozens of tests in that file call — built a real `Agent`
with no override of the memory embedder or semantic memory store. Confirmed by
exact string match: three call sites' literal fixture content matched the leaked
payloads above verbatim (`write_and_read_roundtrip`'s `"hello memory"`,
`memory_write_still_works_without_embedder`'s `"hello"`, and a third site's
`"References \`src/a.rs\`."`), plus `tests/integration.rs`'s equivalent
`project_with_files` helper for the `"We chose Rust for performance."` sample.

Earlier hypothesis 1 (below) correctly ruled out `src/tools/memory/tests.rs`'s
EXPLICITLY-ISOLATED tests, but missed that most tests in the same file call the
shared helper directly, with no isolation at all — the helper itself, not any
individual test body, was the gap.

`ctx.agent.memory_embedder()` / `ctx.agent.semantic_memory_store()` (called via
`cross_embed_memory` on every `Memory` write) resolve from AMBIENT CONFIGURATION
(env vars / `.env` files) whenever nothing has pre-populated the seam. On this
specific development machine, ambient env vars point at a real local embedder
(llama-server) and a real local Qdrant — so these nominally-isolated unit tests
silently performed real network writes into the live `memories` collection. This
is the exact mirror image of
`docs/issues/archive/2026-08-26-ci-test-lanes-red-because-one-test-reads-ambient-embedder-config.md`
(same root shape — a test resolving config ambiently instead of explicitly — but
that one caused CI failures where no ambient config existed; this one caused a
silent live-data leak where it did).

`docs/conventions/test-env-isolation.md` is the convention this violates.

## Evidence

### Real-named projects, by point count

```
    29  MRV-poc            17  eduplanner-ui       7  extended-crawler
    21  backend-kotlin      10  advisory-proposal…  7  prompt-engineering
    18  codescout            9  code-explorer       7  ie-pal-engine
                             8  workspace-mcp       7  manger-agent
```

25 real project_ids, 183 points. Every other point in the collection is fixture
data.

## Hypotheses tried

1. **Hypothesis:** these are stale points from human sessions activating
   temporary worktrees.
   **Test:** read the payload content of six sampled tmp-named points.
   **Verdict:** rejected. Content is `hello` / `test-topic` /
   `References \`src/a.rs\`` / `We chose Rust for performance.` — fixture
   strings, not prose anyone wrote as a memory. 1909 distinct tempdirs also
   matches repeated suite runs, not human activation.

## Fix

Two parts, and the order matters:

1. **[done] Stop the writes.** `test_ctx_with_project()` split into
   `test_ctx_with_project_raw()` (unisolated, only for the four tests that
   install their own stub) and `test_ctx_with_project()` (now pre-installs
   `InMemorySemanticMemoryStore` + a shared `FixedEmbedder` before any tool call
   can trigger ambient resolution) — every other call site in the file gets
   isolation with no changes needed. Added `SemanticMemoryStore::as_any()`
   (`#[cfg(test)]`, mirroring `DenseEmbedder::as_any`) so the regression test can
   assert on the resolved store's concrete type rather than only its behaviour —
   a round-trip alone doesn't distinguish the test double from a real store,
   since this sandbox's always-compiled SqliteVec fallback is itself naturally
   tempdir-scoped and round-trips successfully with no fix applied at all.

   `tests/integration.rs` needed a different mitigation: it's an external
   integration-test binary, so it cannot reach the `pub(crate)`
   `InMemorySemanticMemoryStore` or the `#[cfg(test)]`-gated `Agent` setters.
   Pinned its one memory-writing test's embedder to a closed local port via
   `project.toml` instead, mirroring the established recipe in
   `agent::tests::memory_embedder_is_built_from_the_shared_code_embedder`
   (`src/agent/mod.rs`) — `embed_document` fails fast before `cross_embed_memory`
   ever reaches `semantic_memory_store()`, so the store is never touched.

   **SHA:** `9cfca8e8` (`experiments`), patch-id `f8c3925180ca8e4a0c88b4b31dd4244ff9d8f3a7`
   — `src/memory/semantic_store.rs`, `src/memory/sqlite_semantic_store.rs`,
   `src/tools/memory/tests.rs`.
   **SHA:** `a4ee4aa7` (`experiments`), patch-id `944c73c410739ba6fd2ef805d915dbac857c90bb`
   — `tests/integration.rs`.

2. **[still open] Then clean.** A filtered delete on `project_id` matching
   `^\.tmp` is safe now that part 1 has landed. Also drop the two debris points
   named above. **Deliberately not done in this pass** — this is a live mutation
   of the shared Qdrant `memories` collection, used across every Claude Code
   profile and project on this machine, and is being left as a separate,
   explicitly-approved follow-up rather than bundled into the code fix.

**Do not add a "prune tmp projects" maintenance command as the fix.** That
converts a test-isolation bug into a recurring chore, and the chore is what
makes the bug invisible.

## Tests added

`test_ctx_with_project_writes_land_in_an_isolated_store`
(`src/tools/memory/tests.rs`) — asserts the default context's resolved store and
embedder downcast to the test doubles (`InMemorySemanticMemoryStore`,
`FixedEmbedder`), not merely that a write round-trips (which passes on this
sandbox even unfixed, via the tempdir-scoped SqliteVec fallback). Confirmed RED
first: the downcast assertion failed with the resolved store NOT being the
in-memory double, before `test_ctx_with_project()` was split.

All 102 pre-existing tests in `src/tools/memory/tests.rs`, all 9 tests in
`tests/integration.rs`, and the full `cargo test` (4543 passed) stayed green
throughout — including the two `..._without_embedder` tests, whose actual
assertions never depended on the embedder being genuinely absent.

## Workarounds

Nothing is broken for the user today: `recall` filters by `project_id`, so
fixture points under tempdir ids cannot surface in a real project's results.
The cost is collection bloat (11.7× the real data) and the risk that any future
path which queries the collection *unfiltered* inherits 1959 pieces of garbage.
That makes this a latent correctness bug rather than an active one — hence
`severity: high` with no user-visible symptom yet.

## Resume

Part 1 (stop the writes) is done and verified — no further scouting needed
there. What remains is entirely part 2: delete the ~1959 already-leaked fixture
points (`project_id` matching `^\.tmp`) plus the two named debris points
(`zz-probe-delete-me`, the file-less "Auto mode kept on" note) from the live
Qdrant `memories` collection. This is a live-data mutation, deliberately left as
a separate decision rather than folded into the code fix — get explicit
approval before running it, since the collection is shared across every project
and Claude Code profile on this host.

## References

- `docs/conventions/test-env-isolation.md` — the convention violated
- `bug-fix-session-log:W-56` — a test resolving an embedder from ambient config,
  red on every OS for a week; likely the same class
- `bug-fix-session-log:F-62` — the five feature-gated e2e lanes that stopped
  compiling invisibly
- `src/tools/memory/tests.rs:334` — the correct pattern
  (`InMemorySemanticMemoryStore`)
