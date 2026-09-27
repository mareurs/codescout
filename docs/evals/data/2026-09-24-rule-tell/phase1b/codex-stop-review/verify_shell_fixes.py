#!/usr/bin/env python3
"""Run the shell regressions and real mutations on disposable script copies only."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import unittest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[5]
BEFORE = "a5372e000a525f2f8e59d416cf690781598ac02c"
TEST = ROOT / "tests/test_phase1b_stage2_shell.py"
spec = importlib.util.spec_from_file_location("shell_tests", TEST)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
original = mod.exercise


def run(rewrite=None, test=None):
    def exercise(script, failure="", transform=None):
        return original(script, failure, lambda s: rewrite(script, s) if rewrite else s)
    mod.exercise = exercise
    suite = (unittest.TestSuite([mod.Stage2Shell(test)]) if test else
             unittest.defaultTestLoader.loadTestsFromTestCase(mod.Stage2Shell))
    stream = io.StringIO()
    try:
        r = unittest.TextTestRunner(stream=stream, verbosity=2).run(suite)
    finally:
        mod.exercise = original
    return dict(tests=r.testsRun, failures=len(r.failures), errors=len(r.errors),
                passed=r.wasSuccessful(), output=stream.getvalue())


def main():
    # Historical scripts need the same path relocation as current ones. The hook
    # below runs after relocation, so reuse only their control-flow tail beginning
    # at lane()/fail=0. Preconditions/path setup did not change in this fix.
    historical = {}
    for name in ("lanes.sh", "step4.sh", "step5.sh"):
        rel = str((mod.SCRIPTS / name).relative_to(ROOT))
        historical[name] = subprocess.check_output(["git", "show", f"{BEFORE}:{rel}"],
                                                   cwd=ROOT, text=True)
    def before(script, text):
        anchor = "lane() {" if script == "lanes.sh" else "fail=0\n"
        return text[:text.index(anchor)] + historical[script][historical[script].index(anchor):]
    report = dict(before_commit=BEFORE, before=run(before), after=run(), mutations=[])
    assert report["before"]["failures"] == 7 and report["before"]["errors"] == 0
    assert report["after"]["passed"]
    mutations = [
        ("lane drops earlier child failure", "lanes.sh", "    [ $rc = 0 ] || fail=1", "    :", "test_early_n_failure_survives_later_success_and_wait"),
        ("lane returns successful echo status", "lanes.sh", '  return "$fail"', '  return 0', "test_early_n_failure_survives_later_success_and_wait"),
        ("parent ignores N status", "lanes.sh", 'wait "$n_pid" || fail=1', 'wait "$n_pid" || true', "test_early_n_failure_survives_later_success_and_wait"),
        ("parent ignores NC status", "lanes.sh", 'wait "$nc_pid" || fail=1', 'wait "$nc_pid" || true', "test_early_nc_failure_survives_later_success_and_wait"),
        ("lanes exits zero", "lanes.sh", 'exit "$fail"', 'exit 0', "test_early_n_failure_survives_later_success_and_wait"),
        ("common failure not accumulated", "step4.sh", '  [ $rc = 0 ] || fail=1\nfi', '  :\nfi', "test_common_failure_is_not_logged_as_done_without_failure"),
        ("step4 exits zero", "step4.sh", 'exit "$fail"', 'exit 0', "test_calibration_failure_skips_common_and_returns_nonzero"),
        ("summary failure not accumulated", "step5.sh", '  [ $rc = 0 ] || fail=1\nfi', '  :\nfi', "test_summary_failure_is_not_logged_as_done_without_failure"),
        ("step5 exits zero", "step5.sh", 'exit "$fail"', 'exit 0', "test_gate_execution_failure_skips_summary_and_returns_nonzero"),
    ]
    for name, file, old, new, test in mutations:
        def rewrite(script, text):
            if script != file:
                return text
            assert text.count(old) == 1, (name, text.count(old))
            return text.replace(old, new, 1)
        result = run(rewrite, test)
        assert result["failures"] == 1 and result["errors"] == 0, (name, result)
        report["mutations"].append(dict(name=name, file=file, old=old, new=new,
                                        killed=True, result=result))
    report["sha256"] = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                        for p in [TEST, Path(__file__), *sorted(mod.SCRIPTS.glob("*.sh"))]}
    report["survived"] = sum(not m["killed"] for m in report["mutations"])
    (HERE / "shell-verification.json").write_text(json.dumps(report, indent=2)+"\n")
    print(json.dumps({k: report[k] for k in ("survived",)} | {
        "before_failures": report["before"]["failures"], "after_tests": report["after"]["tests"],
        "after_passed": report["after"]["passed"], "mutations_killed": len(report["mutations"])}))


if __name__ == "__main__":
    main()
