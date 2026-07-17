# Tracker Entry-Graph Stage 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make tracker entries globally addressable (`<slug>:<local>`) and let `append_entry` record `cites` edges at write time into a new `entry_cite` table — implementing TMR-1 + TMR-7 per `docs/superpowers/specs/2026-07-17-tracker-entry-graph-stage2-design.md`.

**Architecture:** Additive, no catalog re-key. New nullable `artifact.slug` (UNIQUE) + new `entry_cite` table keyed on `artifact(slug)` (FK, ON DELETE CASCADE, move-durable). `append_entry` mints the slug on first append and, when given `cites`, resolves each ref to a stable `dst_ref` and inserts `entry_cite` rows atomically inside its existing `IMMEDIATE` transaction. `artifact_link` and `link_scan` are untouched (table separation).

**Tech Stack:** Rust, rusqlite (SQLite), the codescout librarian catalog. Tests are `#[test]`/`#[tokio::test]` in-module; run with `cargo test`.

## Global Constraints

- Branch `experiments` (never `master`). All source edits via codescout MCP (`edit_code`/`edit_file`), never native Edit; `run_command` for `cargo`.
- Pre-commit gate every task: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` (the relevant crate).
- `PRAGMA foreign_keys = ON` is enforced in every `Catalog::open*` — entry endpoints must NOT go in `artifact_link` (FK to `artifact(id)`); they go in `entry_cite`.
- Entry id string form is exactly `<slug>:<local>` (slug is `[a-z0-9-]+`, colon-free; local is `<prefix>-<n>`, colon-free) — split on the single colon.
- MVP boundaries (spec § MVP boundaries): slug is catalog-only (no frontmatter mirror); slug minted on first `append_entry`; `cites` are REFUSED from a worktree checkout (`target != a.id`).
- `chrono::Utc::now().timestamp_millis()` is the epoch source for `created_at` (as `src/librarian/tools/link.rs:16`).
- Commit message trailer: `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- **Error-return idiom:** return `RecoverableError` exactly as the surrounding code does — the observed convention inside `append_entry`/tool calls is **bare** `return Err(RecoverableError::new(...))` / `Err(RecoverableError::with_hint(...))` with **no** `.into()`. Where a snippet below shows `.into()`, drop it to match; let `cargo build` confirm the exact coercion. This is an idiom detail, not a logic choice.
- **Transaction handle:** `augmentation::append_entry` runs in a `tx: Transaction`. Pass `&tx` to `ensure_slug`/`resolve_cite_ref`/`entry_cite::insert_with` (which take `&rusqlite::Connection`); `Transaction` derefs to `Connection`, so `&tx` coerces — use `&*tx` if the compiler asks.

---

### Task 1: Schema — `artifact.slug` column + `entry_cite` table

**Files:**
- Modify: `src/librarian/catalog/schema.sql` (fresh-DB shape)
- Modify: `src/librarian/catalog/mod.rs:78-142` (`apply_migrations_in_txn` — add v9 block) and its `tests` module
- Test: `src/librarian/catalog/mod.rs` tests module

**Interfaces:**
- Produces: column `artifact.slug TEXT` (nullable, UNIQUE where not null); table `entry_cite(src_slug, src_local, dst_ref, rel, origin, created_at)` with PK `(src_slug, src_local, dst_ref, rel)` and FK `src_slug REFERENCES artifact(slug) ON DELETE CASCADE`. Consumed by every later task.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/librarian/catalog/mod.rs`:

```rust
#[test]
fn migration_v9_adds_slug_column_and_entry_cite_table() {
    let cat = Catalog::open_in_memory().unwrap();
    assert!(column_exists(&cat.conn, "artifact", "slug").unwrap());
    let has_entry_cite: bool = cat
        .conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='entry_cite'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    assert!(has_entry_cite, "entry_cite table must exist");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout migration_v9_adds_slug_column_and_entry_cite_table`
Expected: FAIL (`column_exists` returns false / table missing).

- [ ] **Step 3: Add the columns/table to `schema.sql` (fresh DBs)**

In `src/librarian/catalog/schema.sql`, add `slug TEXT` to the `artifact` table (after `confidence`), and after the `artifact_link` block add:

```sql
CREATE UNIQUE INDEX IF NOT EXISTS ux_artifact_slug ON artifact(slug) WHERE slug IS NOT NULL;

CREATE TABLE IF NOT EXISTS entry_cite (
  src_slug   TEXT NOT NULL REFERENCES artifact(slug) ON DELETE CASCADE,
  src_local  TEXT NOT NULL,
  dst_ref    TEXT NOT NULL,
  rel        TEXT NOT NULL,
  origin     TEXT NOT NULL DEFAULT 'write',
  created_at INTEGER NOT NULL,
  PRIMARY KEY (src_slug, src_local, dst_ref, rel)
);
CREATE INDEX IF NOT EXISTS idx_entry_cite_dst ON entry_cite(dst_ref);
```

(Add `slug TEXT` on the `artifact` CREATE so it is: `  confidence    REAL NOT NULL DEFAULT 1.0,\n  slug          TEXT` — note the comma moves to `confidence`'s line.)

- [ ] **Step 4: Add the v9 migration block (existing DBs)**

In `apply_migrations_in_txn` (`src/librarian/catalog/mod.rs`), immediately before `migrate_v6::add_columns(conn)?;`, insert:

```rust
    // v9: entry-graph — artifact.slug + entry_cite table (Stage 2, TMR-1/TMR-7).
    if !column_exists(conn, "artifact", "slug")? {
        conn.execute("ALTER TABLE artifact ADD COLUMN slug TEXT", [])?;
    }
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS ux_artifact_slug ON artifact(slug) WHERE slug IS NOT NULL",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS entry_cite (
           src_slug   TEXT NOT NULL REFERENCES artifact(slug) ON DELETE CASCADE,
           src_local  TEXT NOT NULL,
           dst_ref    TEXT NOT NULL,
           rel        TEXT NOT NULL,
           origin     TEXT NOT NULL DEFAULT 'write',
           created_at INTEGER NOT NULL,
           PRIMARY KEY (src_slug, src_local, dst_ref, rel)
         )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_entry_cite_dst ON entry_cite(dst_ref)",
        [],
    )?;
    conn.execute("INSERT OR IGNORE INTO schema_version (version) VALUES (9)", [])?;
```

- [ ] **Step 5: Run tests to verify pass + idempotency**

Run: `cargo test -p codescout migration_v9_adds_slug_column_and_entry_cite_table migrations_are_idempotent`
Expected: PASS (the existing `migrations_are_idempotent` test opens twice; `IF NOT EXISTS` + `column_exists` guards keep v9 idempotent).

- [ ] **Step 6: Commit**

```bash
git add src/librarian/catalog/schema.sql src/librarian/catalog/mod.rs
git commit -m "feat(librarian): v9 schema — artifact.slug + entry_cite table (TMR-1/TMR-7)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: `entry_cite` catalog module

**Files:**
- Create: `src/librarian/catalog/entry_cite.rs`
- Modify: `src/librarian/catalog/mod.rs:7-18` (add `pub mod entry_cite;`)
- Test: in `entry_cite.rs`

**Interfaces:**
- Produces: `EntryCiteRow { src_slug: String, src_local: String, dst_ref: String, rel: String, origin: String, created_at: i64 }`; `insert_with(conn: &rusqlite::Connection, row: &EntryCiteRow) -> Result<()>`; `outgoing(cat: &Catalog, src_slug: &str) -> Result<Vec<EntryCiteRow>>`; `incoming(cat: &Catalog, dst_ref: &str) -> Result<Vec<EntryCiteRow>>`. Consumed by Tasks 4 (insert) and 5 (reads).

- [ ] **Step 1: Write the failing test**

Create `src/librarian/catalog/entry_cite.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{self, TestArtifactRowBuilder};
    use crate::librarian::catalog::Catalog;

    fn seed_slugged(cat: &Catalog, id: &str, slug: &str) {
        artifact::upsert(cat, &TestArtifactRowBuilder::new(id).build()).unwrap();
        cat.conn
            .execute("UPDATE artifact SET slug=?1 WHERE id=?2", rusqlite::params![slug, id])
            .unwrap();
    }

    #[test]
    fn insert_and_read_roundtrip() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_slugged(&cat, "art-a", "tracker-a");
        insert_with(
            &cat.conn,
            &EntryCiteRow {
                src_slug: "tracker-a".into(),
                src_local: "W-1".into(),
                dst_ref: "art-b-id".into(),
                rel: "cites".into(),
                origin: "write".into(),
                created_at: 1,
            },
        )
        .unwrap();
        assert_eq!(outgoing(&cat, "tracker-a").unwrap().len(), 1);
        assert_eq!(incoming(&cat, "art-b-id").unwrap().len(), 1);
    }

    #[test]
    fn cascade_delete_removes_entry_cite_when_artifact_deleted() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_slugged(&cat, "art-a", "tracker-a");
        insert_with(
            &cat.conn,
            &EntryCiteRow {
                src_slug: "tracker-a".into(),
                src_local: "W-1".into(),
                dst_ref: "x".into(),
                rel: "cites".into(),
                origin: "write".into(),
                created_at: 1,
            },
        )
        .unwrap();
        cat.conn.execute("DELETE FROM artifact WHERE id='art-a'", []).unwrap();
        assert_eq!(outgoing(&cat, "tracker-a").unwrap().len(), 0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout entry_cite`
Expected: FAIL to compile (`EntryCiteRow`/`insert_with`/`outgoing`/`incoming` undefined).

- [ ] **Step 3: Implement the module**

Prepend to `src/librarian/catalog/entry_cite.rs` (above the `tests` module):

```rust
use super::Catalog;
use anyhow::Result;
use rusqlite::params;

#[derive(Debug, Clone, PartialEq)]
pub struct EntryCiteRow {
    pub src_slug: String,
    pub src_local: String,
    pub dst_ref: String,
    pub rel: String,
    pub origin: String,
    pub created_at: i64,
}

/// Insert one entry-grain edge. `INSERT OR IGNORE` — the PK
/// (src_slug, src_local, dst_ref, rel) makes duplicates a no-op.
pub fn insert_with(conn: &rusqlite::Connection, row: &EntryCiteRow) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO entry_cite
           (src_slug, src_local, dst_ref, rel, origin, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![row.src_slug, row.src_local, row.dst_ref, row.rel, row.origin, row.created_at],
    )?;
    Ok(())
}

pub fn outgoing(cat: &Catalog, src_slug: &str) -> Result<Vec<EntryCiteRow>> {
    collect(cat, "WHERE src_slug = ?1", params![src_slug])
}

pub fn incoming(cat: &Catalog, dst_ref: &str) -> Result<Vec<EntryCiteRow>> {
    collect(cat, "WHERE dst_ref = ?1", params![dst_ref])
}

fn collect(cat: &Catalog, where_clause: &str, p: impl rusqlite::Params) -> Result<Vec<EntryCiteRow>> {
    let sql = format!(
        "SELECT src_slug, src_local, dst_ref, rel, origin, created_at FROM entry_cite {where_clause}"
    );
    let mut stmt = cat.conn.prepare(&sql)?;
    let rows = stmt
        .query_map(p, |r| {
            Ok(EntryCiteRow {
                src_slug: r.get(0)?,
                src_local: r.get(1)?,
                dst_ref: r.get(2)?,
                rel: r.get(3)?,
                origin: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

Add `pub mod entry_cite;` to the module list in `src/librarian/catalog/mod.rs` (alongside `pub mod links;`).

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p codescout entry_cite`
Expected: PASS (both roundtrip and cascade).

- [ ] **Step 5: Commit**

```bash
git add src/librarian/catalog/entry_cite.rs src/librarian/catalog/mod.rs
git commit -m "feat(librarian): entry_cite catalog module (insert/outgoing/incoming)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Slug minting — `slugify` + `ensure_slug`

**Files:**
- Modify: `src/librarian/catalog/artifact.rs` (add `slugify` + `ensure_slug` + tests)

**Interfaces:**
- Produces: `pub fn slugify(s: &str) -> String`; `pub fn ensure_slug(conn: &rusqlite::Connection, artifact_id: &str) -> Result<String>` — returns the existing `artifact.slug`, or mints (from title, fallback basename), dedups against `ux_artifact_slug`, writes it, returns it. Idempotent. Consumed by Task 4.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/librarian/catalog/artifact.rs`:

```rust
#[test]
fn ensure_slug_mints_dedups_and_is_idempotent() {
    let cat = Catalog::open_in_memory().unwrap();
    upsert(&cat, &TestArtifactRowBuilder::new("a").with_title("My Tracker").build()).unwrap();
    upsert(&cat, &TestArtifactRowBuilder::new("b").with_title("My Tracker").build()).unwrap();

    let s1 = ensure_slug(&cat.conn, "a").unwrap();
    assert_eq!(s1, "my-tracker");
    // Idempotent: second call returns the same slug, does not re-mint.
    assert_eq!(ensure_slug(&cat.conn, "a").unwrap(), "my-tracker");
    // Collision on the same base gets a numeric suffix.
    assert_eq!(ensure_slug(&cat.conn, "b").unwrap(), "my-tracker-2");
}

#[test]
fn slugify_normalizes() {
    assert_eq!(slugify("Fable Tuning — Findings!"), "fable-tuning-findings");
    assert_eq!(slugify("  A/B  test "), "a-b-test");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout ensure_slug_mints slugify_normalizes`
Expected: FAIL to compile (`slugify`/`ensure_slug` undefined).

- [ ] **Step 3: Implement**

Add to `src/librarian/catalog/artifact.rs` (module-level, near `upsert`):

```rust
/// Lowercase, non-alphanumeric runs -> single '-', trimmed of leading/trailing '-'.
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Return `artifact.slug` for `artifact_id`, minting + persisting one if NULL.
/// Base = slugify(title) or, if empty, slugify(abs_path file stem). Dedups with
/// `-2`, `-3`, ... against the unique index. Assumes an open write context on `conn`.
pub fn ensure_slug(conn: &rusqlite::Connection, artifact_id: &str) -> Result<String> {
    let existing: Option<Option<String>> = conn
        .query_row(
            "SELECT slug FROM artifact WHERE id = ?1",
            params![artifact_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(slug_col) = existing else {
        anyhow::bail!("ensure_slug: no artifact with id {artifact_id}");
    };
    if let Some(s) = slug_col {
        return Ok(s);
    }
    let (title, abs_path): (Option<String>, String) = conn.query_row(
        "SELECT title, abs_path FROM artifact WHERE id = ?1",
        params![artifact_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut base = title.as_deref().map(slugify).unwrap_or_default();
    if base.is_empty() {
        let stem = std::path::Path::new(&abs_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("tracker");
        base = slugify(stem);
    }
    if base.is_empty() {
        base = "tracker".to_string();
    }
    let mut candidate = base.clone();
    let mut n = 2;
    loop {
        let taken: bool = conn
            .query_row(
                "SELECT 1 FROM artifact WHERE slug = ?1",
                params![candidate],
                |_| Ok(true),
            )
            .optional()?
            .is_some();
        if !taken {
            break;
        }
        candidate = format!("{base}-{n}");
        n += 1;
    }
    conn.execute(
        "UPDATE artifact SET slug = ?1 WHERE id = ?2",
        params![candidate, artifact_id],
    )?;
    Ok(candidate)
}
```

Ensure `use rusqlite::OptionalExtension;` is in scope (for `.optional()`) — it is already used elsewhere in the catalog; add the import to `artifact.rs` if `cargo build` reports it missing.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p codescout ensure_slug_mints slugify_normalizes`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/catalog/artifact.rs
git commit -m "feat(librarian): slugify + ensure_slug (lazy, deduped, immutable)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: `append_entry` gains `cites` — the write path

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs:179-254` (`append_entry` signature + body; add `resolve_cite_ref`)
- Modify: `src/librarian/tools/append_entry.rs:8-14,20-37` (`Args.cites`; worktree guard; pass `cites`)
- Modify: `src/librarian/tools/artifact.rs` (input_schema: add `cites`)
- Test: in both files

**Interfaces:**
- Consumes: `entry_cite::insert_with`, `entry_cite::EntryCiteRow` (Task 2); `artifact::ensure_slug` (Task 3).
- Produces: `augmentation::append_entry(cat, artifact_id, entry_collection, id_prefix, entry, cites: &[String]) -> Result<String>` (new trailing param); `resolve_cite_ref(conn, ref_str) -> Result<String>`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/librarian/tools/append_entry.rs`:

```rust
#[tokio::test]
async fn append_with_cites_writes_entry_cite_and_not_artifact_link() {
    let ctx = mk_ctx();
    seed(&ctx, "art1"); // seeds an augmented tracker with entry_collection "failures"
    seed(&ctx, "art2");
    let out = call(
        &ctx,
        json!({
            "id": "art1", "entry_collection": "failures", "id_prefix": "F",
            "entry": {"status": "fail"}, "cites": ["art2"]
        }),
    )
    .await
    .unwrap();
    assert_eq!(out["id"], "F-1");
    let cat = ctx.catalog.lock();
    // slug minted on art1; one entry_cite row; zero artifact_link rows.
    let slug: String = cat
        .conn
        .query_row("SELECT slug FROM artifact WHERE id='art1'", [], |r| r.get(0))
        .unwrap();
    let ec = crate::librarian::catalog::entry_cite::outgoing(&cat, &slug).unwrap();
    assert_eq!(ec.len(), 1);
    assert_eq!(ec[0].dst_ref, "art2");
    let al: i64 = cat
        .conn
        .query_row("SELECT COUNT(*) FROM artifact_link", [], |r| r.get(0))
        .unwrap();
    assert_eq!(al, 0, "cites must not touch artifact_link");
}

#[tokio::test]
async fn append_with_unresolvable_cite_writes_nothing() {
    let ctx = mk_ctx();
    seed(&ctx, "art1");
    let err = call(
        &ctx,
        json!({
            "id": "art1", "entry_collection": "failures", "id_prefix": "F",
            "entry": {"status": "fail"}, "cites": ["no-such-target"]
        }),
    )
    .await
    .unwrap_err();
    assert!(err.downcast_ref::<RecoverableError>().is_some());
    let cat = ctx.catalog.lock();
    // atomic: entry NOT appended.
    let aug = augmentation::get(&cat, "art1").unwrap().unwrap();
    assert!(!aug.params.contains("F-1"), "entry must not be written when a cite is bad");
}

#[tokio::test]
async fn append_with_cites_from_worktree_is_refused() {
    let ctx = crate::librarian::tools::worktree::test_support::wt_ctx(
        Catalog::open_in_memory().unwrap(),
    );
    let main_id = {
        let c = ctx.catalog.lock();
        crate::librarian::tools::worktree::test_support::seed_main_tracker(&c)
    };
    let err = call(
        &ctx,
        json!({
            "id": main_id, "entry_collection": "items", "id_prefix": "F",
            "entry": {"t": "x"}, "cites": ["deadbeefdeadbeef"]
        }),
    )
    .await
    .unwrap_err();
    assert!(err.downcast_ref::<RecoverableError>().is_some());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout append_with_cites append_with_unresolvable append_with_cites_from_worktree`
Expected: FAIL to compile (`cites` not accepted; no `entry_cite` write).

- [ ] **Step 3: Add `cites` to the tool `Args` and the worktree guard**

In `src/librarian/tools/append_entry.rs`, extend `Args`:

```rust
#[derive(serde::Deserialize)]
struct Args {
    id: String,
    entry_collection: String,
    id_prefix: String,
    #[serde(default = "default_entry")]
    entry: Value,
    #[serde(default)]
    cites: Vec<String>,
}
```

And in `call`, after resolving `target`:

```rust
    let mut cat = ctx.catalog.lock();
    let target = super::worktree::resolve_write_target(&mut cat, ctx, &a.id)?;
    if !a.cites.is_empty() && target != a.id {
        return Err(RecoverableError::with_hint(
            "append_entry: `cites` is not supported from a worktree checkout".to_string(),
            "Entry-graph edges must key to the main tracker. Omit `cites`, or append from the main checkout.".to_string(),
        )
        .into());
    }
    let id = augmentation::append_entry(
        &mut cat,
        &target,
        &a.entry_collection,
        &a.id_prefix,
        a.entry,
        &a.cites,
    )?;
    Ok(json!({"id": id, "artifact_id": target}))
```

- [ ] **Step 4: Extend `augmentation::append_entry` + add `resolve_cite_ref`**

In `src/librarian/catalog/augmentation.rs`, change the signature to add `cites: &[String]` (trailing), and after the entry id is computed and the params `UPDATE` runs (still inside the `tx`, before `tx.commit()`), insert edges:

```rust
    // ... existing body appends the entry and computes `new_id`, runs the params UPDATE ...

    if !cites.is_empty() {
        let slug = crate::librarian::catalog::artifact::ensure_slug(&tx, artifact_id)?;
        let now = chrono::Utc::now().timestamp_millis();
        for raw in cites {
            let dst_ref = resolve_cite_ref(&tx, raw)?;
            crate::librarian::catalog::entry_cite::insert_with(
                &tx,
                &crate::librarian::catalog::entry_cite::EntryCiteRow {
                    src_slug: slug.clone(),
                    src_local: new_id.clone(),
                    dst_ref,
                    rel: "cites".to_string(),
                    origin: "write".to_string(),
                    created_at: now,
                },
            )?;
        }
    }
    tx.commit()?;
    Ok(new_id)
```

Add the resolver (module-level in `augmentation.rs`):

```rust
/// Resolve a user-supplied cite ref to a stable `entry_cite.dst_ref`.
/// Accepts: a 16-hex artifact id that exists; a `<slug>:<local>` whose slug is a
/// known artifact and whose local exists in that artifact's entry_collection; or a
/// rel_path (suffix of exactly one artifact's abs_path). Rejects anything else.
fn resolve_cite_ref(conn: &rusqlite::Connection, raw: &str) -> Result<String> {
    // 1. artifact id (16 lowercase hex chars).
    let is_hex16 = raw.len() == 16 && raw.bytes().all(|b| b.is_ascii_hexdigit());
    if is_hex16 {
        let exists: bool = conn
            .query_row("SELECT 1 FROM artifact WHERE id=?1", rusqlite::params![raw], |_| Ok(true))
            .optional()?
            .is_some();
        if exists {
            return Ok(raw.to_string());
        }
    }
    // 2. <slug>:<local>
    if let Some((slug, local)) = raw.split_once(':') {
        let coll: Option<Option<String>> = conn
            .query_row(
                "SELECT au.entry_collection
                   FROM artifact a JOIN artifact_augmentation au ON au.artifact_id = a.id
                  WHERE a.slug = ?1",
                rusqlite::params![slug],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(Some(collection)) = coll {
            let params_text: String = conn.query_row(
                "SELECT au.params FROM artifact a
                   JOIN artifact_augmentation au ON au.artifact_id = a.id
                  WHERE a.slug = ?1",
                rusqlite::params![slug],
                |r| r.get(0),
            )?;
            let params: Value = serde_json::from_str(&params_text).unwrap_or_else(|_| json!({}));
            let local_exists = params
                .get(&collection)
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .any(|e| e.get("id").and_then(|v| v.as_str()) == Some(local));
            if local_exists {
                return Ok(raw.to_string());
            }
        }
        return Err(RecoverableError::new(format!(
            "append_entry: cite `{raw}` — no such entry (slug or local id not found)"
        ))
        .into());
    }
    // 3. rel_path suffix match — must resolve to exactly one artifact.
    let like = format!("%/{raw}");
    let mut stmt = conn.prepare("SELECT id FROM artifact WHERE abs_path = ?1 OR abs_path LIKE ?2")?;
    let ids: Vec<String> = stmt
        .query_map(rusqlite::params![raw, like], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    match ids.len() {
        1 => Ok(ids.into_iter().next().unwrap()),
        0 => Err(RecoverableError::with_hint(
            format!("append_entry: cite `{raw}` did not resolve"),
            "Use a 16-hex artifact id, a `<slug>:<local>` entry id, or a unique rel_path.".to_string(),
        )
        .into()),
        _ => Err(RecoverableError::new(format!(
            "append_entry: cite `{raw}` is ambiguous ({} artifacts match)",
            ids.len()
        ))
        .into()),
    }
}
```

Update ALL existing callers of `augmentation::append_entry` to pass the new arg. Known callers (from `grep append_entry src/`): `src/librarian/tools/append_entry.rs` (Step 3 passes `&a.cites`), and the `augmentation.rs` unit tests (pass `&[]`). Fix any others `cargo build` flags.

- [ ] **Step 5: Add `cites` to the `artifact` tool input schema**

In `src/librarian/tools/artifact.rs` `input_schema`, add alongside `entry`:

```rust
                "cites": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "append_entry: optional write-time citations. Each ref is a 16-hex artifact id, a `<slug>:<local>` entry id, or a unique rel_path. Creates entry_cite edges from the new entry atomically; an unresolvable/ambiguous ref aborts the whole call. Not supported from a worktree checkout."
                }
```

- [ ] **Step 6: Run tests to verify pass**

Run: `cargo test -p codescout append_entry append_with_cites append_with_unresolvable append_with_cites_from_worktree`
Expected: PASS (all, including the pre-existing `append_entry_*` tests updated to pass `&[]`).

- [ ] **Step 7: Commit**

```bash
git add src/librarian/catalog/augmentation.rs src/librarian/tools/append_entry.rs src/librarian/tools/artifact.rs
git commit -m "feat(librarian): append_entry write-time cites -> entry_cite (TMR-7)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: Read surface — `get(include_links=true)` surfaces `entry_cite`

**Files:**
- Modify: `src/librarian/tools/get.rs:110-176,289-292` (add `entry_links` block)
- Test: in `get.rs`

**Interfaces:**
- Consumes: `entry_cite::outgoing`/`incoming` (Task 2), `ensure_slug`-set `artifact.slug` (Task 3).
- Produces: `out["entry_links"] = {outgoing: [...], incoming: [...]}` when `include_links=true` and the artifact has a slug.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/librarian/tools/get.rs`:

```rust
#[tokio::test]
async fn include_links_surfaces_entry_cite_edges() {
    use crate::librarian::catalog::entry_cite::{self, EntryCiteRow};
    let cat = Catalog::open_in_memory().unwrap();
    artifact::upsert(&cat, &mk_row("a")).unwrap();
    cat.conn.execute("UPDATE artifact SET slug='tracker-a' WHERE id='a'", []).unwrap();
    entry_cite::insert_with(
        &cat.conn,
        &EntryCiteRow {
            src_slug: "tracker-a".into(),
            src_local: "W-1".into(),
            dst_ref: "some-target".into(),
            rel: "cites".into(),
            origin: "write".into(),
            created_at: 1,
        },
    )
    .unwrap();
    let ctx = /* build a ToolContext wrapping `cat` as the other get tests do */ mk_ctx_with(cat);
    let v = call(&ctx, json!({"id": "a", "include_links": true})).await.unwrap();
    assert_eq!(v["entry_links"]["outgoing"].as_array().unwrap().len(), 1);
    assert_eq!(v["entry_links"]["outgoing"][0]["dst_ref"], "some-target");
}
```

(Use whatever ctx-construction helper the existing `get.rs` tests use — e.g. the pattern in `include_links_direction_out_hides_incoming`. Name the helper `mk_ctx_with` if one must be added; otherwise inline the existing pattern.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout include_links_surfaces_entry_cite_edges`
Expected: FAIL (`entry_links` absent).

- [ ] **Step 3: Implement the `entry_links` block**

In `src/librarian/tools/get.rs`, where `links_json` is assembled under `if want_links { ... }`, add after it a sibling `entry_links_json`:

```rust
    let entry_links_json = if want_links {
        let slug: Option<String> = cat
            .conn
            .query_row(
                "SELECT slug FROM artifact WHERE id = ?1",
                rusqlite::params![a.id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        match slug {
            Some(s) => {
                let out_items: Vec<Value> = crate::librarian::catalog::entry_cite::outgoing(&cat, &s)?
                    .into_iter()
                    .map(|e| json!({"src_local": e.src_local, "dst_ref": e.dst_ref, "rel": e.rel}))
                    .collect();
                let in_items: Vec<Value> = crate::librarian::catalog::entry_cite::incoming(&cat, &a.id)?
                    .into_iter()
                    .chain(crate::librarian::catalog::entry_cite::incoming(&cat, &format!("{s}:%"))?)
                    .map(|e| json!({"src": format!("{}:{}", e.src_slug, e.src_local), "rel": e.rel}))
                    .collect();
                Some(json!({"outgoing": out_items, "incoming": in_items}))
            }
            None => None,
        }
    } else {
        None
    };
```

And near `out["links"] = v;`:

```rust
    if let Some(v) = entry_links_json {
        out["entry_links"] = v;
    }
```

Note: incoming-by-`<slug>:%` is a LIKE convenience; if the `incoming` helper takes an exact `dst_ref`, add an `incoming_like(cat, pattern)` variant in `entry_cite.rs` using `LIKE` instead, and call that. Keep the exact-match `incoming` for artifact-id targets.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p codescout include_links_surfaces_entry_cite_edges include_links`
Expected: PASS (new test + existing `include_links_*` tests unaffected).

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/get.rs src/librarian/catalog/entry_cite.rs
git commit -m "feat(librarian): get(include_links) surfaces entry_cite edges

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Out of scope (deferred follow-ons)

- **Frontmatter slug mirror** for rebuild-durability (MVP is catalog-only).
- **Worktree `cites`** via main-root slug resolution (MVP refuses cites-from-worktree).
- **`graph` tool + `librarian(context)`** traversal of entry nodes (MVP surfaces edges only via `get`).
- **Standalone "who cites entry X" query API.**
- **rel_path-sha artifact-id re-key** (its own spec — Stage 2b/3).

## Whole-stage verification

After Task 5, on `experiments`: `cargo fmt && cargo clippy -- -D warnings && cargo test -p codescout`. Then a live check via `cargo rb` + `/mcp` reconnect: `artifact(append_entry, id=<a tracker>, cites=["<another tracker rel_path>"])` returns a `<slug>:local` id; `artifact(get, id=<tracker>, include_links=true)` shows the edge under `entry_links`; `librarian(link_scan, write=true)` leaves it untouched.
