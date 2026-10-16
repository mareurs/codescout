"""Regression coverage for scripts/probe_guide_injection.py's session de-duplication.

docs/issues/2026-09-11-subagent-transcripts-are-byte-identical-across-profile-dirs.md: this
machine's three Claude Code profile dirs hold byte-identical copies of the same transcripts
(measured 2026-10-16: 584 subagent groups same-size, 6 subagent and 27 main groups where the
copies differ in size, copies present under 2 or 3 profiles). `collect` collapses them, but it
keyed the collapse on the file's BASENAME alone, which is only a session identity for
`<uuid>.jsonl` and `agent-<id>.jsonl`. The prompt-test harness writes a `session.jsonl` into each
of 573 distinct directories (measured 2026-10-16); keyed by basename, 573 sessions are one
session, so the "unique sessions" figure and every per-session share is wrong while looking fine.
Nothing told the reader how many copies had been collapsed either.

Fixtures are the real marker format (`OPEN_MARK` / `END_MARK` from the script itself).
"""

import contextlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/probe_guide_injection.py"
SPEC = importlib.util.spec_from_file_location("probe_guide_injection", SCRIPT)
probe = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = probe
SPEC.loader.exec_module(probe)

UUID_A = "11111111-2222-3333-4444-555555555555.jsonl"
UUID_B = "66666666-7777-8888-9999-000000000000.jsonl"


def transcript(n_injections: int = 1) -> str:
    """A minimal transcript the probe counts: one codescout tool_use plus n injected results."""
    lines = [json.dumps({
        "timestamp": "2026-10-01T00:00:00Z",
        "message": {"model": "m", "content": [
            {"type": "tool_use", "id": f"t{i}", "name": "mcp__codescout__run_command"}
            for i in range(n_injections)]},
    })]
    for i in range(n_injections):
        text = f"{probe.OPEN_MARK}librarian') BODY\n{probe.END_MARK} get_guide('librarian') -->"
        lines.append(json.dumps({
            "timestamp": "2026-10-01T00:00:01Z",
            "message": {"content": [
                {"type": "tool_result", "tool_use_id": f"t{i}", "content": text}]},
        }))
    return "\n".join(lines) + "\n"


def write(profile: Path, rel: str, body: str) -> None:
    p = profile / "projects" / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(body, encoding="utf-8")


def run_collect(profiles: list[Path]) -> tuple[list[dict], str]:
    err = io.StringIO()
    with contextlib.redirect_stderr(err):
        rows = probe.collect([str(p) for p in profiles])
    return rows, err.getvalue()


class CollectDedupe(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.a = Path(self._tmp.name) / "a"
        self.b = Path(self._tmp.name) / "b"

    def test_same_uuid_in_two_profiles_is_one_session(self):
        # control: passes before and after
        write(self.a, f"proj/{UUID_A}", transcript())
        write(self.b, f"proj/{UUID_A}", transcript())
        rows, _ = run_collect([self.a, self.b])
        self.assertEqual(len(rows), 1)

    def test_same_subagent_in_two_profiles_is_one_session(self):
        # control: passes before and after
        rel = f"proj/{UUID_A[:-6]}/subagents/agent-abc123.jsonl"
        write(self.a, rel, transcript())
        write(self.b, rel, transcript())
        rows, _ = run_collect([self.a, self.b])
        self.assertEqual(len(rows), 1)

    def test_generic_filename_in_distinct_directories_is_distinct_sessions(self):
        write(self.a, "-tmp-prompt-test-one/session.jsonl", transcript())
        write(self.a, "-tmp-prompt-test-two/session.jsonl", transcript())
        rows, _ = run_collect([self.a])
        self.assertEqual(len(rows), 2)

    def test_generic_filename_at_the_same_relative_path_across_profiles_is_one_session(self):
        write(self.a, "-tmp-prompt-test-one/session.jsonl", transcript())
        write(self.b, "-tmp-prompt-test-one/session.jsonl", transcript())
        rows, _ = run_collect([self.a, self.b])
        self.assertEqual(len(rows), 1)

    def test_collapsed_copies_are_reported(self):
        write(self.a, f"proj/{UUID_A}", transcript())
        write(self.b, f"proj/{UUID_A}", transcript())
        write(self.a, f"proj/{UUID_B}", transcript())
        _, err = run_collect([self.a, self.b])
        self.assertIn("duplicate copies collapsed: 1", err)
        self.assertIn("0 differ in size", err)

    def test_diverged_copies_are_reported_and_the_larger_is_kept(self):
        write(self.a, f"proj/{UUID_A}", transcript(1))
        write(self.b, f"proj/{UUID_A}", transcript(3))
        rows, err = run_collect([self.a, self.b])
        self.assertIn("duplicate copies collapsed: 1", err)
        self.assertIn("1 differ in size", err)
        self.assertEqual(len(rows[0]["injections"]), 3)

    def test_missing_profile_and_empty_profile_do_not_raise(self):
        (self.b / "projects").mkdir(parents=True)
        rows, err = run_collect([self.a, self.b])
        self.assertEqual(rows, [])
        self.assertIn("duplicate copies collapsed: 0", err)

    def test_unreadable_transcript_does_not_raise(self):
        write(self.a, f"proj/{UUID_A}", "not json at all\n\x00\xff")
        rows, _ = run_collect([self.a])
        self.assertEqual(rows, [])


if __name__ == "__main__":
    unittest.main()
