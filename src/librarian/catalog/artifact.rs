use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::Catalog;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactRow {
    pub id: String,
    pub abs_path: std::path::PathBuf,
    pub kind: String,
    pub status: String,
    pub title: Option<String>,
    pub owners: Vec<String>,
    pub tags: Vec<String>,
    pub topic: Option<String>,
    pub time_scope: Option<String>,
    pub source: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub file_mtime: i64,
    pub file_sha256: String,
    pub confidence: f64,
}

#[cfg(test)]
pub(crate) struct TestArtifactRowBuilder {
    id: String,
    abs_path: std::path::PathBuf,
    kind: String,
    status: String,
    title: Option<String>,
    owners: Vec<String>,
    tags: Vec<String>,
    source: Option<String>,
    created_at: i64,
    updated_at: i64,
    file_mtime: i64,
    file_sha256: String,
}

#[cfg(test)]
impl TestArtifactRowBuilder {
    pub(crate) fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            abs_path: std::path::PathBuf::from(format!("/test/r/{id}.md")),
            kind: "spec".to_string(),
            status: "active".to_string(),
            title: None,
            owners: vec![],
            tags: vec![],
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: String::new(),
        }
    }

    pub(crate) fn with_abs_path(mut self, abs_path: impl Into<std::path::PathBuf>) -> Self {
        self.abs_path = abs_path.into();
        self
    }

    pub(crate) fn with_kind(mut self, kind: &str) -> Self {
        self.kind = kind.to_string();
        self
    }

    pub(crate) fn with_status(mut self, status: &str) -> Self {
        self.status = status.to_string();
        self
    }

    pub(crate) fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub(crate) fn with_owners(mut self, owners: Vec<String>) -> Self {
        self.owners = owners;
        self
    }

    pub(crate) fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub(crate) fn with_source(mut self, source: &str) -> Self {
        self.source = Some(source.to_string());
        self
    }

    pub(crate) fn with_created_at(mut self, created_at: i64) -> Self {
        self.created_at = created_at;
        self
    }

    pub(crate) fn with_updated_at(mut self, updated_at: i64) -> Self {
        self.updated_at = updated_at;
        self
    }

    pub(crate) fn with_file_mtime(mut self, file_mtime: i64) -> Self {
        self.file_mtime = file_mtime;
        self
    }

    pub(crate) fn with_file_sha256(mut self, file_sha256: &str) -> Self {
        self.file_sha256 = file_sha256.to_string();
        self
    }

    pub(crate) fn build(self) -> ArtifactRow {
        ArtifactRow {
            id: self.id,
            abs_path: self.abs_path,
            kind: self.kind,
            status: self.status,
            title: self.title,
            owners: self.owners,
            tags: self.tags,
            topic: None,
            time_scope: None,
            source: self.source,
            created_at: self.created_at,
            updated_at: self.updated_at,
            file_mtime: self.file_mtime,
            file_sha256: self.file_sha256,
            confidence: 1.0,
        }
    }
}

pub fn upsert(cat: &Catalog, row: &ArtifactRow) -> Result<()> {
    // F-6a fix (bug-tracker #5): the artifact schema declares
    // `abs_path TEXT NOT NULL UNIQUE`, but the INSERT below only handles
    // `ON CONFLICT(id)`. A row at the same abs_path with a *different* id
    // (e.g. caused by an id-algorithm change across catalog versions, or
    // path normalization drift between walks) would trigger an unhandled
    // UNIQUE constraint failure.
    //
    // The safe pre-clean: remove any row whose abs_path matches but id
    // differs. The natural identity of a file in this catalog is its
    // abs_path; the id is a derived hash. When the two diverge, the
    // abs_path wins (file content survives across id-algorithm changes;
    // the old id-based row is stale).
    let abs_path_str = crate::util::fs::RepoPath::from(&row.abs_path);
    cat.conn.execute(
        "DELETE FROM artifact WHERE abs_path = ?1 AND id != ?2",
        params![abs_path_str, row.id],
    )?;

    cat.conn.execute(
        "INSERT INTO artifact (id, abs_path, kind, status, title, owners, tags,
            topic, time_scope, source, created_at, updated_at, file_mtime, file_sha256, confidence)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
         ON CONFLICT(id) DO UPDATE SET
            abs_path=excluded.abs_path,
            kind=excluded.kind, status=excluded.status,
            title=excluded.title, owners=excluded.owners, tags=excluded.tags,
            topic=excluded.topic, time_scope=excluded.time_scope,
            source=excluded.source, updated_at=excluded.updated_at,
            file_mtime=excluded.file_mtime, file_sha256=excluded.file_sha256,
            confidence=excluded.confidence",
        params![
            row.id,
            abs_path_str,
            row.kind,
            row.status,
            row.title,
            serde_json::to_string(&row.owners)?,
            serde_json::to_string(&row.tags)?,
            row.topic,
            row.time_scope,
            row.source,
            row.created_at,
            row.updated_at,
            row.file_mtime,
            row.file_sha256,
            row.confidence,
        ],
    )?;
    Ok(())
}

pub fn get(cat: &Catalog, id: &str) -> Result<Option<ArtifactRow>> {
    cat.conn
        .prepare("SELECT id, abs_path, kind, status, title, owners, tags,
                  topic, time_scope, source, created_at, updated_at, file_mtime, file_sha256, confidence
                  FROM artifact WHERE id = ?1")?
        .query_row(params![id], row_from_sql)
        .optional()
        .map_err(Into::into)
}
/// The content hash as of this artifact's last SUCCESSFUL embed — `None` when it
/// has never been embedded, or predates the v11 column.
///
/// Deliberately NOT a field on [`ArtifactRow`], for the reason `slug` and
/// `missing_since` are not: it is lifecycle state owned by a different
/// subsystem rather than part of the row's content, so it is read and written
/// on its own instead of through every construction site.
///
/// This is the stamp the embed decision reads. `file_sha256` means only "this
/// content was written to the catalog" — reading that as "embedded" is the
/// defect in
/// docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md,
/// because the row write is unconditional while the embed is not.
pub fn embedded_sha256(cat: &Catalog, id: &str) -> Result<Option<String>> {
    cat.conn
        .prepare("SELECT embedded_sha256 FROM artifact WHERE id = ?1")?
        .query_row(params![id], |r| r.get::<_, Option<String>>(0))
        .optional()
        .map(|v| v.flatten())
        .map_err(Into::into)
}

/// Record that `sha` is embedded for `id`.
///
/// **Call this only once EVERY chunk of the artifact has been stored.** The
/// embed queue is chunk-grained — ~20 chunks per artifact on this corpus — so
/// stamping after the first successful chunk would rebuild the very trap this
/// column closes, one level down: the artifact would read as embedded while
/// most of its chunks had no vector, and no ordinary run would ever retry them.
pub fn set_embedded_sha256(cat: &Catalog, id: &str, sha: &str) -> Result<()> {
    cat.conn.execute(
        "UPDATE artifact SET embedded_sha256 = ?2 WHERE id = ?1",
        params![id, sha],
    )?;
    Ok(())
}

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
///
/// **A minted slug is never 16 hex characters.** `entry_cite.dst_ref` is free TEXT
/// holding *either* a 16-hex artifact id (the hex-id / rel_path citation forms) *or* a
/// `<slug>:<local>` pair, and `gc::apply_rehome`'s `dst_ref` UPDATE tells them apart on
/// exactly that shape — "a slug never equals an id, so it's correctly left untouched".
/// Nothing enforced it. `slugify` emits `[a-z0-9-]`, so a title or stem of sixteen hex
/// characters produces a slug indistinguishable from an id, and a rehome would then
/// rewrite an entry-grain citation that merely *looked* like a file-grain one.
///
/// Measured 2026-08-20 before adding this: zero of 4106 catalogued artifacts have a
/// title or stem that would slugify to that shape, so this guard changes no existing
/// row. It is here because the bulk backfill ([`mint_missing_slugs`]) multiplies the
/// population that could trip it from 2 to ~4106 in a single call, and the failure is
/// silent — a rewritten `dst_ref`, not an error.
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
    base = truncate_slug_base(&base, SLUG_BASE_MAX);
    let mut candidate = base.clone();
    let mut n = 2;
    loop {
        // Never hand out a slug shaped like an artifact id — see the doc comment.
        // Suffixing rather than rejecting keeps the mint total: a caller asking for a
        // slug always gets one, and `-2` is already the collision vocabulary.
        let looks_like_id = looks_like_artifact_id(&candidate);
        let taken: bool = conn
            .query_row(
                "SELECT 1 FROM artifact WHERE slug = ?1",
                params![candidate],
                |_| Ok(true),
            )
            .optional()?
            .is_some();
        if !taken && !looks_like_id {
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

/// `upsert` plus an idempotent slug mint — the entry point for any call site that gives
/// an artifact row its identity for the first time (create, update, reindex).
///
/// **Not for `mv`/`graft_rows`.** A move mints a fresh `id` for the same underlying
/// artifact while the old row (with its existing slug) still exists until `graft_rows`
/// deletes it — mint here first and `ensure_slug`'s dedup check finds the old row's slug
/// still "taken" and hands the new row a needless `-2` suffix, permanently (slugs are
/// immutable once minted). Moves carry the old slug forward explicitly in `graft_rows`
/// instead. docs/issues/archive/2026-08-21-create-mints-no-slug-so-new-entries-never-reach-the-entry-graph.md
pub fn upsert_and_mint_slug(cat: &Catalog, row: &ArtifactRow) -> Result<()> {
    upsert(cat, row)?;
    ensure_slug(&cat.conn, &row.id)?;
    Ok(())
}

/// Longest base a minted slug may have, before any `-2`/`-3` dedup suffix.
///
/// A slug is **immutable once non-null** and `entry_cite.src_slug` FKs it, so the
/// derivation is a one-way door: re-deriving later means re-keying a column that
/// entry-grain citations depend on. It therefore has to be right before the backfill
/// runs, not after. A slug is also not purely internal — `entry_cite.dst_ref` stores the
/// `<slug>:<local>` form, so an over-long slug lands in a citation-shaped position.
///
/// **50 is measured, not guessed.** Simulating the real `slugify` over the 4104 unslugged
/// rows of the live catalog, at caps of none/60/50/40/30:
///
/// | cap | rows needing a suffix | max collision depth | median len | max len |
/// |---|---|---|---|---|
/// | none | 115 | 10 | 44 | **232** |
/// | 60 | 134 | 10 | 44 | 61 |
/// | **50** | **173** | **10** | **41** | **52** |
/// | 40 | 269 | 10 | 34 | 42 |
/// | 30 | 532 | 10 | 25 | 32 |
///
/// Two things that table settles. **Truncation adds no collision depth** — the worst
/// chain is 10 at every cap including none, because it comes from ten artifacts whose
/// titles all slugify to `skill`, a string no cap touches. Capping cannot produce the
/// long `-2…-47` chains the obvious objection predicts. And **the marginal cost of 50
/// over 60 is 39 rows**, while 40 nearly triples the suffixed count for ten more
/// characters, which is why the knee is here.
const SLUG_BASE_MAX: usize = 50;

/// Trim `base` to at most `cap` bytes, preferring to cut at a `-` boundary so the slug
/// ends on a whole word.
///
/// Three cases, in order:
///
/// 1. Already within budget — returned untouched.
/// 2. The cut lands **exactly** on a separator (`base[cap] == '-'`), so the first `cap`
///    bytes are already whole words. Keep them. Without this case the next branch trims
///    one more word off a base that needed no trimming at all.
/// 3. Otherwise cut back to the last `-` inside the budget — but only if that leaves at
///    least half the budget. A base whose first word is longer than `cap/2` would
///    otherwise collapse to a stub, which is worse than a mid-word cut because stubs
///    collide with each other.
///
/// `slugify` emits ASCII `[a-z0-9-]` only, so byte slicing is char-safe by construction.
fn truncate_slug_base(base: &str, cap: usize) -> String {
    if base.len() <= cap {
        return base.to_string();
    }
    let cut = &base[..cap];
    if base.as_bytes().get(cap) == Some(&b'-') {
        return cut.to_string();
    }
    match cut.rfind('-') {
        Some(i) if i >= cap / 2 => cut[..i].to_string(),
        _ => cut.to_string(),
    }
}

/// Whether `s` has the exact shape of a catalog artifact id: 16 lowercase hex chars.
///
/// `librarian::ids::artifact_id_from_abs` emits that shape, and several columns hold
/// "an id OR something else" discriminated by it. Kept next to [`ensure_slug`] because
/// the mint is the one place that could manufacture a counterfeit.
fn looks_like_artifact_id(s: &str) -> bool {
    s.len() == 16
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// One artifact that had no slug and now does.
#[derive(Debug, Clone, PartialEq)]
pub struct MintedSlug {
    pub id: String,
    pub abs_path: String,
    pub slug: String,
}

/// Mint a slug for every artifact whose `slug` is NULL, in one transaction.
///
/// The backfill [`ensure_slug`] was always going to need: `entry_cite.src_slug` FKs
/// `artifact(slug)`, so an entry-grain edge can only exist for a source that has one,
/// and slugs are minted lazily on first `append_entry(cites=…)`. Measured 2026-08-20:
/// **2 of 4106** catalogued artifacts had one.
///
/// **Delegates to [`ensure_slug`] per row rather than reimplementing the rule.** The
/// base/fallback/dedup logic (title → file stem → `"tracker"`, `-2`/`-3` suffixes, the
/// id-shape guard) lives in one place, so the bulk path and the lazy path cannot drift.
/// A second implementation that agreed today and diverged later is the whole failure
/// class this codebase keeps finding.
///
/// **Ordered by `abs_path`, and that is load-bearing rather than tidiness.** Dedup is
/// first-come-first-served against the unique index, so iteration order decides which of
/// six artifacts titled "Changelog" gets `changelog` and which get `changelog-2..6` —
/// and a slug is immutable once non-null, so the order is baked in permanently on the
/// first run. Without a total order the assignment would vary run to run and machine to
/// machine. Measured on this corpus: 54 title values are shared by 139 rows (max group
/// 10), and 105 rows have no title at all and resolve through the stem fallback.
///
/// **`confirm=false` rolls back rather than simulating.** The dry run executes the real
/// mint and discards it, so the preview cannot disagree with what applying would do —
/// a hand-written "what would happen" pass is a second implementation with the same
/// drift risk as above.
///
/// Not scoped to a project, unlike `fix=repair_frontmatter_id`: that one WRITES FILES in
/// whatever repo it sweeps, while this writes only machine-local catalog rows. Minting
/// the whole corpus in one deterministic pass also makes assignment a pure function of
/// (corpus, already-minted slugs) instead of depending on which repo happened to run it
/// first.
pub fn mint_missing_slugs(conn: &rusqlite::Connection, confirm: bool) -> Result<Vec<MintedSlug>> {
    let pending: Vec<(String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT id, abs_path FROM artifact WHERE slug IS NULL ORDER BY abs_path, id",
        )?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };

    let tx = conn.unchecked_transaction()?;
    let mut minted = Vec::with_capacity(pending.len());
    for (id, abs_path) in pending {
        let slug = ensure_slug(&tx, &id)?;
        minted.push(MintedSlug { id, abs_path, slug });
    }
    if confirm {
        tx.commit()?;
    } else {
        tx.rollback()?;
    }
    Ok(minted)
}

/// `(with_slug, without_slug)` — the backfill's progress, for `catalog_health`.
pub fn slug_coverage(conn: &rusqlite::Connection) -> Result<(usize, usize)> {
    let with: i64 = conn.query_row(
        "SELECT COUNT(*) FROM artifact WHERE slug IS NOT NULL",
        [],
        |r| r.get(0),
    )?;
    let without: i64 = conn.query_row(
        "SELECT COUNT(*) FROM artifact WHERE slug IS NULL",
        [],
        |r| r.get(0),
    )?;
    Ok((with as usize, without as usize))
}

pub fn delete(cat: &Catalog, id: &str) -> Result<bool> {
    Ok(cat
        .conn
        .execute("DELETE FROM artifact WHERE id = ?1", params![id])?
        > 0)
}

/// Delete rows whose `abs_path` is under one of `scope_roots` but **not** under
/// any path in `active_roots`. Returns the number removed.
///
/// `scope_roots` bounds the blast radius — a row outside every scope root is
/// never touched, even if it is also outside every active root. This guards the
/// single machine-global catalog against the cross-workspace wipe (`3ea49090`):
/// callers pass the active workspace's own roots as `scope_roots`, so the sweep
/// can only prune within that workspace's territory. Empty `active_roots` or
/// empty `scope_roots` is a no-op (returns 0) — never a `DELETE FROM artifact`.
pub fn delete_orphan_repos(
    cat: &Catalog,
    active_roots: &[&std::path::Path],
    scope_roots: &[&std::path::Path],
) -> Result<usize> {
    // Never an unbounded wipe: with no active roots (nothing to keep) or no scope
    // (no bounded territory to prune within), do nothing. The catalog is a single
    // machine-global DB, so `DELETE FROM artifact` here would erase every other
    // workspace's rows (bug 3ea49090).
    if active_roots.is_empty() || scope_roots.is_empty() {
        return Ok(0);
    }
    // Forward-slash normalize to match the form abs_paths are stored in
    // (artifact::upsert writes forward-slash via RepoPath). Without this, on
    // Windows a LIKE pattern would use backslash and match NOTHING.
    let scope_likes: Vec<String> = scope_roots
        .iter()
        .map(|p| format!("{}/%", crate::util::fs::RepoPath::from_path(p)))
        .collect();
    let active_likes: Vec<String> = active_roots
        .iter()
        .map(|p| format!("{}/%", crate::util::fs::RepoPath::from_path(p)))
        .collect();

    // Delete rows that are UNDER some scope root but NOT under any active root.
    // The scope clause is the blast-radius guard: a row outside every scope root
    // is never matched, even when it is also outside every active root.
    let in_scope: Vec<String> = (1..=scope_likes.len())
        .map(|i| format!("abs_path LIKE ?{i}"))
        .collect();
    let under_active: Vec<String> = (scope_likes.len() + 1
        ..=scope_likes.len() + active_likes.len())
        .map(|i| format!("abs_path LIKE ?{i}"))
        .collect();
    let sql = format!(
        "DELETE FROM artifact WHERE ({}) AND NOT ({})",
        in_scope.join(" OR "),
        under_active.join(" OR "),
    );
    let params: Vec<String> = scope_likes.into_iter().chain(active_likes).collect();
    let param_refs: Vec<&dyn rusqlite::ToSql> =
        params.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    let n = cat
        .conn
        .execute(&sql, rusqlite::params_from_iter(param_refs.iter().copied()))?;
    Ok(n)
}

pub(crate) fn row_from_sql(r: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactRow> {
    let owners_s: String = r.get(5)?;
    let tags_s: String = r.get(6)?;
    let owners: Vec<String> = serde_json::from_str(&owners_s).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let tags: Vec<String> = serde_json::from_str(&tags_s).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let abs_path_s: String = r.get(1)?;
    Ok(ArtifactRow {
        id: r.get(0)?,
        abs_path: std::path::PathBuf::from(abs_path_s),
        kind: r.get(2)?,
        status: r.get(3)?,
        title: r.get(4)?,
        owners,
        tags,
        topic: r.get(7)?,
        time_scope: r.get(8)?,
        source: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
        file_mtime: r.get(12)?,
        file_sha256: r.get(13)?,
        confidence: r.get(14)?,
    })
}

/// Hydrate a frontmatter map from an `ArtifactRow`. Used as the seed for
/// `state_at::replay_state_at` (which then layers `field_patch` /
/// `status_change` events on top) and anywhere else that needs an
/// initial frontmatter view derived from catalog state.
///
/// Centralised here so the field list cannot drift between consumers.
pub fn build_frontmatter_map(art: &ArtifactRow) -> serde_json::Map<String, serde_json::Value> {
    use serde_json::Value;
    let mut m = serde_json::Map::new();
    m.insert("status".into(), Value::String(art.status.clone()));
    if let Some(ref t) = art.title {
        m.insert("title".into(), Value::String(t.clone()));
    }
    m.insert("kind".into(), Value::String(art.kind.clone()));
    m.insert(
        "tags".into(),
        serde_json::to_value(&art.tags).unwrap_or(Value::Null),
    );
    m.insert(
        "owners".into(),
        serde_json::to_value(&art.owners).unwrap_or(Value::Null),
    );
    if let Some(ref t) = art.topic {
        m.insert("topic".into(), Value::String(t.clone()));
    }
    if let Some(ref t) = art.time_scope {
        m.insert("time_scope".into(), Value::String(t.clone()));
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str) -> ArtifactRow {
        TestArtifactRowBuilder::new(id)
            .with_title("T")
            .with_owners(vec!["marius".into()])
            .with_tags(vec!["a".into(), "b".into()])
            .with_source("repo")
            .with_created_at(1)
            .with_updated_at(2)
            .with_file_mtime(3)
            .with_file_sha256("abc")
            .build()
    }

    #[test]
    fn ensure_slug_mints_dedups_and_is_idempotent() {
        let cat = Catalog::open_in_memory().unwrap();
        upsert(
            &cat,
            &TestArtifactRowBuilder::new("a")
                .with_title("My Tracker")
                .build(),
        )
        .unwrap();
        upsert(
            &cat,
            &TestArtifactRowBuilder::new("b")
                .with_title("My Tracker")
                .build(),
        )
        .unwrap();

        let s1 = ensure_slug(&cat.conn, "a").unwrap();
        assert_eq!(s1, "my-tracker");
        // Idempotent: second call returns the same slug, does not re-mint.
        assert_eq!(ensure_slug(&cat.conn, "a").unwrap(), "my-tracker");
        // Collision on the same base gets a numeric suffix.
        assert_eq!(ensure_slug(&cat.conn, "b").unwrap(), "my-tracker-2");
    }

    #[test]
    fn ensure_slug_never_mints_a_slug_shaped_like_an_artifact_id() {
        // `entry_cite.dst_ref` holds EITHER a 16-hex artifact id OR a `<slug>:<local>`
        // pair, and `gc::apply_rehome` discriminates them by that shape alone. A title
        // of sixteen hex characters slugifies to a counterfeit id, and the damage is a
        // silently rewritten citation rather than an error.
        let cat = Catalog::open_in_memory().unwrap();
        upsert(
            &cat,
            &TestArtifactRowBuilder::new("a")
                .with_title("deadbeefcafe1234")
                .build(),
        )
        .unwrap();
        let s = ensure_slug(&cat.conn, "a").unwrap();
        assert_ne!(
            s, "deadbeefcafe1234",
            "a 16-hex slug is indistinguishable from an artifact id in entry_cite.dst_ref"
        );
        assert_eq!(
            s, "deadbeefcafe1234-2",
            "suffix rather than reject: a caller asking for a slug must still get one"
        );

        // Seventeen hex chars is NOT id-shaped, so it is left alone — the guard keys on
        // the exact shape, not on "looks hexish", which would refuse legitimate slugs.
        upsert(
            &cat,
            &TestArtifactRowBuilder::new("b")
                .with_title("deadbeefcafe12345")
                .build(),
        )
        .unwrap();
        assert_eq!(ensure_slug(&cat.conn, "b").unwrap(), "deadbeefcafe12345");

        // And a 16-char slug containing a non-hex letter is fine.
        upsert(
            &cat,
            &TestArtifactRowBuilder::new("c")
                .with_title("zzzzbeefcafe1234")
                .build(),
        )
        .unwrap();
        assert_eq!(ensure_slug(&cat.conn, "c").unwrap(), "zzzzbeefcafe1234");
    }

    #[test]
    fn truncate_slug_base_cuts_on_a_dash_but_not_into_a_stub() {
        // Under cap: untouched.
        assert_eq!(truncate_slug_base("short-one", 50), "short-one");

        // Over cap, cut landing EXACTLY on a separator: the first `cap` bytes are
        // already whole words, so nothing more is trimmed. (Getting this wrong costs a
        // word on every base whose length happens to align with the cap.)
        let long = "configurable-anthropic-upstream-fail-open-headroom-trial-permanent";
        let t = truncate_slug_base(long, 50);
        assert_eq!(t, "configurable-anthropic-upstream-fail-open-headroom");
        assert_eq!(t.len(), 50);
        assert!(!t.ends_with('-'));

        // Over cap, cut landing MID-word: trim back to the last dash.
        let t = truncate_slug_base(long, 55);
        assert_eq!(t, "configurable-anthropic-upstream-fail-open-headroom");
        assert!(t.len() <= 55 && !t.ends_with('-'));

        // No dash at all inside the budget: nothing to cut back to, so hard-cut.
        let one_word = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bb";
        assert_eq!(truncate_slug_base(one_word, 10), "aaaaaaaaaa");

        // A dash exists but sits in the first half of the budget. Cutting back to it
        // would leave a 2-char stub that collides with every other base starting "ab",
        // so the hard cut wins. This is the case the `i >= cap / 2` guard exists for —
        // the no-dash case above reaches the same result through a different branch and
        // does NOT exercise it.
        let early_dash = "ab-cccccccccccccccccccccccc";
        assert_eq!(
            truncate_slug_base(early_dash, 10),
            "ab-ccccccc",
            "an early dash must not collapse the base to a stub"
        );
    }

    #[test]
    fn ensure_slug_caps_the_base_and_dedups_the_collisions_capping_creates() {
        // Two distinct long titles that agree on their first 50 characters. Uncapped
        // they are distinct slugs; capped they collide, and the existing -2 machinery
        // has to absorb it. Measured on the live catalog: capping at 50 turns 115
        // already-suffixed rows into 173, and adds no collision depth.
        let cat = Catalog::open_in_memory().unwrap();
        upsert(
            &cat,
            &TestArtifactRowBuilder::new("a")
                .with_title(
                    "Configurable Anthropic Upstream Fail Open Headroom Trial Permanent Gateway",
                )
                .build(),
        )
        .unwrap();
        upsert(
            &cat,
            &TestArtifactRowBuilder::new("b")
                .with_title(
                    "Configurable Anthropic Upstream Fail Open Headroom Trial Temporary Shim",
                )
                .build(),
        )
        .unwrap();

        let a = ensure_slug(&cat.conn, "a").unwrap();
        let b = ensure_slug(&cat.conn, "b").unwrap();
        assert_eq!(a, "configurable-anthropic-upstream-fail-open-headroom");
        assert!(
            a.len() <= SLUG_BASE_MAX,
            "base must be capped: {a} is {} chars",
            a.len()
        );
        assert_eq!(
            b, "configurable-anthropic-upstream-fail-open-headroom-2",
            "capping creates a collision the dedup suffix must absorb"
        );
    }

    #[test]
    fn ensure_slug_leaves_a_short_title_completely_alone() {
        // The cap must trim the tail only — the measured median is 44 chars and must
        // pass through untouched, or the change is a rename of the whole corpus.
        let cat = Catalog::open_in_memory().unwrap();
        upsert(
            &cat,
            &TestArtifactRowBuilder::new("a")
                .with_title("Tool Usage Patterns")
                .build(),
        )
        .unwrap();
        assert_eq!(ensure_slug(&cat.conn, "a").unwrap(), "tool-usage-patterns");
    }

    /// Seed n artifacts sharing one title, in an order that is NOT abs_path order,
    /// so a test asserting determinism is actually testing the ORDER BY rather than
    /// insertion order happening to agree with it.
    fn seed_titled(cat: &Catalog, rows: &[(&str, &str, &str)]) {
        for (id, path, title) in rows {
            let row = TestArtifactRowBuilder::new(id)
                .with_title(*title)
                .with_abs_path(*path)
                .build();
            upsert(cat, &row).unwrap();
        }
    }

    #[test]
    fn mint_missing_slugs_assigns_by_abs_path_order_not_insertion_order() {
        // Dedup is first-come-first-served and a slug is immutable once set, so the
        // iteration order permanently decides who gets the un-suffixed name. Insert
        // deliberately backwards: if the mint followed insertion (or rowid) order,
        // /z.md would take `shared` and this would fail.
        let cat = Catalog::open_in_memory().unwrap();
        seed_titled(
            &cat,
            &[
                ("z", "/repo/z.md", "Shared"),
                ("m", "/repo/m.md", "Shared"),
                ("a", "/repo/a.md", "Shared"),
            ],
        );

        let minted = mint_missing_slugs(&cat.conn, true).unwrap();
        let by_id: std::collections::BTreeMap<&str, &str> = minted
            .iter()
            .map(|m| (m.id.as_str(), m.slug.as_str()))
            .collect();
        assert_eq!(by_id["a"], "shared", "/repo/a.md sorts first");
        assert_eq!(by_id["m"], "shared-2");
        assert_eq!(by_id["z"], "shared-3");
    }

    #[test]
    fn mint_missing_slugs_dry_run_writes_nothing_but_reports_what_apply_would_do() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_titled(
            &cat,
            &[("a", "/repo/a.md", "Shared"), ("b", "/repo/b.md", "Shared")],
        );

        let preview = mint_missing_slugs(&cat.conn, false).unwrap();
        assert_eq!(preview.len(), 2);
        let (with, without) = slug_coverage(&cat.conn).unwrap();
        assert_eq!(
            (with, without),
            (0, 2),
            "confirm=false must leave every slug NULL"
        );

        // The preview is the real mint, rolled back — so applying must reproduce it
        // exactly, not merely agree in count.
        let applied = mint_missing_slugs(&cat.conn, true).unwrap();
        assert_eq!(
            applied, preview,
            "a dry run that can disagree with the apply is a second implementation"
        );
        assert_eq!(slug_coverage(&cat.conn).unwrap(), (2, 0));
    }

    #[test]
    fn mint_missing_slugs_is_idempotent_and_never_re_mints() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_titled(
            &cat,
            &[("a", "/repo/a.md", "One"), ("b", "/repo/b.md", "Two")],
        );

        let first = mint_missing_slugs(&cat.conn, true).unwrap();
        assert_eq!(first.len(), 2);
        let second = mint_missing_slugs(&cat.conn, true).unwrap();
        assert!(
            second.is_empty(),
            "a second pass must mint nothing: slugs are immutable once non-null, and a \
             re-mint would silently re-key entry_cite.src_slug"
        );
        assert_eq!(slug_coverage(&cat.conn).unwrap(), (2, 0));
    }

    #[test]
    fn mint_missing_slugs_falls_back_to_the_file_stem_for_untitled_rows() {
        // 105 of 4106 live rows have no title — auto-indexed READMEs with no
        // frontmatter. They must still get a usable slug, not "tracker" collisions.
        let cat = Catalog::open_in_memory().unwrap();
        for (id, path) in [("a", "/repo/docs/alpha.md"), ("b", "/repo/docs/beta.md")] {
            let mut row = TestArtifactRowBuilder::new(id).with_abs_path(path).build();
            // A genuinely untitled row — NULL in the catalog, as an auto-indexed
            // README with no frontmatter produces.
            row.title = None;
            upsert(&cat, &row).unwrap();
        }
        let minted = mint_missing_slugs(&cat.conn, true).unwrap();
        let slugs: Vec<&str> = minted.iter().map(|m| m.slug.as_str()).collect();
        assert_eq!(slugs, vec!["alpha", "beta"]);
    }

    #[test]
    fn mint_missing_slugs_leaves_an_already_minted_slug_alone() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_titled(
            &cat,
            &[("a", "/repo/a.md", "Shared"), ("b", "/repo/b.md", "Shared")],
        );
        // `b` is minted first, out of abs_path order, exactly as the lazy
        // append_entry path would have done at some arbitrary earlier time.
        assert_eq!(ensure_slug(&cat.conn, "b").unwrap(), "shared");

        let minted = mint_missing_slugs(&cat.conn, true).unwrap();
        assert_eq!(minted.len(), 1, "only the NULL row is touched");
        assert_eq!(minted[0].id, "a");
        assert_eq!(
            minted[0].slug, "shared-2",
            "the backfill yields to a slug that already exists rather than re-keying it"
        );
    }

    #[test]
    fn slugify_normalizes() {
        assert_eq!(slugify("Fable Tuning — Findings!"), "fable-tuning-findings");
        assert_eq!(slugify("  A/B  test "), "a-b-test");
    }

    #[test]
    fn upsert_and_get_roundtrip() {
        let cat = Catalog::open_in_memory().unwrap();
        let row = sample("id1");
        upsert(&cat, &row).unwrap();
        let fetched = get(&cat, "id1").unwrap().unwrap();
        assert_eq!(fetched, row);
    }

    #[test]
    fn upsert_updates_on_conflict() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut row = sample("id1");
        upsert(&cat, &row).unwrap();
        row.status = "archived".into();
        row.updated_at = 99;
        upsert(&cat, &row).unwrap();
        let fetched = get(&cat, "id1").unwrap().unwrap();
        assert_eq!(fetched.status, "archived");
        assert_eq!(fetched.updated_at, 99);
    }

    #[test]
    fn upsert_and_mint_slug_mints_where_bare_upsert_does_not() {
        let cat = Catalog::open_in_memory().unwrap();
        let row = TestArtifactRowBuilder::new("id1")
            .with_title("My Tracker")
            .build();
        upsert(&cat, &row).unwrap();
        let bare: Option<String> = cat
            .conn
            .query_row("SELECT slug FROM artifact WHERE id='id1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(bare, None, "bare upsert must not mint a slug");

        upsert_and_mint_slug(&cat, &row).unwrap();
        let minted: Option<String> = cat
            .conn
            .query_row("SELECT slug FROM artifact WHERE id='id1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(minted.as_deref(), Some("my-tracker"));
    }

    #[test]
    fn delete_removes_row() {
        let cat = Catalog::open_in_memory().unwrap();
        upsert(&cat, &sample("id1")).unwrap();
        assert!(delete(&cat, "id1").unwrap());
        assert!(get(&cat, "id1").unwrap().is_none());
    }

    #[test]
    fn delete_orphan_repos_drops_inactive() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut a = sample("a1");
        a.abs_path = std::path::PathBuf::from("/roots/alive/a.md");
        let mut b = sample("b1");
        b.abs_path = std::path::PathBuf::from("/roots/alive/b.md");
        let mut c = sample("c1");
        c.abs_path = std::path::PathBuf::from("/roots/ghost/c.md");
        // A row belonging to ANOTHER workspace tree, outside the prune scope.
        let mut d = sample("d1");
        d.abs_path = std::path::PathBuf::from("/other-workspace/d.md");
        for r in [&a, &b, &c, &d] {
            upsert(&cat, r).unwrap();
        }
        let alive = std::path::Path::new("/roots/alive");
        let scope = std::path::Path::new("/roots");
        // Prune within /roots, keeping only /roots/alive: ghost (c) is removed.
        let removed = delete_orphan_repos(&cat, &[alive], &[scope]).unwrap();
        assert_eq!(removed, 1);
        assert!(
            get(&cat, "c1").unwrap().is_none(),
            "ghost is under scope but not active → removed"
        );
        assert!(get(&cat, "a1").unwrap().is_some(), "alive kept");
        assert!(
            get(&cat, "d1").unwrap().is_some(),
            "row outside the scope root is NEVER touched (cross-workspace safety)"
        );
    }

    #[test]
    fn delete_orphan_repos_empty_active_is_noop() {
        // Empty active_roots must NOT wipe the catalog (the 3ea49090 foot-gun:
        // this used to run `DELETE FROM artifact`).
        let cat = Catalog::open_in_memory().unwrap();
        upsert(&cat, &sample("x")).unwrap();
        let scope = std::path::Path::new("/roots");
        let n = delete_orphan_repos(&cat, &[], &[scope]).unwrap();
        assert_eq!(n, 0, "empty active is a no-op, never DELETE FROM artifact");
        assert!(get(&cat, "x").unwrap().is_some());
    }

    #[test]
    fn delete_orphan_repos_empty_scope_is_noop() {
        // Empty scope_roots means no bounded territory → prune nothing.
        let cat = Catalog::open_in_memory().unwrap();
        let mut a = sample("a1");
        a.abs_path = std::path::PathBuf::from("/roots/ghost/a.md");
        upsert(&cat, &a).unwrap();
        let alive = std::path::Path::new("/roots/alive");
        let n = delete_orphan_repos(&cat, &[alive], &[]).unwrap();
        assert_eq!(n, 0, "empty scope is a no-op");
        assert!(get(&cat, "a1").unwrap().is_some());
    }

    #[test]
    fn get_surfaces_malformed_tags_json() {
        let cat = Catalog::open_in_memory().unwrap();
        // Insert a row bypassing upsert, with malformed tags JSON.
        cat.conn
            .execute(
                "INSERT INTO artifact (id, abs_path, kind, status, owners, tags,
                 created_at, updated_at, file_mtime, file_sha256, confidence)
                 VALUES ('bad', '/test/x.md', 'spec', 'active', '[]',
                         '{not valid json',
                         0, 0, 0, 'sha', 1.0)",
                [],
            )
            .unwrap();
        let err = get(&cat, "bad").unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("conversion")
                || err.to_string().contains("json")
        );
    }
}
