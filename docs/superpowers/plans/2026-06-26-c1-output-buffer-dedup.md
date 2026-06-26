# C-1 Content-Addressed Dedup for `@tool_*` Output Buffers — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `OutputBuffer::store_tool` return the existing `@tool_*` handle when handed content byte-identical to a still-buffered tool entry, instead of minting a new handle every call.

**Architecture:** A secondary `content_index: HashMap<contenthash, handleid>` on `BufferInner`, keyed by the existing SHA-256 `content_hash()`. The three duplicated eviction blocks (`store`, `store_file`, `store_tool`) that share the `entries`/`order` maps are centralized into one `evict_oldest_locked` helper that also clears the index, so a tool entry evicted by a *shell* `store` never leaves a dead index slot. A dedup hit bumps LRU via a shared `bump_lru_locked` helper.

**Tech Stack:** Rust. Existing in-tree `crate::retrieval::sync::content_hash` (SHA-256). No new dependencies.

## Global Constraints

- Single file touched: `src/tools/output_buffer.rs`. No public API change, no handle-format change, no caller modified.
- Scope is `store_tool` only. `store` (`@cmd_*`), `store_file` (`@file_*`), `store_dangerous`, `store_background` do NOT dedup.
- Pre-commit gate (run before every commit): `cargo fmt` && `cargo clippy -- -D warnings` && `cargo test`. Clippy treats warnings as errors — no `dead_code`/unused allowed.
- Branch: `experiments` (never commit to `master`).
- Commit trailer (end every commit message with): `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`
- Spec: `docs/superpowers/specs/2026-06-26-c1-output-buffer-dedup-design.md`.

---

## File Structure

`src/tools/output_buffer.rs` — only file. Changes by region:
- Import block (top): add `content_hash` import.
- `BufferEntry` struct: add `content_hash: Option<String>` field.
- `BufferInner` struct: add `content_index: HashMap<String, String>` field.
- `OutputBuffer::new`: initialize `content_index`.
- New private helpers `evict_oldest_locked` + `bump_lru_locked` + `#[cfg(test)] content_index_len`.
- `store`, `store_file`: eviction block → helper call; literal gets `content_hash: None`.
- `store_tool`: full rewrite with the dedup prologue.
- `get_with_refresh_flag`: inline LRU bump → `bump_lru_locked` (DRY).
- `mod tests`: 5 new tests.

---

## Task 1: Scaffolding — fields, helpers, centralized eviction (behavior-preserving refactor)

**Files:**
- Modify: `src/tools/output_buffer.rs` (imports, both structs, `new`, helpers, `store`, `store_file`, `store_tool` eviction sites)

**Interfaces:**
- Produces:
  - `BufferEntry.content_hash: Option<String>` — `None` for all kinds at this stage.
  - `BufferInner.content_index: HashMap<String, String>` — empty, read/cleared by `evict_oldest_locked`.
  - `fn evict_oldest_locked(inner: &mut BufferInner)` — private assoc fn on `impl OutputBuffer`.
  - `fn bump_lru_locked(inner: &mut BufferInner, id: &str)` — private assoc fn.
  - `#[cfg(test)] fn content_index_len(&self) -> usize` — test-only index-size accessor.

This task changes no observable behavior; its gate is that the existing test suite stays green.

- [ ] **Step 1: Add the `content_hash` import**

In the import block at the top of the file, add the line below directly above `use crate::tools::RecoverableError;`:

```rust
use crate::retrieval::sync::content_hash;
use crate::tools::RecoverableError;
```

- [ ] **Step 2: Add `content_hash` field to `BufferEntry`**

Change the end of the `BufferEntry` struct from:

```rust
    /// Set only for `@file_*` entries. Enables mtime-based auto-refresh in `get()`.
    pub source_path: Option<PathBuf>,
}
```

to:

```rust
    /// Set only for `@file_*` entries. Enables mtime-based auto-refresh in `get()`.
    pub source_path: Option<PathBuf>,
    /// SHA-256 of `stdout`, set only for `@tool_*` entries (content-dedup key).
    /// `None` for every other store kind — the type encodes the store_tool-only scope.
    pub content_hash: Option<String>,
}
```

- [ ] **Step 3: Add `content_index` field to `BufferInner`**

Change this region of `BufferInner` from:

```rust
    max_entries: usize,
    counter: u64,
    // --- pending-ack store (commands) ---
```

to:

```rust
    max_entries: usize,
    counter: u64,
    /// Content-hash → handle id, for `@tool_*` dedup. Kept in sync with
    /// `entries` by `evict_oldest_locked`.
    content_index: HashMap<String, String>,
    // --- pending-ack store (commands) ---
```

- [ ] **Step 4: Initialize `content_index` in `new`**

In `OutputBuffer::new`, change:

```rust
                    max_entries,
                    counter: 0,
                    pending_acks: HashMap::new(),
```

to:

```rust
                    max_entries,
                    counter: 0,
                    content_index: HashMap::new(),
                    pending_acks: HashMap::new(),
```

- [ ] **Step 5: Add the two helpers + the test-only accessor**

Insert these three items into `impl OutputBuffer`, immediately after the closing brace of `new`:

```rust
    /// Evict the least-recently-used entry from the shared `entries`/`order`
    /// maps when at capacity, clearing its `content_index` slot if it had one.
    /// Shared by every store_* method that uses those maps (`store`,
    /// `store_file`, `store_tool`), so a tool entry evicted by a shell `store`
    /// never leaves a dangling index slot.
    fn evict_oldest_locked(inner: &mut BufferInner) {
        if inner.entries.len() >= inner.max_entries {
            if let Some(oldest_id) = inner.order.first().cloned() {
                inner.order.remove(0);
                if let Some(entry) = inner.entries.remove(&oldest_id) {
                    if let Some(h) = entry.content_hash {
                        // Only clear the slot if it still points at the evicted
                        // id — never invalidate a live, re-pointed slot.
                        if inner.content_index.get(&h) == Some(&oldest_id) {
                            inner.content_index.remove(&h);
                        }
                    }
                }
            }
        }
    }

    /// Move `id` to the most-recently-used end of `order`.
    fn bump_lru_locked(inner: &mut BufferInner, id: &str) {
        if let Some(pos) = inner.order.iter().position(|k| k == id) {
            inner.order.remove(pos);
            inner.order.push(id.to_string());
        }
    }

    #[cfg(test)]
    fn content_index_len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .content_index
            .len()
    }
```

- [ ] **Step 6: Route `store` through the eviction helper + set `content_hash: None`**

In `store`, replace the eviction block:

```rust
            // Evict oldest if at capacity
            if inner.entries.len() >= inner.max_entries {
                if let Some(oldest_id) = inner.order.first().cloned() {
                    inner.order.remove(0);
                    inner.entries.remove(&oldest_id);
                }
            }
```

with:

```rust
            Self::evict_oldest_locked(&mut inner);
```

Then in the same method's `BufferEntry { … }` literal, add `content_hash: None,` after `source_path: None,`:

```rust
            let entry = BufferEntry {
                command,
                stdout,
                stderr,
                exit_code,
                timestamp: now,
                source_path: None,
                content_hash: None,
            };
```

- [ ] **Step 7: Route `store_file` through the eviction helper + set `content_hash: None`**

In `store_file`, replace the eviction block:

```rust
            if inner.entries.len() >= inner.max_entries {
                if let Some(oldest_id) = inner.order.first().cloned() {
                    inner.order.remove(0);
                    inner.entries.remove(&oldest_id);
                }
            }
```

with:

```rust
            Self::evict_oldest_locked(&mut inner);
```

Then add `content_hash: None,` to that method's literal:

```rust
            let entry = BufferEntry {
                command: path.clone(),
                stdout: content,
                stderr: String::new(),
                exit_code: 0,
                timestamp: now,
                source_path,
                content_hash: None,
            };
```

- [ ] **Step 8: Route `store_tool` through the eviction helper + set `content_hash: None` (interim — rewritten in Task 2)**

In `store_tool`, replace the eviction block:

```rust
            if inner.entries.len() >= inner.max_entries {
                if let Some(oldest_id) = inner.order.first().cloned() {
                    inner.order.remove(0);
                    inner.entries.remove(&oldest_id);
                }
            }
```

with:

```rust
            Self::evict_oldest_locked(&mut inner);
```

Then add `content_hash: None,` to its literal:

```rust
            let entry = BufferEntry {
                command: tool_name.to_string(),
                stdout: content,
                stderr: String::new(),
                exit_code: 0,
                timestamp: now,
                source_path: None,
                content_hash: None,
            };
```

- [ ] **Step 9: Run the existing suite — verify no behavior change**

Run: `cargo test --lib output_buffer`
Expected: PASS. In particular `store_and_get`, `lru_eviction`, `get_refreshes_lru_order`, `store_tool_generates_tool_ref` are green.

- [ ] **Step 10: Run the pre-commit gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean — no warnings (both new fields are read by `evict_oldest_locked`; `content_index_len` is `#[cfg(test)]`), all tests pass.

- [ ] **Step 11: Commit**

```bash
git add src/tools/output_buffer.rs
git commit -m "refactor(output-buffer): centralize eviction + add dedup scaffolding

Add BufferEntry.content_hash + BufferInner.content_index (unused this commit,
None/empty everywhere) and an evict_oldest_locked helper that all three shared-map
store_* methods now call. Behavior-preserving prep for C-1 store_tool dedup.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `store_tool` content-dedup path

**Files:**
- Modify: `src/tools/output_buffer.rs` (`store_tool` full rewrite; `get_with_refresh_flag` LRU-bump extraction)
- Test: `src/tools/output_buffer.rs` `mod tests` (3 tests)

**Interfaces:**
- Consumes: `content_hash`, `evict_oldest_locked`, `bump_lru_locked` (Task 1).
- Produces: `store_tool` returns the existing handle on a content match; mints + indexes on a miss.

- [ ] **Step 1: Write the failing dedup tests**

Add to the `mod tests` block:

```rust
    #[test]
    fn store_tool_dedups_identical_content() {
        let buf = OutputBuffer::new(10);
        let id1 = buf.store_tool("symbols", "{\"symbols\":[1,2,3]}".to_string());
        let id2 = buf.store_tool("symbols", "{\"symbols\":[1,2,3]}".to_string());
        assert_eq!(id1, id2, "identical tool content must reuse the handle");
        assert_eq!(buf.content_index_len(), 1, "one unique content → one index slot");
        assert_eq!(buf.get(&id1).unwrap().stdout, "{\"symbols\":[1,2,3]}");
    }

    #[test]
    fn store_tool_distinct_content_distinct_handles() {
        let buf = OutputBuffer::new(10);
        let id1 = buf.store_tool("symbols", "A".to_string());
        let id2 = buf.store_tool("symbols", "B".to_string());
        assert_ne!(id1, id2, "different content must mint different handles");
        assert_eq!(buf.content_index_len(), 2);
    }

    #[test]
    fn store_tool_dedup_hit_bumps_lru() {
        let buf = OutputBuffer::new(2);
        let a = buf.store_tool("t", "A".to_string()); // order [a]
        let b = buf.store_tool("t", "B".to_string()); // order [a, b], at capacity
        let a2 = buf.store_tool("t", "A".to_string()); // dedup hit → bump a → [b, a]
        assert_eq!(a, a2);
        let c = buf.store_tool("t", "C".to_string()); // evicts order.first() = b
        assert!(buf.get(&b).is_none(), "b was LRU and should be evicted");
        assert!(buf.get(&a).is_some(), "a survived because the hit bumped it");
        assert!(buf.get(&c).is_some());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib output_buffer::tests::store_tool_dedup`
Expected: FAIL — `store_tool_dedups_identical_content` fails on `assert_eq!(id1, id2)` (handles currently differ); `store_tool_dedup_hit_bumps_lru` fails (without dedup, `a2` is a new handle and `a` gets evicted, not `b`).

- [ ] **Step 3: Rewrite `store_tool` with the dedup prologue**

Replace the entire `store_tool` method body with:

```rust
    /// Store tool output under a `@tool_*` handle, deduplicating by content.
    ///
    /// If `content` is byte-identical to a still-buffered tool entry, the
    /// existing handle is returned (and bumped to most-recently-used) instead
    /// of minting a new one. `command` holds the tool name for diagnostics.
    pub fn store_tool(&self, tool_name: &str, content: String) -> String {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());

        // Content-dedup: identical tool output already buffered → reuse handle.
        let hash = content_hash(&content);
        if let Some(existing) = inner.content_index.get(&hash).cloned() {
            if inner.entries.contains_key(&existing) {
                Self::bump_lru_locked(&mut inner, &existing); // re-access (ADR-3)
                return existing;
            }
            inner.content_index.remove(&hash); // defensive: stale slot, fall through
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        inner.counter = inner.counter.wrapping_add(1);
        let id = format!("@tool_{:08x}", now.wrapping_add(inner.counter) as u32);

        Self::evict_oldest_locked(&mut inner);

        let entry = BufferEntry {
            command: tool_name.to_string(),
            stdout: content,
            stderr: String::new(),
            exit_code: 0,
            timestamp: now,
            source_path: None,
            content_hash: Some(hash.clone()),
        };
        inner.entries.insert(id.clone(), entry);
        inner.order.push(id.clone());
        inner.content_index.insert(hash, id.clone());
        id
    }
```

- [ ] **Step 4: Run the dedup tests to verify they pass**

Run: `cargo test --lib output_buffer::tests::store_tool_dedup output_buffer::tests::store_tool_distinct`
Expected: PASS (all three new tests green).

- [ ] **Step 5: DRY — route `get_with_refresh_flag`'s LRU bump through the helper**

In `get_with_refresh_flag`, replace its trailing inline bump:

```rust
            // Refresh LRU order: move to end.
            if let Some(pos) = inner.order.iter().position(|k| k == canonical) {
                inner.order.remove(pos);
                inner.order.push(canonical.to_string());
            }
```

with:

```rust
            // Refresh LRU order: move to end.
            Self::bump_lru_locked(&mut inner, canonical);
```

- [ ] **Step 6: Run the bump regression test**

Run: `cargo test --lib output_buffer::tests::get_refreshes_lru_order`
Expected: PASS (the extracted helper is behavior-identical to the inline bump).

- [ ] **Step 7: Run the pre-commit gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean, all tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/tools/output_buffer.rs
git commit -m "feat(output-buffer): content-addressed dedup for @tool_* handles

store_tool now reuses the existing handle (and bumps LRU) when content is
byte-identical to a buffered tool entry, keyed by SHA-256 content_hash. DRY the
LRU bump into bump_lru_locked, shared with get_with_refresh_flag. Implements
cross-pollination candidate C-1.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Edge-case guards — index cleanup on eviction + scope boundary

**Files:**
- Test: `src/tools/output_buffer.rs` `mod tests` (2 tests)

**Interfaces:**
- Consumes: `store_tool` (dedup), `store`, `store_file`, `content_index_len` (test-only).

These tests guard two invariants that the public handle API alone cannot distinguish (the defensive stale-slot branch masks dead handles either way): the index must actually shrink when a tool entry is evicted, and non-tool kinds must never dedup.

- [ ] **Step 1: Write the guard tests**

Add to `mod tests`:

```rust
    #[test]
    fn eviction_clears_content_index() {
        let buf = OutputBuffer::new(1);
        let _a = buf.store_tool("t", "A".to_string());
        assert_eq!(buf.content_index_len(), 1);
        // A shell store shares entries/order and evicts the tool entry at cap 1.
        let _c = buf.store("echo hi".to_string(), "hi".to_string(), String::new(), 0);
        assert_eq!(
            buf.content_index_len(),
            0,
            "evicting the tool entry must clear its index slot"
        );
    }

    #[test]
    fn dedup_is_tool_only() {
        let buf = OutputBuffer::new(10);
        // Shell output: identical stdout must NOT dedup.
        let c1 = buf.store("cmd".to_string(), "SAME".to_string(), String::new(), 0);
        let c2 = buf.store("cmd".to_string(), "SAME".to_string(), String::new(), 0);
        assert_ne!(c1, c2, "store (@cmd_) must not dedup");
        // File content: identical content under different paths must NOT dedup.
        // (Do not call get() on these — the fake paths would stat-evict.)
        let f1 = buf.store_file("/tmp/codescout-a".to_string(), "SAME".to_string());
        let f2 = buf.store_file("/tmp/codescout-b".to_string(), "SAME".to_string());
        assert_ne!(f1, f2, "store_file (@file_) must not dedup");
    }
```

- [ ] **Step 2: Run the guard tests**

Run: `cargo test --lib output_buffer::tests::eviction_clears_content_index output_buffer::tests::dedup_is_tool_only`
Expected: PASS. (If `eviction_clears_content_index` is red, `evict_oldest_locked` is not clearing the slot — re-check Task 1 Step 5.)

- [ ] **Step 3: Run the full pre-commit gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean, all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/tools/output_buffer.rs
git commit -m "test(output-buffer): guard C-1 index cleanup + tool-only scope

eviction_clears_content_index proves evict_oldest_locked unwinds the index
(white-box via content_index_len); dedup_is_tool_only pins that @cmd_/@file_
never collapse identical content.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:**
- §2 scope (store_tool only) → Task 1 sets `content_hash: None` in `store`/`store_file`; Task 3 `dedup_is_tool_only` guards it. ✓
- §3 ADR-1 secondary index → Task 1 fields + Task 2 insert. ✓
- §3 ADR-2 hash-only key → Task 2 `content_hash(&content)`. ✓
- §3 ADR-3 LRU bump on hit → Task 2 `bump_lru_locked`; `store_tool_dedup_hit_bumps_lru`. ✓
- §3 ADR-4 centralized eviction + index cleanup → Task 1 `evict_oldest_locked`; Task 3 `eviction_clears_content_index`. ✓
- §3 ADR-4 `get_with_refresh_flag` file-eviction excluded (content_hash None) → unchanged; bump extraction guarded by `get_refreshes_lru_order`. ✓
- §5 data model + behavior → Tasks 1–2. ✓
- §6 error handling (no new failure modes) → no `Result` added. ✓
- §7 tests: all five new tests present (dedups, distinct, bumps_lru, clears_index, tool_only) + three regression guards run (`store_and_get`, `lru_eviction`, `get_refreshes_lru_order`, `store_tool_generates_tool_ref`). ✓
- §8 single file → all tasks touch only `output_buffer.rs`. ✓

**Placeholder scan:** none — every code step shows complete code.

**Type consistency:** `content_hash: Option<String>`, `content_index: HashMap<String, String>`, `evict_oldest_locked(&mut BufferInner)`, `bump_lru_locked(&mut BufferInner, &str)`, `content_index_len(&self) -> usize` used consistently across Tasks 1–3.
