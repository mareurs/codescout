---
kind: spec
status: draft
title: C-1 — Content-addressed dedup for @tool_* output buffers
owners: []
tags:
  - output-buffers
  - dedup
  - cross-pollination
  - headroom
created: 2026-06-26
---

# C-1 — Content-addressed dedup for `@tool_*` output buffers

## 1. Goal & the question

codescout's `OutputBuffer` mints a fresh `@tool_*` handle on every
`store_tool` call (`src/tools/output_buffer.rs`), keyed by a wrapping `u32` of
`SystemTime` + a monotonic counter. Re-running a deterministic tool on
unchanged input (e.g. `symbols`/`grep` on a file that has not moved) therefore
stores **byte-identical content twice** under two different handles, and the
model has no signal that it has already seen that exact output.

**Goal:** when `store_tool` is handed content byte-identical to a still-buffered
tool entry, return the **existing** handle instead of minting a new one. Two
observable wins:

1. Storage collapses — one entry per unique tool output, not one per call.
2. The model can recognize repetition — the same handle coming back means
   "this is the exact output you already have."

This is candidate **C-1** from `docs/trackers/headroom-cross-pollination.md`
(the only `high`-priority entry), shape-verified in R-19
(`docs/trackers/reconnaissance-patterns.md`, datapoints 1 & 3). It is the
*source-side* analogue of Headroom's content-addressed CCR; the two remain
complementary (Headroom dedups on the wire, codescout dedups at the buffer).

## 2. Scope

**In scope:** `store_tool` only (`@tool_*` handles).

**Explicitly out of scope (decided during design):**

- `store` (`@cmd_*` shell output) — two *different* commands with identical
  stdout would collapse to one entry whose `command` field is whichever ran
  first, losing the second command's provenance. Marginal benefit, real cost.
- `store_file` (`@file_*`) — entries carry a `source_path` used by
  `get_with_refresh_flag` for staleness; two paths with identical content
  sharing one entry would break per-path invalidation.
- `store_dangerous` / `store_background` — separate maps
  (`pending_acks` / `background_jobs`), unrelated to content buffering.

`store_tool`'s `command` field holds only the *tool name*, so two
identical-content tool calls already share their tool name → deduping them loses
no provenance. This is what makes `store_tool` the clean, safe scope.

## 3. Architecture decisions

### ADR-1 — Secondary `content → id` index, not content-addressed handles

**Decision:** add a `content_index: HashMap<String, String>` (content-hash →
handle id) alongside the existing `entries`/`order` maps, and reuse the existing
SHA-256 `content_hash()` (`src/retrieval/sync.rs:34`) as the key. Handles keep
their current `@tool_{:08x}` time+counter format.

**Rejected alternative — content-addressed primary key** (`@tool_<hash-prefix>`,
Headroom-CCR style): observationally equivalent (identical content → identical
handle) but far more invasive — it changes the handle *format* (breaks
`store_tool_generates_tool_ref` and the 8-hex shape assertions), diverges
`@tool_` from the time+counter scheme every other `store_*` kind shares, and
drops the per-call timestamp. Same model-visible behavior, larger contract
surface. Prefer the design confined inside `store_tool` + eviction.

**Rejected alternative — linear scan** of `order` per store: O(n) for no
data-model simplicity once "scan only tool entries" is accounted for. Strictly
worse than the index.

### ADR-2 — Dedup key is the content hash alone

Under `store_tool`-only scope, identical bytes already imply identical tool
name, so a compound `(tool_name, hash)` key buys nothing. Key on the hash.

### ADR-3 — A dedup hit is a re-access; it bumps LRU

When an incoming content matches a live entry, move that entry to the
most-recent position in `order` (mirroring `get_with_refresh_flag`'s existing
LRU bump) before returning its handle. Rationale: a model that keeps citing the
same handle is actively using it; it must not be evicted out from under that
use just because no *new* entry was minted.

### ADR-4 — Eviction must clear the index, centrally

The eviction block that pops `order.first()` and removes from `entries` is
**byte-identical and duplicated across three methods** that share those maps:
`store` (`88-95`), `store_file` (`213-218`), `store_tool` (`253-258`).
Consequence: a `store_tool` entry can be evicted by a later **shell** `store` or
**file** `store_file` call. Therefore index cleanup cannot live in `store_tool`
alone — it must run wherever a shared-map eviction happens. The three inline
blocks are replaced by one `evict_oldest_locked` helper that also clears the
evicted entry's `content_index` slot. This is the load-bearing change: without
it, the index works in every `store_tool`-only test and then leaks dead handles
the first time a shell `store` evicts a tool entry in a real session.

One eviction path is deliberately *excluded*: `get_with_refresh_flag`'s
file-staleness eviction (`order.retain` + `entries.remove`, used when an
`@file_*` entry's `source_path` has vanished) only ever removes `@file_*`
entries, which carry `content_hash: None`. It therefore needs no index cleanup
— a fact the `Option` type makes self-evident.

## 4. Data model changes (`src/tools/output_buffer.rs`)

- `BufferEntry`: add `content_hash: Option<String>`. Set to `Some(hash)` **only**
  in `store_tool`; `None` in `store` and `store_file`. Because it is `None` for
  every non-tool path, the type itself encodes the store_tool-only scope and
  proves those paths cannot dedup.
- `BufferInner`: add `content_index: HashMap<String, String>` (content-hash →
  handle id), initialized empty in `new()`.

## 5. Behavior

### 5.1 `evict_oldest_locked` (new helper)

```rust
fn evict_oldest_locked(inner: &mut BufferInner) {
    if inner.entries.len() >= inner.max_entries {
        if let Some(oldest_id) = inner.order.first().cloned() {
            inner.order.remove(0);
            if let Some(entry) = inner.entries.remove(&oldest_id) {
                if let Some(h) = entry.content_hash {
                    // Only clear the slot if it still points at the evicted id —
                    // never invalidate a live, re-pointed slot.
                    if inner.content_index.get(&h) == Some(&oldest_id) {
                        inner.content_index.remove(&h);
                    }
                }
            }
        }
    }
}
```

`store`, `store_file`, and `store_tool` each replace their inline eviction block
with a call to this helper. For `store`/`store_file` the evicted entry's
`content_hash` is `None`, so their behavior is unchanged — except that a tool
entry *they* happen to evict now gets its index slot cleaned.

### 5.2 `store_tool` (dedup path)

```rust
let hash = crate::retrieval::sync::content_hash(&content);

// Dedup hit: identical content already buffered → reuse handle, bump LRU.
if let Some(existing) = inner.content_index.get(&hash).cloned() {
    if inner.entries.contains_key(&existing) {
        // move `existing` to the back of `order` (LRU bump, ADR-3)
        return existing;
    }
    // Defensive: stale slot (should not happen with central eviction) — drop it.
    inner.content_index.remove(&hash);
}

// Miss: mint handle as today (time+counter), then record the hash both
// on the entry (for eviction reversal) and in the index (for future hits):
//   entry.content_hash = Some(hash.clone());
//   inner.content_index.insert(hash, id.clone());
```

The LRU bump finds `existing` in `order`, removes it, and pushes it to the back
— byte-for-byte the mechanism `get_with_refresh_flag` already uses for its
"Refresh LRU order: move to end" step (`position` → `remove(pos)` → `push`,
`output_buffer.rs:186-190`). Factor it to a shared `bump_lru_locked` helper if
convenient.

## 6. Error handling

No new failure modes:

- `content_hash` returns `String` infallibly.
- Lock-poison handling is unchanged (`unwrap_or_else(|e| e.into_inner())`).
- The stale-slot branch is defensive insurance; with central eviction it cannot
  trigger, but it costs one `HashMap` probe and removes a class of latent bug.

## 7. Testing

New tests in the existing `tests` module of `output_buffer.rs`:

- `store_tool_dedups_identical_content` — two identical `store_tool` calls return
  the same handle; `entries.len() == 1`.
- `store_tool_distinct_content_distinct_handles` — different content → different
  handles; two entries.
- `store_tool_dedup_hit_bumps_lru` — store A, store B, re-store A (hit), then
  force exactly one eviction → **B** is evicted, A survives (the hit bumped it).
- `eviction_clears_content_index` — buffer a tool entry, evict it by overflowing
  the shared map via `store`, then re-store the identical content → a **fresh**
  handle is minted (no dead-handle reuse), proving the index was cleaned.
- `dedup_is_tool_only` — identical content through `store` (→ two distinct
  `@cmd_`) and `store_file` (→ two distinct `@file_`) still mints distinct
  handles.

Regression — these existing tests must stay green:
`store_tool_generates_tool_ref`, `lru_eviction`, `get_refreshes_lru_order`,
`store_and_get`.

## 8. Files touched

- `src/tools/output_buffer.rs` — only file. Struct fields (`BufferEntry`,
  `BufferInner`), `new()` initializer, three eviction sites → `evict_oldest_locked`,
  `store_tool` body, new tests.

No public API change, no handle-format change, no caller modified. `content_hash`
is already `pub` at `src/retrieval/sync.rs:34`.

## 9. Out of scope / future

- Deduping `@cmd_*` / `@file_*` (see §2 — provenance and staleness costs).
- Cross-session persistence of the index (buffer is process-local by design).
- Surfacing "you've seen this before" explicitly in the compact summary — the
  returned identical handle is the signal; a louder hint is a separate idea.
