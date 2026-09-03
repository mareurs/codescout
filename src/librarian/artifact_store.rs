//! Backend-agnostic artifact vector storage with a configurable backend:
//! **Qdrant** (default) or **sqlite-vec** (the daemon-free escape hatch for
//! low-end / locked-down machines that can't run a Qdrant daemon — e.g. the
//! `vdi-windows` worktree).
//!
//! Both backends return identical, **project-scoped** results: the
//! authoritative scope is the catalog's filter AST applied at hydration (see
//! [`crate::librarian::catalog::find::find_by_ids_filtered`]). The Qdrant
//! backend additionally pre-filters its KNN by `project_id` for efficiency; the
//! sqlite-vec KNN is unscoped (the catalog filter narrows it). Selection is via
//! [`ArtifactBackend::resolve`] — env `CODESCOUT_ARTIFACT_BACKEND`, then
//! `[librarian] vector_backend` in `project.toml`, else the default (Qdrant).

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::librarian::catalog::Catalog;
#[cfg(feature = "server-stack")]
use crate::retrieval::qdrant::QdrantWrap;

/// Which vector backend the librarian artifact index uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactBackend {
    /// Default — a shared Qdrant `artifacts` collection. Needs a reachable
    /// Qdrant daemon.
    Qdrant,
    /// Escape hatch — the in-process sqlite-vec `artifact_vec` table. No
    /// daemon; works fully offline on low-end / locked-down machines.
    SqliteVec,
}

impl ArtifactBackend {
    /// Resolve the backend. Layered, highest-priority first:
    /// 1. `CODESCOUT_ARTIFACT_BACKEND=qdrant|sqlite-vec` env var.
    /// 2. `[librarian] vector_backend = "qdrant"|"sqlite-vec"` in
    ///    `<project>/.codescout/project.toml`.
    /// 3. Default: Qdrant on the server build; the daemon-free sqlite-vec store
    ///    on a lean build (no `server-stack` feature — Qdrant isn't compiled in).
    ///
    /// Mirrors `crate::server::librarian_enabled_at_runtime`.
    pub fn resolve(project_path: Option<&str>) -> Self {
        if let Ok(v) = std::env::var("CODESCOUT_ARTIFACT_BACKEND") {
            if let Some(b) = Self::parse(&v) {
                return b;
            }
        }
        if let Some(root) = project_path {
            let cfg = std::path::Path::new(root)
                .join(".codescout")
                .join("project.toml");
            if let Ok(text) = std::fs::read_to_string(&cfg) {
                if let Ok(parsed) = toml::from_str::<toml::Value>(&text) {
                    if let Some(v) = parsed
                        .get("librarian")
                        .and_then(|t| t.get("vector_backend"))
                        .and_then(|v| v.as_str())
                    {
                        if let Some(b) = Self::parse(v) {
                            return b;
                        }
                    }
                }
            }
        }
        #[cfg(feature = "server-stack")]
        {
            ArtifactBackend::Qdrant
        }
        #[cfg(not(feature = "server-stack"))]
        {
            ArtifactBackend::SqliteVec
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "qdrant" => Some(Self::Qdrant),
            "sqlite-vec" | "sqlite_vec" | "sqlite" | "local" => Some(Self::SqliteVec),
            _ => None,
        }
    }
}

/// Qdrant collection name for one project's artifact chunk vectors.
///
/// `prefix` already carries `RetrievalConfig`'s benchmark-isolation prefix — the
/// callers build it as `config.collection("artifact_chunks_")`, so a
/// `CODESCOUT_QDRANT_COLLECTION_PREFIX` still isolates a bench run.
///
/// **Basename AND hash, not either alone.** The basename is what makes the name
/// readable in the Qdrant UI, and it is not unique: a worktree, a clone, or
/// `work/foo` beside `archive/foo` all share one. Two projects silently sharing
/// a collection is the failure this exists to prevent, and it would look exactly
/// like working software. The hash is [`artifact_id_from_abs`] — the same
/// derivation artifact ids use, reused rather than re-invented so there is one
/// path-hashing rule in this codebase and not two.
///
/// Non-alphanumeric bytes in the basename become `_`: Qdrant collection names
/// admit no `/`, `.` or spaces, and a directory name may hold all three.
pub fn artifact_collection_name(prefix: &str, project_root: &str) -> String {
    let path = std::path::Path::new(project_root);
    let base = path
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown");
    let sanitized: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let hash = crate::librarian::ids::artifact_id_from_abs(path);
    format!("{prefix}{sanitized}_{hash}")
}

/// Backend-agnostic artifact vector store. Implementations:
/// - [`QdrantArtifactStore`] — default.
/// - [`SqliteVecArtifactStore`] — the daemon-free escape hatch.
///
/// **Grain, stated once because the two ids are easy to swap and nothing
/// downstream would notice.** Vectors are CHUNK-grain: one per `artifact_chunk`
/// row. Everything the catalog then does with a result — filtering, project
/// scope, hydration — is ARTIFACT-grain. So `upsert` takes both ids, `knn`
/// returns the chunk id, and `delete` takes the artifact id and removes every
/// chunk vector under it.
///
/// Until 2026-09-03 `upsert` had a single `id` slot holding, in practice, a
/// chunk id — while `delete`'s held an artifact id, and these docs called both
/// "an artifact's embedding". sqlite-vec tolerated the ambiguity because it can
/// recover the artifact id by joining `artifact_chunk`; Qdrant has no join and
/// silently spent its one id twice. See
/// `docs/issues/2026-09-03-editing-an-artifact-removes-it-from-qdrant-backed-semantic-search.md`.
#[async_trait]
pub trait ArtifactVectorStore: Send + Sync {
    /// Upsert one CHUNK's embedding. `chunk_id` is the vector's identity;
    /// `artifact_id` is the catalog key it belongs to and is what `delete`
    /// later matches on. Idempotent on `chunk_id` — a second call with the same
    /// one overwrites the vector in place.
    async fn upsert(
        &self,
        project_id: &str,
        chunk_id: &str,
        artifact_id: &str,
        vector: &[f32],
    ) -> Result<()>;

    /// Delete EVERY chunk vector belonging to `artifact_id`. Idempotent — an
    /// artifact with no vectors is a no-op.
    ///
    /// Artifact-grain on purpose: an artifact owns N chunk vectors, so a
    /// chunk-grain delete would leave N−1 orphans that no sweep collects.
    async fn delete(&self, artifact_id: &str) -> Result<()>;

    /// Dense KNN → ranked `(chunk_id, distance)` pairs, closest first.
    ///
    /// **The returned id is a CHUNK id**, which the caller resolves to its
    /// artifact through `artifact_chunk` — see `semantic_find`. (This doc
    /// promised `artifact_id` until 2026-09-03, a claim the sqlite backend had
    /// already stopped honouring.)
    ///
    /// `project_id = Some` narrows to one project (single-project scope); `None`
    /// searches all (the catalog's scoped filter narrows after hydration either
    /// way).
    ///
    /// **`distance` is LOWER-IS-CLOSER on every backend, by contract.** The two
    /// implementations disagree natively — Qdrant's cosine `score` is a similarity
    /// (higher is better), sqlite-vec's `distance` column is L2 (lower is better)
    /// — so each converts to this polarity before returning. Returning the raw
    /// value would make the number's meaning depend on which backend answered,
    /// with Qdrant's default status hiding it from almost everyone.
    ///
    /// The SCALE remains backend-defined (cosine distance vs L2). Values are
    /// comparable **within one response**, which is what ranking and
    /// starvation-detection need; they are NOT comparable across backends, so no
    /// caller may threshold on an absolute number.
    async fn knn(
        &self,
        project_id: Option<&str>,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(String, f32)>>;
}

// ---------------------------------------------------------------------------
// Qdrant backend (default)
// ---------------------------------------------------------------------------

#[cfg(feature = "server-stack")]
pub struct QdrantArtifactStore {
    qdrant: QdrantWrap,
    /// `{config_prefix}artifact_chunks_` — every artifact collection starts with
    /// this, which is also how the cross-project fan-out enumerates them.
    prefix: String,
    /// Collections already bootstrapped in this process. Was a `OnceCell<()>`
    /// when there was exactly one collection; a set, now that there is one per
    /// project and a single reindex can touch a sub-project's.
    ensured: tokio::sync::Mutex<std::collections::HashSet<String>>,
}

#[cfg(feature = "server-stack")]
impl QdrantArtifactStore {
    /// Construct over a connected Qdrant. `prefix` is
    /// `config.collection("artifact_chunks_")`.
    ///
    /// **The store deliberately does NOT know the active project.** It used to, purely
    /// to serve as the fallback collection when a caller passed an empty `project_id` —
    /// and that fallback silently filed every non-active target's vectors under the
    /// active project
    /// (`docs/issues/2026-09-03-per-project-vector-collections-follow-the-server-cwd-not-the-workspace-param.md`).
    ///
    /// Removing the field does more than delete dead code. With no notion of an active
    /// project, defaulting to one becomes *unrepresentable* rather than merely unused,
    /// so the defect cannot come back by someone restoring a fallback branch — the
    /// same shape as `read_only.unwrap_or(!is_home)`, where the fix was a form in which
    /// the wrong arm could not be written.
    ///
    /// Every collection name is therefore derived from the `project_id` the caller
    /// passes. Collections are bootstrapped lazily on first write (dim taken from the
    /// first vector), so a remote embedder whose dimension is only known after the
    /// first embed still works.
    pub fn new(qdrant: QdrantWrap, prefix: impl Into<String>) -> Self {
        Self {
            qdrant,
            prefix: prefix.into(),
            ensured: tokio::sync::Mutex::new(std::collections::HashSet::new()),
        }
    }

    /// Which collection a write or a project-scoped read for `project_id` belongs in.
    ///
    /// **An empty `project_id` is a BUG and is refused.** It used to fall back to the
    /// active project's collection, and that fallback is what let
    /// `docs/issues/2026-09-03-per-project-vector-collections-follow-the-server-cwd-not-the-workspace-param.md`
    /// ship: `reindex` computed the id by looking its target up in the deprecated
    /// `[[roots]]` registry, missed for every project under the `[[project]]` model,
    /// and passed `""` — so vectors for any target that was not the active project were
    /// filed under the active one and the call reported success.
    ///
    /// The earlier note here cited "4395 of the 5388 live points carry an empty
    /// `project_id`" as evidence the empty case was real and had to be tolerated. That
    /// count was taken by scrolling the pre-rewrite `artifacts` collection, where the
    /// empty id was the DEFECT being characterised — not a supported input. Restated
    /// here at its author's request, because the original sentence reads as
    /// "legitimate and common" and is the reason the fallback looked deliberate.
    ///
    /// Refusing is safe because the caller set is bounded and known: `upsert` has
    /// exactly one production caller (`tools::reindex`), which now derives the id from
    /// the target it is indexing, and project-scoped `knn` passes
    /// `current_project.abs_path`. Neither can be empty. A future caller that forgets
    /// gets an error naming the problem instead of a silently misfiled vector — which
    /// is the whole difference this class turns on.
    fn collection_for(&self, project_id: &str) -> Result<String> {
        if project_id.is_empty() {
            anyhow::bail!(
                "artifact vector write/read with an EMPTY project_id. The collection is \
                 named from the project path, so there is no correct collection for \
                 this call and the previous fallback (the active project's) silently \
                 misfiled it. Derive the id from the project being indexed or read."
            );
        }
        Ok(artifact_collection_name(&self.prefix, project_id))
    }

    async fn ensure(&self, collection: &str, dim: u64) -> Result<()> {
        if self.ensured.lock().await.contains(collection) {
            return Ok(());
        }
        self.qdrant
            .ensure_artifacts_collection(collection, dim)
            .await?;
        self.ensured.lock().await.insert(collection.to_string());
        Ok(())
    }
}

#[cfg(feature = "server-stack")]
#[async_trait]
impl ArtifactVectorStore for QdrantArtifactStore {
    async fn upsert(
        &self,
        project_id: &str,
        chunk_id: &str,
        artifact_id: &str,
        vector: &[f32],
    ) -> Result<()> {
        if vector.is_empty() {
            anyhow::bail!("artifact embedding dim is 0 (embedder returned an empty vector)");
        }
        // The 16-hex grain guard that stood here until 2026-09-03 is GONE, and
        // its absence is the fix rather than a relaxation. It existed because
        // this backend could not represent a chunk id — one id slot, spent as
        // both the point id and the payload's `artifact_id`. Now that both ids
        // travel, the input it refused is exactly the input this path is for.
        let collection = self.collection_for(project_id)?;
        self.ensure(&collection, vector.len() as u64).await?;
        self.qdrant
            .artifact_upsert(
                &collection,
                project_id,
                chunk_id,
                artifact_id,
                vector.to_vec(),
            )
            .await
    }

    async fn delete(&self, artifact_id: &str) -> Result<()> {
        // Fans out over EVERY artifact collection, because the trait's `delete`
        // carries no project and an artifact id does not say which project it
        // belongs to. Scanning them all is affordable precisely because this has
        // no production caller today — it exists for an explicit vector purge —
        // and being wrong in the other direction would silently strand vectors.
        for collection in self.qdrant.artifact_collections(&self.prefix).await? {
            // Delete by payload FILTER, not by derived point id: an artifact
            // owns N chunk points now, and the old form would remove at most one
            // and leave the rest as orphans no sweep collects.
            self.qdrant
                .artifact_delete(&collection, artifact_id)
                .await?;
        }
        Ok(())
    }

    /// `project_id = Some` reads the active project's collection; `None` fans
    /// out across every artifact collection and merges.
    ///
    /// **`None` is not "no filter" here, it is "all projects", and that
    /// distinction is what per-project collections would otherwise have eaten.**
    /// `Find::call` passes `Some` only for `Scope::Project` and `None` for
    /// `repo` / `umbrella` / `all` — the scopes CLAUDE.md documents as reaching
    /// across repos. With one collection per project, honouring `None` by
    /// dropping a payload filter would have silently narrowed those scopes to
    /// the active project, returning a plausible short page and no error.
    ///
    /// Merging is sound because every artifact collection is created by
    /// `ensure_artifacts_collection` with the same `Distance::Cosine`, so the
    /// distances compare directly. A future collection on another metric would
    /// break that silently — which is why the metric lives in one place.
    async fn knn(
        &self,
        project_id: Option<&str>,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(String, f32)>> {
        let collections: Vec<String> = match project_id {
            Some(pid) => vec![self.collection_for(pid)?],
            None => self.qdrant.artifact_collections(&self.prefix).await?,
        };

        let mut merged: Vec<(String, f32)> = Vec::new();
        for collection in collections {
            if !self.qdrant.collection_exists(&collection).await? {
                continue;
            }
            // `artifact_knn_scored` performs the similarity→distance flip; see
            // its doc comment for why that conversion lives there and not here.
            // Each collection is asked for the full `k`: a project holding all
            // the best hits must be able to supply all of them.
            merged.extend(
                self.qdrant
                    .artifact_knn_scored(&collection, None, query.to_vec(), k)
                    .await?,
            );
        }
        merged.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        merged.truncate(k);
        Ok(merged)
    }
}

// ---------------------------------------------------------------------------
// sqlite-vec backend (escape hatch)
// ---------------------------------------------------------------------------

pub struct SqliteVecArtifactStore {
    catalog: Arc<parking_lot::Mutex<Catalog>>,
}

impl SqliteVecArtifactStore {
    pub fn new(catalog: Arc<parking_lot::Mutex<Catalog>>) -> Self {
        Self { catalog }
    }
}

#[async_trait]
impl ArtifactVectorStore for SqliteVecArtifactStore {
    async fn upsert(
        &self,
        _project_id: &str,
        chunk_id: &str,
        // ACCEPTED AND DELIBERATELY UNUSED — do not "clean this up" by dropping
        // it from the trait. `artifact_vec_v2` is keyed by `chunk_id`, and this
        // backend recovers the artifact id whenever it needs one by joining
        // `artifact_chunk` (see `delete` below, and `semantic_find`). Qdrant has
        // no join, so the trait must carry both; that this impl can ignore one
        // is a property of sqlite, not a redundancy in the signature.
        _artifact_id: &str,
        vector: &[f32],
    ) -> Result<()> {
        // Delegate to the catalog's batch writer — reuses its dimension
        // validation and the BUG-045 DELETE-then-INSERT idempotency contract
        // verbatim (so the sqlite-vec backend behaves exactly as before).
        let cat = self.catalog.lock();
        // Chunk-grain: the embed queue is keyed by `chunk_id` (see
        // `indexer::embed_queue_items`), so vectors belong in the chunk-keyed
        // `artifact_vec_v2`, never the artifact-keyed v1 table.
        crate::librarian::indexer::write_embeddings_v2(
            &cat,
            &[(chunk_id.to_string(), vector.to_vec())],
        )
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let cat = self.catalog.lock();
        // `id` here is an ARTIFACT id — the trait's delete is artifact-grain —
        // so the chunk vectors must be reached via `artifact_chunk` rather than
        // deleted by id. The `artifact_vec_v2_cascade_delete` trigger is NOT
        // available on this path: it fires on `artifact_chunk` deletion, and no
        // artifact row is removed here. Missing this reds nothing; the cost is
        // an accumulating orphan set that no sweep collects.
        crate::librarian::indexer::delete_chunk_vectors(&cat, id)?;
        cat.conn.execute(
            "DELETE FROM artifact_vec WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    async fn knn(
        &self,
        _project_id: Option<&str>,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(String, f32)>> {
        // sqlite-vec has no project_id column; the catalog's scoped filter does
        // the project narrowing after hydration (results match the Qdrant path).
        //
        // `artifact_vec` is declared `vec0(id, embedding FLOAT[768])` in schema.sql
        // with no distance metric, so sqlite-vec's default L2 applies and the
        // `distance` column is already lower-is-closer — the polarity the trait
        // requires. No conversion here, unlike the Qdrant path.
        let blob: Vec<u8> = query.iter().flat_map(|f| f.to_le_bytes()).collect();
        let cat = self.catalog.lock();
        let mut stmt = cat.conn.prepare(
            "SELECT id, distance FROM artifact_vec_v2 WHERE embedding MATCH vec_f32(?1) ORDER BY distance LIMIT ?2",
        )?;
        let hits = stmt
            .query_map(rusqlite::params![blob, k as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)? as f32))
            })?
            .collect::<rusqlite::Result<Vec<(String, f32)>>>()?;
        Ok(hits)
    }
}

#[cfg(test)]
pub mod test_support {
    //! In-memory artifact store for trait-level + coordinator tests.
    use super::*;
    use std::collections::HashMap;

    /// `(project_id, artifact_id, vector)`, keyed by chunk id.
    ///
    /// `artifact_id` is not bookkeeping: `delete` is artifact-grain by contract,
    /// so without it this fixture could only ever remove one point and would
    /// silently pass a chunk-grain delete that leaves orphans — the exact defect
    /// the Qdrant filtered delete exists to avoid.
    type StoredPoint = (String, String, Vec<f32>);

    #[derive(Default)]
    pub struct InMemoryArtifactStore {
        points: parking_lot::Mutex<HashMap<String, StoredPoint>>,
    }

    impl InMemoryArtifactStore {
        /// Every distinct `project_id` this store was written with, sorted.
        ///
        /// Exists so a coordinator test can assert WHICH project a write was filed
        /// under, not merely that a write happened. That distinction is the whole of
        /// `docs/issues/2026-09-03-per-project-vector-collections-follow-the-server-cwd-not-the-workspace-param.md`:
        /// the misfiling reindex reported `embedded: N` and `embed_error_count: 0`, so
        /// every count-shaped assertion passed while the vectors went to another
        /// project's collection. A test reading only counters cannot fail on it.
        pub fn project_ids(&self) -> Vec<String> {
            let mut ids: Vec<String> = self
                .points
                .lock()
                .values()
                .map(|(pid, _, _)| pid.clone())
                .collect();
            ids.sort();
            ids.dedup();
            ids
        }
    }

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let na = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let nb = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if na == 0.0 || nb == 0.0 {
            0.0
        } else {
            dot / (na * nb)
        }
    }

    #[async_trait]
    impl ArtifactVectorStore for InMemoryArtifactStore {
        async fn upsert(
            &self,
            project_id: &str,
            chunk_id: &str,
            artifact_id: &str,
            vector: &[f32],
        ) -> Result<()> {
            self.points.lock().insert(
                chunk_id.to_string(),
                (
                    project_id.to_string(),
                    artifact_id.to_string(),
                    vector.to_vec(),
                ),
            );
            Ok(())
        }

        async fn delete(&self, artifact_id: &str) -> Result<()> {
            // Artifact-grain: remove EVERY chunk point under this artifact, which
            // is what the trait promises and what the Qdrant filtered delete and
            // sqlite's `delete_chunk_vectors` both do. Removing by key would make
            // this fixture agree with a one-point delete and hide the orphan bug.
            self.points
                .lock()
                .retain(|_, (_, aid, _)| aid != artifact_id);
            Ok(())
        }

        async fn knn(
            &self,
            project_id: Option<&str>,
            query: &[f32],
            k: usize,
        ) -> Result<Vec<(String, f32)>> {
            let pts = self.points.lock();
            // Cosine SIMILARITY internally, converted to the trait's
            // lower-is-closer distance on the way out — the same flip the Qdrant
            // backend does, so a test fixture cannot accidentally encode the
            // opposite polarity from production.
            let mut scored: Vec<(String, f32)> = pts
                .iter()
                .filter(|(_, (pid, _, _))| project_id.is_none_or(|p| p == pid))
                .map(|(id, (_, _, v))| (id.clone(), 1.0 - cosine(query, v)))
                .collect();
            scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(k);
            Ok(scored)
        }
    }
}
#[cfg(test)]
mod backend_tests {
    use super::test_support::InMemoryArtifactStore;
    use super::*;

    #[test]
    fn parse_recognizes_known_backends() {
        assert_eq!(
            ArtifactBackend::parse("qdrant"),
            Some(ArtifactBackend::Qdrant)
        );
        assert_eq!(
            ArtifactBackend::parse("sqlite-vec"),
            Some(ArtifactBackend::SqliteVec)
        );
        assert_eq!(
            ArtifactBackend::parse("  SQLite  "),
            Some(ArtifactBackend::SqliteVec)
        );
        assert_eq!(
            ArtifactBackend::parse("local"),
            Some(ArtifactBackend::SqliteVec)
        );
        assert_eq!(ArtifactBackend::parse("nonsense"), None);
    }

    /// Two projects sharing a basename get DIFFERENT collections.
    ///
    /// This is the whole reason the name carries a hash as well as a basename.
    /// `work/foo` beside `archive/foo`, a clone, or a git worktree all share a
    /// directory name, and two projects silently sharing one collection would
    /// look exactly like working software: queries return, results are
    /// plausible, and one project's artifacts answer the other's questions.
    #[test]
    fn two_projects_sharing_a_basename_get_different_collections() {
        let a = artifact_collection_name("pfx_", "/home/u/work/foo");
        let b = artifact_collection_name("pfx_", "/home/u/archive/foo");
        assert_ne!(a, b, "same basename must not collapse to one collection");
        // Both still READ as `foo`, which is the point of keeping the basename:
        // the hash disambiguates without making the name opaque.
        assert!(a.starts_with("pfx_foo_"), "got {a}");
        assert!(b.starts_with("pfx_foo_"), "got {b}");
    }

    /// The name is stable across calls and legal as a Qdrant collection name.
    #[test]
    fn a_collection_name_is_deterministic_and_has_no_illegal_characters() {
        let root = "/home/u/my project.v2/repo-name";
        let first = artifact_collection_name("pfx_", root);
        assert_eq!(first, artifact_collection_name("pfx_", root), "not stable");
        assert!(
            first.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "Qdrant admits no '/', '.' or space in a collection name, and a \
             directory may hold all three: {first}"
        );
        // The prefix is what the cross-project fan-out enumerates on, so it has
        // to survive verbatim — sanitising it away would make `knn(None)` find
        // nothing while every single-project query kept working.
        assert!(
            first.starts_with("pfx_"),
            "prefix must be verbatim: {first}"
        );
    }

    #[tokio::test]
    async fn knn_filters_by_project_id() {
        let store = InMemoryArtifactStore::default();
        store.upsert("p1", "a", "art-a", &[1.0, 0.0]).await.unwrap();
        store.upsert("p2", "b", "art-b", &[1.0, 0.0]).await.unwrap();

        // Scoped to p1 → only "a".
        let scoped: Vec<String> = store
            .knn(Some("p1"), &[1.0, 0.0], 10)
            .await
            .unwrap()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(scoped, vec!["a".to_string()]);
        // Unscoped → both.
        let mut all: Vec<String> = store
            .knn(None, &[1.0, 0.0], 10)
            .await
            .unwrap()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        all.sort();
        assert_eq!(all, vec!["a".to_string(), "b".to_string()]);
    }

    /// The trait's polarity contract: `knn` returns a DISTANCE, lower-is-closer,
    /// on every backend. The in-memory store computes cosine similarity natively
    /// and must flip it, exactly as the Qdrant backend flips `ScoredPoint.score`.
    ///
    /// Without this, a fixture encoding the OPPOSITE polarity from production
    /// would let every downstream ranking test pass while the two real backends
    /// disagreed about which direction is good — and Qdrant's default status
    /// would keep that hidden from all but escape-hatch users.
    ///
    /// BUG docs/issues/archive/2026-08-27-semantic-find-fills-the-page-past-relevance-with-no-score.md
    #[tokio::test]
    async fn knn_returns_lower_is_closer_distance() {
        let store = InMemoryArtifactStore::default();
        store
            .upsert("p", "near", "art-near", &[1.0, 0.0])
            .await
            .unwrap();
        store
            .upsert("p", "far", "art-far", &[0.0, 1.0])
            .await
            .unwrap();

        let hits = store.knn(Some("p"), &[1.0, 0.0], 10).await.unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].0, "near", "closest must sort first");
        assert!(
            hits[0].1 < hits[1].1,
            "distance must ASCEND with dissimilarity, got {hits:?}"
        );
        assert!(
            hits[0].1.abs() < 1e-6,
            "an identical vector is distance ~0, got {}",
            hits[0].1
        );
    }

    #[tokio::test]
    async fn delete_is_idempotent() {
        let store = InMemoryArtifactStore::default();
        // `delete` is ARTIFACT-grain: the id it takes is deliberately NOT the id
        // `upsert` keyed the point on. Using one value for both here would let a
        // chunk-grain delete pass this test unchanged.
        store.upsert("p", "chunk-a", "art-a", &[1.0]).await.unwrap();
        store.delete("art-a").await.unwrap();
        store.delete("art-a").await.unwrap(); // already gone → no-op
        assert!(store.knn(None, &[1.0], 10).await.unwrap().is_empty());
    }

    /// `delete` removes EVERY chunk point under one artifact, not just one.
    ///
    /// The failure this exists for is silent: an artifact owns N points, so a
    /// delete that removes one leaves N-1 orphans that still answer queries and
    /// that no sweep collects. A single-chunk fixture cannot express it — one
    /// point behaves identically under "delete all" and "delete first" — so the
    /// THREE chunks below are load-bearing, and so is the fourth point under a
    /// DIFFERENT artifact, which is what stops a delete-everything from passing.
    #[tokio::test]
    async fn deleting_an_artifact_removes_every_one_of_its_chunk_points() {
        let store = InMemoryArtifactStore::default();
        for c in ["c1", "c2", "c3"] {
            store.upsert("p", c, "art-a", &[1.0, 0.0]).await.unwrap();
        }
        store.upsert("p", "c9", "art-b", &[1.0, 0.0]).await.unwrap();

        store.delete("art-a").await.unwrap();

        let left: Vec<String> = store
            .knn(None, &[1.0, 0.0], 10)
            .await
            .unwrap()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(
            left,
            vec!["c9".to_string()],
            "every chunk of art-a must go, and art-b's must stay"
        );
    }

    #[tokio::test]
    async fn the_sqlite_store_writes_a_chunk_id_into_v2_and_never_into_v1() {
        // LOAD-BEARING: this drives `SqliteVecArtifactStore::upsert` — the
        // PRODUCTION writer — and NOT `write_embeddings_v2` directly. A test
        // that calls the new function proves the function works and says
        // nothing about whether anything calls it, which is exactly how this
        // task would otherwise have shipped `write_embeddings_v2` as dead code.
        //
        // The ids MUST come from `embed_queue_items`, never a literal. Every
        // existing `semantic_find` test hand-feeds an ARTIFACT id, which is
        // precisely why none of them noticed when the queue was re-keyed to
        // chunk ids. Replace the queue with literals here and this test
        // silently rejoins that population.
        let cat = Catalog::open_in_memory().unwrap();
        crate::librarian::catalog::artifact::upsert(
            &cat,
            &crate::librarian::catalog::artifact::TestArtifactRowBuilder::new("a")
                .with_kind("tracker")
                .with_status("active")
                .build(),
        )
        .unwrap();
        let queue = crate::librarian::indexer::embed_queue_items(
            &cat,
            "a",
            Some("T".into()),
            "# T\n\n## W-1 — x\n\nalpha\n\n## W-2 — y\n\nbeta\n",
            crate::librarian::catalog::chunk::ChunkGrain::Chunk,
        )
        .unwrap();
        // Without >1 chunk the grain bug is UNREPRESENTABLE by this fixture: a
        // single-chunk artifact's chunk id and artifact id fail the same way,
        // so the test would pass under both the broken and the fixed writer.
        assert!(
            queue.len() > 1,
            "fixture must yield >1 chunk, got {}",
            queue.len()
        );

        let cat = std::sync::Arc::new(parking_lot::Mutex::new(cat));
        let store = SqliteVecArtifactStore::new(cat.clone());
        for item in &queue {
            store
                .upsert(
                    "proj",
                    &item.chunk_id,
                    &item.artifact_id,
                    &vec![0.5f32; 768],
                )
                .await
                .unwrap();
        }

        let guard = cat.lock();
        for item in &queue {
            let id = &item.chunk_id;
            let n: i64 = guard
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM artifact_vec_v2 WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "chunk {id} must reach artifact_vec_v2");
        }
        // BOTH halves are required. The first is monotone under a writer that
        // writes v2 AND v1 — which is exactly what a half-finished re-point
        // looks like, and is a state this task passes through.
        let v1: i64 = guard
            .conn
            .query_row("SELECT COUNT(*) FROM artifact_vec", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v1, 0, "no chunk id may reach the artifact-keyed v1 table");
    }
}
