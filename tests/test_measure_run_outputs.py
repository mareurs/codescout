"""Gate report paths must be writable and exclusively claimed before model spend."""
import importlib.util
import json
import pathlib
import tempfile
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("measure_run_outputs", ROOT / "scripts/measure/run.py")
run = importlib.util.module_from_spec(spec)
spec.loader.exec_module(run)


class OutputTests(unittest.TestCase):
    def args(self, *args):
        return run._parser().parse_args(["gate", *map(str, args)])

    def test_existing_outputs_refuse_before_judging_in_live_and_dry_modes(self):
        with tempfile.TemporaryDirectory() as td:
            p = pathlib.Path(td) / "saved"
            for flag in ("--out", "--json-out"):
                for dry in ([], ["--dry"]):
                    with self.subTest(flag=flag, dry=dry):
                        p.write_text("first evidence")
                        with mock.patch.object(run.judge, "run_gate", return_value={"passed":False}) as gate, \
                                mock.patch.object(run.judge, "format_gate", return_value="new evidence"):
                            with self.assertRaises((ValueError, FileExistsError)):
                                run._gate(self.args(flag, p, *dry), object())
                            gate.assert_not_called()
                        self.assertEqual(p.read_text(), "first evidence")

    def test_text_and_json_cannot_name_the_same_output(self):
        with tempfile.TemporaryDirectory() as td:
            p = pathlib.Path(td) / "same"
            with mock.patch.object(run.judge, "run_gate", return_value={"passed":True}) as gate, \
                    mock.patch.object(run.judge, "format_gate", return_value="report"):
                with self.assertRaises(ValueError):
                    run._gate(self.args("--out", p, "--json-out", p.parent / "." / p.name), object())
                gate.assert_not_called()

    def test_unwritable_json_parent_fails_before_judging(self):
        with tempfile.TemporaryDirectory() as td:
            p = pathlib.Path(td) / "file"
            p.write_text("not a directory")
            with mock.patch.object(run.judge, "run_gate", return_value={"passed":True}) as gate, \
                    mock.patch.object(run.judge, "format_gate", return_value="report"):
                with self.assertRaises(OSError):
                    run._gate(self.args("--json-out", p / "result.json"), object())
                gate.assert_not_called()

    def test_fresh_nested_outputs_preserve_the_negative_result(self):
        with tempfile.TemporaryDirectory() as td:
            text = pathlib.Path(td) / "text" / "gate.txt"
            data = pathlib.Path(td) / "json" / "gate.json"
            with mock.patch.object(run.judge, "run_gate", return_value={"passed":False}) as gate, \
                    mock.patch.object(run.judge, "format_gate", return_value="negative report\n"):
                rc = run._gate(self.args("--out", text, "--json-out", data), object())
                gate.assert_called_once()
            self.assertEqual(rc, 1)
            self.assertEqual(text.read_text(), "negative report\n")
            self.assertEqual(json.loads(data.read_text()), {"passed":False})

    def test_racing_writer_cannot_be_overwritten_after_preflight(self):
        with tempfile.TemporaryDirectory() as td:
            p = pathlib.Path(td) / "gate.txt"
            original = pathlib.Path.open
            raced = []
            def open_race(path, mode="r", *args, **kwargs):
                if path == p and mode in ("x", "w") and not raced:
                    raced.append(True)
                    with original(p, "w") as f:
                        f.write("other writer")
                return original(path, mode, *args, **kwargs)
            with mock.patch.object(pathlib.Path, "open", open_race), \
                    mock.patch.object(run.judge, "run_gate", return_value={"passed":True}) as gate, \
                    mock.patch.object(run.judge, "format_gate", return_value="replacement"):
                with self.assertRaises((ValueError, FileExistsError)):
                    run._gate(self.args("--out", p), object())
                gate.assert_not_called()
            self.assertEqual(p.read_text(), "other writer")


if __name__ == "__main__":
    unittest.main()
