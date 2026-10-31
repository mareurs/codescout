import hashlib
import json
import tempfile
import unittest
from pathlib import Path

import multilang_records as m
import select_tasks

RULE = {"created_at_min": "2026-07-01", "min_files": 2, "min_added_lines": 40, "max_added_lines": 600,
        "require": ["FAIL_TO_PASS", "test_cmds", "log_parser"], "draw": {"salt": "test-salt"}}
FIXTURE = Path(__file__).resolve().parent / "fixtures" / "multilang" / "records-mini.jsonl"


def patch(files, added):
    out = ""
    for i in range(files):
        out += f"diff --git a/f{i}.rs b/f{i}.rs\n--- a/f{i}.rs\n+++ b/f{i}.rs\n@@ -1 +1 @@\n"
        out += "".join(f"+line{j}\n" for j in range(added if i == 0 else 0))
    return out


def record(iid="o__r-1", files=2, added=5, created_at="2026-07-15T08:12:03Z", **over):
    r = {"instance_id": iid, "repo": "o/r", "base_commit": "abc", "created_at": created_at,
         "patch": patch(files, added), "test_patch": "", "FAIL_TO_PASS": ["a::t"], "PASS_TO_PASS": ["a::u"],
         "test_cmds": ["cargo test"], "print_cmds": ["cat x"], "log_parser": "def parser(log): return {}",
         "docker_image": "img"}
    r.update(over)
    return r


def cand(**over):
    return m.candidate(record(**over))


def write(path, records):
    path.write_text("".join(json.dumps(r) + "\n" for r in records))


class LoadTests(unittest.TestCase):
    def test_load_records_checks_sha256(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "r.jsonl"
            write(p, [record()])
            good = hashlib.sha256(p.read_bytes()).hexdigest()
            self.assertEqual(len(m.load_records(p, good)), 1)
            self.assertEqual(len(m.load_records(p)), 1)
            with self.assertRaises(ValueError):
                m.load_records(p, "0" * 64)

    def test_load_records_rejects_missing_key_and_duplicate_id(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "r.jsonl"
            bad = record()
            del bad["log_parser"]
            write(p, [bad])
            with self.assertRaisesRegex(ValueError, "log_parser"):
                m.load_records(p)
            write(p, [record("o__r-1"), record("o__r-1")])
            with self.assertRaisesRegex(ValueError, "duplicate"):
                m.load_records(p)
    def test_load_records_rejects_unparseable_created_at_naming_path_and_line(self):
        for bad in ("unknown", "July 15, 2026", None, 20260715):
            with tempfile.TemporaryDirectory() as d:
                p = Path(d) / "r.jsonl"
                write(p, [record("o__r-1"), record("o__r-2", created_at=bad)])
                with self.assertRaisesRegex(ValueError, rf"{p.name}:2: created_at"):
                    m.load_records(p)


    def test_fixture_loads_and_selects_one(self):
        records = m.load_records(FIXTURE)
        self.assertEqual(len(records), 4)
        self.assertEqual(m.select_eligible(records, m.load_rule()), ["h4ckf0r0day__obscura-426"])


class CandidateTests(unittest.TestCase):
    def test_candidate_counts_files_and_added_lines(self):
        c = cand(files=2, added=5)
        self.assertEqual(c["gold_files"], ["f0.rs", "f1.rs"])
        self.assertEqual(c["gold_added_lines"], 5)  # the +++ headers are not counted
        self.assertEqual(c["expected_tests"], 2)
        self.assertEqual((c["id"], c["repo"], c["created_at"]), ("o__r-1", "o/r", "2026-07-15T08:12:03Z"))
    def test_candidate_projects_to_exactly_the_registered_keys(self):
        c = m.candidate(record(**{"pass": True, "cost": 1.5}))
        self.assertEqual(set(c), {"id", "repo", "gold_files", "gold_added_lines", "created_at",
                                  "expected_tests", "nonempty"})



class EligibleTests(unittest.TestCase):
    def test_eligible_rejects_before_cutoff(self):
        self.assertFalse(m.eligible(cand(created_at="2026-06-30T23:59:59Z"), RULE))
        self.assertTrue(m.eligible(cand(created_at="2026-07-01T00:00:00Z"), RULE))
    def test_eligible_fails_closed_on_unparseable_created_at(self):
        for bad in ("unknown", "July 15, 2026", None, 20260715):
            c = cand()
            c["created_at"] = bad
            self.assertIs(m.eligible(c, RULE), False, repr(bad))
            self.assertIs(m.eligible(c, RULE, ignore_size=True), False, repr(bad))


    def test_eligible_size_is_or_of_files_and_lines(self):
        self.assertTrue(m.eligible(cand(files=2, added=1), RULE))    # files only
        self.assertTrue(m.eligible(cand(files=1, added=40), RULE))   # lines only
        self.assertFalse(m.eligible(cand(files=1, added=39), RULE))  # neither

    def test_eligible_rejects_over_max_lines(self):
        self.assertTrue(m.eligible(cand(files=1, added=600), RULE))
        self.assertFalse(m.eligible(cand(files=1, added=601), RULE))

    def test_eligible_requires_nonempty_fields(self):
        for field, empty in (("FAIL_TO_PASS", []), ("test_cmds", []), ("log_parser", "")):
            self.assertFalse(m.eligible(cand(**{field: empty}), RULE), field)

    def test_eligible_ignore_size_still_enforces_cutoff_and_required_fields(self):
        self.assertTrue(m.eligible(cand(files=1, added=1), RULE, ignore_size=True))
        self.assertTrue(m.eligible(cand(files=1, added=601), RULE, ignore_size=True))
        self.assertFalse(m.eligible(cand(created_at="2026-06-30T00:00:00Z"), RULE, ignore_size=True))
        self.assertFalse(m.eligible(cand(FAIL_TO_PASS=[]), RULE, ignore_size=True))


class SelectTests(unittest.TestCase):
    def test_select_eligible_order_is_draw_key_order_and_ignores_unknown_fields(self):
        records = [record(f"o__r-{i}") for i in range(8)] + [record("o__r-old", created_at="2026-01-01T00:00:00Z")]
        got = m.select_eligible(records, RULE)
        self.assertEqual(sorted(got), sorted(f"o__r-{i}" for i in range(8)))
        keys = [select_tasks.draw_key(RULE, i) for i in got]
        self.assertEqual(keys, sorted(keys))
        for i, r in enumerate(records):
            r["pass"] = i % 2 == 0
        self.assertEqual(m.select_eligible(records, RULE), got)

    def test_registered_rule_loads(self):
        rule = m.load_rule()
        self.assertEqual((rule["created_at_min"], rule["min_files"], rule["min_added_lines"],
                          rule["max_added_lines"]), ("2026-07-01", 2, 40, 600))
        self.assertEqual(rule["draw"], {"salt": "setup-ab-live-20261031"})


if __name__ == "__main__":
    unittest.main()
