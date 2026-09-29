"""Task 3 of the labelled sample: the operator's labelling tool (scripts/measure/label.py).

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_label.py -v

Everything here is SYNTHETIC (temp set directories); nothing reads a real corpus or packet.
`summary` and `verify` are the only subcommands an agent may run, so their output is a
blindness boundary: every "X is absent" assertion below has a positive control showing X IS on
disk (labels.jsonl / the packet) so the absence cannot be the silence of a dead fixture.
"""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
import pathlib
import random
import sys
import tempfile
import unittest
from unittest import mock
from datetime import datetime, timedelta, timezone

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
MEASURE = REPO_ROOT / "scripts" / "measure"
sys.path.insert(0, str(MEASURE))


def _load(name):
    spec = importlib.util.spec_from_file_location(f"measure_{name}", MEASURE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


label = _load("label")

NOTE_SENTINEL = "NOTE-SENTINEL-7731"
PACKET_SENTINEL = "PACKET-SENTINEL-5520"
NOW = datetime(2026, 9, 29, 12, 0, 0, tzinfo=timezone.utc)


def _sha(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def _iso(dt):
    return dt.strftime("%Y-%m-%dT%H:%M:%SZ")


def make_set(root, ids, seed=7, name="set"):
    """A synthetic label-set directory: draw.json, packets/<id>.md. Returns the set dir."""
    d = pathlib.Path(root) / name
    (d / "packets").mkdir(parents=True)
    cases = []
    for cid in ids:
        text = f"{PACKET_SENTINEL} body of {cid}\n"
        (d / "packets" / f"{cid}.md").write_text(text, encoding="utf-8")
        cases.append({"case_id": cid, "sha256": _sha(text)})
    draw = {"set_id": "synthetic", "seed": seed, "cases": cases, "order": list(ids)}
    (d / "draw.json").write_text(json.dumps(draw), encoding="utf-8")
    return d


def record(cid, packet_sha, labels, delivery, seconds=10.0, note="", recall="n", when=NOW):
    # every field a real record carries (fixture must look like the real thing)
    return {
        "case_id": cid, "packet_sha256": packet_sha, "labels": labels, "delivery": delivery,
        "note": note, "recall": recall, "seconds": seconds, "labelled_at": _iso(when),
    }


def write_jsonl(path, recs):
    with open(path, "w", encoding="utf-8") as f:
        for r in recs:
            f.write(json.dumps(r) + "\n")


def read_jsonl(path):
    return [json.loads(l) for l in pathlib.Path(path).read_text(encoding="utf-8").splitlines() if l]


def sha_of(set_dir, cid):
    for c in json.loads((pathlib.Path(set_dir) / "draw.json").read_text())["cases"]:
        if c["case_id"] == cid:
            return c["sha256"]
    raise KeyError(cid)


def scripted(answers):
    it = iter(answers)
    prompts = []

    def ask(prompt):
        prompts.append(prompt)
        return next(it)

    ask.prompts = prompts
    return ask


class Clock:
    def __init__(self, *ticks):
        self.ticks = list(ticks)

    def __call__(self):
        return self.ticks.pop(0)


class Base(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = pathlib.Path(self._tmp.name)


class ValidateTests(unittest.TestCase):
    def test_validate_enforces_label_delivery_pairing(self):
        ok = [
            (["verify"], "quiet"), (["verify"], "interrupt"),
            (["qualify"], "quiet"), (["qualify"], "interrupt"),
            (["correct"], "quiet"), (["correct"], "interrupt"),
            (["verify", "qualify"], "quiet"), (["verify", "qualify", "correct"], "interrupt"),
            (["none"], "silent"), (["unresolved"], "silent"),
        ]
        for labels, delivery in ok:
            label.validate(labels, delivery)  # must not raise
        bad = [
            (["verify"], "silent"), (["qualify"], "silent"), (["correct"], "silent"),
            (["verify", "correct"], "silent"),
            (["none"], "quiet"), (["none"], "interrupt"),
            (["unresolved"], "quiet"), (["unresolved"], "interrupt"),
            (["none", "verify"], "quiet"), (["none", "verify"], "silent"),
            (["unresolved", "none"], "silent"), (["verify", "unresolved"], "quiet"),
            (["verify", "verify"], "quiet"),
            ([], "silent"), ([], "quiet"),
            (["bogus"], "quiet"), (["verify"], "loud"), (["none"], "loud"),
        ]
        for labels, delivery in bad:
            with self.assertRaises(ValueError, msg=f"{labels} {delivery}"):
                label.validate(labels, delivery)


class LabelOneTests(unittest.TestCase):
    def test_label_one_reasks_on_invalid_input_and_records_seconds(self):
        shown = []
        # labels: 'z' invalid, then 'n'+'v' mixed invalid, then 'cv' valid (two labels, order
        # delivery: 'w' invalid then 'i'; recall: 'maybe' invalid then 'y'.
        ask = scripted(["z", "nv", "cv", "w", "i", "maybe", "y", "my note"])
        rec = label.label_one("c1", "abc123", "PKT", ask, shown.append, Clock(100.0, 137.5))
        self.assertEqual(shown, ["PKT"])
        self.assertEqual(rec["labels"], ["verify", "correct"])
        self.assertEqual(rec["delivery"], "interrupt")
        self.assertEqual(rec["recall"], "y")
        self.assertEqual(rec["note"], "my note")
        self.assertEqual(rec["seconds"], 37.5)
        self.assertEqual(rec["case_id"], "c1")
        self.assertEqual(rec["packet_sha256"], "abc123")
        self.assertEqual(rec["labelled_at"], "1970-01-01T00:02:17Z")
        self.assertEqual(
            sorted(rec),
            ["case_id", "delivery", "labelled_at", "labels", "note", "packet_sha256", "recall",
             "seconds"],
        )
        # 3 label prompts + 2 delivery prompts + 2 recall prompts + 1 note prompt
        self.assertEqual(len(ask.prompts), 8)

    def test_label_one_none_and_unresolved_letters(self):
        r = label.label_one("c", "h", "P", scripted(["n", "s", "n", ""]), lambda t: None, Clock(0.0, 1.0))
        self.assertEqual((r["labels"], r["delivery"], r["recall"], r["note"]), (["none"], "silent", "n", ""))
        r = label.label_one("c", "h", "P", scripted(["u", "s", "y", ""]), lambda t: None, Clock(0.0, 1.0))
        self.assertEqual((r["labels"], r["recall"]), (["unresolved"], "y"))

    def test_label_one_reasks_from_the_top_when_the_pairing_is_forbidden(self):
        # 'v' + 's' is a forbidden pair: the answers are discarded and asked again
        ask = scripted(["v", "s", "q", "q", "n", ""])
        r = label.label_one("c", "h", "P", ask, lambda t: None, Clock(0.0, 1.0))
        self.assertEqual((r["labels"], r["delivery"]), (["qualify"], "quiet"))

    def test_quit_letter_raises(self):
        with self.assertRaises(label.Quit):
            label.label_one("c", "h", "P", scripted(["x"]), lambda t: None, Clock(0.0, 1.0))


class NextLoopTests(Base):
    def _answers_for(self, n):
        # every case: verify / quiet / recall n / note ''
        return ["v", "q", "n", ""] * n

    def test_next_resumes_after_labelled_cases(self):
        d = make_set(self.root, ["a", "b", "c", "d"])
        shown = []
        # first session: label a, b then quit on c
        ask = scripted(self._answers_for(2) + ["x"])
        clock = Clock(*[float(i) for i in range(100)])
        out = label.run_next(d, False, ask, shown.append, clock)
        self.assertEqual(out["labelled"], 2)
        self.assertEqual([r["case_id"] for r in read_jsonl(d / "labels.jsonl")], ["a", "b"])
        self.assertEqual(len(shown), 3)  # a, b, and c (quit while c was on screen)
        # second session: resumes at c, skips a and b
        shown2 = []
        out = label.run_next(d, False, scripted(self._answers_for(2)), shown2.append, Clock(*[float(i) for i in range(100)]))
        self.assertEqual(out["labelled"], 2)
        self.assertEqual([r["case_id"] for r in read_jsonl(d / "labels.jsonl")], ["a", "b", "c", "d"])
        self.assertEqual(len(shown2), 2)
        self.assertIn("body of c", shown2[0])
        self.assertIn("body of d", shown2[1])
        # third session: nothing left
        shown3 = []
        out = label.run_next(d, False, scripted([]), shown3.append, Clock())
        self.assertEqual((out["labelled"], shown3), (0, []))

    def test_records_carry_the_draw_hash_and_are_appended_before_the_next_packet(self):
        d = make_set(self.root, ["a", "b"])
        seen_on_disk = []

        def show(text):
            p = d / "labels.jsonl"
            seen_on_disk.append(len(read_jsonl(p)) if p.exists() else 0)

        label.run_next(d, False, scripted(self._answers_for(2)), show, Clock(*[float(i) for i in range(20)]))
        # when b is shown, a's record is already on disk (a crash loses at most the case in progress)
        self.assertEqual(seen_on_disk, [0, 1])
        recs = read_jsonl(d / "labels.jsonl")
        self.assertEqual(recs[0]["packet_sha256"], sha_of(d, "a"))
        self.assertEqual(recs[1]["packet_sha256"], sha_of(d, "b"))

    def test_a_crash_mid_case_loses_only_that_case(self):
        d = make_set(self.root, ["a", "b"])
        ask = scripted(self._answers_for(1) + ["v"])  # then the iterator is exhausted mid-case b

        with self.assertRaises(StopIteration):
            label.run_next(d, False, ask, lambda t: None, Clock(*[float(i) for i in range(20)]))
        self.assertEqual([r["case_id"] for r in read_jsonl(d / "labels.jsonl")], ["a"])

    def test_labels_jsonl_is_append_only(self):
        d = make_set(self.root, ["a", "b"])
        old = record("a", sha_of(d, "a"), ["none"], "silent")
        write_jsonl(d / "labels.jsonl", [old])
        before = (d / "labels.jsonl").read_bytes()
        label.run_next(d, False, scripted(self._answers_for(1)), lambda t: None, Clock(1.0, 2.0))
        after = (d / "labels.jsonl").read_bytes()
        self.assertTrue(after.startswith(before))
        self.assertGreater(len(after), len(before))

    def test_an_edited_packet_is_refused_and_never_shown(self):
        d = make_set(self.root, ["a", "b"])
        (d / "packets" / "a.md").write_text("EDITED-PACKET-TEXT\n", encoding="utf-8")
        shown = []
        out = label.run_next(d, False, scripted(self._answers_for(1)), shown.append, Clock(1.0, 2.0))
        self.assertEqual(out["refused"], ["a"])
        self.assertEqual(out["labelled"], 1)
        self.assertFalse(any("EDITED-PACKET-TEXT" in s for s in shown))
        # positive control: the untouched packet WAS shown
        self.assertEqual(len(shown), 1)
        self.assertIn("body of b", shown[0])
        self.assertEqual([r["case_id"] for r in read_jsonl(d / "labels.jsonl")], ["b"])

    def test_writing_a_set_inside_the_repo_is_refused(self):
        d = make_set(self.root, ["a"])
        label.archive.REPO_ROOT = self.root  # the temp tree stands in for the repo
        self.addCleanup(setattr, label.archive, "REPO_ROOT", REPO_ROOT)
        with self.assertRaises(ValueError):
            label.run_next(d, False, scripted(self._answers_for(1)), lambda t: None, Clock(1.0, 2.0))
        self.assertFalse((d / "labels.jsonl").exists())

    def test_relabel_iterates_only_relabel_ids_and_writes_relabels_jsonl(self):
        ids = [f"c{i:02d}" for i in range(12)]
        d = make_set(self.root, ids)
        old = NOW - timedelta(days=5)
        write_jsonl(d / "labels.jsonl", [record(c, sha_of(d, c), ["none"], "silent", when=old) for c in ids])
        before = (d / "labels.jsonl").read_bytes()
        wanted = label.relabel_ids(d, 7, now=NOW)
        self.assertEqual(len(wanted), 10)
        shown = []
        out = label.run_next(d, True, scripted(self._answers_for(10)), shown.append,
                             Clock(*[float(i) for i in range(50)]), now=NOW)
        self.assertEqual(out["labelled"], 10)
        self.assertEqual(len(shown), 10)
        recs = read_jsonl(d / "relabels.jsonl")
        self.assertEqual(sorted(r["case_id"] for r in recs), sorted(wanted))
        self.assertEqual((d / "labels.jsonl").read_bytes(), before)  # first-pass file untouched
        # resume: relabelled ids are skipped in the relabel file
        out = label.run_next(d, True, scripted([]), lambda t: None, Clock(), now=NOW)
        self.assertEqual(out["labelled"], 0)


def _cli(argv, **kw):
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        rc = label.main(argv, **kw)
    return rc, buf.getvalue()


class SummaryTests(Base):
    def _labelled_set(self):
        d = make_set(self.root, ["case-SECRET-1", "case-SECRET-2", "case-SECRET-3", "case-SECRET-4", "case-SECRET-5"])
        recs = [
            record("case-SECRET-1", sha_of(d, "case-SECRET-1"), ["verify", "correct"], "interrupt", seconds=10.0, note=NOTE_SENTINEL, recall="y"),
            record("case-SECRET-2", sha_of(d, "case-SECRET-2"), ["qualify"], "quiet", seconds=20.0),
            record("case-SECRET-3", sha_of(d, "case-SECRET-3"), ["none"], "silent", seconds=30.0),
            record("case-SECRET-4", sha_of(d, "case-SECRET-4"), ["unresolved"], "silent", seconds=100.0, recall="y"),
        ]
        write_jsonl(d / "labels.jsonl", recs)
        return d

    def test_summary_counts(self):
        d = self._labelled_set()
        s = label.summary(d)
        self.assertEqual(s["cases"], 5)
        self.assertEqual(s["labelled"], 4)
        self.assertEqual(s["labels"], {"verify": 1, "qualify": 1, "correct": 1, "none": 1, "unresolved": 1})
        self.assertEqual(s["delivery"], {"silent": 2, "quiet": 1, "interrupt": 1})
        self.assertEqual(s["unresolved"], 1)
        self.assertEqual(s["recall"], 2)
        self.assertEqual(s["median_seconds"], 25.0)  # mean would be 40.0; even count averages the middle two

    def test_summary_contains_no_note_text_and_no_per_case_labels(self):
        d = self._labelled_set()
        # positive controls: the sentinels ARE on disk, so their absence below means something
        self.assertIn(NOTE_SENTINEL, (d / "labels.jsonl").read_text())
        self.assertIn("case-SECRET-1", (d / "labels.jsonl").read_text())
        self.assertIn(PACKET_SENTINEL, (d / "packets" / "case-SECRET-1.md").read_text())
        blob = json.dumps(label.summary(d))
        for leak in (NOTE_SENTINEL, PACKET_SENTINEL, "case-SECRET", "SECRET", "7731"):
            self.assertNotIn(leak, blob)
        rc, out = _cli(["summary", str(d)])
        self.assertEqual(rc, 0)
        self.assertIn('"labelled": 4', out)  # positive control: the CLI printed the summary
        for leak in (NOTE_SENTINEL, PACKET_SENTINEL, "case-SECRET"):
            self.assertNotIn(leak, out)

    def test_summary_of_an_unlabelled_set(self):
        d = make_set(self.root, ["a", "b"])
        s = label.summary(d)
        self.assertEqual((s["cases"], s["labelled"], s["unresolved"], s["recall"]), (2, 0, 0, 0))
        self.assertIsNone(s["median_seconds"])


class VerifyTests(Base):
    def test_verify_clean(self):
        d = make_set(self.root, ["a", "b"])
        write_jsonl(d / "labels.jsonl", [record("a", sha_of(d, "a"), ["none"], "silent")])
        self.assertEqual(label.verify(d), {"ok": 2, "mismatch": []})

    def test_verify_flags_an_edited_packet(self):
        d = make_set(self.root, ["a", "b", "c"])
        (d / "packets" / "b.md").write_text("tampered\n", encoding="utf-8")
        v = label.verify(d)
        self.assertEqual(v, {"ok": 2, "mismatch": ["b"]})

    def test_verify_flags_an_edited_draw_hash(self):
        d = make_set(self.root, ["a", "b"])
        draw = json.loads((d / "draw.json").read_text())
        draw["cases"][0]["sha256"] = "0" * 64
        (d / "draw.json").write_text(json.dumps(draw))
        self.assertEqual(label.verify(d), {"ok": 1, "mismatch": ["a"]})

    def test_verify_flags_a_label_bound_to_a_different_packet_hash(self):
        d = make_set(self.root, ["a", "b"])
        write_jsonl(d / "labels.jsonl", [record("b", "f" * 64, ["none"], "silent")])
        self.assertEqual(label.verify(d), {"ok": 1, "mismatch": ["b"]})

    def test_verify_flags_a_missing_packet(self):
        d = make_set(self.root, ["a", "b"])
        (d / "packets" / "a.md").unlink()
        self.assertEqual(label.verify(d), {"ok": 1, "mismatch": ["a"]})

    def test_verify_output_names_no_label_text_or_packet_text(self):
        d = make_set(self.root, ["case-SECRET-1", "case-SECRET-2"])
        write_jsonl(d / "labels.jsonl", [
            record("case-SECRET-1", sha_of(d, "case-SECRET-1"), ["verify"], "quiet", note=NOTE_SENTINEL),
            record("case-SECRET-2", sha_of(d, "case-SECRET-2"), ["correct"], "interrupt"),
        ])
        (d / "packets" / "case-SECRET-2.md").write_text(f"{PACKET_SENTINEL} edited\n", encoding="utf-8")
        blob = json.dumps(label.verify(d))
        rc, out = _cli(["verify", str(d)])
        # positive control: the one mismatching id IS reported, and it is the only id
        self.assertIn("case-SECRET-2", blob)
        self.assertIn("case-SECRET-2", out)
        for text in (blob, out):
            self.assertNotIn("case-SECRET-1", text)
            for leak in (NOTE_SENTINEL, PACKET_SENTINEL, "verify\"", "correct", "interrupt", "quiet"):
                self.assertNotIn(leak, text)
        self.assertNotEqual(rc, 0)  # a mismatch is a non-zero exit


class RelabelIdsTests(Base):
    def _set_with_ages(self, ages, name="set"):
        ids = [f"c{i:02d}" for i in range(len(ages))]
        d = make_set(self.root, ids, name=name)
        write_jsonl(d / "labels.jsonl", [
            record(c, sha_of(d, c), ["none"], "silent", when=NOW - a) for c, a in zip(ids, ages)
        ])
        return d, ids

    def test_relabel_ids_only_picks_cases_at_least_three_days_old(self):
        ages = [
            timedelta(days=3),                                  # exactly 3 days: eligible
            timedelta(days=2, hours=23, minutes=59, seconds=59),  # ONE SECOND short: not eligible
            timedelta(days=10),                                 # eligible
            timedelta(hours=1),                                 # not eligible
        ]
        d, ids = self._set_with_ages(ages)
        got = label.relabel_ids(d, 1, now=NOW)
        self.assertEqual(sorted(got), ["c00", "c02"])

    def test_relabel_ids_caps_at_n_and_takes_all_when_fewer(self):
        d, ids = self._set_with_ages([timedelta(days=9)] * 11)
        got = label.relabel_ids(d, 3, now=NOW)
        self.assertEqual(len(got), 10)  # 11 eligible, n=10
        self.assertEqual(len(set(got)), 10)
        self.assertTrue(set(got) <= set(ids))
        self.assertEqual(len(label.relabel_ids(d, 3, n=5, now=NOW)), 5)
        d2, ids2 = self._set_with_ages([timedelta(days=9)] * 10, name="s10")
        self.assertEqual(sorted(label.relabel_ids(d2, 3, now=NOW)), sorted(ids2))  # exactly 10: all
        d3, ids3 = self._set_with_ages([timedelta(days=9)] * 4, name="s4")
        self.assertEqual(sorted(label.relabel_ids(d3, 3, now=NOW)), sorted(ids3))  # fewer than n: all

    def test_relabel_ids_is_deterministic_in_the_seed_and_seed_dependent(self):
        d, ids = self._set_with_ages([timedelta(days=9)] * 30)
        a = label.relabel_ids(d, 42, now=NOW)
        self.assertEqual(a, label.relabel_ids(d, 42, now=NOW))
        others = {tuple(label.relabel_ids(d, s, now=NOW)) for s in range(1, 6)}
        self.assertGreater(len(others | {tuple(a)}), 1)  # not seed-blind

    def test_relabel_ids_ignores_relabels_already_written(self):
        # resume stability: writing relabels.jsonl must not change which ids are picked
        d, ids = self._set_with_ages([timedelta(days=9)] * 30)
        before = label.relabel_ids(d, 5, now=NOW)
        write_jsonl(d / "relabels.jsonl", [record(before[0], sha_of(d, before[0]), ["none"], "silent")])
        self.assertEqual(label.relabel_ids(d, 5, now=NOW), before)


class CliTests(Base):
    def test_cli_next_uses_injected_io_and_appends(self):
        d = make_set(self.root, ["a"])
        shown = []
        rc, out = _cli(["next", str(d)], ask=scripted(["n", "s", "n", ""]), show=shown.append,
                       clock=Clock(1.0, 2.0))
        self.assertEqual(rc, 0)
        self.assertEqual(len(shown), 1)
        self.assertIn(PACKET_SENTINEL, shown[0])  # positive control: the packet WAS shown, to show()
        self.assertEqual(len(read_jsonl(d / "labels.jsonl")), 1)
        self.assertNotIn(PACKET_SENTINEL, out)  # packet text goes only through show()

    def test_cli_rejects_an_unknown_subcommand(self):
        with self.assertRaises(SystemExit):
            _cli(["frobnicate", "/nonexistent"])


class _Tty(io.StringIO):
    def isatty(self):
        return True


class TtyGuardTests(Base):
    def _refused(self, stdin):
        d = make_set(self.root, ["a"])
        err = io.StringIO()
        with contextlib.redirect_stderr(err), mock.patch.object(sys, "stdin", stdin):
            rc, out = _cli(["next", str(d)])
        return d, rc, out, err.getvalue()

    def test_next_without_a_terminal_refuses_and_prints_no_packet(self):
        d, rc, out, err = self._refused(io.StringIO(""))
        self.assertEqual(rc, 1)
        self.assertIn("interactive terminal", err)  # positive control: the refusal speaks
        self.assertNotIn(PACKET_SENTINEL, out + err)
        self.assertFalse((d / "labels.jsonl").exists())

    def test_next_needs_both_stdin_and_stdout_to_be_terminals(self):
        # stdin IS a tty, stdout (captured by _cli) is not: still refused, nothing printed
        d, rc, out, err = self._refused(_Tty(""))
        self.assertEqual(rc, 1)
        self.assertNotIn(PACKET_SENTINEL, out + err)

    def test_the_same_set_labels_normally_with_injected_io(self):
        d = make_set(self.root, ["a"])
        shown = []
        rc, out = _cli(["next", str(d)], ask=scripted(["n", "s", "n", ""]), show=shown.append,
                       clock=Clock(1.0, 2.0))
        self.assertEqual(rc, 0)
        self.assertIn(PACKET_SENTINEL, shown[0])


class PagerTests(unittest.TestCase):
    def test_empty_or_missing_pager_falls_back_to_print(self):
        for value in ("", "/nonexistent/pager-binary"):
            buf = io.StringIO()
            with mock.patch.dict(os.environ, {"PAGER": value}), contextlib.redirect_stdout(buf):
                label._pager_show("PAGER-FALLBACK-TEXT")
            self.assertIn("PAGER-FALLBACK-TEXT", buf.getvalue(), msg=repr(value))


class QuitLetterTests(Base):
    def test_x_quits_at_the_delivery_prompt(self):
        with self.assertRaises(label.Quit):
            label.label_one("c", "h", "P", scripted(["v", "x"]), lambda t: None, Clock(0.0, 1.0))

    def test_x_quits_at_the_recall_prompt(self):
        with self.assertRaises(label.Quit):
            label.label_one("c", "h", "P", scripted(["v", "q", "x"]), lambda t: None, Clock(0.0, 1.0))

    def test_x_at_delivery_or_recall_writes_nothing(self):
        for answers in (["v", "x"], ["v", "q", "x"]):
            d = make_set(self.root, ["a"], name="s" + str(len(answers)))
            out = label.run_next(d, False, scripted(answers), lambda t: None, Clock(0.0, 1.0))
            self.assertEqual(out["labelled"], 0)
            self.assertFalse((d / "labels.jsonl").exists())

    def test_a_note_of_x_is_a_note_not_a_quit(self):
        r = label.label_one("c", "h", "P", scripted(["n", "s", "n", "x"]), lambda t: None, Clock(0.0, 1.0))
        self.assertEqual(r["note"], "x")


class ParseVariantTests(unittest.TestCase):
    def test_label_parsing_variants(self):
        p = label._parse_labels
        self.assertEqual(p("V"), ["verify"])                       # uppercase
        self.assertEqual(p("N"), ["none"])
        self.assertEqual(p("U"), ["unresolved"])
        self.assertEqual(p("v, q"), ["verify", "qualify"])         # spaces and commas
        self.assertEqual(p(" c  v "), ["verify", "correct"])
        self.assertEqual(p("vv"), ["verify"])                      # duplicates dedup
        self.assertIsNone(p("vn"))                                 # mixed forms rejected
        self.assertIsNone(p("nq"))
        self.assertIsNone(p("nu"))
        self.assertIsNone(p("nn"))                                 # pinned: rejected, not deduped
        self.assertIsNone(p(""))
        self.assertIsNone(p("  , "))

    def test_delivery_and_recall_are_case_insensitive(self):
        r = label.label_one("c", "h", "P", scripted(["V", "I", "Y", ""]), lambda t: None, Clock(0.0, 1.0))
        self.assertEqual((r["labels"], r["delivery"], r["recall"]), (["verify"], "interrupt", "y"))


class FsyncTests(Base):
    def test_each_record_is_fsynced_before_the_next_packet_is_shown(self):
        d = make_set(self.root, ["a", "b"])
        events = []
        real = os.fsync

        def fake_fsync(fd):
            events.append("fsync")
            return real(fd)

        with mock.patch.object(label.os, "fsync", fake_fsync):
            label.run_next(d, False, scripted(["n", "s", "n", ""] * 2), lambda t: events.append("show"),
                           Clock(*[float(i) for i in range(10)]))
        self.assertEqual(events, ["show", "fsync", "show", "fsync"])


class ReadOnceTests(Base):
    def test_the_packet_is_read_once_and_the_same_bytes_are_hashed_and_shown(self):
        d = make_set(self.root, ["a"])
        calls = []
        orig_b, orig_t = pathlib.Path.read_bytes, pathlib.Path.read_text

        def rb(self, *a, **k):
            calls.append(self.name)
            return orig_b(self, *a, **k)

        def rt(self, *a, **k):
            calls.append(self.name)
            return orig_t(self, *a, **k)

        with mock.patch.object(pathlib.Path, "read_bytes", rb), mock.patch.object(pathlib.Path, "read_text", rt):
            label.run_next(d, False, scripted(["n", "s", "n", ""]), lambda t: None, Clock(0.0, 1.0))
        self.assertEqual(calls.count("a.md"), 1)
        self.assertIn("draw.json", calls)  # positive control: the counter sees reads


class DrawValidationTests(Base):
    def test_order_naming_an_unknown_case_is_a_clear_error(self):
        d = make_set(self.root, ["a"])
        draw = json.loads((d / "draw.json").read_text())
        draw["order"] = ["a", "ghost-id"]
        (d / "draw.json").write_text(json.dumps(draw))
        for fn in (lambda: label.run_next(d, False, scripted([]), lambda t: None, Clock()),
                   lambda: label.summary(d), lambda: label.verify(d)):
            with self.assertRaises(ValueError) as cm:
                fn()
            self.assertIn("ghost-id", str(cm.exception))


class TornLineTests(Base):
    def _base(self):
        d = make_set(self.root, ["a", "b"])
        good = json.dumps(record("a", sha_of(d, "a"), ["none"], "silent"))
        return d, good

    def test_torn_final_unterminated_line_is_skipped_with_a_warning_and_repaired_on_append(self):
        d, good = self._base()
        (d / "labels.jsonl").write_text(good + "\n" + '{"case_id": "b", "packet_sh', encoding="utf-8")
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.assertEqual(label.summary(d)["labelled"], 1)
        self.assertIn("torn", err.getvalue())  # positive control: a warning is emitted
        shown = []
        with contextlib.redirect_stderr(io.StringIO()):
            out = label.run_next(d, False, scripted(["n", "s", "n", ""]), shown.append, Clock(1.0, 2.0))
        self.assertEqual(out["labelled"], 1)   # b was re-shown
        self.assertEqual(len(shown), 1)
        self.assertEqual([r["case_id"] for r in read_jsonl(d / "labels.jsonl")], ["a", "b"])  # file parses

    def test_unterminated_but_valid_final_line_is_kept(self):
        d, good = self._base()
        (d / "labels.jsonl").write_text(good, encoding="utf-8")  # valid JSON, no newline
        shown = []
        label.run_next(d, False, scripted(["n", "s", "n", ""]), shown.append, Clock(1.0, 2.0))
        self.assertEqual(len(shown), 1)  # only b shown; a counted
        self.assertEqual([r["case_id"] for r in read_jsonl(d / "labels.jsonl")], ["a", "b"])

    def test_corrupt_non_final_line_raises(self):
        d, good = self._base()
        (d / "labels.jsonl").write_text('{"bad\n' + good + "\n", encoding="utf-8")
        with self.assertRaises(ValueError):
            label.summary(d)

    def test_corrupt_final_line_that_is_newline_terminated_raises(self):
        d, good = self._base()
        (d / "labels.jsonl").write_text(good + "\n" + '{"bad\n', encoding="utf-8")
        with self.assertRaises(ValueError):
            label.summary(d)


class RelabelPickTests(Base):
    IDS = [f"c{i:02d}" for i in range(15)]
    FIRST_TEN = IDS[:10]

    def _set(self, old, young=(), name="set"):
        """`old` ids labelled 5 days before NOW, `young` ids 1 day before NOW."""
        d = make_set(self.root, self.IDS, name=name)
        recs = [record(c, sha_of(d, c), ["none"], "silent", when=NOW - timedelta(days=5)) for c in old]
        recs += [record(c, sha_of(d, c), ["none"], "silent", when=NOW - timedelta(days=1)) for c in young]
        write_jsonl(d / "labels.jsonl", recs)
        return d

    def _ans(self, n):
        return ["n", "s", "n", ""] * n

    def test_the_pick_is_persisted_and_reused_across_resumes(self):
        d = self._set(self.FIRST_TEN, young=self.IDS[10:])
        clock = Clock(*[float(i) for i in range(100)])
        out = label.run_next(d, True, scripted(self._ans(3) + ["x"]), lambda t: None, clock, now=NOW)
        self.assertEqual(out["labelled"], 3)
        pick = json.loads((d / "relabel_pick.json").read_text())
        self.assertEqual(pick["ids"], self.FIRST_TEN)  # hard-coded: exactly 10 eligible at session 1
        self.assertEqual((pick["seed"], pick["n"], pick["min_days"], pick["now"]), (7, 10, 3, "2026-09-29T12:00:00Z"))
        # time passes: the five young cases become eligible, so a fresh draw would differ
        later = NOW + timedelta(days=5)
        self.assertNotEqual(label.relabel_ids(d, 7, now=later), self.FIRST_TEN)  # control: re-sampling WOULD differ
        out = label.run_next(d, True, scripted(self._ans(7)), lambda t: None,
                             Clock(*[float(i) for i in range(100)]), now=later)
        self.assertEqual(out["labelled"], 7)
        ids = [r["case_id"] for r in read_jsonl(d / "relabels.jsonl")]
        self.assertEqual(ids, self.FIRST_TEN)

    def test_an_existing_pick_file_is_used_without_consulting_relabel_ids(self):
        d = self._set(self.IDS[:5])  # only 5 eligible: relabel_ids would refuse
        (d / "relabel_pick.json").write_text(json.dumps(
            {"ids": ["c01", "c03"], "seed": 7, "n": 10, "min_days": 3, "now": "2026-09-01T00:00:00Z"}))
        shown = []
        out = label.run_next(d, True, scripted(self._ans(2)), shown.append, Clock(*[float(i) for i in range(10)]), now=NOW)
        self.assertEqual(out["labelled"], 2)
        self.assertEqual([r["case_id"] for r in read_jsonl(d / "relabels.jsonl")], ["c01", "c03"])

    def test_fewer_than_n_eligible_refuses_and_writes_nothing(self):
        d = self._set(self.IDS[:9], young=["c09"])  # 9 eligible, the 10th becomes eligible 2026-10-01T12:00:00Z
        with self.assertRaises(label.RelabelRefused) as cm:
            label.run_next(d, True, scripted([]), lambda t: None, Clock(), now=NOW)
        msg = str(cm.exception)
        self.assertIn("9", msg)
        self.assertIn("2026-10-01T12:00:00Z", msg)
        self.assertFalse((d / "relabel_pick.json").exists())
        self.assertFalse((d / "relabels.jsonl").exists())

    def test_exactly_n_eligible_proceeds(self):
        d = self._set(self.FIRST_TEN, young=self.IDS[10:], name="ten")
        out = label.run_next(d, True, scripted(self._ans(1) + ["x"]), lambda t: None,
                             Clock(*[float(i) for i in range(10)]), now=NOW)
        self.assertEqual(out["labelled"], 1)
        self.assertTrue((d / "relabel_pick.json").exists())

    def test_cli_refusal_is_a_message_not_a_traceback(self):
        d = self._set(self.IDS[:9], young=["c09"], name="cli")
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            rc, out = _cli(["next", str(d), "--relabel"], ask=scripted([]), show=lambda t: None, clock=Clock(), now=NOW)
        self.assertEqual(rc, 1)
        self.assertIn("9", err.getvalue() + out)
        self.assertIn("eligible", err.getvalue() + out)


if __name__ == "__main__":
    unittest.main()
