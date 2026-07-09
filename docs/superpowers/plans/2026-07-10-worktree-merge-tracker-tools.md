# Worktree-Merge Tracker Safety — codescout Tools Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the codescout MCP catalog primitives that make a worktree→master tracker merge safe: `artifact(action="graft")` (fold one catalog row's history into another) and a `doctor` worktree-scoped check + `fix=reseat_worktree`.

**Architecture:** The catalog is one machine-global SQLite DB keyed by `sha256(abs_path)`. Merging a worktree branch orphans worktree-scoped rows. `graft` re-points a row's events / observations / links / augmentation params onto a survivor row (delete-last, so `ON DELETE CASCADE` never wipes history mid-op), then deletes the source. `doctor` detects worktree-scoped rows and auto-reseats the no-collision ones. All work is intra-DB row re-pointing — no cross-DB import.

**Tech Stack:** Rust, `rusqlite` (SQLite, WAL, FK cascade ON), `serde_json`, `anyhow`. Design spec: `docs/superpowers/specs/2026-07-10-worktree-merge-tracker-safety-design.md`.

## Global Constraints

- **Pre-commit gate (every task):** `cargo fmt` && `cargo clippy -- -D warnings` && `cargo test` must all pass before the task's commit.
- **Error handling:** recoverable, user-facing failures use `crate::librarian::tools::RecoverableError::new(..)` / `::with_hint(..)` (returns `anyhow::Error`); never `anyhow::bail!` for user-input errors. (memory `conventions`, `get_guide("error-handling")`.)
- **No-echo writes:** MCP tool success returns a compact `json!({..})`, never the written content back.
- **Catalog identity is immutable:** `graft` NEVER mutates an `id`. It re-points foreign keys (`events.artifact_id`, `artifact_observation.artifact_id`, `artifact_link.src_id/dst_id`, `artifact_augmentation.artifact_id`) and deletes the source row.
- **Delete-last invariant:** because `events` / `artifact_observation` / `artifact_link` / `artifact_augmentation` all `REFERENCES artifact(id) ON DELETE CASCADE`, the source row is deleted ONLY after all its history has been re-pointed. Violating this order silently destroys history.
- **Transactions:** multi-write catalog ops run in one `conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)` (the `append_entry` pattern, `catalog/augmentation.rs:190`) — atomic under cross-process WAL contention.
- **Tests:** unit tests live in a `#[cfg(test)] mod tests` in the same file; use `Catalog::open_in_memory()`. Assert catalog state by direct `SELECT`, not tool output.

---

### Task 1: `graft_rows` catalog helper — re-point events/observations/links, delete source

**Files:**
- Create: `src/librarian/catalog/graft.rs`
- Modify: `src/librarian/catalog/mod.rs` (add `pub mod graft;` next to the other `pub mod` declarations near the top)
- Test: in `src/librarian/catalog/graft.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `crate::librarian::catalog::Catalog` (`{ pub conn: Connection }`, `mod.rs:28`); table shapes `events(id, artifact_id, …)`, `artifact_observation(id, artifact_id, …)`, `artifact_link(src_id, dst_id, rel, created_at)` PK `(src_id,dst_id,rel)` (`catalog/schema.sql:20`).
- Produces: `pub struct GraftReport { events_repointed, observations_repointed, links_repointed, links_dropped, entries_merged, entries_renumbered: usize, remap: BTreeMap<String,String>, suspicious: Vec<serde_json::Value> }` (later tasks fill the params fields) and `pub fn graft_rows(cat: &mut Catalog, from_id: &str, into_id: &str) -> anyhow::Result<GraftReport>`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::{artifact::ArtifactRow, Catalog};
    use crate::librarian::catalog::{events::TestEventRowBuilder, events};

    fn art(cat: &Catalog, id: &str, path: &str) {
        let mut row = ArtifactRow::new(id, std::path::Path::new(path), "tracker");
        row.title = Some(id.to_string());
        crate::librarian::catalog::artifact::upsert(cat, &row).unwrap();
    }

    #[test]
    fn graft_repoints_events_and_deletes_source_last() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", "/wt/x.md");
        art(&cat, "into", "/main/x.md");
        events::insert(&cat, &TestEventRowBuilder::new("from", "note").with_id("e1").build()).unwrap();
        events::insert(&cat, &TestEventRowBuilder::new("from", "note").with_id("e2").build()).unwrap();

        let report = graft_rows(&mut cat, "from", "into").unwrap();

        assert_eq!(report.events_repointed, 2);
        // History survived onto `into` (delete-last invariant held).
        let n: i64 = cat.conn
            .query_row("SELECT COUNT(*) FROM events WHERE artifact_id='into'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2, "events re-pointed, not cascade-deleted");
        // Source row is gone.
        let src: i64 = cat.conn
            .query_row("SELECT COUNT(*) FROM artifact WHERE id='from'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(src, 0);
    }

    #[test]
    fn graft_dedups_conflicting_link_and_drops_it() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", "/wt/x.md");
        art(&cat, "into", "/main/x.md");
        art(&cat, "dst", "/main/y.md");
        let mk = |src: &str| crate::librarian::catalog::links::LinkRow {
            src_id: src.into(), dst_id: "dst".into(), rel: "cites".into(), created_at: 1,
        };
        // Both from->dst and into->dst exist with the same rel: re-pointing from->dst
        // onto into->dst is a PK conflict, so it must be DROPPED, not error.
        crate::librarian::catalog::links::insert(&cat, &mk("from")).unwrap();
        crate::librarian::catalog::links::insert(&cat, &mk("into")).unwrap();

        let report = graft_rows(&mut cat, "from", "into").unwrap();

        assert_eq!(report.links_dropped, 1);
        let n: i64 = cat.conn
            .query_row("SELECT COUNT(*) FROM artifact_link WHERE src_id='into' AND dst_id='dst'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "single surviving edge, no duplicate");
    }

    #[test]
    fn graft_errors_on_unknown_id() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "into", "/main/x.md");
        let err = graft_rows(&mut cat, "nope", "into").unwrap_err();
        assert!(err.to_string().contains("nope"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout graft_ -- --nocapture`
Expected: FAIL — `graft.rs` / `graft_rows` do not exist (compile error).

- [ ] **Step 3: Write minimal implementation**

Create `src/librarian/catalog/graft.rs`:

```rust
//! Fold one artifact row's catalog history into another (worktree-merge safety).
//!
//! Re-points every FK-linked table (`events`, `artifact_observation`,
//! `artifact_link`) off `from_id` onto `into_id`, then deletes `from_id`.
//! DELETE IS LAST: all four child tables `REFERENCES artifact(id) ON DELETE
//! CASCADE`, so deleting the source before re-pointing would destroy the very
//! history we migrate. Augmentation params merge is added in a later task.

use crate::librarian::catalog::Catalog;
use crate::librarian::tools::RecoverableError;
use anyhow::Result;
use rusqlite::params;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Default, serde::Serialize)]
pub struct GraftReport {
    pub events_repointed: usize,
    pub observations_repointed: usize,
    pub links_repointed: usize,
    pub links_dropped: usize,
    pub entries_merged: usize,
    pub entries_renumbered: usize,
    pub remap: BTreeMap<String, String>,
    pub suspicious: Vec<Value>,
}

fn row_exists(conn: &rusqlite::Connection, id: &str) -> Result<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM artifact WHERE id=?1", [id], |r| r.get(0))?;
    Ok(n > 0)
}

pub fn graft_rows(cat: &mut Catalog, from_id: &str, into_id: &str) -> Result<GraftReport> {
    if from_id == into_id {
        return Err(RecoverableError::new("graft: from_id and into_id are the same row"));
    }
    if !row_exists(&cat.conn, from_id)? {
        return Err(RecoverableError::new(format!("graft: unknown from_id `{from_id}`")));
    }
    if !row_exists(&cat.conn, into_id)? {
        return Err(RecoverableError::new(format!("graft: unknown into_id `{into_id}`")));
    }

    let tx = cat
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut report = GraftReport::default();

    // 1. Events — plain re-point (event id is unique, no conflict).
    report.events_repointed = tx.execute(
        "UPDATE events SET artifact_id=?1 WHERE artifact_id=?2",
        params![into_id, from_id],
    )?;

    // 2. Observations — same shape.
    report.observations_repointed = tx.execute(
        "UPDATE artifact_observation SET artifact_id=?1 WHERE artifact_id=?2",
        params![into_id, from_id],
    )?;

    // 3. Links — directional, PK (src_id,dst_id,rel). Re-point both endpoints
    //    with OR IGNORE; a re-point that would duplicate an existing edge is
    //    skipped (row keeps from_id) and cascade-deleted with the source below.
    let links_before: i64 = tx.query_row(
        "SELECT COUNT(*) FROM artifact_link WHERE src_id=?1 OR dst_id=?1",
        [from_id],
        |r| r.get(0),
    )?;
    let u1 = tx.execute(
        "UPDATE OR IGNORE artifact_link SET src_id=?1 WHERE src_id=?2",
        params![into_id, from_id],
    )?;
    let u2 = tx.execute(
        "UPDATE OR IGNORE artifact_link SET dst_id=?1 WHERE dst_id=?2",
        params![into_id, from_id],
    )?;
    let links_left: i64 = tx.query_row(
        "SELECT COUNT(*) FROM artifact_link WHERE src_id=?1 OR dst_id=?1",
        [from_id],
        |r| r.get(0),
    )?;
    report.links_repointed = u1 + u2;
    report.links_dropped = links_left as usize; // conflicting dups, cascade-deleted next

    // 4. Delete source LAST — cascades any leftover dup links / vec row.
    tx.execute("DELETE FROM artifact WHERE id=?1", [from_id])?;

    tx.commit()?;
    Ok(report)
}
```

Add to `src/librarian/catalog/mod.rs` alongside the sibling `pub mod` lines (near `pub mod augmentation;`):

```rust
pub mod graft;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout graft_ -- --nocapture`
Expected: PASS (3 tests). Then `cargo fmt && cargo clippy -- -D warnings`.

Note on `links_repointed` count: a self-edge `from→from` is counted by both `u1` and `u2`; this is a reporting approximation, acceptable for a diagnostic count. If the `graft_dedups…` test's `links_repointed` assertion is added later, account for it; this task asserts only `links_dropped`.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/catalog/graft.rs src/librarian/catalog/mod.rs
git commit -m "feat(librarian): graft_rows re-points events/observations/links, delete-last

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: `graft_rows` augmentation merge — migrate, renumber collisions, flag near-dups

**Files:**
- Modify: `src/librarian/catalog/graft.rs` (add `merge_augmentation` + one call before the delete)
- Modify: `src/librarian/catalog/augmentation.rs` (make `next_index` `pub(crate)`)
- Test: `src/librarian/catalog/graft.rs` (extend `mod tests`)

**Interfaces:**
- Consumes: `artifact_augmentation(artifact_id PK, params TEXT, entry_collection TEXT, …)`; `catalog::augmentation::next_index(existing_ids: &[String], id_prefix: &str) -> usize` (bump visibility to `pub(crate)`).
- Produces: fills `GraftReport.{entries_merged, entries_renumbered, remap, suspicious}`. Renumber policy: the **incoming (`from_id`)** side's ids that collide with an existing `into_id` id are reassigned `<prefix>-<into_max+k>`; non-colliding ids are preserved. `suspicious` = incoming entries whose object (minus `id`) deep-equals a surviving entry (candidate same-finding-twice; reported, still renumbered).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn graft_migrates_augmentation_when_into_has_none() {
    let mut cat = Catalog::open_in_memory().unwrap();
    art(&cat, "from", "/wt/x.md");
    art(&cat, "into", "/main/x.md");
    let mut a = crate::librarian::catalog::augmentation::AugmentationRow::test_new("from");
    a.entry_collection = Some("failures".into());
    a.params = r#"{"failures":[{"id":"F-1","t":"a"}]}"#.into();
    crate::librarian::catalog::augmentation::upsert(&cat, &a).unwrap();

    graft_rows(&mut cat, "from", "into").unwrap();

    let moved = crate::librarian::catalog::augmentation::get(&cat, "into").unwrap().unwrap();
    let p: Value = serde_json::from_str(&moved.params).unwrap();
    assert_eq!(p["failures"][0]["id"], "F-1", "augmentation migrated wholesale");
}

#[test]
fn graft_renumbers_colliding_incoming_ids_and_reports_remap() {
    let mut cat = Catalog::open_in_memory().unwrap();
    art(&cat, "from", "/wt/x.md");
    art(&cat, "into", "/main/x.md");
    let aug = |cat: &Catalog, id: &str, params: &str| {
        let mut a = crate::librarian::catalog::augmentation::AugmentationRow::test_new(id);
        a.entry_collection = Some("failures".into());
        a.params = params.into();
        crate::librarian::catalog::augmentation::upsert(cat, &a).unwrap();
    };
    aug(&cat, "into", r#"{"failures":[{"id":"F-1","t":"keep1"},{"id":"F-2","t":"keep2"}]}"#);
    // Incoming F-2 collides (distinct content) -> renumber to F-3; F-9 is free -> kept.
    aug(&cat, "from", r#"{"failures":[{"id":"F-2","t":"incoming"},{"id":"F-9","t":"free"}]}"#);

    let report = graft_rows(&mut cat, "from", "into").unwrap();

    assert_eq!(report.entries_renumbered, 1);
    assert_eq!(report.remap.get("F-2").map(String::as_str), Some("F-3"));
    let p: Value = serde_json::from_str(
        &crate::librarian::catalog::augmentation::get(&cat, "into").unwrap().unwrap().params,
    ).unwrap();
    let ids: Vec<&str> = p["failures"].as_array().unwrap().iter()
        .map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["F-1", "F-2", "F-3", "F-9"]);
}

#[test]
fn graft_flags_near_dup_as_suspicious() {
    let mut cat = Catalog::open_in_memory().unwrap();
    art(&cat, "from", "/wt/x.md");
    art(&cat, "into", "/main/x.md");
    let aug = |cat: &Catalog, id: &str, params: &str| {
        let mut a = crate::librarian::catalog::augmentation::AugmentationRow::test_new(id);
        a.entry_collection = Some("failures".into());
        a.params = params.into();
        crate::librarian::catalog::augmentation::upsert(cat, &a).unwrap();
    };
    aug(&cat, "into", r#"{"failures":[{"id":"F-5","t":"same bug"}]}"#);
    // Same content, different id string: same finding discovered twice.
    aug(&cat, "from", r#"{"failures":[{"id":"F-1","t":"same bug"}]}"#);

    let report = graft_rows(&mut cat, "from", "into").unwrap();

    assert_eq!(report.suspicious.len(), 1);
    assert_eq!(report.suspicious[0]["t"], "same bug");
}
```

(If `AugmentationRow::test_new` does not exist, add a small `#[cfg(test)] pub fn test_new(artifact_id: &str) -> Self` to `augmentation.rs` mirroring its `tests::aug` helper — a minimal builder with empty prompt/params and default flags.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout graft_ -- --nocapture`
Expected: FAIL — `merge_augmentation` not implemented; params untouched.

- [ ] **Step 3: Write minimal implementation**

In `augmentation.rs`, change `fn next_index` to `pub(crate) fn next_index`.

In `graft.rs`, add before `report` is returned (i.e. **before** step 4's `DELETE`, inside the transaction) a call:

```rust
    // 3b. Augmentation params — migrate or merge (before the delete).
    merge_augmentation(&tx, from_id, into_id, &mut report)?;
```

and add the function + helpers:

```rust
use crate::librarian::catalog::augmentation::next_index;

/// Split "F-12" -> ("F", 12). Returns None for ids without a trailing -<int>.
fn split_id(id: &str) -> Option<(&str, u64)> {
    let (prefix, num) = id.rsplit_once('-')?;
    num.parse::<u64>().ok().map(|n| (prefix, n))
}

fn strip_id(entry: &Value) -> Value {
    let mut e = entry.clone();
    if let Some(o) = e.as_object_mut() {
        o.remove("id");
    }
    e
}

fn merge_augmentation(
    tx: &rusqlite::Transaction<'_>,
    from_id: &str,
    into_id: &str,
    report: &mut GraftReport,
) -> Result<()> {
    let fetch = |id: &str| -> Result<Option<(String, Option<String>)>> {
        tx.query_row(
            "SELECT params, entry_collection FROM artifact_augmentation WHERE artifact_id=?1",
            [id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other.into()),
        })
    };

    let from_aug = fetch(from_id)?;
    let Some((from_params, from_coll)) = from_aug else {
        return Ok(()); // nothing to merge
    };
    let into_aug = fetch(into_id)?;

    // into has no augmentation -> migrate from's wholesale (re-point PK).
    if into_aug.is_none() {
        tx.execute(
            "UPDATE artifact_augmentation SET artifact_id=?1 WHERE artifact_id=?2",
            params![into_id, from_id],
        )?;
        return Ok(());
    }
    let (into_params, into_coll) = into_aug.unwrap();

    // Both augmented but no shared entry_collection -> leave into's params as-is;
    // from's augmentation row cascade-deletes with the source.
    let coll = match (&from_coll, &into_coll) {
        (Some(a), Some(b)) if a == b => a.clone(),
        _ => return Ok(()),
    };

    let mut into_json: Value = serde_json::from_str(&into_params).unwrap_or_else(|_| serde_json::json!({}));
    let from_json: Value = serde_json::from_str(&from_params).unwrap_or_else(|_| serde_json::json!({}));
    let into_arr = into_json.get(&coll).and_then(Value::as_array).cloned().unwrap_or_default();
    let from_arr = from_json.get(&coll).and_then(Value::as_array).cloned().unwrap_or_default();

    let into_ids: std::collections::HashSet<String> = into_arr.iter()
        .filter_map(|e| e.get("id").and_then(Value::as_str).map(String::from)).collect();
    let into_id_list: Vec<String> = into_ids.iter().cloned().collect();

    let mut merged = into_arr.clone();
    for entry in &from_arr {
        let old = entry.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
        // Near-dup detection: same object (minus id) already present on the survivor.
        if into_arr.iter().any(|e| strip_id(e) == strip_id(entry)) {
            report.suspicious.push(entry.clone());
        }
        let mut e = entry.clone();
        if into_ids.contains(&old) {
            // Collision: renumber to <prefix>-<into_max+k> for this prefix.
            if let Some((prefix, _)) = split_id(&old) {
                let same_prefix: Vec<String> = into_id_list.iter()
                    .chain(merged.iter().filter_map(|m| m.get("id").and_then(Value::as_str).map(String::from)).collect::<Vec<_>>().iter())
                    .cloned().collect();
                let next = next_index(&same_prefix, prefix);
                let new_id = format!("{prefix}-{next}");
                if let Some(o) = e.as_object_mut() {
                    o.insert("id".into(), serde_json::json!(new_id));
                }
                report.remap.insert(old, new_id);
                report.entries_renumbered += 1;
            }
        }
        merged.push(e);
        report.entries_merged += 1;
    }

    into_json[&coll] = Value::Array(merged);
    tx.execute(
        "UPDATE artifact_augmentation SET params=?1 WHERE artifact_id=?2",
        params![serde_json::to_string(&into_json)?, into_id],
    )?;
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout graft_ -- --nocapture`
Expected: PASS (all Task 1 + Task 2 tests). Then `cargo fmt && cargo clippy -- -D warnings && cargo test`.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/catalog/graft.rs src/librarian/catalog/augmentation.rs
git commit -m "feat(librarian): graft merges augmentation params, renumbers collisions, flags near-dups

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Wire `artifact(action="graft")` MCP surface

**Files:**
- Create: `src/librarian/tools/graft.rs`
- Modify: `src/librarian/tools/mod.rs` (add `mod graft;` near the other tool modules ~line 244)
- Modify: `src/librarian/tools/artifact.rs` (enum list ~line 33; dispatch match ~line 200; two action-list error strings ~line 191 and ~line 208)
- Test: `src/librarian/tools/graft.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `catalog::graft::graft_rows(&mut Catalog, from_id, into_id) -> Result<GraftReport>`; `ToolContext.catalog: Arc<Mutex<Catalog>>`.
- Produces: `pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value>` returning `json!({ "grafted": true, "from_id":.., "into_id":.., "report": <GraftReport> })`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::{artifact::ArtifactRow, events::{self, TestEventRowBuilder}, Catalog};
    use crate::librarian::tools::TestToolContextBuilder;
    use serde_json::json;

    #[tokio::test]
    async fn graft_action_folds_and_reports() {
        let cat = Catalog::open_in_memory().unwrap();
        for (id, p) in [("from", "/wt/x.md"), ("into", "/main/x.md")] {
            let mut r = ArtifactRow::new(id, std::path::Path::new(p), "tracker");
            r.title = Some(id.into());
            crate::librarian::catalog::artifact::upsert(&cat, &r).unwrap();
        }
        events::insert(&cat, &TestEventRowBuilder::new("from", "note").with_id("e1").build()).unwrap();
        let ctx = TestToolContextBuilder::new(cat).build();

        let out = call(&ctx, json!({"from_id":"from","into_id":"into"})).await.unwrap();
        assert_eq!(out["grafted"], true);
        assert_eq!(out["report"]["events_repointed"], 1);
    }

    #[tokio::test]
    async fn graft_action_requires_both_ids() {
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = TestToolContextBuilder::new(cat).build();
        let err = call(&ctx, json!({"from_id":"a"})).await.unwrap_err();
        assert!(err.to_string().contains("into_id"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout graft_action -- --nocapture`
Expected: FAIL — `tools::graft::call` does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `src/librarian/tools/graft.rs`:

```rust
//! `artifact(action="graft")` — fold one catalog row's history into another.
use crate::librarian::tools::{RecoverableError, ToolContext};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct Args {
    from_id: String,
    into_id: String,
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args)
        .map_err(|e| RecoverableError::new(format!("graft requires 'from_id' and 'into_id': {e}")))?;
    let mut cat = ctx.catalog.lock();
    let report = crate::librarian::catalog::graft::graft_rows(&mut cat, &a.from_id, &a.into_id)?;
    drop(cat);
    Ok(json!({
        "grafted": true,
        "from_id": a.from_id,
        "into_id": a.into_id,
        "report": report,
    }))
}
```

In `src/librarian/tools/mod.rs`, add near the other tool `mod` lines (~244):

```rust
mod graft;
```

In `src/librarian/tools/artifact.rs`:
- Add `"graft"` to the `enum` array in the input schema (line ~33): `…, "delete", "graft", "link", …`.
- Add the dispatch arm in the `match action` block (after the `"delete"` arm, ~line 200):

```rust
            "graft"    => super::graft::call(ctx, args).await,
```

- Add `graft` to BOTH action-list error strings (the "action required" message ~line 191 and the `other =>` unknown-action message ~line 208): insert `graft` after `delete`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout graft -- --nocapture`
Expected: PASS (Task 1–3 tests). Then `cargo fmt && cargo clippy -- -D warnings && cargo test`.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/graft.rs src/librarian/tools/mod.rs src/librarian/tools/artifact.rs
git commit -m "feat(librarian): expose artifact(action=graft) MCP surface

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: `doctor` worktree-scoped check (read-only detection + classification)

**Files:**
- Modify: `src/librarian/tools/doctor.rs` (add `scan_worktree_scoped` + a `main_path_for` helper; call it in `call`, ~line 100)
- Modify: `src/librarian/current_project.rs` (add `pub(crate) fn worktree_main_root(worktree_root: &Path) -> Option<PathBuf>` deriving the main repo root from the `.git`-file `gitdir:` pointer)
- Test: `src/librarian/tools/doctor.rs` (`mod tests`) and `src/librarian/current_project.rs` (`mod tests`)

**Interfaces:**
- Consumes: `current_project::is_linked_worktree(&Path) -> bool` (`current_project.rs:56`); `Violation::new(check, artifact_id, path, detail)` (`doctor.rs:71`); artifact rows via `SELECT id, abs_path FROM artifact`.
- Produces: `pub(crate) fn worktree_main_root(&Path) -> Option<PathBuf>`; `fn scan_worktree_scoped(conn) -> Result<Vec<Violation>>` emitting `check="worktree_scoped_row"`, `detail` = JSON `{ "main_path":.., "classification":"no_collision"|"collision", "collision_with": <into_id?>, "id_overlap": [..] }`.

- [ ] **Step 1: Write the failing test**

```rust
// in current_project.rs tests
#[test]
fn worktree_main_root_from_gitdir_pointer() {
    let tmp = TempDir::new().unwrap();
    let wt = tmp.path().join("main/.worktrees/feat");
    std::fs::create_dir_all(&wt).unwrap();
    std::fs::write(wt.join(".git"),
        format!("gitdir: {}/main/.git/worktrees/feat\n", tmp.path().display())).unwrap();
    let main = worktree_main_root(&wt).unwrap();
    assert_eq!(main, tmp.path().join("main"));
}
```

```rust
// in doctor.rs tests
#[test]
fn scan_worktree_scoped_classifies_collision_and_overlap() {
    let cat = Catalog::open_in_memory().unwrap();
    // Simulate: a worktree row and a main row at the logical same path.
    // main_path derivation is unit-tested separately; here we seed abs_paths
    // that share the post-worktree suffix and assert classification given a
    // stubbed worktree root set. (Use a temp worktree so is_linked_worktree fires.)
    // ... build a temp main repo + linked worktree, seed one augmented row on
    // each side with overlapping ids, then:
    let violations = scan_worktree_scoped(&cat.conn).unwrap();
    // With no worktrees on disk, the scan is empty (safe default).
    assert!(violations.is_empty());
}
```

(The full collision-fixture test builds a `TempDir` main repo + `git worktree`-style `.git` pointer file, seeds an augmented row per side, and asserts one `collision` violation whose `id_overlap` lists the shared ids. Write it concretely against the temp layout from the `current_project` test above.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout worktree_ -- --nocapture`
Expected: FAIL — `worktree_main_root` / `scan_worktree_scoped` undefined.

- [ ] **Step 3: Write minimal implementation**

In `current_project.rs`:

```rust
/// Given a linked-worktree root, derive its MAIN repo root from the `.git`-file
/// `gitdir: <main>/.git/worktrees/<name>` pointer. Returns None if `root` is not
/// a linked worktree.
pub(crate) fn worktree_main_root(root: &std::path::Path) -> Option<std::path::PathBuf> {
    let pointer = std::fs::read_to_string(root.join(".git")).ok()?;
    let gitdir = pointer.lines().find_map(|l| l.strip_prefix("gitdir:").map(str::trim))?;
    // <main>/.git/worktrees/<name>  ->  <main>
    let p = std::path::Path::new(gitdir);
    let mut comps = p.components();
    let mut main = std::path::PathBuf::new();
    for c in comps.by_ref() {
        if c.as_os_str() == ".git" { return Some(main); }
        main.push(c);
    }
    None
}
```

In `doctor.rs`, add and wire the scan. `scan_worktree_scoped`:
1. `SELECT id, abs_path FROM artifact`; for each, walk ancestors to find a dir that `is_linked_worktree`; if found, `worktree_main_root` → compute `main_path = main_root.join(abs_path.strip_prefix(worktree_root))`.
2. Compute the main-path row id with `crate::librarian::ids::artifact_id_from_abs(&main_path)`; classify `collision` if that id exists in `artifact`, else `no_collision`.
3. On collision, if both rows have an `artifact_augmentation` with a shared `entry_collection`, read both params and compute the id overlap.
4. Emit a `Violation { check: "worktree_scoped_row", artifact_id: Some(id), path: abs_path, detail: <json string> }`.

Add `all_violations.extend(scan_worktree_scoped(&cat.conn)?);` in `call` alongside the existing `scan_*` calls (~line 100).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout worktree_ -- --nocapture`
Expected: PASS. Then `cargo fmt && cargo clippy -- -D warnings && cargo test`.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/doctor.rs src/librarian/current_project.rs
git commit -m "feat(librarian): doctor worktree_scoped_row check + main-path derivation

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: `doctor fix=reseat_worktree` (auto-reseat the no-collision rows)

**Files:**
- Modify: `src/librarian/tools/doctor.rs` (extend `run_fix` ~line 149; add `reseat_worktree`)
- Test: `src/librarian/tools/doctor.rs` (`mod tests`)

**Interfaces:**
- Consumes: `scan_worktree_scoped` (Task 4) to enumerate + classify; `worktree_main_root`.
- Produces: `run_fix(ctx, "reseat_worktree", _)` → re-point `no_collision` rows' `abs_path` to their main path (`UPDATE artifact SET abs_path=?1 WHERE id=?2`, no filesystem rename); leave `collision` rows untouched. Returns `json!({ "fix":"reseat_worktree", "reseated":[{id,new_path}], "collisions":[{id,main_path,into_id}] })`.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn reseat_worktree_repoints_no_collision_row_without_rename() {
    // temp main repo + linked worktree; seed ONE worktree-scoped row with no
    // main-side counterpart; the merged file already lives at the main path.
    // (Reuse the temp layout from Task 4.)
    // ... after building ctx with that catalog:
    let out = run_fix(&ctx, "reseat_worktree", None).await.unwrap();
    assert_eq!(out["fix"], "reseat_worktree");
    assert_eq!(out["reseated"].as_array().unwrap().len(), 1);
    // The row's abs_path now points at the main path; the id is unchanged.
    // ... assert via SELECT abs_path FROM artifact WHERE id=<worktree row id>.
}

#[tokio::test]
async fn reseat_worktree_leaves_collisions_for_graft() {
    // temp layout where BOTH main and worktree rows exist -> collision.
    let out = run_fix(&ctx, "reseat_worktree", None).await.unwrap();
    assert!(out["reseated"].as_array().unwrap().is_empty());
    assert_eq!(out["collisions"].as_array().unwrap().len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout reseat_worktree -- --nocapture`
Expected: FAIL — `run_fix` rejects the unknown `reseat_worktree` fix.

- [ ] **Step 3: Write minimal implementation**

Extend `run_fix` (`doctor.rs:149`) to branch on `fix`:

```rust
async fn run_fix(ctx: &ToolContext, fix: &str, root: Option<&str>) -> Result<Value> {
    match fix {
        "prune_missing" => { /* existing body unchanged */ }
        "reseat_worktree" => reseat_worktree(ctx),
        other => Err(RecoverableError::new(format!(
            "unknown fix '{other}' — expected 'prune_missing' or 'reseat_worktree'"
        ))),
    }
}

fn reseat_worktree(ctx: &ToolContext) -> Result<Value> {
    let cat = ctx.catalog.lock();
    let violations = scan_worktree_scoped(&cat.conn)?;
    let mut reseated = Vec::new();
    let mut collisions = Vec::new();
    for v in &violations {
        let detail: Value = serde_json::from_str(&v.detail).unwrap_or_default();
        let main_path = detail["main_path"].as_str().unwrap_or_default();
        match detail["classification"].as_str() {
            Some("no_collision") => {
                cat.conn.execute(
                    "UPDATE artifact SET abs_path=?1 WHERE id=?2",
                    rusqlite::params![main_path, v.artifact_id],
                )?;
                reseated.push(json!({ "id": v.artifact_id, "new_path": main_path }));
            }
            _ => collisions.push(json!({
                "id": v.artifact_id, "main_path": main_path,
                "into_id": detail["collision_with"].clone(),
            })),
        }
    }
    Ok(json!({ "fix": "reseat_worktree", "reseated": reseated, "collisions": collisions }))
}
```

(Preserve the existing `prune_missing` arm body verbatim inside the match. `reseat_worktree` is synchronous; keep `run_fix` async and call it without `.await`.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout reseat_worktree -- --nocapture`
Expected: PASS. Then the full gate: `cargo fmt && cargo clippy -- -D warnings && cargo test`.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/doctor.rs
git commit -m "feat(librarian): doctor fix=reseat_worktree auto-repoints no-collision rows

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

**1. Spec coverage:**
- Tool 1 (doctor check + `fix=reseat_worktree`) → Tasks 4, 5. ✓
- Tool 2 (`artifact(action="graft")`, events+links+params, remap+suspicious) → Tasks 1–3. ✓
- Delete-last cascade ordering → Task 1 (Global Constraints + test). ✓
- Link both-direction dedup → Task 1. ✓
- Renumber-incoming policy + remap → Task 2. ✓
- Near-dup `suspicious` (archetype 3) → Task 2. ✓
- Main-path derivation → Task 4. ✓
- **Deferred to the companion-skill plan (not this plan):** the skill itself, the rebase-invariant gate, citation rewrite in the tree, the conflict cookbook, the trigger eval, `graft` capability-probe. These are prompt-artifact work in `claude-plugins`, correctly out of scope here (spec §Architecture two-repo split).

**2. Placeholder scan:** Task 4/5 tests carry prose sketches for the `TempDir` worktree fixture rather than full literal bodies — flagged inline as "build concretely against the Task 4 temp layout." Every *implementation* step has complete code. The fixture-construction prose is the one soft spot; the executing agent must write the temp-repo scaffold. Acceptable (it's test scaffolding, not production code), but the reviewer should confirm the fixture actually creates a `.git`-pointer file so `is_linked_worktree` fires.

**3. Type consistency:** `graft_rows(&mut Catalog, &str, &str) -> Result<GraftReport>` is consistent across Tasks 1→3. `GraftReport` fields set in Task 1 (events/obs/links) and Task 2 (entries/remap/suspicious) are the same struct. `scan_worktree_scoped(&Connection) -> Result<Vec<Violation>>` consistent across Tasks 4→5. `worktree_main_root(&Path) -> Option<PathBuf>` consistent 4→5.

## Execution Handoff

Two execution options:

1. **Subagent-Driven (recommended)** — fresh subagent per task, two-stage review between tasks.
2. **Inline Execution** — batch execution with checkpoints in this session.

Per the project's model floor (CLAUDE.md): Sonnet is the implementer floor; budget at least one **Opus** review pass on the `graft` catalog core (Tasks 1–2) — it is shared infrastructure the skill builds on, exactly the "load-bearing, review harder" class.
