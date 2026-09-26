"""Stage 0 archive/manifest: freeze() snapshots a corpus, verify() checks it back.

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_archive.py -v
"""
import importlib.util
import pathlib
import sqlite3
import sys
import tempfile
import unittest
from datetime import datetime, timedelta

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
MEASURE = REPO_ROOT / "scripts" / "measure"
sys.path.insert(0, str(MEASURE))


def _load(name):
    spec = importlib.util.spec_from_file_location(f"measure_{name}", MEASURE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


archive = _load("archive")


def _make_transcript_dir(root, profile, project_slug, sessions):
    """Build <root>/<profile>/projects/<project_slug>/ in the real transcript layout:
    top-level <sid>.jsonl for ordinary sessions, <sid>/subagents/agent-*.jsonl for subagent
    transcripts. `sessions` is {sid: [subagent_names]}. Returns the project_slug directory —
    the thing `sources["transcript_dirs"]` entries point at.
    """
    project_dir = root / profile / "projects" / project_slug
    project_dir.mkdir(parents=True, exist_ok=True)
    for sid, subagents in sessions.items():
        (project_dir / f"{sid}.jsonl").write_text('{"type": "user", "sid": "%s"}\n' % sid)
        if subagents:
            sub_dir = project_dir / sid / "subagents"
            sub_dir.mkdir(parents=True, exist_ok=True)
            for name in subagents:
                (sub_dir / f"{name}.jsonl").write_text('{"type": "assistant"}\n')
    return project_dir


def _make_usage_db(path):
    conn = sqlite3.connect(str(path))
    conn.execute(
        "CREATE TABLE tool_calls (id INTEGER PRIMARY KEY, tool_name TEXT, session_id TEXT)"
    )
    conn.executemany(
        "INSERT INTO tool_calls (tool_name, session_id) VALUES (?, ?)",
        [("read_file", "s1"), ("grep", "s1"), ("doc", "s2")],
    )
    conn.commit()
    conn.close()


class FreezeAndVerify(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        tmp_path = pathlib.Path(self.tmp.name)
        self.out_root = tmp_path / "corpora"
        sources_root = tmp_path / "sources"
        sources_root.mkdir()

        self.project_dir = _make_transcript_dir(
            sources_root,
            "claude-kat",
            "-home-marius-work-claude-codescout",
            {"sid-a": [], "sid-b": ["agent-1"]},
        )
        self.usage_db = sources_root / "usage.db"
        _make_usage_db(self.usage_db)

        self.sources = {
            "transcript_dirs": [str(self.project_dir)],
            "usage_dbs": [str(self.usage_db)],
            "repos": {},
            "bounds": {"start_utc": "2026-08-26T00:00:00Z", "end_utc": "2026-09-26T00:00:00Z"},
        }

    def test_verify_is_empty_on_an_intact_corpus(self):
        manifest = archive.freeze("c-intact", self.sources, self.out_root)
        corpus_dir = self.out_root / manifest["corpus_id"]
        self.assertEqual(archive.verify(corpus_dir), [])

    def test_decision_window_is_the_last_seven_days_of_the_retained_window(self):
        manifest = archive.freeze("c-window", self.sources, self.out_root)
        end = datetime.fromisoformat("2026-09-26T00:00:00+00:00")
        expected_start = (end - timedelta(days=7)).strftime("%Y-%m-%dT%H:%M:%SZ")
        bounds = manifest["bounds"]
        self.assertEqual(bounds["retained"]["start_utc"], "2026-08-26T00:00:00Z")
        self.assertEqual(bounds["retained"]["end_utc"], "2026-09-26T00:00:00Z")
        self.assertEqual(bounds["decision"]["end_utc"], "2026-09-26T00:00:00Z")
        self.assertEqual(bounds["decision"]["start_utc"], expected_start)

    def test_verify_names_a_changed_file(self):
        manifest = archive.freeze("c-changed", self.sources, self.out_root)
        corpus_dir = self.out_root / manifest["corpus_id"]
        target_rel = next(iter(manifest["files"]))
        target = corpus_dir / target_rel
        data = bytearray(target.read_bytes())
        data[0] ^= 0xFF
        target.write_bytes(bytes(data))
        self.assertEqual(archive.verify(corpus_dir), [target_rel])

    def test_freeze_refuses_an_out_root_inside_the_repo(self):
        inside = REPO_ROOT / "scripts" / "measure" / "_should_not_exist"
        with self.assertRaises(ValueError) as ctx:
            archive.freeze("c-inside", self.sources, inside)
        self.assertIn("Global Constraint", str(ctx.exception))

    def test_manifest_counts_subagent_transcripts_separately(self):
        manifest = archive.freeze("c-counts", self.sources, self.out_root)
        self.assertEqual(manifest["counts"]["transcripts"], 2)
        self.assertEqual(manifest["counts"]["subagent_transcripts"], 1)

    def test_manifest_json_is_never_listed_in_files(self):
        # R1: manifest.json describes the corpus; it is not itself a corpus member.
        manifest = archive.freeze("c-r1", self.sources, self.out_root)
        self.assertNotIn("manifest.json", manifest["files"])
        corpus_dir = self.out_root / manifest["corpus_id"]
        self.assertTrue((corpus_dir / "manifest.json").exists())
    def test_multi_profile_transcripts_are_kept_distinct(self):
        # Review round 1, finding 1: a mutation breaking `profile_name` derivation (wrong
        # parent depth, or the "profile" fallback always firing) must not leave this green.
        # Two distinct synthetic profile roots, same project slug, same-named session file.
        tmp_path = pathlib.Path(self.tmp.name)
        two_profile_root = tmp_path / "two_profiles"
        dir_a = two_profile_root / ".claude-a" / "projects" / "p"
        dir_b = two_profile_root / ".claude-b" / "projects" / "p"
        dir_a.mkdir(parents=True)
        dir_b.mkdir(parents=True)
        (dir_a / "s1.jsonl").write_text('{"profile": "a"}\n')
        (dir_b / "s1.jsonl").write_text('{"profile": "b"}\n')
        sub_dir = dir_b / "s1" / "subagents"
        sub_dir.mkdir(parents=True)
        (sub_dir / "agent-x.jsonl").write_text('{"type": "assistant"}\n')

        sources = dict(self.sources)
        sources["transcript_dirs"] = [str(dir_a), str(dir_b)]
        manifest = archive.freeze("c-multiprofile", sources, self.out_root)
        corpus_dir = self.out_root / manifest["corpus_id"]

        rels = [r for r in manifest["files"] if r.startswith("transcripts/")]
        a_rels = [r for r in rels if ".claude-a" in r]
        b_rels = [r for r in rels if ".claude-b" in r]
        self.assertTrue(a_rels, "profile .claude-a's name must appear in a manifest files key")
        self.assertTrue(b_rels, "profile .claude-b's name must appear in a manifest files key")

        a_s1 = [r for r in a_rels if r.endswith("/s1.jsonl")]
        b_s1 = [r for r in b_rels if r.endswith("/s1.jsonl")]
        self.assertEqual(len(a_s1), 1)
        self.assertEqual(len(b_s1), 1)
        self.assertNotEqual(a_s1[0], b_s1[0])  # distinct destination paths — no overwrite

        a_bytes = (corpus_dir / a_s1[0]).read_bytes()
        b_bytes = (corpus_dir / b_s1[0]).read_bytes()
        self.assertIn(b'"profile": "a"', a_bytes)
        self.assertIn(b'"profile": "b"', b_bytes)
        self.assertNotEqual(a_bytes, b_bytes)

        self.assertEqual(manifest["counts"]["transcripts"], 2)
        self.assertEqual(manifest["counts"]["subagent_transcripts"], 1)

    def test_verify_names_an_added_file(self):
        manifest = archive.freeze("c-added", self.sources, self.out_root)
        corpus_dir = self.out_root / manifest["corpus_id"]
        extra = corpus_dir / "transcripts" / "intruder.jsonl"
        extra.parent.mkdir(parents=True, exist_ok=True)
        extra.write_text('{"not": "in manifest"}\n')
        self.assertEqual(archive.verify(corpus_dir), ["transcripts/intruder.jsonl"])

    def test_verify_names_a_deleted_file(self):
        manifest = archive.freeze("c-deleted", self.sources, self.out_root)
        corpus_dir = self.out_root / manifest["corpus_id"]
        target_rel = next(iter(manifest["files"]))
        (corpus_dir / target_rel).unlink()
        self.assertEqual(archive.verify(corpus_dir), [target_rel])

    def test_usage_db_wal_rows_are_captured_by_backup_not_a_file_copy(self):
        # R20 discriminator: rows committed in WAL mode but never checkpointed sit only in the
        # `-wal` file alongside the main `.db` file. A plain `shutil.copy` of the main file alone
        # would miss them (or find no `tool_calls` table at all); `sqlite3.Connection.backup`
        # goes through SQLite's own consistent-snapshot machinery and must not.
        tmp_path = pathlib.Path(self.tmp.name)
        wal_db = tmp_path / "wal_usage.db"
        conn = sqlite3.connect(str(wal_db))
        conn.execute("PRAGMA journal_mode=WAL")
        conn.execute(
            "CREATE TABLE tool_calls (id INTEGER PRIMARY KEY, tool_name TEXT, session_id TEXT)"
        )
        rows = [
            ("read_file", "s1"),
            ("grep", "s1"),
            ("doc", "s2"),
            ("edit_code", "s2"),
            ("symbols", "s3"),
        ]
        conn.executemany(
            "INSERT INTO tool_calls (tool_name, session_id) VALUES (?, ?)", rows
        )
        conn.commit()
        # Deliberately left OPEN and uncheckpointed: these rows sit in `wal_usage.db-wal`, not
        # necessarily in `wal_usage.db` itself.
        self.addCleanup(conn.close)

        sources = dict(self.sources)
        sources["usage_dbs"] = [str(wal_db)]
        manifest = archive.freeze("c-wal", sources, self.out_root)
        corpus_dir = self.out_root / manifest["corpus_id"]

        frozen = next((corpus_dir / "usage_dbs").iterdir())
        check_conn = sqlite3.connect(str(frozen))
        try:
            count = check_conn.execute("SELECT COUNT(*) FROM tool_calls").fetchone()[0]
        finally:
            check_conn.close()

        self.assertEqual(count, len(rows))
        self.assertEqual(manifest["counts"]["usage_rows"], len(rows))


if __name__ == "__main__":
    unittest.main()
