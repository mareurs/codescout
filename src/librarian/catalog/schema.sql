-- Schema v6 (final shape — abs_path replaces repo/rel_path)
CREATE TABLE IF NOT EXISTS artifact (
  id            TEXT PRIMARY KEY,
  abs_path      TEXT NOT NULL UNIQUE,
  kind          TEXT NOT NULL,
  status        TEXT NOT NULL,
  title         TEXT,
  owners        TEXT NOT NULL DEFAULT '[]',
  tags          TEXT NOT NULL DEFAULT '[]',
  topic         TEXT,
  time_scope    TEXT,
  source        TEXT,
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL,
  file_mtime    INTEGER NOT NULL,
  file_sha256   TEXT NOT NULL,
  confidence    REAL NOT NULL DEFAULT 1.0,
  slug          TEXT,
  missing_since INTEGER, -- v10: catalog GC lifecycle (epoch-ms; NULL = present)
  -- v11: the content hash as of the last SUCCESSFUL embed of this artifact.
  -- NULL = never embedded. Deliberately SEPARATE from file_sha256, which means
  -- only "this content was written to the catalog": the indexer used to read
  -- that as "this content has been embedded", and since the row write is
  -- unconditional while the embed is not, the write manufactured the very
  -- "unchanged" condition the embed decision then declined on. Written by the
  -- code that actually embeds, and only once every chunk of the artifact has
  -- been stored -- the queue is chunk-grained, so stamping on the first
  -- success would rebuild the same trap one level down.
  -- docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md
  embedded_sha256 TEXT
);

CREATE TABLE IF NOT EXISTS artifact_link (
  src_id        TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  dst_id        TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  rel           TEXT NOT NULL,
  created_at    INTEGER NOT NULL,
  PRIMARY KEY (src_id, dst_id, rel)
);

-- ux_artifact_slug / entry_cite are NOT created here even though this is the
-- fresh-DB shape: SCHEMA_SQL runs as an unconditional execute_batch on every
-- Catalog::open, including against pre-existing on-disk catalogs where
-- `artifact` already exists without a `slug` column (CREATE TABLE IF NOT
-- EXISTS is then a no-op). Referencing artifact(slug) here would fail with
-- "no such column: slug" for every such catalog. They are instead created by
-- the v9 block in apply_migrations_in_txn (mod.rs), which runs after the
-- ALTER TABLE that guarantees the column exists — idempotent (IF NOT EXISTS)
-- so it's a correct no-op for genuinely fresh DBs too, where schema.sql above
-- already created the `slug` column as part of CREATE TABLE artifact.

CREATE TABLE IF NOT EXISTS artifact_observation (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  artifact_id   TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  text          TEXT NOT NULL,
  source        TEXT,
  created_at    INTEGER NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS artifact_vec USING vec0(
  id            TEXT PRIMARY KEY,
  embedding     FLOAT[768]
);

CREATE TRIGGER IF NOT EXISTS artifact_vec_cascade_delete
AFTER DELETE ON artifact
BEGIN
  DELETE FROM artifact_vec WHERE id = OLD.id;
END;

CREATE INDEX IF NOT EXISTS idx_artifact_kind_status ON artifact(kind, status);
CREATE INDEX IF NOT EXISTS idx_link_dst ON artifact_link(dst_id, rel);

CREATE TABLE IF NOT EXISTS schema_version (
  version INTEGER PRIMARY KEY
);
INSERT OR IGNORE INTO schema_version (version) VALUES (1);

-- v2: TimeMachine event log + narrative graph
CREATE TABLE IF NOT EXISTS events (
  id            TEXT PRIMARY KEY,
  artifact_id   TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  kind          TEXT NOT NULL CHECK (kind IN (
                  'note', 'reviewed', 'status_change', 'field_patch',
                  'superseded_by', 'external_signal',
                  'intent', 'verdict', 'worktree_fork', 'worktree_merge'
                )),
  payload       TEXT NOT NULL,
  anchor_commit TEXT,
  head_commit   TEXT,
  author        TEXT,
  created_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_artifact ON events(artifact_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_events_head_commit ON events(head_commit);
CREATE INDEX IF NOT EXISTS idx_events_anchor_commit ON events(anchor_commit);
CREATE INDEX IF NOT EXISTS idx_events_kind ON events(kind);

CREATE TABLE IF NOT EXISTS commits (
  hash         TEXT PRIMARY KEY,
  git_root     TEXT NOT NULL,
  authored_at  INTEGER,
  subject      TEXT,
  topo_order   INTEGER
);

CREATE TABLE IF NOT EXISTS sources (
  id           TEXT PRIMARY KEY,
  uri          TEXT NOT NULL,
  kind         TEXT NOT NULL CHECK (kind IN (
                  'chat','jira','gmail','confluence','drive','calendar','manual'
                )),
  payload      TEXT,
  ingested_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS event_edges (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  src_event_id    TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,
  dst_event_id    TEXT REFERENCES events(id) ON DELETE CASCADE,
  dst_artifact_id TEXT REFERENCES artifact(id) ON DELETE CASCADE,
  dst_source_id   TEXT REFERENCES sources(id) ON DELETE CASCADE,
  rel             TEXT NOT NULL CHECK (rel IN (
                    'parent', 'mutates', 'triggered_by', 'merges_with', 'resolves'
                  ))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_event_edges_unique ON event_edges(
  src_event_id, rel,
  COALESCE(dst_event_id, ''),
  COALESCE(dst_artifact_id, ''),
  COALESCE(dst_source_id, '')
);
CREATE INDEX IF NOT EXISTS idx_event_edges_src ON event_edges(src_event_id, rel);
CREATE INDEX IF NOT EXISTS idx_event_edges_dst_artifact ON event_edges(dst_artifact_id);
CREATE INDEX IF NOT EXISTS idx_event_edges_dst_event ON event_edges(dst_event_id);

INSERT OR IGNORE INTO schema_version (version) VALUES (2);

-- v3: artifact augmentation (prompt + params for AI-maintained artifacts)
CREATE TABLE IF NOT EXISTS artifact_augmentation (
  artifact_id       TEXT    NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  prompt            TEXT    NOT NULL,
  params            TEXT    NOT NULL DEFAULT '{}',
  last_refreshed_at TEXT,
  refresh_count     INTEGER NOT NULL DEFAULT 0,
  created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (artifact_id)
);
CREATE INDEX IF NOT EXISTS idx_augmentation_artifact ON artifact_augmentation(artifact_id);

INSERT OR IGNORE INTO schema_version (version) VALUES (3);

-- v6: legacy repo/rel_path columns dropped; abs_path replaces them.
INSERT OR IGNORE INTO schema_version (version) VALUES (6);

-- Worktree overlay: durable registration of linked git worktrees that have
-- written to the catalog. Survives `git worktree remove`; the merge flow
-- (librarian action=merge_worktree) closes it. See
-- docs/superpowers/specs/2026-07-17-worktree-overlay-design.md.
CREATE TABLE IF NOT EXISTS worktree_registration (
    worktree_root TEXT PRIMARY KEY,
    main_root     TEXT NOT NULL,
    branch        TEXT,
    created_at    INTEGER NOT NULL,
    status        TEXT NOT NULL DEFAULT 'active',
    closed_at     INTEGER
);

-- v10: catalog GC lifecycle key-value store (e.g. gc_grace_days). See
-- src/librarian/catalog/gc.rs. Functionally inert here (also created by the
-- v10 migration in mod.rs, both IF NOT EXISTS) — kept so schema.sql stays an
-- authoritative fresh-DB reference.
CREATE TABLE IF NOT EXISTS catalog_meta (
    key   TEXT PRIMARY KEY,
    value TEXT
);

-- catalog_audit + its triggers are NOT created here: audit::install (audit.rs)
-- creates them on every open, AFTER migrations, rebuilding triggers from live
-- PRAGMA table_info so table-copy migrations can never orphan them. See
-- docs/superpowers/specs/2026-09-01-catalog-audit-trail-design.md.
