---
id: '2b5fc16fb0a0ef7c'
kind: bug
status: open
title: 'BUG: the librarian ToolContext carries no progress reporter, so a long reindex emits nothing to its caller and the client aborts the call while the server keeps working'
owners:
- marius
tags:
- cluster/declared-not-wired
- librarian
- reindex
- mcp
topic: librarian progress reporting
---

## Summary

`ProgressReporter` exists, is wired into `server.rs::call_tool`, throttles to 2 Hz, and
documents its own contract as *"Tools call `ctx.progress.as_ref()` — it is a no-op when
`None`."* That sentence is true of **core** tools and impossible for **librarian** tools:
they receive a different `ToolContext` struct (`src/librarian/tools/mod.rs`) which has no
`progress` field at all.

So `librarian(action="reindex", reembed=true)` runs for many minutes emitting nothing to
its caller, the MCP client's idle timeout fires, and the call is aborted **while the
server keeps working**. The caller loses the report of a run that then completes
invisibly.

## Reproduction — observed, not constructed

Session `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`, 2026-09-04, laptop (`archlinux`).

1. `librarian(action="reindex", reembed=true)` on this corpus (~28k chunks).
2. At 1800 s the MCP call aborted with an idle timeout. No progress notification had
   been delivered at any point.
3. The server had **not** stopped. Progress was reconstructable afterwards from Qdrant
   point counts and `artifact_chunk`, and the run's failure data was recovered from
   `catalog_meta` (`last_reindex_embed_error_count`), which is how the 7 oversized
   chunks were identified at all.

The abort is therefore a *reporting* failure, not a work failure — the most expensive
shape, because the work is paid for and the result discarded.

## Mechanism

Two `ToolContext` types, and only one carries the reporter:

| | core `ToolContext` (`src/tools/core/types.rs`) | librarian `ToolContext` (`src/librarian/tools/mod.rs`) |
|---|---|---|
| `progress` | **yes** | **absent** |
| others | `agent`, `lsp`, `workspace_override`, … | `catalog`, `artifact_store`, `embedding`, `rules`, `lsp`, … |

`LibrarianAdapter` builds the second from the first (`src/librarian/adapter.rs:500`,
`Arc::new(LibToolContext { … })`) and simply does not carry `progress` across. Every
librarian tool is downstream of that omission, so `grep -n progress src/librarian/tools/reindex.rs`
returns **0 matches** — not because the loop forgot, but because there is nothing to call.

## Why this is invisible to the party best placed to catch it

The author of a librarian tool reads `ProgressReporter`'s doc comment, which describes a
capability in terms of `ctx.progress` — a field their `ctx` does not have. The absence
presents as *"this tool does not report progress"*, an ordinary choice, rather than as
*"this tool cannot"*. Nothing in `reindex.rs` names the missing field, because a field
that was never plumbed leaves no site to look at.

The core-tool side is fully wired and tested, so every test of the progress mechanism
passes while an entire subsystem is unreachable from it — the shape `ListFunctions` /
`ListDocs` had (trait implemented, registered nowhere, green for months).

## Not the same bug as the sibling record

`docs/issues/2026-09-03-a-long-reindex-cannot-be-distinguished-from-a-wedged-one.md`
covers a **different observer**. Its reader is a *peer process* sharing the catalog file,
and its Fix plan is a durable `reindex_in_progress` row in `catalog_meta` with a pid whose
liveness a reader can check. That is the right remedy for that observer and does nothing
for this one: the caller of an MCP tool cannot read the catalog, only notifications.

Checked before filing — that record mentions `busy_timeout` six times and the client idle
timeout, `notifications/progress` and `ProgressReporter` zero times.

The two remedies are complementary and neither substitutes for the other.

## Consequence

This is currently on the critical path. Both remaining fixes in the embedding area need a
full `librarian(action="reindex", reembed=true)`:

- repairing the 7 artifacts left vectorless before `8acec9c7`, and
- `docs/issues/2026-09-04-librarian-embeds-stored-artifacts-through-the-query-seam.md`,
  whose seam change invalidates every stored vector and therefore *must* be followed by a
  full re-embed in the same operation.

So a re-embed that cannot be run to completion through the tool interface blocks both.

## Suggested direction (not a plan — reproduce first)

Carry `progress` across the adapter boundary and call it from the reindex loop. The
librarian `ToolContext` has few construction sites — measured 2026-09-04: **2 production,
4 test** — so the field addition is compiler-enforced and small, unlike the core context's
137.

`ProgressReporter::report()` already throttles to 2 Hz, so a per-item call is safe and
needs no batching of its own. Note the BUG-038 constraint recorded on the type: the token
comes only from `CallToolRequestParams._meta.progressToken` and is **never** synthesized —
a reporter is `None` when the client sent no token, and every call is then a no-op. That
is the correct behaviour and must survive the change.

## Tests owed

The `ProgressSink` trait exists precisely so this is testable without a live server
(`src/tools/progress.rs:22`). The guard should be a recording sink asserting that a
multi-item reindex emits **more than one** progress event — and, in the other direction,
that a reindex with no progress token emits **none**, since an unsolicited notification
is what BUG-038 was.

## Resume

Not started. Read `src/librarian/adapter.rs` around `derive_ctx` first — that is the one
place the two context types meet, and it is where the field has to cross.

