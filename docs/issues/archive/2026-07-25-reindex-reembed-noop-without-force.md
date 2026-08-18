---
id: '8c9bb90ffa9a7f36'
kind: bug
status: fixed
title: librarian(reindex, reembed=true) silently no-ops without force=true — reports success, embeds nothing
tags:
- librarian
- reindex
- embeddings
- silent-success
closed: 2026-07-25
opened: 2026-07-25
owner: marius
related: []
severity: high
---

# BUG: `librarian(reindex, reembed=true)` silently no-ops without `force=true`

## Summary

`librarian(action="reindex", reembed=true)` returns a clean success envelope —
including an explicit `backfill_error_count: 0` — while sending **zero** requests
to the embedding server. Embeddings are never written. The flag only takes effect
when paired with `force=true`. This is a silent-success failure: every indicator
reports healthy while the operation accomplishes nothing.

## Symptom (Effect)

Two consecutive calls on a project with 870 indexed artifacts and a healthy
embedder, returning in well under a second:

```json
{
  "added": 0, "updated": 0, "removed": 0,
  "unchanged": 870,
  "orphans_removed": 0, "unknown_count": 0,
  "backfill_error_count": 0, "backfill_errors": [],
  "scope": "project", "targets": [""]
}
```

Embedding server request count over the same window:

```
$ docker logs --since 5m codescout-dense-cpu 2>&1 | grep -c "POST /v1/embeddings"
0
```

Adding `force=true` changes the behaviour immediately — the call exceeds
`tool_timeout_secs` (60) and traffic appears:

```
$ docker logs --since 3m codescout-dense-cpu 2>&1 | grep -c "POST /v1/embeddings"
373
```

`artifact(action="find", semantic=...)` returns `count: 0` before, and correctly
ranked results after.

## Reproduction

Commit `52fcaf0118d9a6388a8c5828f1447b818d05f360`, branch `experiments`,
codescout 0.15.0.

1. Have an already-indexed project (content hashes cached, rows present).
2. Ensure a reachable embedder — `LIBRARIAN_EMBED_URL`, `LIBRARIAN_EMBED_MODEL` set.
3. `librarian(action="reindex", scope="project", reembed=true)`
   → returns `unchanged: N`, `backfill_error_count: 0`; embedder receives nothing.
4. `librarian(action="reindex", scope="project", force=true, reembed=true)`
   → embedder receives traffic; embeddings land.

Confirm with `artifact(action="find", semantic="<some topic>")` — zero results
after step 3, ranked results after step 4.

## Environment

Arch Linux (7.1.3-arch1-2), codescout 0.15.0 MCP over stdio, branch `experiments`.
Embedder: `llama.cpp:server` serving `CodeRankEmbed-Q4_K_M.gguf` on
`127.0.0.1:48081`, OpenAI protocol, 768-dim. Catalog at
`~/.local/share/librarian/catalog.db`.

## Root cause

**Confirmed 2026-07-25** (the earlier "unknown, working hypothesis" text was
right about the area and is superseded by the exact mechanism below).

The embed-queue branch sits *downstream* of the unchanged-row early return, so
`force_embed` is unreachable unless `force_rewalk` is also set.

`src/librarian/indexer.rs:212` — the early return:

```rust
if !force_rewalk && content_unchanged && meta_unchanged {
    seen_ids.push(id);
    report.unchanged += 1;
    continue;                 // <-- leaves the loop iteration here
}
```

`src/librarian/indexer.rs:243` — the enqueue it never reaches:

```rust
if want_embeddings && (!content_unchanged || force_embed) {
    ...
    embed_queue.push((id.clone(), title, first_chunk));
}
```

With `reembed=true, force=false` on an already-indexed project, every file takes
the `continue` at 212 and the queue stays empty — hence `unchanged: 870` with
zero embed traffic.

This contradicts the function's own doc comment
(`src/librarian/indexer.rs:57-64`), which states the intended contract
explicitly:

> `force_rewalk` ... does NOT by itself force re-embedding. `force_embed` is the
> **separate, explicit lever** for "queue this file for embedding even though its
> content hash is unchanged" ... Without it, already-indexed unchanged content
> never gets embedded, silently, forever.

So this is a **behavioural** defect, not a documentation one — the doc describes
the design intent correctly and the code does not implement it. That resolves the
fix-direction question raised in § Fix.

### Why it survived: the test encodes the bug as correct

`src/librarian/indexer.rs::tests::index_repo_sync_force_embed_requeues_unchanged_content`
(`:1175`) passes `force_rewalk=true` in **both** its negative and positive passes
(`:1201`, `:1209`). It therefore proves `force_embed` works *given* `force_rewalk`
and never exercises `force_embed=true, force_rewalk=false` — the one combination
the `reindex(reembed=true)` tool call actually produces. A mutation flipping the
`|| force_embed` term at `:243` would still pass that test on the path that
matters.
## Evidence

### Doc/behaviour contradiction

The two flags are documented as independent, with `force` explicitly disclaiming
the re-embed responsibility and pointing at `reembed` as the way to do it:

- `force`: *"ignore cached file hashes and re-walk every file (re-classification;
  **does NOT by itself force re-embedding — see reembed**)"*
- `reembed`: *"**also** queue every file for re-embedding even when its content
  hash is unchanged. Use after enabling embeddings for the first time, or after
  switching embedding models/backends, **on an already-indexed project** —
  otherwise unchanged content is silently never (re-)embedded."*

`reembed`'s own text names this exact scenario ("already-indexed project",
"enabling embeddings for the first time") as its use case, and warns about the
very failure it then exhibits. Nothing in either description implies the two must
be combined.

### Request-count evidence

Counted at the embedding server, which is outside the component under test — the
reindex's own report cannot distinguish "870 files needed no work" from "870
files were skipped by mistake," since both render as `unchanged: 870`.

## Hypotheses tried

1. **Hypothesis:** the embedder was unreachable, so embedding failed silently.
   **Test:** `curl 127.0.0.1:48081/health` → `{"status":"ok"}`; a manual
   `POST /v1/embeddings` returned a 768-dim unit-norm vector.
   **Verdict:** rejected — the embedder was healthy and reachable throughout.

2. **Hypothesis:** the librarian's embedder handle was constructed at MCP startup
   while the embedder was down, leaving a dead client.
   **Test:** restarted the MCP server (`/mcp`) with the embedder already up, then
   re-ran `reindex(reembed=true)`.
   **Verdict:** rejected — still zero requests after a clean restart.

3. **Hypothesis:** `reembed` requires `force` to have a non-empty working set.
   **Test:** `reindex(force=true, reembed=true)`.
   **Verdict:** confirmed — 373 batched embed requests, semantic search restored.

## Fix

Applied 2026-07-25 (uncommitted on `experiments`, base `52fcaf01`). Two parts:
the behavioural defect, and the diagnostic gap that hid it.

**1. Behavioural — `src/librarian/indexer.rs`.** The unchanged-row early return
now performs the forced re-embed before it `continue`s:

```rust
if !force_rewalk && content_unchanged && meta_unchanged {
    if want_embeddings && force_embed {
        if let Some(item) = embed_queue_item(&id, title, body) {
            embed_queue.push(item);
        }
    }
    seen_ids.push(id);
    report.unchanged += 1;
    continue;
}
```

The row deliberately stays `unchanged` rather than falling through to the upsert
path. Nothing about the row changed — only its vector needs recomputing — so
falling through would rewrite every row and misreport them as `updated`.

The enqueue logic (chunking + the empty-body skip) is now a shared helper,
`embed_queue_item`, called from both sites. Duplicating it was the alternative,
and duplication is how the two paths would have drifted — the empty-body guard
exists because of a prior incident
(`2026-05-17-reindex-embedding-dim-mismatch.md`) and must not be forgotten on one
branch.

**2. Diagnostic — `src/librarian/tools/reindex.rs`.** The response envelope could
not express "embedded nothing," which is what let two clean-looking reindexes
pass. Added:

- `embedded: N` — incremented in the loop that actually writes vectors
- `embeddings_enabled: bool` — distinguishes "0 because no embedder is
  configured" from "0 because nothing needed one"
- `embed_note` — names the ambiguous case out loud. When an embedder IS
  configured and 0 were embedded against N unchanged, it says so and points at
  `reembed=true`; otherwise it is just `"<N> embedded"`.

Note `IndexReport.embedded` already existed but is only incremented by
`index_repo` (`indexer.rs:629`, `:634`), a different function. The MCP tool path
goes through `index_repo_sync` and embeds in the tool layer, so that field is
always 0 here — hence the separate `total_embedded` accumulator rather than
reusing it. Unifying the two indexing paths is out of scope for this bug.
## Tests added

**`indexer::tests::index_repo_sync_force_embed_alone_requeues_without_force_rewalk`**
(`src/librarian/indexer.rs`) — the regression test. Exercises
`force_embed=true, force_rewalk=false`, the exact combination
`reindex(reembed=true)` produces. Asserts the file IS queued, that the row is
still reported `unchanged` (not `updated`), and that a no-flags pass remains a
true no-op.

Demonstrated RED before the fix, for the right reason — an assertion, not a
compile error:

```
index_repo_sync_force_embed_alone_requeues_without_force_rewalk ... FAILED
  assertion `left == right` failed: force_embed alone must queue unchanged content
  left: 0
 right: 1
index_repo_sync_force_embed_requeues_unchanged_content ... ok   <- sibling unaffected
```

GREEN after. The sibling test was left in place: it still covers
`force_embed` + `force_rewalk` together, which is a distinct path.

**`reindex::tests::envelope_reports_embedding_state`**
(`src/librarian/tools/reindex.rs`) — covers the new envelope fields on the
no-embedder branch, including that `embed_note` does *not* suggest `reembed=true`
when no embedder is configured (passing it would change nothing).

**Known coverage gap, stated rather than papered over:** no test asserts a
NON-zero `embedded` through the tool layer. That needs a mock `EmbeddingService`
plus artifact store, and `TestToolContextBuilder` has no `with_embedding` setter
today. The populated path is covered one layer down by the indexer test, which
asserts on the embed *queue*. Adding the builder seam is the obvious follow-up if
the envelope grows more embedding-dependent fields.

**Gate:** `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean,
`cargo test` 3404 passed / 0 failed / 43 ignored. Chained with `&&` — an earlier
run chained with `;` reported the test's exit code and masked a real clippy
failure (`assert_eq!` with a literal bool), which is the same
trusted-the-wrong-signal shape as this bug.
## Workarounds

Always pass both flags when re-embedding an already-indexed project:

```
librarian(action="reindex", scope="project", force=true, reembed=true)
```

Verify at the embedder rather than trusting the return envelope:

```
docker logs --since 5m codescout-dense-cpu 2>&1 | grep -c "POST /v1/embeddings"
```

Note this will exceed the default `tool_timeout_secs = 60` on a project of any
size. The work continues server-side after the MCP call times out — poll the
embedder until idle rather than re-issuing the command.

## Resume

**Live-MCP verification COMPLETE (2026-07-29).** The step this section previously
listed as outstanding has now been run; nothing here is pending except the
master-side SHA.

Shipped as **`c3512dc2`** *fix(librarian): requeue embeddings when reembed is
passed without force* — on **`experiments`** only (`git merge-base --is-ancestor
c3512dc2 master` → false). Per CLAUDE.md, the master-side SHA still needs
recording here after cherry-pick; an `experiments` SHA orphans on rebase.

### Verification evidence

Release binary rebuilt (`cargo rb`) and reconnected. The exact call that failed:
`librarian(action="reindex", scope="project", reembed=true)` — `force` omitted,
nothing changed on disk:

```json
{
  "added": 0, "updated": 0, "removed": 0,
  "unchanged": 893,
  "embedded": 891,
  "embeddings_enabled": true,
  "embed_note": "891 embedded"
}
```

Before the fix the same call returned instantly with `unchanged: N`,
`backfill_error_count: 0`, and **zero** requests at the embedding server.

Two confirmations beyond the headline number:

- **893 unchanged vs 891 embedded.** The 2-file gap is the empty-body skip —
  `embed_queue_item` returns `None` for whitespace-only first chunks. That guard
  (from `2026-05-17-reindex-embedding-dim-mismatch.md`, re-landed as `2b1a348e`)
  survived extraction into the shared helper, which is what extracting it was
  meant to protect.
- **Rows stayed `unchanged`, not `updated`.** The deliberate choice held under
  real data: the re-embed pass recomputed 891 vectors without rewriting 893
  catalog rows or misreporting them as modified.

### Operational note for the next person

This call exceeds the default `tool_timeout_secs = 60` on a project this size
(893 artifacts, CPU embedder) — it was run with the value temporarily raised to
600 and then restored. **A timeout here is not a failure:** the work continues
server-side, but the response envelope is lost, so `embedded: N` never reaches
the caller. Anyone re-verifying should raise the timeout rather than conclude
from a timeout that the call did nothing — which is the same
misread-the-signal trap as the original bug.
## References

- [Dependency review session log](../trackers/dependency-review-session-log.md)
  (`4232733980fe92e9`) — same session
- [CodeRankEmbed GGUF source 401](2026-07-25-coderankembed-gguf-source-404.md)
  (`19b108e4d047d0ac`) — the sibling bug that had the embedder down in the first
  place, and why this went unnoticed
- Catalog repair context: catalog was ~8 weeks stale on this machine
  (`catalog.db` is machine-local, bulk-built 2026-05-29)

## Fix provenance

- **SHA:** `52fcaf0118d9` (experiments-only) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `ec239e797ea2e38659f06626176af3b0fbc170ce` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes —
codescout's Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep ec239e797ea2 /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several
branches (cherry-pick) and any of them is the fix. Recorded 2026-08-19.
