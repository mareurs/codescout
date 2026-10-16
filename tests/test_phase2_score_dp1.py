"""Regression coverage for scripts/phase2-score-dp1.py: per-row persistence and the clean
judge channel.

Codex review, 2026-09-24 (docs/research/2026-09-24-codex-rule-tell-review.md): the scorer
kept only per-arm totals, so after a checker change nobody could say which replies had
changed verdict without paying for the judge calls again; and unlike the phase-1 selector it
did not refuse a contaminated judge profile, whose default was ~/.claude-kat.

No model call is made: `judge` is replaced by a deterministic fake and the judge profile
is a temp dir holding a dummy credentials file.
"""

import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/phase2-score-dp1.py"


def clean_profile(root: Path) -> Path:
    root.mkdir(parents=True, exist_ok=True)
    (root / ".credentials.json").write_text("{}")  # INERT: only its existence is checked
    # syncClaudeAiSkills:false is part of the clean channel (bug 2026-09-25): left unset, the CLI
    # syncs the account's claude.ai skills into skills/synced and registers them in every session.
    (root / "settings.json").write_text(
        '{"enabledPlugins":{},"hooks":{},"syncClaudeAiSkills":false}')
    return root


SPEC = importlib.util.spec_from_file_location("phase2_score_dp1", SCRIPT)
sc = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = sc
SPEC.loader.exec_module(sc)

# A rule of the test's own, so no real rule's question or extra_gate fixtures are involved.
sc.RULES["t"] = {"question": "Q", "observable": lambda r: "doc" in r["tools"]}


def fake_judge(question, text, retries=3):
    # LOAD-BEARING: "VIOLATE" is what makes a text a YES. The request fixture's recorded
    # output carries it and the corrected/unrelated gate texts do not, so the gate passes.
    return "YES" if "VIOLATE" in text else "NO"


ROWS = [
    {"arm": "0", "run": 0, "tools": ["doc"], "text": "VIOLATE here"},   # -> violation
    {"arm": "0", "run": 1, "tools": ["doc"], "text": "fine"},           # -> compliant
    {"arm": "1b", "run": 0, "tools": [], "text": "VIOLATE unseen"},     # -> not-observable
    {"arm": "1b", "run": 1, "error": "usage cap"},                      # -> errored
]


class Scorer(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        d = Path(self.tmp.name)
        self.original_profile = sc._p.config_dir
        sc._p.config_dir = str(clean_profile(d / "judge"))
        self.request = d / "request.json"
        self.request.write_text(json.dumps(
            {"output": [{"type": "tool_use", "input": {"body": "VIOLATE recorded"}}]}))
        self.corrected = d / "corrected.txt"
        self.corrected.write_text("a corrected text")
        self.replays = d / "replays.jsonl"
        self.replays.write_text("".join(json.dumps(r) + "\n" for r in ROWS))
        self.out = d / "out.jsonl"
        self.calls = 0

    def tearDown(self):
        sc._p.config_dir = self.original_profile
        self.tmp.cleanup()

    def run_main(self, *extra, judge=fake_judge, out=True):
        def counted(*a, **k):
            self.calls += 1
            return judge(*a, **k)
        argv = ["phase2-score-dp1.py", "--rule", "t", "--request", str(self.request),
                "--replays", str(self.replays), "--corrected", str(self.corrected), *extra]
        if out:
            argv += ["--out", str(self.out)]
        with mock.patch.object(sys, "argv", argv), mock.patch.object(sc, "judge", counted), \
                contextlib.redirect_stdout(io.StringIO()):
            return sc.main()

    def lines(self):
        return [json.loads(l) for l in self.out.read_text().splitlines()]

    def test_every_row_is_written_with_its_votes_and_outcome(self):
        self.assertEqual(self.run_main(), 0)
        header, *rows = self.lines()
        self.assertEqual(len(rows), len(ROWS))  # one line per input row, none dropped
        self.assertEqual(rows, [
            {"arm": "0", "run": 0, "outcome": "violation", "votes": ["YES"] * 3},
            {"arm": "0", "run": 1, "outcome": "compliant", "votes": ["NO"] * 3},
            {"arm": "1b", "run": 0, "outcome": "not-observable", "votes": None},
            {"arm": "1b", "run": 1, "outcome": "errored", "votes": None},
        ])

    def test_the_header_names_the_checker_the_judge_and_the_input(self):
        self.run_main()
        h = self.lines()[0]["header"]
        self.assertEqual(h["rule"], "t")
        self.assertEqual(h["question_sha256"], hashlib.sha256(b"Q").hexdigest())
        self.assertEqual(h["judge_model"], sc._p.model)
        self.assertEqual(h["judge_channel"], "clean")
        self.assertEqual(h["replays_sha256"], hashlib.sha256(self.replays.read_bytes()).hexdigest())
        self.assertEqual([g["fixture"] for g in h["gate"]], ["recorded", "corrected", "unrelated"])
        self.assertTrue(h["gate_passed"])

    def test_a_failed_gate_is_recorded_and_no_row_is_scored(self):
        self.assertEqual(self.run_main(judge=lambda q, t, retries=3: "NO"), 1)
        lines = self.lines()
        self.assertEqual(len(lines), 1)
        self.assertFalse(lines[0]["header"]["gate_passed"])
        self.assertEqual(self.calls, 9)  # 3 gate fixtures x 3 runs, and nothing after

    def test_out_is_required(self):
        with self.assertRaises(SystemExit) as e:
            self.run_main(out=False)
        self.assertEqual(e.exception.code, 2)
        self.assertEqual(self.calls, 0)

    def test_a_dirty_judge_profile_is_refused_before_any_judge_call(self):
        dirty = clean_profile(Path(self.tmp.name) / "dirty")
        (dirty / "CLAUDE.md").write_text("ALWAYS VERIFY")
        sc._p.config_dir = str(dirty)
        with self.assertRaises(SystemExit) as e:
            self.run_main()
        self.assertIn("not clean (CLAUDE.md present)", str(e.exception.code))
        self.assertEqual(self.calls, 0)
        self.assertFalse(self.out.exists())

    def test_allow_dirty_judge_runs_and_labels_the_channel(self):
        dirty = clean_profile(Path(self.tmp.name) / "dirty")
        (dirty / "CLAUDE.md").write_text("ALWAYS VERIFY")
        sc._p.config_dir = str(dirty)
        self.assertEqual(self.run_main("--allow-dirty-judge"), 0)
        self.assertEqual(self.lines()[0]["header"]["judge_channel"], "dirty: CLAUDE.md present")


class DirtyReasons(unittest.TestCase):
    """Each dirty case breaks exactly one condition, so each exercises only the check it names."""

    NO_SYNC = '"syncClaudeAiSkills":false'

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = clean_profile(Path(self.tmp.name))

    def tearDown(self):
        self.tmp.cleanup()

    def _sync_a_skill(self, name="docx"):
        """The on-disk shape the CLI writes (read from a real profile, 2026-10-16):
        skills/synced/<org>_<account>/<skill>/SKILL.md, beside hidden bucket markers."""
        bucket = self.d / "skills" / "synced" / "org_acct"
        (bucket / name).mkdir(parents=True)
        (bucket / name / "SKILL.md").write_text("---\nname: docx\n---\n")
        (self.d / "skills" / "synced" / ".bucket-org_acct").write_text("")
        (bucket / "manifest.json").write_text("{}")

    def test_a_clean_profile_has_no_reasons(self):
        self.assertEqual(sc.dirty_reasons(str(self.d)), [])

    def test_a_user_claude_md_is_dirty(self):
        (self.d / "CLAUDE.md").write_text("x")
        self.assertEqual(sc.dirty_reasons(str(self.d)), ["CLAUDE.md present"])

    def test_an_enabled_plugin_is_dirty(self):
        (self.d / "settings.json").write_text(
            '{"enabledPlugins":{"p@m":true},"hooks":{},' + self.NO_SYNC + '}')
        self.assertEqual(sc.dirty_reasons(str(self.d)), ["plugins enabled"])

    def test_a_configured_hook_is_dirty(self):
        (self.d / "settings.json").write_text(
            '{"enabledPlugins":{},"hooks":{"Stop":[]},' + self.NO_SYNC + '}')
        self.assertEqual(sc.dirty_reasons(str(self.d)), ["hooks configured"])

    def test_a_missing_settings_file_is_dirty_not_clean(self):
        # A profile with no settings.json loads the defaults: plugins, hooks and claude.ai skill sync.
        (self.d / "settings.json").unlink()
        self.assertEqual(sc.dirty_reasons(str(self.d)),
                         ["plugins enabled", "hooks configured", "claude.ai skill sync enabled"])

    def test_a_synced_claude_ai_skill_is_dirty(self):
        # Passes the three older checks: no CLAUDE.md, no plugins, no hooks. Bug 2026-09-25.
        self._sync_a_skill()
        self.assertEqual(sc.dirty_reasons(str(self.d)), ["claude.ai skills synced"])

    def test_hidden_markers_with_no_synced_skill_are_not_dirty(self):
        # A bucket marker and a manifest without a SKILL.md registers nothing.
        (self.d / "skills" / "synced" / "org_acct").mkdir(parents=True)
        (self.d / "skills" / "synced" / ".bucket-org_acct").write_text("")
        (self.d / "skills" / "synced" / "org_acct" / "manifest.json").write_text("{}")
        self.assertEqual(sc.dirty_reasons(str(self.d)), [])

    def test_a_user_skill_outside_synced_is_not_the_synced_check(self):
        # skills/<name>/SKILL.md is a user skill (the CLAUDE.md-class channel), not a claude.ai sync.
        (self.d / "skills" / "mine").mkdir(parents=True)
        (self.d / "skills" / "mine" / "SKILL.md").write_text("x")
        self.assertEqual(sc.dirty_reasons(str(self.d)), [])

    def test_skill_sync_left_unset_is_dirty(self):
        # The flag is only off when it is exactly false: unset and true both leave syncing on.
        for settings in ('{"enabledPlugins":{},"hooks":{}}',
                         '{"enabledPlugins":{},"hooks":{},"syncClaudeAiSkills":true}'):
            (self.d / "settings.json").write_text(settings)
            self.assertEqual(sc.dirty_reasons(str(self.d)), ["claude.ai skill sync enabled"], settings)

    def test_both_sync_reasons_are_reported_together(self):
        self._sync_a_skill()
        (self.d / "settings.json").write_text('{"enabledPlugins":{},"hooks":{}}')
        self.assertEqual(sc.dirty_reasons(str(self.d)),
                         ["claude.ai skills synced", "claude.ai skill sync enabled"])


if __name__ == "__main__":
    unittest.main()
