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
