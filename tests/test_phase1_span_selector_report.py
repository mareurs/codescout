"""Regression coverage for scripts/phase1-span-selector.py's Score A completeness checks.

`report_corpus` scores whatever rows file `--report` is handed, so it must refuse (exit 2)
any file that is not the whole corpus sweep. Two holes, both found by the Codex review of
2026-09-24 and both exiting 0 on an undeclared subset:

- a text with only some of its 22 rules judged scored as a complete text
  (docs/issues/2026-09-24-codex-phase1-partial-sweep-scoring.md, fixed 0fef5562);
- whole texts absent from the file were never noticed, since the rule check only sees texts
  that have rows (docs/issues/2026-09-24-codex-phase1-missing-case-groups.md).

Fixtures are the committed S0 form-2b sweep, so no model call is made. Each case breaks the
file in exactly one way that every OTHER check admits, so it exercises only the check it names.
"""

import contextlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/phase1-span-selector.py"
SPEC = importlib.util.spec_from_file_location("phase1_span_selector", SCRIPT)
sel = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = sel
SPEC.loader.exec_module(sel)

# LOAD-BEARING: the complete committed sweep (42 texts x 22 rules, 0 errored). The control
# test asserts it scores clean; if this file were ever a subset, every "exit 2" below would
# pass for the wrong reason.
CORPUS = ROOT / "docs/evals/data/2026-09-24-rule-tell/p1s-S0b-corpus.jsonl"


def load_rows():
    return [json.loads(line) for line in CORPUS.read_text().splitlines() if line.strip()]


def report(rows):
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        code = sel.report_corpus(rows)
    return code, out.getvalue()


class ScoreACompleteness(unittest.TestCase):
    def setUp(self):
        self.rows = load_rows()
        self.first = (self.rows[0]["case"], self.rows[0]["side"])

    def test_the_complete_committed_sweep_scores_clean(self):
        code, out = report(self.rows)
        self.assertEqual(code, 0, out)
        self.assertIn("42 of 42 texts", out)
        self.assertNotIn("⚠", out)

    def test_a_whole_text_missing_is_refused_by_name(self):
        rows = [r for r in self.rows if (r["case"], r["side"]) != self.first]
        code, out = report(rows)
        self.assertEqual(code, 2, out)
        self.assertIn(f"MISSING {self.first}", out)
        self.assertIn("41 of 42 texts", out)

    def test_an_empty_file_is_refused(self):
        code, out = report([])
        self.assertEqual(code, 2, out)
        self.assertIn("42 missing", out)

    def test_one_rule_missing_from_a_present_text_is_refused_by_name(self):
        drop = next(i for i, r in enumerate(self.rows) if (r["case"], r["side"]) == self.first)
        rows = self.rows[:drop] + self.rows[drop + 1:]
        code, out = report(rows)
        self.assertEqual(code, 2, out)
        self.assertIn(f"INCOMPLETE {self.first}", out)
        self.assertIn("0 missing", out)

    def test_a_text_not_in_the_corpus_is_refused_by_name(self):
        # A full 22-rule copy under an unknown id, so the rule check admits it.
        extra = [dict(r, case="NOT-A-CASE") for r in self.rows
                 if (r["case"], r["side"]) == self.first]
        code, out = report(self.rows + extra)
        self.assertEqual(code, 2, out)
        self.assertIn(f"UNEXPECTED ('NOT-A-CASE', '{self.first[1]}')", out)
        self.assertIn("0 incomplete", out)


if __name__ == "__main__":
    unittest.main()
