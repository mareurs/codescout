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


if __name__ == "__main__":
    unittest.main()
