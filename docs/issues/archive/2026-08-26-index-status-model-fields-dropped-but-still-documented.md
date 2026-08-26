---
kind: bug
status: fixed
tags:
- index
- reporting
- doc-drift
- dead-code
- test-fabrication
closed: 2026-08-26
opened: 2026-08-26
owner: marius
related:
- docs/issues/2026-08-26-index-status-claims-complete-without-checking-coverage.md
- docs/issues/archive/2026-08-26-force-reindex-cannot-migrate-embedding-dimensions.md
severity: medium
unverified: `embedding_count`, `db_path` and `by_source` were dropped by the same 79e0e4f2 sweep and are still absent; only the two fields the manual's recipe named were restored. The superseded design plan listing all five is deliberately left unedited as a historical snapshot.
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

**Shipped by a concurrent session on this branch, verified here rather than
re-implemented.** This bug file was still `status: open` (a zombie-open — the fix
landed without a commit message naming the tracker entry) when checked 2026-08-26.

**SHA:** `394931d8` (`experiments`)
**patch-id:** `e90b8c9dde07a52ab55248c347baf580e4d2bbfb`

`feat(index): record which model built the index, and report a mismatch`. All three
parts landed, not just the two originally scoped as mechanical:

1. **`indexed_at`** — `IndexStatus::call` (`src/tools/semantic/index.rs`) now reads
   `IndexState.last_indexed_at` from the sidecar and sets `result["indexed_at"]`.
2. **The fabricating test** — `format_index_status_shows_model_and_timestamp`
   (`src/tools/semantic/tests.rs:512-538`) is kept as a pure formatter unit test
   (by design, per its own updated doc comment), but the keys it exercises are now
   genuinely produced on the live path — backed by a real-path assertion via the new
   `preserve_does_not_erase_a_recorded_model` / `record_replaces_a_previously_recorded_model`
   tests in `src/retrieval/index_state.rs`.
3. **`indexed_with_model`** — option (a) from this bug's original analysis was chosen:
   the model spec is persisted into `IndexState` at sync time and the STORED value is
   reported, never the configured one. `configured_model` is reported separately
   (always present when knowable), and a `model_mismatch` block fires when they
   disagree — exactly the manual's original comparison, now backed by real data.

Verified 2026-08-26: `cargo test --features librarian model` — 39 passed, 0 failed,
including all of the above.

### Addendum — the first cut shipped a fresh overclaim of the exact kind this file is about

*Added by the authoring session after the archive above was written, which was accurate
as of `394931d8` but predates the correction.*

- **SHA:** `899c5212` (`experiments`)
- **patch-id:** `4e5c9b77070f126688f750d01833131715a632c5`

`394931d8`'s `model_mismatch` hint asserted *"scores are being compared across two
embedding spaces"*. Its **first live invocation** reported `all-minilm` against a
configured `CodeRankEmbed` — which looked like a textbook hit. Measuring the endpoint
instead of trusting the label:

```
CodeRankEmbed          dim=768  first=0.078631
all-minilm             dim=768  first=0.078631
total-nonsense-model   dim=768  first=0.078631
```

llama-server ignores the requested model entirely and serves whichever gguf is loaded. The
stored vectors *were* CodeRankEmbed vectors; the label was wrong and the embedding space
was one. So the report was right that two writers disagreed, and wrong about every
consequence it drew from that.

The discriminator was already in the config, unread when the hint was written:
`embedder_url: None` means the backend is resolved **from** the model spec, so two names
are two models and the strong claim holds; a url set means the name is a field in an
OpenAI-compatible request the server may ignore. `model_mismatch` now carries
`name_is_authoritative`, the hint splits into two strengths, and the compact line says
`MODEL MISMATCH` only when the name actually determines the vectors — otherwise
`model label differs … the endpoint may ignore the name; check before rebuilding`.

**Worth stating plainly, because it is the lesson rather than the patch:** this file exists
because `status` implied something it had not established. The fix for it shipped a hint
doing the same thing one layer down. The overclaim did not disappear — it relocated into
the prose explaining the fix, and only reading the live bytes caught it. Third such catch
by `docs/RELEASE.md`'s live-output step in one day, second authored by the session doing
the fixing.

### The mislabel was a real defect, and a worse one than expected

Chasing where `all-minilm` came from — it is in no config file, no current source default,
and no live code path — found two `codescout start` processes from **Aug 24** and **Aug
25** still running, both executing **deleted** binaries (`ls -l /proc/<pid>/exe`), neither
carrying a model env var, therefore each using its own binary's compiled-in default and
stamping it into this project's shared sidecar.

Filed as
`docs/issues/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`.
The vectors survived only because the backend ignores the model name; a zombie holding a
`local:` spec would have written 384-d vectors into a 768-d collection.

So `indexed_with_model` earned its place on day one — by surfacing a live cross-process
defect that had been invisible — even though its first verdict was an overclaim. Both
facts belong in the record.
## Tests added

By the fix commit (`394931d8`):

- `retrieval::index_state::tests::record_replaces_a_previously_recorded_model`
- `retrieval::index_state::tests::preserve_does_not_erase_a_recorded_model`
- `tools::semantic::tests::format_index_status_shows_model_and_timestamp` (updated,
  not new — its doc comment now explains why the fixture is intentionally still
  hand-built)
- `tools::semantic::tests::format_index_status_has_no_mismatch_banner_when_models_agree`
- `tools::semantic::tests::format_index_status_leads_with_model_mismatch_over_a_vector_hole`
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
