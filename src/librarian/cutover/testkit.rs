//! Test fixtures: an old catalog filled through the same row writers the live code uses.

use std::path::Path;

use crate::librarian::catalog::artifact::{self, TestArtifactRowBuilder};
use crate::librarian::catalog::augmentation::{self, AugmentationRow};
use crate::librarian::catalog::event_edges::{self, EdgeRow};
use crate::librarian::catalog::events::{self, EventRow};
use crate::librarian::catalog::Catalog;
use crate::librarian::derived::diff::OldCatalog;

/// An in-memory catalog filled by `fill`.
pub(crate) fn catalog_with(fill: impl FnOnce(&Catalog)) -> Catalog {
    let cat = Catalog::open_in_memory().expect("an in-memory catalog opens");
    fill(&cat);
    cat
}

/// The filled catalog as an [`OldCatalog`] (the connection is moved into it).
pub(crate) fn old_catalog_with(fill: impl FnOnce(&Catalog)) -> OldCatalog {
    OldCatalog::from_connection(catalog_with(fill).conn).expect("the fixture is a readable catalog")
}

/// The filled catalog written to `dest`, a plain file with no side file (`VACUUM INTO`).
pub(crate) fn old_catalog_file(dest: &Path, fill: impl FnOnce(&Catalog)) {
    let cat = catalog_with(fill);
    crate::librarian::catalog::snapshot_catalog(&cat.conn, dest).expect("the snapshot is written");
}

/// A document row at `<root>/<rel>`, with a minted slug. Returns the slug.
pub(crate) fn add_doc(cat: &Catalog, root: &Path, id: &str, rel: &str) -> String {
    let row = TestArtifactRowBuilder::new(id)
        .with_abs_path(root.join(rel))
        .build();
    artifact::upsert_and_mint_slug(cat, &row).expect("the artifact is stored");
    artifact::ensure_slug(&cat.conn, id).expect("the slug is minted")
}

/// A document row at an arbitrary absolute path (a spelling the tests choose).
pub(crate) fn add_doc_at(cat: &Catalog, id: &str, abs_path: &str) {
    let row = TestArtifactRowBuilder::new(id)
        .with_abs_path(abs_path)
        .build();
    artifact::upsert(cat, &row).expect("the artifact is stored");
}

/// An event row of `kind` for `artifact_id`.
pub(crate) fn add_event(cat: &Catalog, id: &str, artifact_id: &str, kind: &str, payload: &str) {
    events::insert(
        cat,
        &EventRow {
            id: id.to_owned(),
            artifact_id: artifact_id.to_owned(),
            kind: kind.to_owned(),
            payload: payload.to_owned(),
            anchor_commit: None,
            head_commit: None,
            author: None,
            created_at: 1_000,
        },
    )
    .expect("the event is stored");
}

/// One edge from `src_event_id`.
pub(crate) fn add_edge(
    cat: &Catalog,
    src_event_id: &str,
    rel: &str,
    dst_event_id: Option<&str>,
    dst_artifact_id: Option<&str>,
    dst_source_id: Option<&str>,
) {
    event_edges::insert_many(
        cat,
        &[EdgeRow {
            src_event_id: src_event_id.to_owned(),
            dst_event_id: dst_event_id.map(str::to_owned),
            dst_artifact_id: dst_artifact_id.map(str::to_owned),
            dst_source_id: dst_source_id.map(str::to_owned),
            rel: rel.to_owned(),
        }],
    )
    .expect("the edge is stored");
}

/// An augmentation row with the given params JSON and entry collection.
pub(crate) fn add_augmentation(
    cat: &Catalog,
    artifact_id: &str,
    params: &str,
    entry_collection: Option<&str>,
) {
    augmentation::upsert(
        cat,
        &AugmentationRow {
            artifact_id: artifact_id.to_owned(),
            prompt: "keep it current".to_owned(),
            params: params.to_owned(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
            render_template: None,
            params_schema: None,
            append_mode: false,
            history_cap: None,
            entry_collection: entry_collection.map(str::to_owned),
            refreshed_at_commit: None,
        },
    )
    .expect("the augmentation is stored");
}
