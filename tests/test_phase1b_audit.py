"""Phase-1b Step 1's scripts: draw_audit_sample.py (menu and draw), label_items.py (blinding),
score_audit.py (answers, union rule, admission, masking, counterexample rows, clean-text stops)
and run_labellers.py (per-call checks, the one re-run, batches, prompts, the clean channel; both
models faked, none called). Standard library only:
    python3 tests/test_phase1b_audit.py
"""
import importlib.util
import json
import pathlib
import random
import re
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
PHASE1B = ROOT / "docs/evals/data/2026-09-24-rule-tell/phase1b"


def load(name):
    spec = importlib.util.spec_from_file_location(name, PHASE1B / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


da = load("draw_audit_sample")
li = load("label_items")
sa = load("score_audit")
rl = load("run_labellers")
gc = load("gen_codex_clean")

MENU = ["a_rule", "b_rule", "c_rule"]


def ans(i, rules=(), unsure=()):
    return {"id": i, "rules": list(rules), "unsure": bool(unsure), "unsure_rules": list(unsure), "reason": "x"}


class Draw(unittest.TestCase):
    def population(self):
        # Row k's text has 1 + k % 5 sentences, so the unit draw has different ranges across rows.
        return [{"id": f"r{k}", "set": "train", "rule": MENU[k % 3], "label": k % 2, "target": 0,
                 "text": " ".join(f"S{k}.{j}." for j in range(1 + k % 5))} for k in range(da.POPULATION)]

    def seg(self, text):
        return text.split(" ")

    def test_the_registered_draw(self):
        rows = self.population()
        got = da.draw(rows, seg=self.seg)
        idx = random.Random(20260936).sample(range(2671), 300)
        self.assertEqual([g["row_id"] for g in got], [rows[j]["id"] for j in idx])
        units = [random.Random(20260936 + i).randrange(len(self.seg(rows[j]["text"]))) for i, j in enumerate(idx)]
        self.assertEqual([g["unit_index"] for g in got], units)
        self.assertEqual([g["item"] for g in got], list(range(300)))
        g = got[0]
        self.assertEqual(g["sentence"], self.seg(g["paragraph"])[g["unit_index"]])
        self.assertEqual(g["is_target"], g["unit_index"] == 0)

    def test_refuses_another_population(self):
        with self.assertRaises(SystemExit):
            da.draw(self.population()[:-1], seg=self.seg)

    def test_the_frozen_population_is_the_registered_one(self):
        self.assertEqual(len(da.population()), da.POPULATION)

    def test_menu_is_the_14_local_rules_with_phase_1s_texts(self):
        gate, menu = da.load_gate(), da.local_menu()
        m = da.build_menu(gate, menu)
        self.assertEqual(len(m), 14)
        self.assertEqual(list(m), menu)
        self.assertIs(gate.SPEC_FORMS["2b"], gate.SPECS)       # form 2b is the module's SPECS
        for k in menu:
            self.assertEqual(m[k], {"law": gate.RULES[k], "spec": gate.SPECS[k]})


class Blinding(unittest.TestCase):
    def sources(self):
        sample = [{"item": i, "sentence": f"a{i}", "paragraph": f"A{i}"} for i in range(30)]
        clean = [(f"clean-{i}", f"clean text {i}") for i in range(6, 18)]
        cands = [{"cid": f"cx-{i:02d}", "unit": f"u{i}", "text": f"T{i}"} for i in range(10)]
        return sample, clean, cands

    def test_fixed_order_does_not_depend_on_input_order(self):
        sample, clean, cands = self.sources()
        self.assertEqual(li.entries(sample, clean, cands), li.entries(sample[::-1], clean, cands[::-1]))

    def test_one_seeded_shuffle_then_opaque_ids(self):
        ents = li.entries(*self.sources())
        items, key = li.blind(ents)
        order = list(ents)
        random.Random(20260942).shuffle(order)
        self.assertEqual([k["ref"] for k in key], [e["ref"] for e in order])
        self.assertEqual([i["id"] for i in items], [f"L{n:04d}" for n in range(1, len(ents) + 1)])
        self.assertTrue(all(set(i) == {"id", "sentence", "paragraph"} for i in items))

    def test_kinds_are_mixed_across_ids(self):
        # The first 30 ids would be exactly the audit items if nothing were shuffled.
        _, key = li.blind(li.entries(*self.sources()))
        self.assertLess(sum(k["source"] == "audit" for k in key[:30]), 30)

    def test_a_clean_text_is_its_own_sentence_and_paragraph(self):
        items, key = li.blind(li.entries(*self.sources()))
        for i, k in zip(items, key):
            if k["source"] == "clean":
                self.assertEqual(i["sentence"], i["paragraph"])


class Answers(unittest.TestCase):
    menu = set(MENU)

    def test_valid_shapes(self):
        self.assertTrue(sa.valid(ans("L1"), self.menu))
        self.assertTrue(sa.valid(ans("L1", ["a_rule"], ["b_rule"]), self.menu))

    def test_invalid_shapes(self):
        bad = [dict(ans("L1"), unsure=True),                   # unsure with no rule named
               dict(ans("L1", unsure=["a_rule"]), unsure=False),
               ans("L1", ["z_rule"]),                          # not a menu rule
               ans("L1", unsure=["z_rule"]),
               dict(ans("L1"), rules="a_rule"),                # not a list
               {k: v for k, v in ans("L1").items() if k != "id"},
               # LOAD-BEARING: 0, not "false". 0 == False in Python, so the unsure/unsure_rules
               # consistency check admits it and only the boolean check refuses; "false" is refused
               # by the consistency check first and leaves the boolean check untested.
               dict(ans("L1"), unsure=0)]
        for a in bad:
            self.assertFalse(sa.valid(a, self.menu), a)

    def test_every_item_answered_exactly_once(self):
        ids = {"L1", "L2"}
        good = [json.dumps(ans("L1")), json.dumps(ans("L2"))]
        self.assertEqual(set(sa.load_answers(good, ids, self.menu, "t")), ids)
        for lines in (good[:1],                                         # one unanswered
                      good + [json.dumps(ans("L2"))],                   # a duplicate
                      good + [json.dumps(ans("L9"))],                   # an unknown id
                      good[:1] + [json.dumps(dict(ans("L2"), unsure=True))]):  # an invalid one
            with self.assertRaises(SystemExit):
                sa.load_answers(lines, ids, self.menu, "t")

    def test_union_takes_listed_and_unsure_rules_from_either_labeller(self):
        codex = {"L1": ans("L1", ["a_rule"]), "L2": ans("L2")}
        claude = {"L1": ans("L1", unsure=["b_rule"]), "L2": ans("L2")}
        self.assertEqual(sa.union(codex, claude), {"L1": {"a_rule", "b_rule"}, "L2": set()})


class Wilson(unittest.TestCase):
    def test_reproduces_the_registered_admission_limits(self):
        # diagnostics/audit-arithmetic.txt, committed with the draft: per expected n, the largest
        # flagged count k admitted, and the upper bounds at k and k + 1, to 4 places.
        text = (PHASE1B / "diagnostics/audit-arithmetic.txt").read_text()
        rows = re.findall(r"expected audit cells (\d+)\s+admitted if flagged <= (\d+) "
                          r"\(upper at \d+: ([\d.]+); at \d+: ([\d.]+)\)", text)
        self.assertEqual(len(rows), 14)
        for n, k, at_k, at_next in rows:
            n, k = int(n), int(k)
            self.assertEqual(max(j for j in range(40) if sa.wilson_upper(j, n) <= sa.LIMIT), k, n)
            self.assertEqual(f"{sa.wilson_upper(k, n):.4f}", at_k)
            self.assertEqual(f"{sa.wilson_upper(k + 1, n):.4f}", at_next)

    def test_closed_form_at_zero(self):
        self.assertEqual(sa.wilson_upper(0, 280), 1.96 ** 2 / (280 + 1.96 ** 2))


class Scoring(unittest.TestCase):
    """Head b_rule's audit cells: 282 audit items of rules a_rule and c_rule. The 5 b_rule audit
    items are its own rule's rows and must not count."""

    def build(self, flag_b_other=0, flag_b_own=0):
        sample, key, flagged = {}, [], {}
        for i in range(287):
            rule = "b_rule" if i < 5 else ("a_rule" if i % 2 else "c_rule")
            sample[i] = {"item": i, "rule": rule, "sentence": f"s{i}"}
            key.append({"id": f"L{i}", "source": "audit", "ref": i, "sentence": f"s{i}"})
            own, other = i < 5, i >= 5
            flagged[f"L{i}"] = {"b_rule"} if ((own and i < flag_b_own) or (other and i - 5 < flag_b_other)) else set()
        # LOAD-BEARING: a candidate and a clean text flagged for b_rule, which admission must ignore.
        key += [{"id": "Lcx", "source": "counterexample", "ref": "cx-1", "sentence": "cue unit"},
                {"id": "Lcl", "source": "clean", "ref": "clean-6", "sentence": "a clean text"}]
        flagged.update({"Lcx": {"b_rule"}, "Lcl": {"b_rule"}})
        return key, sample, flagged

    def test_admitted_at_the_limit_and_removed_one_above(self):
        # n = 282: the registered limit is 6 flagged (upper 0.0456); 7 gives 0.0503.
        for k, want in ((6, True), (7, False)):
            key, sample, flagged = self.build(flag_b_other=k)
            adm = sa.admission(key, sample, flagged, MENU)["b_rule"]
            self.assertEqual((adm["cells"], adm["flagged"], adm["admitted"]), (282, k, want))

    def test_own_rule_rows_and_other_kinds_do_not_count(self):
        key, sample, flagged = self.build(flag_b_other=0, flag_b_own=5)
        adm = sa.admission(key, sample, flagged, MENU)["b_rule"]
        self.assertEqual((adm["cells"], adm["flagged"], adm["admitted"]), (282, 0, True))

    def test_masked_takes_audit_and_candidate_flags_not_clean_ones(self):
        key, _, flagged = self.build(flag_b_other=1)
        flagged["L10"] = {"a_rule", "c_rule"}
        self.assertEqual(sa.masked(key, flagged),
                         [{"unit": "cue unit", "head": "b_rule"}, {"unit": "s10", "head": "a_rule"},
                          {"unit": "s10", "head": "c_rule"}, {"unit": "s5", "head": "b_rule"}])

    def test_counterexample_rows(self):
        cands = {"cx-1": {"cid": "cx-1", "head": "a_rule", "fold": "val", "unit_index": 2, "unit": "u1", "text": "T1"},
                 "cx-2": {"cid": "cx-2", "head": "a_rule", "fold": "train", "unit_index": 0, "unit": "u2", "text": "T2"}}
        key = [{"id": "L1", "source": "counterexample", "ref": "cx-1", "sentence": "u1"},
               {"id": "L2", "source": "counterexample", "ref": "cx-2", "sentence": "u2"}]
        # LOAD-BEARING: cx-2 is flagged for ANOTHER rule, not its head: "unflagged" means no rule.
        rows, dropped = sa.counterexample_rows(key, {"L1": set(), "L2": {"c_rule"}}, cands)
        self.assertEqual(rows, [{"id": "cx-1", "set": "val", "rule": "a_rule", "source": "counterexample",
                                 "text": "T1", "target": 2, "label": 0}])
        self.assertEqual(dropped, {"a_rule|train": 1})


class CleanTexts(unittest.TestCase):
    codex_ids = ["codex-1", "codex-2", "codex-3"]
    ids = [f"clean-{i}" for i in range(6, 15)] + codex_ids

    def verdict(self, dropped):
        key = [{"id": f"L{c}", "source": "clean", "ref": c, "sentence": c} for c in self.ids]
        return sa.clean_verdicts(key, {f"L{c}": ({"a_rule"} if c in dropped else set()) for c in self.ids},
                                 self.codex_ids)

    def test_each_stop_rule_alone(self):
        # Each case drops texts that only its own rule counts, so the other two rules pass.
        self.assertEqual(self.verdict({"clean-12", "clean-13"})["stop"], ["1 of 3 swap texts survive"])
        self.assertEqual(self.verdict({"codex-1", "codex-2"})["stop"], ["1 of 3 Codex texts survive"])
        self.assertEqual(self.verdict({f"clean-{i}" for i in range(6, 12)})["stop"], ["6 of 12 new texts survive"])

    def test_at_the_limits_nothing_stops(self):
        # 2 swap, 2 Codex and 7 of 12 surviving: every rule exactly at its limit.
        v = self.verdict({"clean-12", "codex-1", "clean-6", "clean-7", "clean-8"})
        self.assertEqual((v["swap"], v["codex"], v["total"], v["stop"]), (2, 2, 7, []))


class Key(unittest.TestCase):
    def sources(self):
        sample = {0: {"sentence": "s0"}}
        clean = {"clean-6": "c6"}
        cands = {"cx-1": {"unit": "u1"}}
        key = [{"id": "L1", "source": "audit", "ref": 0, "sentence": "s0"},
               {"id": "L2", "source": "clean", "ref": "clean-6", "sentence": "c6"},
               {"id": "L3", "source": "counterexample", "ref": "cx-1", "sentence": "u1"}]
        return key, sample, clean, cands

    def test_an_aligned_key_passes(self):
        sa.check_key(*self.sources())

    def test_refusals(self):
        key, sample, clean, cands = self.sources()
        for bad in (key[:2],                                            # a candidate missing
                    key + [dict(key[0], id="L4")],                      # an item twice
                    [dict(key[0], sentence="other")] + key[1:]):        # a sentence drifted
            with self.assertRaises(SystemExit):
                sa.check_key(bad, sample, clean, cands)


class Labellers(unittest.TestCase):
    menu = set(MENU)
    ids = ["L1", "L2"]

    def raw(self, answers):
        return "\n".join(json.dumps(a) for a in answers)

    def test_check_call(self):
        good = [ans("L1"), ans("L2", ["a_rule"])]
        self.assertIsNone(rl.check_call(good, self.ids, self.menu))
        # Each bad case keeps every other answer valid, so only the check it names can refuse it.
        cases = {"invalid": [dict(ans("L1"), unsure=True), ans("L2")],
                 "not in this call": good + [ans("L9")],
                 "duplicate": good + [ans("L2")],
                 "1 of 2 items unanswered": good[:1]}
        for why, answers in cases.items():
            self.assertIn(why, rl.check_call(answers, self.ids, self.menu), why)

    def attempts(self, outcomes):
        calls = []

        def call(attempt):
            calls.append(attempt)
            out = outcomes[attempt - 1]
            if isinstance(out, Exception):
                raise out
            return out
        with tempfile.TemporaryDirectory() as tmp:
            try:
                got = rl.run_call("c", self.ids, call, self.menu, pathlib.Path(tmp))
            except rl.Stop as e:
                got = e
            files = sorted(p.name for p in pathlib.Path(tmp).iterdir())
        return got, calls, files

    def test_an_error_is_re_run_once(self):
        # LOAD-BEARING: attempt 2 answers in reverse order, so returning the model's order reds.
        got, calls, files = self.attempts([RuntimeError("boom"), self.raw([ans("L2"), ans("L1", ["b_rule"])])])
        self.assertEqual([a["id"] for a in got], ["L1", "L2"])
        self.assertEqual(calls, [1, 2])
        self.assertEqual(files, ["c.a1.err", "c.a2.txt"])

    def test_an_incomplete_answer_is_re_run_once(self):
        got, calls, _ = self.attempts([self.raw([ans("L1")]), "```\n" + self.raw([ans("L1"), ans("L2")]) + "\n```"])
        self.assertEqual(([a["id"] for a in got], calls), (["L1", "L2"], [1, 2]))

    def test_a_second_failure_stops_with_both_reasons(self):
        got, calls, _ = self.attempts([self.raw([ans("L1")]), RuntimeError("down")] + [self.raw([ans("L1"), ans("L2")])])
        self.assertIsInstance(got, rl.Stop)
        self.assertEqual(calls, [1, 2])                        # never a third attempt
        self.assertIn("attempt 1: 1 of 2 items unanswered", str(got))
        self.assertIn("attempt 2: down", str(got))

    def test_batches_of_25_in_file_order(self):
        items = [{"id": f"L{i}"} for i in range(51)]
        parts = rl.batches(items)
        self.assertEqual([len(p) for p in parts], [25, 25, 1])
        self.assertEqual([i for p in parts for i in p], items)

    def test_claude_prompt_is_instruction_menu_and_the_batch(self):
        batch = [{"id": "L7", "sentence": "s", "paragraph": "p"}, {"id": "L8", "sentence": "t", "paragraph": "q"}]
        pr = rl.claude_prompt("THE INSTRUCTION", '{"a_rule": {}}', batch)
        self.assertTrue(pr.startswith("THE INSTRUCTION"))
        self.assertIn('## menu.json\n\n{"a_rule": {}}', pr)
        self.assertEqual(pr.split("## Items\n\n")[1].splitlines(), [json.dumps(b) for b in batch])

    def test_codex_sees_exactly_three_files(self):
        self.assertEqual(rl.codex_files("I", "M", "X"),
                         {"audit-instruction.md": "I", "menu.json": "M", "items.jsonl": "X"})

    def test_the_claude_channel_must_be_clean(self):
        with self.assertRaises(SystemExit) as e:
            rl.check_channel("")
        # The message, not only the refusal: an empty path also fails dirty_reasons (no settings
        # reads as plugins enabled), so only the text separates the unset guard from that one.
        self.assertIn("unset", str(e.exception))
        with tempfile.TemporaryDirectory() as tmp:
            (pathlib.Path(tmp) / "settings.json").write_text('{"enabledPlugins": {}, "hooks": {}}')
            rl.check_channel(tmp)                              # clean: no refusal
            (pathlib.Path(tmp) / "CLAUDE.md").write_text("rules")
            with self.assertRaises(SystemExit):
                rl.check_channel(tmp)


class CodexClean(unittest.TestCase):
    good = [{"id": "codex-clean-1", "text": "one"}, {"id": "codex-clean-2", "text": "two"},
            {"id": "codex-clean-3", "text": "three"}]

    def test_check_texts(self):
        self.assertIsNone(gc.check_texts(self.good))
        # Each bad case keeps the other objects valid, so only the check it names can refuse it.
        for bad in (self.good[:2],                                             # one missing
                    self.good + [self.good[0]],                                # one twice
                    self.good[:2] + [{"id": "codex-clean-4", "text": "x"}],     # not a registered id
                    self.good[:2] + [{"id": "codex-clean-3", "text": "  "}],    # blank text
                    self.good[:2] + [{"id": "codex-clean-3"}],                 # no text
                    # LOAD-BEARING: no id at all. The final ids comparison also refuses an unknown
                    # id, but without the id check a missing one raises KeyError at o["id"],
                    # crashing the run instead of refusing it (observed under that mutation).
                    self.good[:2] + [{"text": "x"}]):
            self.assertIsNotNone(gc.check_texts(bad), bad)

    def generate(self, outcomes):
        calls = []

        def call(attempt):
            calls.append(attempt)
            out = outcomes[attempt - 1]
            if isinstance(out, Exception):
                raise out
            return out
        with tempfile.TemporaryDirectory() as tmp:
            out = pathlib.Path(tmp) / "codex-clean-texts.jsonl"
            rc, reasons = gc.generate(call, out, pathlib.Path(tmp))
            written = out.read_text() if out.exists() else None
            files = sorted(p.name for p in pathlib.Path(tmp).iterdir())
        return rc, reasons, written, calls, files

    def test_re_run_once_then_written_in_id_order(self):
        # LOAD-BEARING: the valid answer comes back out of order; the file must be in id order.
        raw = "\n".join(json.dumps(o) for o in self.good[::-1])
        rc, reasons, written, calls, files = self.generate([RuntimeError("down"), raw])
        self.assertEqual((rc, calls), (0, [1, 2]))
        self.assertEqual([json.loads(line)["id"] for line in written.splitlines()], list(gc.IDS))
        self.assertIn("a1.err", files)

    def test_two_failures_write_nothing(self):
        raw = "\n".join(json.dumps(o) for o in self.good[:2])
        rc, reasons, written, calls, _ = self.generate([raw, raw, raw])
        self.assertEqual((rc, written, calls), (4, None, [1, 2]))
        self.assertEqual(len(reasons), 2)

    def test_refuses_to_generate_twice(self):
        saved, saved_argv = gc.ct.CODEX_CLEAN, sys.argv
        with tempfile.TemporaryDirectory() as tmp:
            try:
                gc.ct.CODEX_CLEAN = pathlib.Path(tmp) / "codex-clean-texts.jsonl"
                sys.argv = ["gen_codex_clean.py", "--dry-run"]
                self.assertEqual(gc.main(), 0)                  # absent: a dry run proceeds
                gc.ct.CODEX_CLEAN.write_text("{}\n")
                with self.assertRaises(SystemExit):             # present: refused, even a dry run
                    gc.main()
            finally:
                gc.ct.CODEX_CLEAN, sys.argv = saved, saved_argv


if __name__ == "__main__":
    unittest.main()
