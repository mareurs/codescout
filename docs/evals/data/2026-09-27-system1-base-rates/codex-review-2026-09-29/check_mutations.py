"""No model calls; apply mutants in imported modules, never to shared source files."""
import importlib.util
import inspect
import io
import json
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[5]


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, ROOT / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main():
    reservation = load("review_reservation", "tests/test_measure_judge_reservation.py")
    outputs = load("review_outputs", "tests/test_measure_run_outputs.py")
    cases = [
        (reservation.judge, "_reserve_gate_logs", 'open("x")', 'open("w")', reservation.ReservationTests,
         "test_two_contenders_observing_empty_directory_only_enter_once"),
        (reservation.judge, "run_gate", "_reserve_gate_logs(log_dir)", "pass", reservation.ReservationTests,
         "test_successful_run_cannot_reenter_or_overwrite_its_logs"),
        (reservation.judge, "_reserve_gate_logs", "if any(path.iterdir()):", "if False:", reservation.ReservationTests,
         "test_legacy_evidence_refused_before_channel_construction"),
        (reservation.judge, "run_gate", "if not dry and log_dir is not None:\n        _reserve_gate_logs(log_dir)", "if log_dir is not None:\n        _reserve_gate_logs(log_dir)", reservation.ReservationTests,
         "test_dry_run_does_not_reserve_or_touch_live_evidence"),
        (reservation.judge, "format_gate", 's["checks"]["control_fires"]["reported_as"]', '""', reservation.ReportTests,
         "test_control_caveat_and_nulls_survive_the_text_report"),
        (outputs.run, "_gate", 'path.open("x")', 'path.open("w")', outputs.OutputTests,
         "test_racing_writer_cannot_be_overwritten_after_preflight"),
    ]
    evidence = []
    for module, name, old, new, cls, method in cases:
        original = getattr(module, name)
        source = inspect.getsource(original)
        assert source.count(old) == 1, (name, old, source.count(old))
        # Same globals as production, but replace only this function object.
        exec(compile(source.replace(old, new), "<applied-review-mutant>", "exec"), module.__dict__)
        stream = io.StringIO()
        try:
            result = unittest.TextTestRunner(stream=stream).run(unittest.TestSuite([cls(method)]))
        finally:
            setattr(module, name, original)
        evidence.append({"symbol": name, "mutation": [old, new], "test": method,
                         "tests_run": result.testsRun, "failures": len(result.failures),
                         "errors": len(result.errors), "survived": result.wasSuccessful(),
                         "output": stream.getvalue()})
    summary = {"model_calls": 0, "applied": len(evidence),
               "survived": sum(e["survived"] for e in evidence), "mutations": evidence}
    print(json.dumps(summary, indent=2))
    return int(summary["survived"] != 0 or any(e["errors"] for e in evidence))


if __name__ == "__main__":
    raise SystemExit(main())
