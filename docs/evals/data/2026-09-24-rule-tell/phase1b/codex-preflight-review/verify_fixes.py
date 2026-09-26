"""Offline regression/mutation checks: no source writes and no model calls."""
import ast
import hashlib
import importlib.util
import io
import json
import pathlib
import unittest
from unittest.mock import patch

ROOT = pathlib.Path(__file__).resolve().parents[6]
BASE = ROOT / "docs/evals/data/2026-09-24-rule-tell/phase1b"
spec = importlib.util.spec_from_file_location("audit_tests", ROOT / "tests/test_phase1b_audit.py")
tests = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tests)
runner = tests.rl
source_path = BASE / "run_labellers.py"
source = source_path.read_text()
bodies = {node.name: "\n".join(source.splitlines()[node.lineno - 1:node.end_lineno])
          for node in ast.parse(source).body if isinstance(node, ast.FunctionDef)}
originals = {name: getattr(runner, name) for name in bodies}

legacy_scheduler = '''def run_claude_batches(items, instruction, menu_text, menu, raw, judge, workers):
    def one(nb):
        n, batch = nb
        return run_call(f"claude-b{n:02d}", [i["id"] for i in batch],
                        lambda _: judge.complete(claude_prompt(instruction, menu_text, batch))[0], menu, raw)
    ex = cf.ThreadPoolExecutor(workers)
    try:
        return [a for part in ex.map(one, enumerate(batches(items))) for a in part]
    finally:
        ex.shutdown(wait=True, cancel_futures=True)
'''

mutations = {
    "legacy_ordered_map": ("run_claude_batches", None, legacy_scheduler),
    "skip_run_reservation": ("main", "raw = reserve_run(d, header)",
                             'raw = d / "raw"; raw.mkdir(exist_ok=True)'),
    "nonexclusive_reservation": ("reserve_run", 'header_path.open("x")', 'header_path.open("w")'),
    "terminal_failure_returns_success": ("main", "return 4", "return 0"),
    "reverse_output_order": ("run_claude_batches", "for n in sorted(parts)", "for n in sorted(parts, reverse=True)"),
    "retry_cancelled_calls": ("run_call", "        except cf.CancelledError:\n            raise\n", ""),
    "allow_retry_after_terminal_failure": ("run_claude_batches",
        "            if stopped.is_set():\n                raise cf.CancelledError()\n", ""),
    "late_worker_validation": ("main",
        '    if args.workers < 1:\n        ap.error("--workers must be positive")\n', ""),
}

def run_tests():
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(tests.LabellerMain)
    with patch.object(runner.subprocess, "run", side_effect=AssertionError("NO MODEL CALLS")):
        outcome = unittest.TextTestRunner(stream=io.StringIO()).run(suite)
    return {"tests": outcome.testsRun,
            "failures": [case.id() for case, _ in outcome.failures],
            "errors": [case.id() for case, _ in outcome.errors]}

result = {"baseline": run_tests(), "mutations": {}}
for label, (name, old, new) in mutations.items():
    body = bodies[name]
    if old is not None:
        assert body.count(old) == 1, (label, "mutation anchor drifted")
        body = body.replace(old, new)
    else:
        body = new
    try:
        exec(compile(body, str(source_path), "exec"), runner.__dict__)
        result["mutations"][label] = run_tests()
    finally:
        setattr(runner, name, originals[name])

result["survived"] = [name for name, run in result["mutations"].items()
                      if not run["failures"] and not run["errors"]]
result["source_unchanged"] = source_path.read_text() == source
result["sha256"] = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                    for p in (source_path, ROOT / "tests/test_phase1b_audit.py")}
(pathlib.Path(__file__).parent / "fix-verification.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
raise SystemExit(bool(result["baseline"]["failures"] or result["baseline"]["errors"]
                      or result["survived"] or not result["source_unchanged"]))
