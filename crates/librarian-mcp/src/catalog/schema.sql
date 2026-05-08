-- v1 schema
CREATE TABLE IF NOT EXISTS artifact (
  id            TEXT PRIMARY KEY,
  repo          TEXT NOT NULL,
  rel_path      TEXT NOT NULL,
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
  abs_path      TEXT,
  UNIQUE(repo, rel_path)
);

CREATE TABLE IF NOT EXISTS artifact_link (
  src_id        TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  dst_id        TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  rel           TEXT NOT NULL,
  created_at    INTEGER NOT NULL,
  PRIMARY KEY (src_id, dst_id, rel)
);

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
CREATE INDEX IF NOT EXISTS idx_artifact_repo ON artifact(repo);
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
                  'intent', 'verdict'
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
  repo         TEXT NOT NULL,
  authored_at  INTEGER,
  subject      TEXT,
  topo_order   INTEGER,
  git_root     TEXT
);
CREATE INDEX IF NOT EXISTS idx_commits_repo_topo ON commits(repo, topo_order);

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
