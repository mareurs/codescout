"""Tests for analyze_study: the frozen driver over synthetic rows. No docker (verify is faked)."""
import contextlib
import io
import json
import shutil
import tempfile
import unittest
from pathlib import Path

import analyze
import analyze_study
import integrity
import run
import test_analyze as ta

S, O = ta.S, ta.O
FOREIGN = ta.TASKS[:6]
PRIVATE = [f"cs-p{i:02d}" for i in range(6)]
MAPPING = {**{t: "foreign" for t in FOREIGN}, **{t: "private" for t in PRIVATE}}


def rows_for(foreign_ratio, private_ratio, models=(S, O)):
    rows = []
    for m in models:
        rows += ta.study(ratios=(foreign_ratio,), spread=1.01, n=6, tasks=FOREIGN, model=m,
                         full_ratios=(foreign_ratio,))
        rows += ta.study(ratios=(private_ratio,), spread=1.01, n=6, tasks=PRIVATE, model=m,
                         full_ratios=(private_ratio,))
    return rows


def report(rows, verify=None, prescan=None):
    return analyze_study.report(rows, MAPPING, registered_min=analyze.REGISTERED_MIN, seed=7,
                                verify_reasons=verify or {}, prescan=prescan or {})


def replace(rows, task, model, arm, **fields):
    out = []
    for r in rows:
        if r["taskId"] == task and r["model"] == model and r["arm"] == arm:
            r = dict(r, **fields)
            if fields.get("outcome") == "invalid-arm":
                r["grade"] = None
        out.append(r)
    return out


class HeadlineTest(unittest.TestCase):
    def test_agreeing_strata_make_the_headline(self):
        out = report(rows_for(0.5, 0.55))
        for m in run.MODELS:
            for comp, effect in (("cs - bare", "codescout"), ("full - bare", "the setup")):
                block = out["models"][m][comp]
                self.assertEqual(block["foreign"]["outcome"], "promising")
                self.assertEqual(block["private"]["outcome"], "promising")
                self.assertEqual(block["headline"], f"{effect}: promising")

    def test_disagreeing_strata_give_no_headline_whatever_the_pooled_says(self):
        out = report(rows_for(0.5, 1.5))
        block = out["models"][S]["cs - bare"]
        self.assertEqual((block["foreign"]["outcome"], block["private"]["outcome"]),
                         ("promising", "worse"))
        self.assertTrue(block["headline"].startswith(f"codescout: {analyze_study.NO_HEADLINE}"))
        self.assertIn("pooled_not_a_headline", block)
        self.assertNotIn(block["pooled_not_a_headline"]["outcome"],
                         block["headline"].split(": ", 1)[1].split(" (")[0])
        # Every key that names a headline is computed from the two strata only.
        self.assertEqual([k for k in block if "headline" in k and k != "headline"],
                         ["pooled_not_a_headline"])

    def test_headline_rule(self):
        d = lambda o: analyze.Decision(o, {}, None, None, None, 0)
        self.assertEqual(analyze_study.headline(d("worse"), d("worse")), "worse")
        self.assertIn("disagree", analyze_study.headline(d("promising"), d("inconclusive")))
        self.assertIn("no registered task", analyze_study.headline(None, d("worse")))

    def test_pilot_only_rows_are_inconclusive_everywhere(self):
        rows = [r for r in rows_for(0.5, 0.55) if r["taskId"] in FOREIGN[:3] + PRIVATE[:3]]
        out = report(rows)
        for m in run.MODELS:
            for comp in ("cs - bare", "full - bare"):
                block = out["models"][m][comp]
                for k in ("foreign", "private", "pooled_not_a_headline"):
                    self.assertEqual(block[k]["outcome"], "inconclusive")
                self.assertTrue(block["headline"].endswith(": inconclusive"))


class InstrumentTest(unittest.TestCase):
    def outcomes(self, out, model, comp):
        b = out["models"][model][comp]
        return {k: b[k]["outcome"] for k in ("foreign", "private", "pooled_not_a_headline")}

    def test_a_verify_or_prescan_reason_invalidates_everything(self):
        for kw in ({"verify": {FOREIGN[0]: ["leak hits: 1"]}},
                   {"prescan": {PRIVATE[0]: ["prescan hits: 2"]}}):
            with self.subTest(kw):
                out = report(rows_for(0.5, 0.55), **kw)
                for m in run.MODELS:
                    for comp in ("cs - bare", "full - bare"):
                        self.assertEqual(set(self.outcomes(out, m, comp).values()),
                                         {"invalid instrument"})
                self.assertTrue(out["instrument"]["study"])

    def test_one_contaminated_run_invalidates_its_model_and_arm_only(self):
        rows = rows_for(0.5, 0.55)
        bad = dict(ta.integ(), ok=False,
                   reasons=[f"{integrity.CONTAMINATION}cs arm has full-only tools in init: Skill"])
        rows = replace(rows, FOREIGN[2], S, "cs", outcome="invalid-arm", integrity=bad)
        out = report(rows)
        self.assertEqual(set(self.outcomes(out, S, "cs - bare").values()), {"invalid instrument"})
        self.assertEqual(self.outcomes(out, S, "full - bare")["foreign"], "promising")
        self.assertEqual(self.outcomes(out, O, "cs - bare")["foreign"], "promising")
        self.assertIn(f"{S} cs", out["instrument"]["arms"])

    def test_invalid_arm_runs_up_to_the_tolerance_only_lower_n_valid(self):
        rows = rows_for(0.5, 0.55)
        for t in (FOREIGN[0], PRIVATE[0]):
            rows = replace(rows, t, S, "cs", outcome="invalid-arm")
        out = report(rows)
        o = self.outcomes(out, S, "cs - bare")
        self.assertEqual(o, {"foreign": "promising", "private": "promising",
                             "pooled_not_a_headline": "promising"})  # 5 of 6, 10 of 12
        self.assertEqual(out["instrument"]["arms"], {})
        rows = replace(rows, FOREIGN[1], S, "cs", outcome="invalid-arm")
        out = report(rows)
        self.assertEqual(set(self.outcomes(out, S, "cs - bare").values()), {"invalid instrument"})
        self.assertIn("3 of 12 tasks have an invalid-arm run", out["instrument"]["arms"][f"{S} cs"][0])
        self.assertEqual(self.outcomes(out, S, "full - bare")["foreign"], "promising")
        # Counted per model: the other model's cs arm is untouched.
        self.assertEqual(list(out["instrument"]["arms"]), [f"{S} cs"])
        self.assertEqual(self.outcomes(out, O, "cs - bare")["foreign"], "promising")

    def test_secondary_and_cap_hits_are_reported(self):
        out = report(rows_for(0.5, 0.55))
        self.assertEqual(out["secondary"][S]["cs"]["n_runs"], 12)
        self.assertEqual(out["cap_hits"][S]["bare"]["runs"], 24)


class PrescanTest(unittest.TestCase):
    def test_prescan_reasons(self):
        tmp = Path(tempfile.mkdtemp(prefix="sab-prescan-"))
        self.addCleanup(shutil.rmtree, tmp, ignore_errors=True)
        good = {"id": PRIVATE[0], "control": "pass", "hits": [], "reasons": []}
        cases = {PRIVATE[0]: good, PRIVATE[1]: dict(good, id=PRIVATE[1], hits=[{"p": 1}]),
                 PRIVATE[2]: dict(good, id=PRIVATE[2], control="FAILED"),
                 PRIVATE[3]: dict(good, id="cs-p99"), PRIVATE[4]: dict(good, id=PRIVATE[4],
                                                                       reasons=["x"])}
        for t, body in cases.items():
            (tmp / t / "prescan").mkdir(parents=True)
            (tmp / t / "prescan" / "prescan.json").write_text(json.dumps(body))
        got = analyze_study.prescan_reasons(MAPPING, tmp)
        self.assertEqual(sorted(got), sorted(PRIVATE[1:]))   # PRIVATE[5] has no file at all
        self.assertIn("unreadable", got[PRIVATE[5]][0])
        self.assertNotIn(FOREIGN[0], got)                     # foreign tasks have no prescan


class MainTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="sab-astudy-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.root = self.tmp / "tasks"
        for t, kind in MAPPING.items():
            (self.root / t / "prescan").mkdir(parents=True)
            (self.root / t / "bundle.json").write_text(json.dumps({"kind": kind}))
            (self.root / t / "prescan" / "prescan.json").write_text(json.dumps(
                {"id": t, "control": "pass", "hits": [], "reasons": []}))
        self.data = self.tmp / "study"
        self.data.mkdir()
        dev = run.make_row(("burntsushi__ripgrep-2576", S, "bare", 0), "s",
                           outcome="dev-scored", costUsd=0.1, grade={"pass": True})
        legacy = {k: v for k, v in dev.items() if k not in run.LATER_ROW_FIELDS}  # Stage 1
        (self.data / "rows.jsonl").write_text("".join(json.dumps(r) + "\n" for r in
                                                      [legacy] + rows_for(0.5, 0.55)))
        (self.tmp / "selection.json").write_text(json.dumps({"selected": list(MAPPING)}))
        self.verified = []

    def verify(self, ids, root, selected=()):
        self.verified.append((list(ids), list(selected)))
        return {t: [] for t in ids}

    def run_main(self, *extra):
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            analyze_study.main(["--data", str(self.data), "--tasks-root", str(self.root),
                                "--selection", str(self.tmp / "selection.json"), *extra],
                               verify=self.verify)
        return json.loads(buf.getvalue())

    def test_main_builds_the_mapping_from_the_bundles_and_runs_every_gate(self):
        out = self.run_main("--seed", "7")
        self.assertEqual(out["registered_tasks"], MAPPING)
        self.assertEqual(out["registered_min"], 10)
        self.assertEqual(self.verified, [(list(MAPPING), list(MAPPING))])
        self.assertEqual(out["models"][S]["cs - bare"]["headline"], "codescout: promising")

    def write_reg(self, body, sign=True):
        path = self.tmp / "reg.json"
        reg = dict(body, registration_sha256=run.registration_digest(body)) if sign else body
        path.write_text(json.dumps(reg))
        return path

    def test_a_registration_must_match_the_bundles(self):
        reg = {"registered_tasks": MAPPING, "registered_min": 10, "bootstrap_seed": 7}
        self.assertEqual(self.run_main("--registration", str(self.write_reg(reg)))["seed"], 7)
        path = self.write_reg(dict(reg, registered_tasks=dict(MAPPING, **{FOREIGN[0]: "private"})))
        with self.assertRaisesRegex(SystemExit, "kinds"):
            self.run_main("--registration", str(path))
        path = self.write_reg(dict(reg, registered_min=12))
        with self.assertRaisesRegex(SystemExit, "registered_min"):
            self.run_main("--registration", str(path))

    def test_an_edited_registration_is_refused(self):
        # Fix round 2 (N-3): the bootstrap seed is read from the registration, so an edit after
        # the data is seen must not pass.
        reg = {"registered_tasks": MAPPING, "registered_min": 10, "bootstrap_seed": 7}
        path = self.write_reg(reg)
        signed = json.loads(path.read_text())
        path.write_text(json.dumps(dict(signed, bootstrap_seed=8)))
        with self.assertRaisesRegex(SystemExit, "does not match its content"):
            self.run_main("--registration", str(path))
        with self.assertRaisesRegex(SystemExit, "does not match its content"):
            self.run_main("--registration", str(self.write_reg(reg, sign=False)))


if __name__ == "__main__":
    unittest.main()
