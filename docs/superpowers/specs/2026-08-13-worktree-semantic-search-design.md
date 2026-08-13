---
id: cf3dee40ca22cbf2
kind: spec
status: draft
title: Worktree semantic search — reuse main's vectors with an exact-bytes bound
tags:
- worktree
- retrieval
- semantic-search
- code-store-parity
- design
topic: worktree semantic search
---

# Worktree semantic search — reuse main's vectors with an exact-bytes bound

**Date:** 2026-08-13
**Bug:** `docs/issues/2026-08-13-enter-worktree-desyncs-codescout-and-strands-semantic-search.md`
**Scope:** half 2 of that bug (semantic search in a worktree). Halves 1 and 3 are
explicitly out of scope — see *Not in scope*.

Every code citation below was **re-derived on `experiments`**. The investigation
that produced the bug file measured against `feat/local-onnx-query-path`, which
carries ~2,592 insertions across the retrieval query path; the mechanism holds on
both, but only the `experiments` line numbers are cited here.

## Problem

A linked git worktree activated as a codescout project gets `project_id` =
its own directory basename, because `project_id` is `project.name`
(`src/tools/semantic/semantic_search.rs:208-213`) which falls back to the root
directory's basename when `.codescout/project.toml` is absent
(`src/config/project.rs:527-529`) — and that file is gitignored, so no linked
worktree ever has one. The Qdrant collection is global
(`src/retrieval/config.rs:65-66`: `collection()` is `prefix + kind`), so
`project_id` is the sole discriminator. Result: zero matches, returned as a bare
`{"results": [], "total": 0}` with no hint — indistinguishable from a query that
legitimately matched nothing.

Serving main's vectors wholesale is not the fix. Measured 2026-08-13: forcing
`project_id` to main's value from inside a worktree returns main's chunks with
main's paths. A worktree's files diverge from main's by construction, so the naive
reuse is confidently-stale output.

## Decision

**Worktree search is query composition above `CodeVectorStore`, not a partition or
filter concept inside it.** A worktree query runs two `query()` calls — main's
`project_id` with the worktree's dirty paths excluded, and a per-worktree delta
`project_id` holding exactly the changed files — merged by score. The dirty set is
derived by content hash, not by a git diff.

**Context.** `project_id` means two different things across the two
`CodeVectorStore` implementations, and the trait signature
(`src/retrieval/code_store.rs:51-62`) hides the difference behind one parameter:

| | Qdrant | sqlite-vec |
|---|---|---|
| `project_id` is | a payload filter value in one global collection | the **storage partition** — `conn_for(project_id)` opens a per-project DB (`src/retrieval/sqlite_code_store.rs:70`) |
| `project_id IN (a, b)` | a filter clause | two separate database files |

Both stores are contract-tested for parity
(`src/retrieval/code_store.rs:565`, `contract_query_excludes_languages_and_scopes_project`),
so a design that assumes either meaning diverges them silently. Composing above
the trait assumes neither.

The exclusion mechanism is **not new machinery**. `exclude_languages` is already
the same shape, shipped, and contract-tested across both stores, with the backend
divergence documented and accepted in-code
(`src/retrieval/sqlite_code_store.rs:278-286`): Qdrant applies a native `must_not`
(`src/retrieval/qdrant.rs:302-323`); sqlite post-filters in Rust and **widens `k`**
for headroom, with the comment *"Exact parity would require storing language as a
vec0 metadata column."* `exclude_paths` is the sibling field, not a new concept.

**Alternatives considered.**

- *Filter-level exclusion only (Qdrant-native).* Rejected: not expressible as one
  query in sqlite-vec, where `project_id` selects a database file. It degenerates
  into the two-query design anyway, so the two-query design is the honest name for it.
- *Per-worktree full index via vector copy.* Rejected: requires a new trait method
  (`chunk_refs` returns no vectors), per-backend copy semantics, ~104 MB per
  worktree (34,635 chunks × 768-dim f32), and a GC obligation for worktrees Claude
  Code creates and destroys casually.
- *`git diff --name-only <base>` as the staleness bound.* Rejected: it forces a
  choice of base commit, and it is a leaky proxy — main's *index* can lag main's
  own working tree, so a file unmodified by git can still be stale in the vectors.
  Content hashing answers the real question directly and needs no base.
- *Lazy delta build on first search.* Rejected: makes a read tool write, with no
  intent gate, from the hottest call site. Also surfaces embedder failures inside
  `semantic_search`, attributing them to the wrong operation.

**Consequences.**

- now easier: exact-bytes correctness; no schema migration; no new trait method;
  no partition semantics; parity in both stores; a third store inherits it free;
  the plugin's *"Do NOT run index in worktrees"* instruction becomes false and gets
  deleted, resolving a live contradiction between two shipped surfaces.
- now harder: a worktree `index` walks and hashes the tree (what incremental sync
  already does — no embedding for unchanged content); sqlite's top-k stays
  approximate under exclusion, exactly as it already is for languages; `ChunkRef`
  gains a field; and in the lite backend each worktree mints **a new database
  file**, because `project_id` is the filename there (`sqlite_code_store.rs:70`).
  That last one is small by volume but is duplication of a different *kind* than
  approach A's — files rather than vectors — and it needs a cleanup story, since
  Claude Code creates worktrees casually. See *Not in scope* for the adjacent open
  bug about where such files land.

**Change scenarios absorbed:** a third code store; per-branch or per-PR corpora;
main's index lagging its own working tree.
**Revisit-when:** a third exclusion axis appears — *then* generalize the three into
a filter list, per the project's rule-of-three discipline, not before. Or
`project_id` stops being sqlite's partition key.
**Confidence:** high on the mechanism (every claim cited above is read from
`experiments`). Medium on the delta lifecycle's ergonomics, which is a judgement
call rather than a measurement.

## Architecture — where each piece sits

Four layers. The trait boundary is touched twice and only additively: one extra
`&[String]` parameter on `query`, and one extra field on the `ChunkRef` it already
returns. No new trait method, and no change to what `project_id` means in either
store.

| Layer | Change | Mirrors / reuses |
|---|---|---|
| Above the trait (`src/retrieval/search.rs`) | two `query()` calls, merged by score | new, ~one function |
| `SearchOpts` | `exclude_paths: Vec<String>` | `exclude_languages` at `search.rs:47` |
| `CodeVectorStore::query` | one more `&[String]` param | `code_store.rs:60` |
| Qdrant | `must_not` on payload `file_path` | `qdrant.rs:317` — 3 lines |
| sqlite-vec | post-filter on `file_path` + existing `k` widening | `sqlite_code_store.rs:286,320` |
| `ChunkRef` | gains `file_path` | both stores already hold it (`sqlite_code_store.rs:292`; Qdrant's scroll uses a `PayloadIncludeSelector` field list) |
| Sync | worktree mode: derive dirty set, embed only it | composes existing `chunk_refs()` + `chunk_id()` |
| `IndexState` | records the dirty-path list + main's index timestamp | `src/retrieval/index_state.rs:25-67` |

## Components

**`dirty_set` — a pure function, and the only place a decision can be wrong.**
Input: main's `Vec<ChunkRef>` and the worktree's walk (per chunk: `rel_path`,
`content_hash`). Output: the dirty path set, and the chunks to embed. Signature is
free of I/O, backends, git, and the embedder, so it is unit-testable everywhere.
Rules:

- worktree chunk whose `chunk_id(main_project_id, rel, hash)` is present in main's
  set → **clean**; main's vector is valid for those exact bytes.
- absent → the path is **dirty**; queue the chunk for embedding under the delta id.
- path present in main but absent from the worktree → **dirty, queue nothing**
  (the deletion case; see *Correctness*).

Membership is tested by recomputing the id with **main's** `project_id`
(`src/retrieval/sync.rs:77-78`, `chunk_id = "{project_id}:{rel_path}:{content_hash}"`),
never by parsing an existing id — `src/retrieval/sqlite_code_store.rs:538-541`
documents a real regression from that, because a `project_id` can itself contain
colons (`lib:foo`).

**Delta project id: `<main_project_id>@<worktree_dir_name>`.** `@`, not `:`, because
`chunk_id` joins on `:`. sqlite maps `project_id` to a filename, so its sanitizer
must be confirmed to handle `@` during implementation.

**Worktree detection: reuse `detect_worktree_info(root)`**
(`src/prompts/mod.rs:204-237`). Filesystem-only, parses the `.git` pointer file,
and convention-agnostic — it requires only a `worktrees` component in the
`gitdir:` pointer, so Claude Code's `.claude/worktrees/<name>` resolves correctly
(measured 2026-08-13). Do **not** add a path-pattern check for `.worktrees/`.

**Two-query merge.** Each source is queried at `k` and the merged top-`k` is
exact — a chunk in the global top-`k` is necessarily in its own source's top-`k`.
No over-fetch is required for the merge. Scores are cosine from the same model, so
they are comparable across the two calls.

## Data flow

**Query, worktree active.**

1. Resolve the project; `detect_worktree_info` reports a worktree.
2. `read_index_state(worktree_root)` → dirty path list, main's `project_id`, and
   the main index timestamp recorded at delta-build time.
3. No state → return an empty result set carrying the hint (see *Error handling*)
   and stop; issue no queries.
4. `query(collection, main_project_id, …, exclude_paths = dirty)`.
5. `query(collection, delta_project_id, …, exclude_paths = [])`.
6. Merge by score, truncate to `k`. Then `read_index_state(main_root)` — main's own
   sidecar, a separate file from the worktree's — and attach the drift note if its
   timestamp is newer than the one the worktree recorded at delta-build time. If
   main has no sidecar (it was indexed by a path that leaves
   `record_index_state` false), drift is undetectable: attach no note and claim
   nothing. `main_root` comes from `detect_worktree_info`, which resolves it from
   the `.git` pointer.

**Sync, `index` in a worktree.**

1. `chunk_refs(collection, main_project_id)` — the same call incremental sync
   already makes to skip unchanged work.
2. Walk the worktree, chunk, hash.
3. `dirty_set(main_refs, walk)` → dirty paths + chunks to embed.
4. Embed and upsert those chunks under the delta id.
5. Prune delta chunks whose files no longer exist (ordinary incremental prune,
   scoped to the delta project).
6. `write_index_state` with the dirty list and main's current index timestamp,
   through the existing `SyncOpts.record_index_state` gate
   (`src/retrieval/sync.rs:26-29,347-348`).

## Correctness properties

- **Exact bytes.** A main chunk is served only if those precise bytes exist at
  that path in the worktree. No base commit, no staleness window.
- **Exact merge at `k`.** Established above; no over-fetch for the merge.
- **Deletion is covered.** A file present in main's index and absent from the
  worktree is never visited by the walk, so it would never enter the dirty set and
  main would keep serving it — deleting a file in a worktree and still finding it
  in search. This is why `ChunkRef` gains `file_path`: the dirty set is
  *(worktree paths whose hashes differ)* ∪ *(main paths absent from the worktree)*.
- **Partition-free.** Nothing assumes `project_id` is a filter value or a filename.

## Error handling

Four states, all **reported in the response** rather than thrown, matching how the
project already surfaces index conditions.

1. **No delta yet** → `semantic_search` returns the speaking hint: the resolved
   `project_id`, that this is a worktree, and both exits — run `index` here, or use
   `symbols` / `grep` / `references`, which are filesystem-computed and therefore
   correct in a worktree (measured 2026-08-13). This is the fix for the reported
   silence. Note that `classify_search_error`
   (`src/tools/semantic/semantic_search.rs:19-26`) cannot serve this: its
   "collection is missing" branch is unreachable when the collection is global and
   present.
2. **Main not indexed at all** → hint points at indexing main first; there is
   nothing to reuse.
3. **Embedder unreachable during a worktree `index`** → fails exactly as a main
   `index` does. No new path.
4. **Main's index moved after the delta was built** → serve results, and attach a
   note naming the drift and suggesting `index` in the worktree. Detected by
   comparing the timestamp in main's own `.codescout/index-state.json` against the
   one the worktree recorded when its delta was built. Absent main sidecar means
   undetectable, which is reported as nothing rather than as reassurance. The
   design never silently serves content it *knows* may be stale; it also never
   claims freshness it cannot establish.

## Lifecycle

**Only `index` builds the delta.** `semantic_search` never writes. This keeps the
side effect at the write verb's chokepoint with an explicit intent gate, and it
surfaces embedder and walk failures under the operation that caused them.

Staleness model is identical to main's — *your index is as fresh as your last
sync* — so there is no second mental model.

**Harness-side automation is the plugin's job.** The companion's existing
`PostToolUse` hook on `EnterWorktree`
(`codescout-companion/hooks/hooks.json:141` → `worktree-activate.mjs`) runs
`index` in the new worktree, so this is automatic under Claude Code while the
server never learns that tool's name (Agent-Agnostic Design). Two edits there:
delete the now-false *"Do NOT run index in worktrees — the shared index is
read-only here"* instruction, and drop the `.codescout/embeddings` symlink for
this purpose — it links the **legacy sqlite store**, which both activations
observed on 2026-08-13 flagged as `legacy_semantic_index`, and is a no-op for
Qdrant-backed semantic search.

## Testing

**Pure functions carry the risk, because they are the only part CI verifies.**
`src/retrieval/qdrant.rs:422` marks the sole real-Qdrant test `#[ignore]`, so it
never runs in CI; sqlite's `real_vec0_*` tests (`:421,:482,:565`) are neither
ignored nor feature-gated and do run, because sqlite-vec needs no daemon. The
backend most users of this design run is the one with no automatic coverage. That
constrains the design: every decision that can be wrong lives in a pure function,
and each backend holds only a mechanical translation.

| Test | Mutation that must kill it |
|---|---|
| `dirty_set`: unchanged file is clean | invert the id-membership check |
| `dirty_set`: modified file is dirty and queued | drop the hash comparison |
| `dirty_set`: file absent from main is dirty and queued | drop the not-in-main branch |
| `dirty_set`: file in main, absent from worktree → dirty, queues nothing | drop the deletion branch |
| merge of two sources at `k` equals the true global top-`k` | take `k` from one source only |
| drift note fires iff main's timestamp is newer | flip the comparison |
| hint fires with no delta **and does not fire with one** | make the hint unconditional |

The last row is deliberate: a guard asserted only in the positive direction passes
whether or not it discriminates.

**Backend parity, labelled honestly.** A `RecordingStore` contract test proves
callers thread `exclude_paths` through and that the trait contract holds — but
`RecordingStore` post-filters in Rust (`src/retrieval/code_store.rs:356-363`), so
it does **not** verify Qdrant's filter. The sqlite half gets a `real_vec0_*` test
that runs in CI. The Qdrant half extends the `#[ignore]`d test and is **manually
verified only**. Do not describe the two as equally covered.

**Fixtures are created, never assumed.** Any test needing a worktree does
`git init` → commit → `git worktree add` into a temp dir. No test may depend on a
worktree existing on the host. This is F-32
(`docs/trackers/release-promotion-session-log.md`) applied directly: a test whose
ability to fail depends on ambient infrastructure is not a guard.

## Not in scope

Named so a reader does not read absence as oversight.

- **Half 1 — the read-tool desync.** `EnterWorktree` already fires a hook that
  hard-denies codescout's *write* tools until `workspace(activate)` is called;
  reads are unguarded and silently hit main. Fix belongs in the plugin
  (`worktree-write-guard.mjs` already has the detection) and is independent of
  this spec.
- **Half 3 — memory and sub-project topology divergence.** A worktree serves the
  git-tracked commit's `.codescout/memories/` (11 topics vs main's 21) and
  auto-detects sub-projects because `workspace.toml` is gitignored (9 vs 2).
  Deserves its own bug file.
- **The `project_id` basename collision hazard.** Any two roots sharing a basename
  collide in the global collection; measured 2026-08-13. Not worktree-specific and
  not fixed here.
- **Qdrant CI coverage.** Making the `#[ignore]`d tests run needs a service
  container; that is a CI decision, not a design one.
- **Main's index lagging main's own working tree.** Pre-existing for every query
  against main. Content hashing means a worktree does not inherit it for clean
  paths, but this spec does not fix it for main itself.
- **Where lite-backend database files are placed.**
  `docs/issues/2026-08-13-tests-leak-sqlite-vec-dbs-into-real-home.md` is open on
  that subject and was filed independently of this design. This spec adds one DB
  file per worktree in that backend, so read that bug before implementing the lite
  half — it may constrain where the delta's DB may live, and it should be resolved
  first rather than have this design multiply whatever it describes. Deleting a
  worktree's delta (project drop / GC) is in scope for implementation but its
  file-placement policy is that bug's to settle.

## References

All verified on `experiments`, 2026-08-13.

- `src/retrieval/sync.rs:61,77-78` — `content_hash`, `chunk_id`
- `src/retrieval/sync.rs:26-29,347-348` — `SyncOpts.record_index_state` gate
- `src/retrieval/index_state.rs:25-67` — `IndexState`, read/write, `.codescout/index-state.json`
- `src/retrieval/code_store.rs:27-96` — the trait; `:51-62` `query`; `:356-363` `RecordingStore` post-filter; `:565` the language contract test
- `src/retrieval/qdrant.rs:302-323` — `must`/`must_not`; `:422` the `#[ignore]`d real test
- `src/retrieval/sqlite_code_store.rs:70` — per-project DB; `:278-320` post-filter + `k` widening; `:292` `file_path` selected; `:421,:482,:565` real tests, not ignored; `:538-541` the colon-bearing `project_id` regression
- `src/retrieval/config.rs:65-66` — global collection
- `src/retrieval/search.rs:45-47,109` — `SearchOpts.exclude_languages`
- `src/tools/semantic/semantic_search.rs:19-26,208-213` — `classify_search_error`, `project_id` resolution
- `src/config/project.rs:527-529` — basename fallback
- `src/prompts/mod.rs:204-237` — `detect_worktree_info`
- `codescout-companion/hooks/hooks.json:141` — the `EnterWorktree` `PostToolUse` hook
