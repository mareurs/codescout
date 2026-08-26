---
status: open
opened: 2026-08-26
closed:
severity: medium
owner: marius
related:
  - docs/issues/2026-08-26-index-status-claims-complete-without-checking-coverage.md
  - docs/issues/archive/2026-08-26-force-reindex-cannot-migrate-embedding-dimensions.md
tags: [index, reporting, doc-drift, dead-code, test-fabrication]
kind: bug
---

# BUG: `index(action="status")` dropped the two model fields its own manual tells users to compare

## Summary

`docs/manual/src/troubleshooting.md:231` instructs a user diagnosing an embedding-model
mismatch to read `configured_model` and `indexed_with_model` off a `status` response and
check that they match. Neither field is in the response. `indexed_with_model` and
`indexed_at` were deliberately dropped from the envelope on 2026-05-13; their **readers**
in `format_index_status` survived, as did a test that passes only because it fabricates
the dropped keys. `configured_model` has never been emitted by any code in this repo.

## Symptom (Effect)

Live call, 2026-08-26, HEAD `5f34fe2b`, release binary, Qdrant backend:

```json
{
  "indexed": true,
  "queryable": true,
  "project_id": "codescout",
  "collection": "code_chunks",
  "file_count": 1614,
  "chunk_count": 47899,
  "chunks_without_vectors": 0,
  "integrity": "ok",
  "coverage": "unchecked",
  "coverage_hint": "file_count/chunk_count are what the store HOLDS, not proof it holds everything eligible. Run index(action='verify') for coverage against the indexer's own walk.",
  "git_sync": {
    "status": "behind",
    "behind_commits": 2,
    "last_indexed_commit": "e8f46d51",
    "head_commit": "5f34fe2b"
  }
}
```

No `indexed_with_model`. No `indexed_at`. No `configured_model`. The user-facing compact
line therefore also carries neither model nor timestamp — the two `push_str` arms that
would add them are unreachable on every real call.

## Reproduction

```
git rev-parse HEAD          # 5f34fe2b at filing
cargo rb && /mcp            # reconnect so the running server is not older than the build
index(action="status")      # read the JSON envelope
```

Then follow the manual's own recipe and observe there is nothing to compare:

```
read_markdown("docs/manual/src/troubleshooting.md",
              heading="### Results seem wrong or irrelevant after changing the model")
```

## Environment

- Linux, branch `experiments`, HEAD `5f34fe2b`
- Vector backend: **Qdrant** (`code_chunks` collection) — the default on
  `scripts/retrieval-stack.sh`, and the backend the dropping commit re-routed to
- MCP transport: stdio, release binary

## Root cause

`79e0e4f2` (2026-05-13, "feat(index): re-route IndexStatus to Qdrant (L-01 step 6.2)")
moved `IndexStatus` off sqlite. Its own commit message enumerates the casualties:

> Dropped fields (sqlite-only metadata):
> - indexed_with_model, indexed_at, embedding_count, db_path
> - by_source (per-source counts)
> - git_sync, last_indexed_commit
> - drift (entire feature …)

The **producers** went. Two **consumers** did not:

- `src/tools/semantic/index.rs:995` — `result["indexed_with_model"].as_str()`
- `src/tools/semantic/index.rs:998` — `result["indexed_at"].as_str()`

Both are `if let Some(...)` over a key nothing writes, so both are silently inert rather
than broken. Measured 2026-08-26: `grep("indexed_with_model", mode="files")` returns four
files repo-wide — the consumer above, the test below, and two docs. Zero producers.
`grep("configured_model")` returns **two** hits, both in docs; no code emits it, and
`git log -S'indexed_with_model' -- src/` lists five commits while `configured_model` has
no such history at all.

**Why nobody noticed.** Two independent maskings, one per surface.

1. **The test fabricates its own input.**
   `src/tools/semantic/tests.rs:502-528`,
   `format_index_status_shows_model_and_timestamp`, hand-builds a `json!` literal
   containing `"indexed_with_model"` and `"indexed_at"` and asserts the formatter
   renders them. It is green, and has been green since the fields were removed, because
   it never touches the code path that builds a real envelope. This is exactly the
   reconnaissance rule *"a green result certifies the path that actually EXECUTED"* —
   the fixture could never reach the gate, so the assertion proves `push_str` works and
   proves nothing about whether the key exists.

2. **Half the dropped list came back.** `git_sync` and `last_indexed_commit` are on
   `79e0e4f2`'s casualty list too, and they are in the live envelope today —
   `src/tools/semantic/index.rs:686` restores them via `index_state::git_sync_status`.
   So a reader comparing that commit message to current behaviour finds it partly
   false, which reads as "stale commit message" rather than "the rest is still missing".

## Evidence

### The manual's recipe, verbatim

`docs/manual/src/troubleshooting.md:227-232`, under the heading
`### Results seem wrong or irrelevant after changing the model` — i.e. this is the
manual's dedicated section for exactly this failure, and the whole of its recipe is
inoperable:

```
 ```json
 { "tool": "workspace", "arguments": { "action": "status" } }
 ```

 The response includes `configured_model` and `indexed_with_model`. They must
 be the same.
```

Note the tool named is `workspace`, not `index` — and `workspace(action="status")`
returns even less: a prose line (`**Semantic index:** Built — semantic_search is ready
to use`) with no model information at all. Verified live 2026-08-26.

### The surviving readers

`src/tools/semantic/index.rs:993-1000`:

```rust
if let Some(model) = result["indexed_with_model"].as_str() {
    out.push_str(&format!(" · {model}"));
}
if let Some(ts) = result["indexed_at"].as_str() {
    out.push_str(&format!(" · {ts}"));
}
```

### The data that *does* exist

`src/retrieval/index_state.rs:31-49` — `IndexState` carries
`last_indexed_at: String` (RFC3339, written by `write_index_state_with_dirty` at
`src/retrieval/index_state.rs:80`). `git_sync_status`
(`src/retrieval/index_state.rs:102-129`) already opens with `read_index_state(root)?`
and so holds the whole struct in hand, using only `last_indexed_commit` from it.

So `indexed_at` is **one field away** from being real.

`IndexState` has **no model field**. So `indexed_with_model` has no source, and that
part needs a decision rather than a line of code — see Fix.

## Hypotheses tried

1. **Hypothesis:** the fields are backend-conditional — present on sqlite-vec, absent on
   Qdrant.
   **Test:** `grep("indexed_with_model", mode="files")` repo-wide, then
   `git log -S'indexed_with_model' -- src/`.
   **Verdict:** rejected. There is no producer on any backend; the sqlite producer was
   deleted in `79e0e4f2`, and the only remaining code references are the two readers and
   the fabricating test.

2. **Hypothesis:** the manual describes a legacy version and the fields were never meant
   to return.
   **Test:** read `79e0e4f2`'s message; check whether the same list's other members
   returned.
   **Verdict:** rejected as a reason to close this. `git_sync` and `last_indexed_commit`
   were on the identical "dropped" list and **were** restored, so the drop was not a
   deliberate permanent narrowing of the surface — it was a migration cost that got paid
   back selectively.

## Fix

*Not yet implemented — filed on notice per CLAUDE.md. Three parts, and only the first
two are mechanical.*

1. **`indexed_at`** — surface `IndexState.last_indexed_at` in the `status` envelope.
   `git_sync_status` (`src/retrieval/index_state.rs:102-129`) already reads the struct;
   either add it to that returned object or read the sidecar once in `IndexStatus::call`
   (`src/tools/semantic/index.rs:686`).

2. **The fabricating test** — `format_index_status_shows_model_and_timestamp` must
   assert against an envelope the product actually builds, or be split so the
   formatter-rendering half is honest about being a pure unit test of `push_str` while a
   second test pins *the keys the live path emits*. As written it is a coverage claim for
   a field that does not exist.

3. **`indexed_with_model` — decide before implementing.** The obvious move is to fill it
   from the configured embedding model, and that would be a **fresh instance of the
   overclaim this session just removed from the same function**. The configured model is
   not the model the stored vectors were built with, and those differ precisely in the
   failure the manual is trying to diagnose (see
   `docs/issues/archive/2026-08-26-force-reindex-cannot-migrate-embedding-dimensions.md`
   — a dimension mismatch *is* a model mismatch). Reporting the configured model under a
   name that says `indexed_with_model` would make the mismatch invisible by
   construction: the two fields the manual says to compare would be the same value read
   twice.

   Honest options: **(a)** persist the model spec into `IndexState` at sync time and
   report the stored value, which makes the manual's comparison work as written;
   **(b)** delete the two dead readers and rewrite the troubleshooting section around
   the dimension check that already exists. (a) is more useful, (b) is smaller. Do not
   ship the shortcut.

## Tests added

None yet — bug is `open`. The regression test for part 1 asserts `indexed_at` is present
in a real `IndexStatus` envelope (not a hand-built literal); for part 3 it asserts the
reported model is the *stored* one after a model change, which is only meaningful once
(a) is chosen.

## Workarounds

To check for a model/index mismatch today, ignore the manual's recipe and use the
dimension guard instead — a mismatch surfaces as a dimension error from
`migrate_or_guard_index_dim` (`src/retrieval/client.rs`) on the next
`index(action="build", force=true)`, which reports the stored and expected dimensions.
Two different models sharing a dimension are still undetectable.

## Resume

Answer part 3 first — it decides whether parts 1 and 2 are one commit or two. Read
`src/retrieval/index_state.rs:31-49` and decide whether `IndexState` gains an
`indexed_with_model: String`. The schema-evolution pattern is already established in
that struct: it carries a `schema_version`, and the `dirty_paths` doc comment spells out
why `#[serde(default)]` is load-bearing — a sidecar written before a field existed must
still parse, because `read_index_state` reads a failed parse as "never indexed" for the
whole project. Then `docs/manual/src/troubleshooting.md:227-232` needs correcting either
way, because it names `workspace(action="status")`, which returns no model information
under any of these options.

## References

- Dropping commit: `79e0e4f2` "feat(index): re-route IndexStatus to Qdrant (L-01 step 6.2)"
- `src/tools/semantic/index.rs:686` (git_sync restore), `:968-1012` (`format_index_status`)
- `src/tools/semantic/tests.rs:502-528` (the fabricating test)
- `src/retrieval/index_state.rs:31-49`, `:102-129`
- `docs/manual/src/troubleshooting.md:227-232`
- `docs/superpowers/plans/2026-03-09-project-status-trim-design.md:72` — the design doc
  that still calls `index_status` "the authoritative source for `configured_model`,
  `indexed_with_model`, `embedding_count`, `db_path`, `by_source` …", none of which it
  emits. Same drift, second surface.
- Sibling reporting-gap bug from the same session:
  `docs/issues/2026-08-26-index-status-claims-complete-without-checking-coverage.md`
