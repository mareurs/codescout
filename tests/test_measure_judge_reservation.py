"""No-model regressions for preserving a gate's first-run vote evidence."""
import concurrent.futures
import importlib.util
import pathlib
import tempfile
import threading
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("judge_existing_tests", ROOT / "tests/test_measure_judge.py")
fixtures = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixtures)
judge = fixtures.judge


class ReservationTests(unittest.TestCase):
    def setup_paths(self, tmp):
        root = pathlib.Path(tmp)
        repo, logs = root / "repo", root / "logs"
        repo.mkdir()
        return repo, logs

    def test_successful_run_cannot_reenter_or_overwrite_its_logs(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo, logs = self.setup_paths(tmp)
            def gate(*args):
                logs.mkdir(exist_ok=True)
                (logs / "vote.log").write_text("first answer")
                return {"passed": False}  # a completed, scientifically negative result
            with mock.patch.object(judge, "_gate", side_effect=gate) as g:
                self.assertEqual(judge.run_gate(repo=repo, complete=object(), log_dir=logs), {"passed": False})
                with self.assertRaisesRegex(ValueError, "already|existing|reserved"):
                    judge.run_gate(repo=repo, complete=object(), log_dir=logs)
                self.assertEqual(g.call_count, 1)
                self.assertEqual((logs / "vote.log").read_text(), "first answer")

    def test_legacy_evidence_refused_before_channel_construction(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo, logs = self.setup_paths(tmp)
            logs.mkdir()
            (logs / "RTD-1.vote1.try1.log").write_text("legacy answer")
            with mock.patch.object(judge, "codex_channel") as channel, mock.patch.object(judge, "_gate") as gate:
                channel.return_value.version = judge.CODEX_CLI_VERSION
                with self.assertRaisesRegex(ValueError, "already|existing|reserved"):
                    judge.run_gate(repo=repo, log_dir=logs)
                channel.assert_not_called()
                gate.assert_not_called()
                self.assertEqual((logs / "RTD-1.vote1.try1.log").read_text(), "legacy answer")

    def test_failure_before_first_log_still_reserves_the_attempt(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo, logs = self.setup_paths(tmp)
            with mock.patch.object(judge, "_gate", side_effect=[RuntimeError("interrupted before log"), {}]) as g:
                with self.assertRaisesRegex(RuntimeError, "interrupted"):
                    judge.run_gate(repo=repo, complete=object(), log_dir=logs)
                with self.assertRaisesRegex(ValueError, "already|existing|reserved"):
                    judge.run_gate(repo=repo, complete=object(), log_dir=logs)
                self.assertEqual(g.call_count, 1)

    def test_empty_precreated_directory_is_admitted_once(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo, logs = self.setup_paths(tmp)
            logs.mkdir()
            with mock.patch.object(judge, "_gate", return_value={"passed": True}) as g:
                self.assertEqual(judge.run_gate(repo=repo, complete=object(), log_dir=logs), {"passed": True})
                self.assertEqual(g.call_count, 1)
                self.assertTrue(list(logs.iterdir()))

    def test_dry_run_does_not_reserve_or_touch_live_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo, logs = self.setup_paths(tmp)
            with mock.patch.object(judge, "_gate", return_value={"dry": True}):
                judge.run_gate(repo=repo, dry=True, log_dir=logs)
                self.assertFalse(logs.exists())
                logs.mkdir()
                (logs / "vote.log").write_text("existing")
                judge.run_gate(repo=repo, dry=True, log_dir=logs)
                self.assertEqual({p.name:p.read_text() for p in logs.iterdir()}, {"vote.log":"existing"})

    def test_two_contenders_observing_empty_directory_only_enter_once(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo, logs = self.setup_paths(tmp)
            logs.mkdir()
            barrier = threading.Barrier(2)
            original = pathlib.Path.iterdir
            def simultaneous_empty(path):
                seen = list(original(path))
                if path == logs:
                    barrier.wait(timeout=3)
                return iter(seen)
            def launch():
                try:
                    return judge.run_gate(repo=repo, complete=object(), log_dir=logs)
                except ValueError:
                    return "refused"
            with mock.patch.object(pathlib.Path, "iterdir", simultaneous_empty), \
                    mock.patch.object(judge, "_gate", return_value="entered") as g, \
                    concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
                outcomes = list(pool.map(lambda _: launch(), range(2)))
            self.assertEqual(sorted(outcomes), ["entered", "refused"])
            self.assertEqual(g.call_count, 1)


class ReportTests(unittest.TestCase):
    def test_control_caveat_and_nulls_survive_the_text_report(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo, _ = fixtures._fixture_gate_repo(tmp)
            fake = fixtures._FakeComplete(['{"is_decision_point":true,"is_mistake":null,"lessons":"abstain","lesson_outcomes":{},"detectability":null,"evidence_present_before":"unknown","evidence_used":"unknown","quote":"a claim quoted here","is_correction":false,"origin_uuid":null}'] * 15)
            result = judge.run_gate(repo=repo, complete=fake, log_dir=pathlib.Path(tmp) / "logs",
                                    rtd_doc=repo / "eval/rtd.md", controls_doc=repo / "eval/controls.md",
                                    global_claude_md=fixtures._global_claude_md(tmp), population=None)
        controls = [r for r in result["results"] if r["kind"] == "control"]
        self.assertEqual(len(controls), 2)
        controls[0]["majority"]["is_mistake"] = True
        rendered = judge.format_gate(result)
        self.assertIn("a firing rate, not a false-positive rate", rendered)
        self.assertIn("true=1, false=0, null=1", rendered)
        self.assertIn("null is not a clean verdict", rendered)
        controls[0]["majority"]["is_mistake"] = False
        self.assertIn("true=0, false=1, null=1", judge.format_gate(result))


if __name__ == "__main__":
    unittest.main()
