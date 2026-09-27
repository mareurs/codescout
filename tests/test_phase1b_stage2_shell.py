"""Exercise the real Stage-2 shell control flow with fake, cheap Python children.

Only the scripts' four machine-local path bindings change in the sandbox copies.
No model, checkpoint, frozen fold, or live experiment directory is read or written.
"""
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "docs/evals/data/2026-09-24-rule-tell/phase1b/stage2"
FAKE = '''#!/usr/bin/env python3
import json, os, pathlib, sys
args = sys.argv[1:]
if args[0] == "-u": args = args[1:]
with open(os.environ["FAKE_CALLS"], "a") as f:
    f.write(json.dumps(args) + "\\n")
fail = os.environ.get("FAKE_FAIL")
if fail and fail in " ".join(args): sys.exit(23)
if "--out" in args:
    p = pathlib.Path(args[args.index("--out") + 1])
    if not p.is_dir():
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text("{}\\n")
'''


def exercise(script, failure="", transform=None):
    """Return the observed process status, child invocations, and wrapper log."""
    with tempfile.TemporaryDirectory(prefix="phase1b-shell-") as td:
        base = Path(td)
        repo, out = base / "repo", base / "runs"
        d = repo / "docs/evals/data/2026-09-24-rule-tell"
        phase = d / "phase1b"
        for p in (out, d / "stage3", phase / "audit"):
            p.mkdir(parents=True)
        # Real provenance commands operate on this disposable repository only.
        subprocess.run(["git", "init", "-q", str(repo)], check=True)
        subprocess.run(["git", "-C", str(repo), "-c", "user.name=Test", "-c",
                        "user.email=test@invalid", "-c", "core.hooksPath=/dev/null",
                        "commit", "--allow-empty", "--no-gpg-sign", "-qm", "fixture"],
                       check=True)
        for p in (d / "stage3/train_arm.py", phase / "audit/admission.json",
                  phase / "audit/counterexamples.jsonl", phase / "step5_gate.py",
                  repo / "scripts/phase1-span-selector.py"):
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text("fixture\n")
        if script != "lanes.sh":
            (out / "lanes.out").write_text("LANES_EXIT=0\n")
            (out / "lanes.log").write_text("train fixture exit 0\n" * 6)
        if script == "step5.sh":
            (phase / "stage2").mkdir()
            (phase / "stage2/common.json").write_text("{}\n")
        fake = base / "python-child"
        fake.write_text(FAKE)
        fake.chmod(0o755)
        text = (SCRIPTS / script).read_text()
        for key, value in {"REPO": repo, "OUT": out, "PY": fake, "S1": base / "s1"}.items():
            text, n = re.subn(rf"^{key}=.*$", f"{key}={shlex.quote(str(value))}",
                              text, flags=re.M)
            if key != "S1" or script == "step4.sh":
                assert n == 1, (script, key, n)
        if transform:
            text = transform(text)
        copy = base / script
        copy.write_text(text)
        calls = base / "calls.jsonl"
        env = dict(os.environ, FAKE_CALLS=str(calls), FAKE_FAIL=failure)
        proc = subprocess.run(["bash", str(copy)], env=env, capture_output=True,
                              text=True, timeout=10)
        log = out / {"lanes.sh": "lanes.log", "step4.sh": "step4.log",
                     "step5.sh": "step5.log"}[script]
        return (proc.returncode, [json.loads(s) for s in calls.read_text().splitlines()],
                log.read_text(), proc.stderr)


class Stage2Shell(unittest.TestCase):
    def test_success_runs_every_child_and_returns_zero(self):
        for script, expected in (("lanes.sh", 6), ("step4.sh", 19), ("step5.sh", 10)):
            with self.subTest(script=script):
                rc, calls, log, err = exercise(script)
                self.assertEqual(rc, 0, (log, err))
                self.assertEqual(len(calls), expected)
                if script != "lanes.sh":
                    self.assertIn("DONE fail=0", log)

    def test_early_n_failure_survives_later_success_and_wait(self):
        self.check_training_failure("s2-n-20260935")

    def test_early_nc_failure_survives_later_success_and_wait(self):
        self.check_training_failure("s2-nc-20260935")

    def check_training_failure(self, failure):
        rc, calls, log, err = exercise("lanes.sh", failure)
        self.assertIn("exit 23", log)
        self.assertEqual(len(calls), 6)  # both lanes finished; no abandoned child
        self.assertNotEqual(rc, 0, (log, err))

    def test_score_failure_skips_its_calibration_and_common_and_returns_nonzero(self):
        rc, calls, log, err = exercise("step4.sh", "scored-b-20260935.json")
        self.assertIn("score b-20260935 exit 23", log)
        self.assertEqual(len(calls), 17)
        self.assertFalse(any("--common" in c for c in calls))
        self.assertIn("STEP4_DONE fail=1", log)
        self.assertNotEqual(rc, 0, (log, err))

    def test_calibration_failure_skips_common_and_returns_nonzero(self):
        rc, calls, log, err = exercise("step4.sh", "step4-b-20260935.json")
        self.assertIn("step4 b-20260935 exit 23", log)
        self.assertEqual(len(calls), 18)
        self.assertFalse(any("--common" in c for c in calls))
        self.assertIn("STEP4_DONE fail=1", log)
        self.assertNotEqual(rc, 0, (log, err))

    def test_common_failure_is_not_logged_as_done_without_failure(self):
        rc, calls, log, err = exercise("step4.sh", "--common")
        self.assertIn("common exit 23", log)
        self.assertEqual(len(calls), 19)
        self.assertIn("STEP4_DONE fail=1", log)
        self.assertNotEqual(rc, 0, (log, err))

    def test_gate_execution_failure_skips_summary_and_returns_nonzero(self):
        rc, calls, log, err = exercise("step5.sh", "step4-b-20260935.json")
        self.assertIn("gate b-20260935 exit 23", log)
        self.assertEqual(len(calls), 9)
        self.assertFalse(any("--summary" in c for c in calls))
        self.assertIn("STEP5_DONE fail=1", log)
        self.assertNotEqual(rc, 0, (log, err))

    def test_summary_failure_is_not_logged_as_done_without_failure(self):
        rc, calls, log, err = exercise("step5.sh", "--summary")
        self.assertIn("summary exit 23", log)
        self.assertEqual(len(calls), 10)
        self.assertIn("STEP5_DONE fail=1", log)
        self.assertNotEqual(rc, 0, (log, err))


if __name__ == "__main__":
    unittest.main()
