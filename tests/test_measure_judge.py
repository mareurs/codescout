"""Task 9a tests: the judge harness and its dry gate (spec § Judge protocol (Codex), Amendments
1 A1.3/A1.4 and 7 (b); rulings R116-R126 in
.superpowers/sdd/2026-09-26-system1-base-rate-measurement/task-9-context.md).

No test makes a model call: every `complete` is a fake injected here, and the dry gate is
asserted to make zero calls. Every git fixture is a synthetic temp repo whose committer dates
differ from its author dates (R125). Run with:
    ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_judge.py -v
"""
import dataclasses
import importlib.util
import json
import os
import pathlib
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest import mock

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
MEASURE = REPO_ROOT / "scripts" / "measure"
sys.path.insert(0, str(MEASURE))


def _load(name):
    spec = importlib.util.spec_from_file_location(f"measure_{name}", MEASURE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


join = _load("join")
judge = _load("judge")
run = _load("run")


# --- fixtures ------------------------------------------------------------------------------------

SID = "sess-1"


def _events_db(tmp, rows):
    """`rows` are (sid, agent_path, uuid, ts, role, kind, text), inserted IN THE ORDER GIVEN, so
    rowid order is transcript order -- the order `build_input` reads "preceding" in."""
    path = pathlib.Path(tmp) / "events.db"
    conn = sqlite3.connect(path)
    join._create_schema(conn)
    conn.executemany(
        "INSERT INTO turns (sid, agent_path, uuid, ts, role, kind, text, message_id, tokens) "
        "VALUES (?, ?, ?, ?, ?, ?, ?, NULL, NULL)", rows)
    conn.commit()
    conn.close()
    return path


def _turn(uuid, ts, kind="assistant_text", text=None, agent_path=None, role=None):
    role = role or ("user" if kind in ("prompt", "tool_result") else "assistant")
    return (SID, agent_path, uuid, ts, role, kind, text if text is not None else f"text of {uuid}")


def _git(root, *args, env=None):
    return subprocess.run(["git", *args], cwd=str(root), check=True, capture_output=True,
                          text=True, env=env).stdout


def _init_repo(root):
    root.mkdir(parents=True, exist_ok=True)
    _git(root, "init", "-q")
    _git(root, "config", "user.name", "t")
    _git(root, "config", "user.email", "t@t")
    _git(root, "config", "commit.gpgsign", "false")


def _write(root, rel, content):
    p = root / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(content)


def _day(n):
    return f"2026-01-{n:02d}T10:00:00+00:00"


def _commit(root, message, day):
    """Author date = day `day`; committer date = day 28 - `day`, so committer order is the
    REVERSE of author order and every committer date differs from its author date (R125). A
    dater that read %cI, or mapped by committer order, gets every date in these tests wrong."""
    import os
    env = dict(os.environ)
    env["GIT_AUTHOR_DATE"] = _day(day)
    env["GIT_COMMITTER_DATE"] = _day(28 - day)
    _git(root, "add", "-A", env=env)
    _git(root, "commit", "-q", "--allow-empty", "-m", message, env=env)
    return _git(root, "rev-parse", "HEAD").strip()
def _plumb_commit(root, parents, files, day, committer_day, message):
    """A commit built with plumbing (a private index, `commit-tree`), so a fixture can hold a
    merge without ever checking a branch out. `files` overlays the first parent's tree."""
    idx = root / ".git" / "fixture-index"
    if idx.exists():
        idx.unlink()
    env = dict(os.environ, GIT_AUTHOR_DATE=_day(day), GIT_COMMITTER_DATE=_day(committer_day),
               GIT_INDEX_FILE=str(idx))
    _git(root, "read-tree", parents[0], env=env)
    for rel, content in files.items():
        blob = subprocess.run(["git", "hash-object", "-w", "--stdin"], cwd=str(root), input=content,
                              capture_output=True, text=True, check=True).stdout.strip()
        _git(root, "update-index", "--add", "--cacheinfo", f"100644,{blob},{rel}", env=env)
    tree = _git(root, "write-tree", env=env).strip()
    args = ["commit-tree", tree]
    for p in parents:
        args += ["-p", p]
    return _git(root, *args, "-m", message, env=env).strip()



_OP_RULES = """# Operator rules

| id | title | status |
|---|---|---|
| OP-1 | Verify | active |

## OP-1 — Verify

**Imperative:** Always verify before claiming.
"""


def _global_claude_md(tmp):
    p = pathlib.Path(tmp) / "global-CLAUDE.md"
    p.write_text("# Global\n\n## Personal habits\n\n- **Prefer short replies.** Keep it tight.\n")
    return p


def _same_instant(a, b):
    return join.utc(a) == join.utc(b)  # git may print +00:00 as Z; the instant is what counts


def _entry(entries, lesson_id_suffix):
    hits = [e for e in entries if e["id"].endswith(lesson_id_suffix)]
    assert len(hits) == 1, (lesson_id_suffix, [e["id"] for e in entries])
    return hits[0]


FAR_FUTURE = "2027-01-01T00:00:00Z"


# --- R116: leakage ---------------------------------------------------------------------------------


class LeakageTests(unittest.TestCase):
    def _db(self, tmp, preceding_ts):
        return _events_db(tmp, [
            _turn("u1", "2026-09-26T10:00:00Z", kind="prompt", text="please count the rows"),
            _turn("u2", preceding_ts, text="the earlier turn says 18 rows"),
            _turn("u3", "2026-09-26T10:00:03Z", text="There are 18 rows."),
        ])

    def _audit_item(self):
        return {"id": "i1", "sid": SID, "decision_uuid": "u3", "decision_ts": "2026-09-26T10:00:03Z"}

    def test_an_input_with_a_later_timestamp_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            db = self._db(tmp, "2026-09-26T10:00:05Z")  # precedes in order, later in time
            with self.assertRaises(judge.LeakageError):
                judge.build_input(self._audit_item(), db, [], "audit")

    def test_an_audit_context_turn_at_the_decision_ts_is_refused(self):
        # The same instant written with a fractional part: a string comparison reads
        # "…03.000Z" as EARLIER than "…03Z" ('.' < 'Z'), so only join.utc refuses it.
        for ts in ("2026-09-26T10:00:03Z", "2026-09-26T10:00:03.000Z"):
            with self.subTest(ts=ts), tempfile.TemporaryDirectory() as tmp:
                db = self._db(tmp, ts)
                with self.assertRaises(judge.LeakageError):
                    judge.build_input(self._audit_item(), db, [], "audit")

    def test_a_strictly_earlier_context_is_accepted(self):
        with tempfile.TemporaryDirectory() as tmp:
            db = self._db(tmp, "2026-09-26T10:00:02.999Z")
            inp = judge.build_input(self._audit_item(), db, [], "audit")
            self.assertEqual([c["uuid"] for c in inp["context"]], ["u2", "u1"])
            self.assertEqual(inp["decision"], "There are 18 rows.")
            self.assertIn("the earlier turn says 18 rows", inp["pre_evidence"])
            self.assertIn("There are 18 rows.", inp["pre_evidence"])

    def test_a_correction_field_in_audit_mode_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            db = self._db(tmp, "2026-09-26T10:00:01Z")
            item = dict(self._audit_item(), correction="No -- it is 7 rows.")
            with self.assertRaises(judge.LeakageError):
                judge.build_input(item, db, [], "audit")

    def test_the_correction_text_is_never_pre_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            db = self._db(tmp, "2026-09-26T10:00:01Z")
            item = dict(self._audit_item(), correction="No -- the table actually holds 7 rows.")
            inp = judge.build_input(item, db, [], "correction")
            self.assertEqual(inp["correction"], "No -- the table actually holds 7 rows.")
            self.assertNotIn("actually holds 7 rows", inp["pre_evidence"])
            self.assertIn("actually holds 7 rows", inp["prompt"])  # shown, but as its own field
            quoting_correction = judge.Verdict(
                mode="correction", is_correction=True, detectability="in-trace",
                lessons=["x"], quote="the table actually holds 7 rows")
            self.assertEqual(judge.verify_quote(quoting_correction, inp["pre_evidence"]).lessons,
                             "abstain")
            quoting_context = dataclasses.replace(quoting_correction,
                                                  quote="the earlier turn says 18 rows")
            self.assertEqual(judge.verify_quote(quoting_context, inp["pre_evidence"]).lessons, ["x"])

    def test_a_document_item_whose_context_holds_its_own_correction_is_refused(self):
        item = {
            "id": "d1", "decision": "All rows are closed.", "decision_ts": None,
            "document": {"sha": "a" * 40, "path": "docs/x.md",
                         "excerpts": ["intro. All rows are closed. Later: CORRECTED, two are open."]},
            "must_not_contain": ["CORRECTED, two are open."],
        }
        with self.assertRaises(judge.LeakageError):
            judge.build_input(item, None, [], "audit")
        ok = dict(item, must_not_contain=["this text is absent"])
        inp = judge.build_input(ok, None, [], "audit")
        self.assertIn("All rows are closed.", inp["pre_evidence"])

    def test_a_document_item_without_blob_provenance_is_refused(self):
        item = {"id": "d2", "decision": "x", "decision_ts": None,
                "document": {"sha": "", "path": "docs/x.md", "excerpts": ["x"]}}
        with self.assertRaises(judge.LeakageError):
            judge.build_input(item, None, [], "audit")

    def test_an_item_decision_ts_that_disagrees_with_the_events_db_is_refused(self):
        # R135(e), Task 10's path: the item's decision_ts must be the events db's own ts for the
        # decision turn. LOAD-BEARING: the item's ts is LATER than the db's, so every context
        # turn is still strictly before it and the leakage guard admits the input -- only the
        # consistency check can refuse it (a plain ValueError, not a LeakageError).
        with tempfile.TemporaryDirectory() as tmp:
            db = self._db(tmp, "2026-09-26T10:00:01Z")
            item = dict(self._audit_item(), decision_ts="2026-09-26T10:00:09Z")
            with self.assertRaisesRegex(ValueError, "disagrees with the events db") as cm:
                judge.build_input(item, db, [], "audit")
            self.assertNotIsInstance(cm.exception, judge.LeakageError)
            judge.build_input(self._audit_item(), db, [], "audit")  # the db's own ts is accepted


# --- R117: bounded context and origin candidates ---------------------------------------------------


class BoundedContextTests(unittest.TestCase):
    def test_truncation_keeps_the_newest_turns(self):
        with tempfile.TemporaryDirectory() as tmp:
            rows = [_turn(f"p{i:02d}", f"2026-09-26T10:00:{i:02d}Z", kind="prompt", text=f"t{i}")
                    for i in range(15)]
            rows.insert(7, _turn("sub", "2026-09-26T10:00:06.5Z", text="subagent",
                                 agent_path="agent-1"))
            rows.append(_turn("d", "2026-09-26T10:01:00Z", text="decision"))
            db = _events_db(tmp, rows)
            inp = judge.build_input({"id": "i", "sid": SID, "decision_uuid": "d",
                                     "decision_ts": "2026-09-26T10:01:00Z"}, db, [], "audit")
            self.assertEqual([c["uuid"] for c in inp["context"]],
                             [f"p{i:02d}" for i in range(14, 2, -1)])  # the 12 newest, newest first

        with tempfile.TemporaryDirectory() as tmp:
            rows = [_turn(f"b{i}", f"2026-09-26T10:00:0{i}Z", kind="tool_result", text=str(i) * 3000)
                    for i in range(8)]
            rows.append(_turn("d", "2026-09-26T10:01:00Z", text="decision"))
            db = _events_db(tmp, rows)
            inp = judge.build_input({"id": "i", "sid": SID, "decision_uuid": "d",
                                     "decision_ts": "2026-09-26T10:01:00Z"}, db, [], "audit")
            # 6 x 3000 = 18,000 fits the 20,000 budget; a 7th would make 21,000.
            self.assertEqual([c["uuid"] for c in inp["context"]],
                             ["b7", "b6", "b5", "b4", "b3", "b2"])

        with tempfile.TemporaryDirectory() as tmp:
            rows = [_turn(f"e{i}", f"2026-09-26T10:00:0{i}Z", kind="tool_result", text=str(i) * 4000)
                    for i in range(6)]
            rows.append(_turn("d", "2026-09-26T10:01:00Z", text="decision"))
            db = _events_db(tmp, rows)
            inp = judge.build_input({"id": "i", "sid": SID, "decision_uuid": "d",
                                     "decision_ts": "2026-09-26T10:01:00Z"}, db, [], "audit")
            # 5 x 4000 is EXACTLY 20,000, which is "up to 20,000": all five are kept.
            self.assertEqual([c["uuid"] for c in inp["context"]], ["e5", "e4", "e3", "e2", "e1"])

    def test_origin_candidates_are_previous_top_level_assistant_text_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            rows = []
            for i in range(7):
                rows.append(_turn(f"a{i}", f"2026-09-26T10:0{i}:00Z", text=f"answer {i}"))
                rows.append(_turn(f"t{i}", f"2026-09-26T10:0{i}:01Z", kind="tool_use"))
                rows.append(_turn(f"s{i}", f"2026-09-26T10:0{i}:02Z",
                                  text=f"sub {i}", agent_path="agent-1"))
            rows.append(_turn("origin", "2026-09-26T10:09:30Z", text="the origin claim"))
            db = _events_db(tmp, rows)
            item = {"id": "i", "sid": SID, "decision_uuid": "origin",
                    "decision_ts": "2026-09-26T10:09:30Z", "correction": "that is wrong"}
            inp = judge.build_input(item, db, [], "correction")
            self.assertEqual([c["uuid"] for c in inp["origin_candidates"]],
                             ["a6", "a5", "a4", "a3", "a2"])
            self.assertEqual(inp["offered_uuids"], ["a6", "a5", "a4", "a3", "a2"])
            audit = judge.build_input(dict(item, correction=None), db, [], "audit")
            self.assertEqual(audit["origin_candidates"], [])


# --- R118 / R125: lesson dating --------------------------------------------------------------------


class LessonDatingTests(unittest.TestCase):
    def _repo(self, tmp):
        root = pathlib.Path(tmp) / "repo"
        _init_repo(root)
        _write(root, "docs/trackers/operator-rules.md", _OP_RULES)
        return root

    def _dates(self, root, sha="HEAD"):
        return judge.lessons_for(root, sha, FAR_FUTURE, global_claude_md=None)

    def test_a_line_drafted_in_a_tracker_then_promoted_dates_at_the_promotion(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self._repo(tmp)
            _write(root, "docs/trackers/x.md", "# X\n\n- **Count the list, never the corpus.** Why.\n")
            _commit(root, "draft", 1)
            _write(root, "README.md", "unrelated\n")
            _commit(root, "unrelated", 2)
            _write(root, "CLAUDE.md", "# P\n\n## Rules\n\n- **Count the list, never the corpus.** Why.\n")
            _commit(root, "promote", 3)
            e = _entry(self._dates(root), "CLAUDE.md#rules/count-the-list-never-the-corpus")
            self.assertTrue(_same_instant(e["date"], _day(3)), e["date"])
            self.assertEqual(e["status"], "dated")

    def test_a_lesson_moved_between_sources_dates_at_the_move(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self._repo(tmp)
            _write(root, ".codescout/memories/a.md", "# A\n\n## Keep the unit\n\nAlways.\n")
            _commit(root, "memory a", 1)
            _git(root, "mv", ".codescout/memories/a.md", ".codescout/memories/b.md")
            _commit(root, "move to b", 3)
            e = _entry(self._dates(root), "memory:b#keep-the-unit")
            self.assertTrue(_same_instant(e["date"], _day(3)), e["date"])

    def test_a_rewrap_of_the_anchor_keeps_the_original_date(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self._repo(tmp)
            # Both directions of a rewrap: the first bullet's whitespace turns irregular after
            # day 1, the second's turns regular; each also moves words off its first line. A
            # regex that did not normalise whitespace would miss one direction or the other.
            _write(root, "CLAUDE.md",
                   "# P\n\n## Rules\n\n- **Name the scope you searched.** alpha beta gamma delta\n"
                   "- **Stamp  the\tinstant always.** one two three four\n")
            _commit(root, "bullet", 1)
            _write(root, "CLAUDE.md",
                   "# P\n\n## Rules\n\n- **Name the  scope you\tsearched.** alpha\n  beta gamma delta\n"
                   "- **Stamp the instant always.** one\n  two three four\n")
            _commit(root, "rewrap", 4)
            entries = self._dates(root)
            e = _entry(entries, "CLAUDE.md#rules/name-the-scope-you-searched")
            self.assertTrue(_same_instant(e["date"], _day(1)), e["date"])
            e = _entry(entries, "CLAUDE.md#rules/stamp-the-instant-always")
            self.assertTrue(_same_instant(e["date"], _day(1)), e["date"])

    def test_a_repeated_headings_second_copy_dates_at_its_own_introduction(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self._repo(tmp)
            _write(root, "CLAUDE.md", "# P\n\n## Notes\n\nfirst.\n")
            _commit(root, "notes 1", 1)
            _write(root, "CLAUDE.md", "# P\n\n## Notes\n\nfirst.\n\n## Other\n\nx.\n")
            _commit(root, "other", 2)
            _write(root, "CLAUDE.md", "# P\n\n## Notes\n\nfirst.\n\n## Other\n\nx.\n\n## Notes\n\nsecond.\n")
            _commit(root, "notes 2", 3)
            entries = self._dates(root)
            self.assertTrue(_same_instant(_entry(entries, "CLAUDE.md#notes")["date"], _day(1)))
            self.assertTrue(_same_instant(_entry(entries, "CLAUDE.md#notes-2")["date"], _day(3)))
    def test_a_heading_anchor_never_matches_a_longer_heading(self):
        # `## Notes` is a prefix of `## Notes on style`, which came first; a pickaxe that is not
        # held to the whole line dates `## Notes` at the longer heading's commit. (Measured on the
        # real inventory: `## Error Handling` and `## Commit Style` would date at
        # `## Error Handling Pattern` / `### Commit Style`.) LOAD-BEARING: the longer heading is
        # RENAMED away, so it is gone at the freeze. Were it still there, it would count as a copy
        # and push the anchor's occurrence to 2, and occurrence counting would mask the defect.
        with tempfile.TemporaryDirectory() as tmp:
            root = self._repo(tmp)
            _write(root, "CLAUDE.md", "# P\n\nNotes.\n\n## Notes on style\n\nx.\n")
            _commit(root, "longer heading", 1)
            _write(root, "CLAUDE.md", "# P\n\nNotes.\n\n## Notes\n\nx.\n")
            _commit(root, "renamed to the heading itself", 3)
            e = _entry(self._dates(root), "CLAUDE.md#notes")
            self.assertTrue(_same_instant(e["date"], _day(3)), e["date"])

    def test_a_merge_with_restamped_committer_dates_dates_at_the_ancestor(self):
        # c0 introduces `## Keep`; x1 (a child of c0) re-spaces it, so x1 is a -G hit too. With a
        # merge, git's default order is by COMMITTER date and emits c0 (restamped late) before its
        # own child x1, so `--reverse` would put x1 first and date the lesson at x1. Topological
        # order keeps the ancestor first whatever the committer dates say.
        with tempfile.TemporaryDirectory() as tmp:
            root = self._repo(tmp)
            base = "# P\n\nNotes.\n\n## Keep\n\nx\n\nfiller one\nfiller two\nfiller three\n"
            _write(root, "CLAUDE.md", base)
            c0 = _commit(root, "c0", 1)  # author day 1, committer day 27
            respaced = base.replace("## Keep", "##  Keep")
            other = "\n## Other\n\ny\n"
            x1 = _plumb_commit(root, [c0], {"CLAUDE.md": respaced}, 5, 2, "x1: re-space")
            y1 = _plumb_commit(root, [c0], {"CLAUDE.md": base + other}, 3, 20, "y1: other")
            m = _plumb_commit(root, [x1, y1], {"CLAUDE.md": respaced + other}, 6, 21, "merge")
            entries = judge.lessons_for(root, m, FAR_FUTURE, global_claude_md=None)
            e = _entry(entries, "CLAUDE.md#keep")
            self.assertTrue(_same_instant(e["date"], _day(1)), e["date"])

    def test_the_author_date_wins_over_the_committer_date(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self._repo(tmp)
            _write(root, "CLAUDE.md", "# P\n\n## Stamp the instant\n\nAlways.\n")
            _commit(root, "one", 5)  # author day 5, committer day 23
            entries = judge.lessons_for(root, "HEAD", "2026-01-10T00:00:00Z", global_claude_md=None)
            self.assertTrue(_same_instant(_entry(entries, "CLAUDE.md#stamp-the-instant")["date"],
                                          _day(5)))

    def test_lessons_for_keeps_only_lessons_dated_strictly_before_the_decision(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self._repo(tmp)
            # The preamble carries a prose line: a title-only preamble has no non-heading anchor
            # line, so R125 cannot date it and it would (correctly) list as undatable.
            _write(root, "CLAUDE.md", "# P\n\nProject notes.\n\n## Early rule\n\nOne.\n")
            _commit(root, "early", 2)
            _write(root, "CLAUDE.md",
                   "# P\n\nProject notes.\n\n## Early rule\n\nOne.\n\n## Late rule\n\nTwo.\n")
            _commit(root, "late", 6)
            g = _global_claude_md(tmp)
            at_late = judge.lessons_for(root, "HEAD", "2026-01-06T10:00:00Z", global_claude_md=g)
            ids = [e["id"] for e in at_late]
            self.assertTrue(any(i.endswith("CLAUDE.md#early-rule") for i in ids), ids)
            self.assertFalse(any(i.endswith("CLAUDE.md#late-rule") for i in ids), ids)  # == is not <
            after = judge.lessons_for(root, "HEAD", "2026-01-06T10:00:01Z", global_claude_md=g)
            self.assertTrue(any(e["id"].endswith("CLAUDE.md#late-rule") for e in after))
            undated = [e for e in at_late if e["source"] == "global-CLAUDE.md"]
            self.assertTrue(undated)
            self.assertTrue(all(e["status"] == "undated" for e in undated))
            self.assertTrue(all(e["status"] == "dated" for e in at_late
                                if e["source"] != "global-CLAUDE.md"))

    def test_an_undatable_repo_lesson_is_listed_undated_and_counted(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self._repo(tmp)
            _write(root, "CLAUDE.md", "# P\n\n## A rule\n\nOne.\n")
            _commit(root, "one", 2)
            stats = {}
            with mock.patch.object(judge, "date_anchor", return_value=None):
                entries = judge.lessons_for(root, "HEAD", "2026-01-01T00:00:00Z",
                                            global_claude_md=None, stats=stats)
            e = _entry(entries, "CLAUDE.md#a-rule")
            self.assertEqual(e["status"], "undated")
            self.assertEqual(stats.get("lesson_undatable"), len(entries))

    def test_the_lesson_index_is_bounded_and_never_the_full_text(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self._repo(tmp)
            tail = "Second sentence ENDMARK."  # short: were the full text shown, it would fit
            wordy = " ".join(f"clause{i}" for i in range(80))
            _write(root, "CLAUDE.md", f"# P\n\n## A long rule\n\nFirst sentence here. {tail}\n"
                                      f"\n- **A bullet whose first sentence is long** {wordy}.\n")
            _commit(root, "long", 2)
            entries = judge.lessons_for(root, "HEAD", FAR_FUTURE, global_claude_md=_global_claude_md(tmp))
            index = judge.render_lesson_index(entries)
            lines = index.splitlines()
            self.assertEqual(len(lines), len(entries))
            for e, line in zip(entries, lines):
                self.assertLessEqual(len(line), 300)  # R124, literally
                self.assertIn(e["id"], line)
                self.assertIn(e["status"], line)
            capped = next((l for l in lines if "a-bullet-whose-first-sentence-is-long" in l), "")
            self.assertEqual(len(capped), 300)  # this entry's full description is ~700 chars
            self.assertTrue(capped.endswith("…"))
            long_line = next((l for l in lines if "CLAUDE.md#a-long-rule (" in l), "")
            self.assertIn("## A long rule", long_line)
            self.assertIn("First sentence here.", long_line)
            self.assertNotIn("ENDMARK", index)

    def _assert_every_id_resolves(self, root, sha, kinds):
        lessons = judge._lessons.lessons_at(root, sha)
        cache = {}
        unresolved = [l.id for l in lessons if judge._lesson_anchor(root, sha, l, cache) is None]
        self.assertEqual(unresolved, [])
        paths = {l.source.partition(":")[2] for l in lessons}
        for kind in kinds:
            self.assertIn(kind, paths)  # not vacuous: every source kind named is present
        self.assertTrue(any(p.startswith(".codescout/memories/") for p in paths), paths)
        return lessons

    def test_every_lessons_at_id_resolves_to_an_anchor_for_every_source_kind(self):
        # R141: judge.py re-derives lessons.py's id minting to find each lesson's anchor; a drift
        # between the two would silently turn dated lessons `undatable`. LOAD-BEARING shapes:
        # a CLAUDE.md preamble, bullets, section prose and a repeated heading; OP, R and T
        # entries (T at level 3); a memory with a preamble and sections, and a section-less one.
        with tempfile.TemporaryDirectory() as tmp:
            root = self._repo(tmp)
            _write(root, "CLAUDE.md", "# P\n\nPreamble prose.\n\n## Rules\n\n"
                                      "- **Bold lead one.** body\n- **Bold lead two.** body\n\n"
                                      "Section prose.\n\n## Notes\n\nx.\n\n## Notes\n\ny.\n")
            _write(root, "docs/trackers/reconnaissance-patterns.md",
                   "# R\n\n## R-1 — a pattern\n\n**Status:** promoted\n\nBody.\n")
            _write(root, "docs/trackers/tool-usage-patterns.md",
                   "# T\n\n## Entries\n\n### T-1 — a pattern\n\n**Status:** promoted\n\nBody.\n")
            _write(root, ".codescout/memories/m.md",
                   "# M\n\nMemory preamble.\n\n## One\n\nx\n\n## Two\n\ny\n")
            _write(root, ".codescout/memories/sub/flat.md", "# Flat\n\nNo sections here.\n")
            sha = _commit(root, "every source kind", 1)
            kinds = ("CLAUDE.md", "docs/trackers/operator-rules.md",
                     "docs/trackers/reconnaissance-patterns.md",
                     "docs/trackers/tool-usage-patterns.md")
            ids = {l.id.partition(":")[2] for l in self._assert_every_id_resolves(root, sha, kinds)}
            for bare in ("CLAUDE.md#-preamble", "CLAUDE.md#rules/bold-lead-one", "CLAUDE.md#rules",
                         "CLAUDE.md#notes-2", "operator-rules.md#OP-1",
                         "reconnaissance-patterns.md#R-1", "tool-usage-patterns.md#T-1",
                         "memory:m#-preamble", "memory:m#two", "memory:sub/flat"):
                self.assertIn(bare, ids)

    def test_every_lessons_at_id_resolves_at_the_controls_tree(self):
        # R141, on the real repo at the controls' tree; skipped only when its objects are absent.
        # Its four source kinds: no tool-usage entry is promoted there, so T-N has no lesson.
        probe = subprocess.run(["git", "-C", str(REPO_ROOT), "cat-file", "-e", "27eded91^{commit}"],
                               capture_output=True)
        if probe.returncode != 0:
            self.skipTest("commit 27eded91 is not in this clone")
        kinds = ("CLAUDE.md", "docs/trackers/operator-rules.md",
                 "docs/trackers/reconnaissance-patterns.md")
        self.assertGreater(len(self._assert_every_id_resolves(
            REPO_ROOT, judge._resolve(REPO_ROOT, "27eded91"), kinds)), 200)


# --- R119: the Verdict ----------------------------------------------------------------------------

AUDIT_FULL = {
    "is_decision_point": True, "is_mistake": True, "lessons": ["L-1"],
    "lesson_outcomes": {"L-1": "missed", "L-2": "applied"}, "detectability": "in-trace",
    "evidence_present_before": "yes", "evidence_used": "no", "quote": "the earlier turn says 18",
}


class VerdictTests(unittest.TestCase):
    def test_a_quote_not_in_the_evidence_downgrades_to_abstain(self):
        evidence = "The earlier turn says\n  18 rows were closed."
        good = judge.Verdict(mode="audit", is_mistake=True, detectability="in-trace",
                             lessons=["L-1"], quote="earlier turn says 18 rows")  # re-wrapped
        self.assertEqual(judge.verify_quote(good, evidence), good)
        bad = dataclasses.replace(good, quote="the earlier turn says 17 rows")
        out = judge.verify_quote(bad, evidence)
        self.assertEqual(out.lessons, "abstain")
        self.assertIsNone(out.detectability)
        self.assertIn("quote_not_verbatim", out.flags)
        obtainable = dataclasses.replace(bad, detectability="obtainable")
        self.assertEqual(judge.verify_quote(obtainable, evidence).lessons, ["L-1"])  # in-trace only
        short = dataclasses.replace(good, quote="18 rows")  # a word, not a claim (< 12 chars)
        self.assertEqual(judge.verify_quote(short, evidence).lessons, "abstain")

    def test_audit_verdict_carries_context_and_transfer_fields(self):
        v = judge.parse_verdict(json.dumps(AUDIT_FULL), mode="audit",
                                offered_lessons=["L-1", "L-2"])
        self.assertEqual(v.evidence_present_before, "yes")
        self.assertEqual(v.evidence_used, "no")
        self.assertEqual(v.lesson_outcomes, {"L-1": "missed", "L-2": "applied"})
        self.assertEqual(v.flags, [])
        missing = {k: v_ for k, v_ in AUDIT_FULL.items()
                   if k not in ("evidence_present_before", "evidence_used", "lesson_outcomes")}
        m = judge.parse_verdict(json.dumps(missing), mode="audit", offered_lessons=["L-1", "L-2"])
        self.assertEqual(m.evidence_present_before, "unknown")
        self.assertEqual(m.evidence_used, "unknown")
        self.assertEqual(m.lesson_outcomes, {})
        for f in ("evidence_present_before", "evidence_used", "lesson_outcomes"):
            self.assertIn(f"missing:{f}", m.flags)
        bad_values = dict(AUDIT_FULL, evidence_used="probably",
                          lesson_outcomes={"L-1": "partly", "L-2": "applied"})
        b = judge.parse_verdict(json.dumps(bad_values), mode="audit", offered_lessons=["L-1", "L-2"])
        self.assertEqual(b.evidence_used, "unknown")
        self.assertEqual(b.lesson_outcomes, {"L-2": "applied"})
        self.assertIn("invalid:evidence_used", b.flags)
        self.assertTrue(any(f.startswith("invalid:lesson_outcomes") for f in b.flags), b.flags)

    def test_an_origin_uuid_not_offered_becomes_none(self):
        reply = {"is_correction": True, "origin_uuid": "zzz", "is_decision_point": True,
                 "lessons": "uncovered", "detectability": "obtainable", "quote": "x" * 20}
        v = judge.parse_verdict(json.dumps(reply), mode="correction", offered_uuids=["a1", "a2"])
        self.assertIsNone(v.origin_uuid)
        self.assertIn("origin_uuid_not_offered", v.flags)
        ok = judge.parse_verdict(json.dumps(dict(reply, origin_uuid="a2")), mode="correction",
                                 offered_uuids=["a1", "a2"])
        self.assertEqual(ok.origin_uuid, "a2")
        self.assertEqual(ok.lessons, "uncovered")
        self.assertIs(ok.is_correction, True)

    def test_a_missing_is_mistake_parses_to_none_and_is_flagged(self):
        reply = {k: v for k, v in AUDIT_FULL.items() if k != "is_mistake"}
        v = judge.parse_verdict(json.dumps(reply), mode="audit", offered_lessons=["L-1", "L-2"])
        self.assertIsNone(v.is_mistake)
        self.assertIn("missing:is_mistake", v.flags)
        garbage = judge.parse_verdict("I think it is fine.", mode="audit")
        self.assertIsNone(garbage.is_mistake)
        self.assertIn("unparseable", garbage.flags)

    def test_a_lesson_id_not_in_the_index_is_dropped_and_flagged(self):
        reply = dict(AUDIT_FULL, lessons=["L-1", "L-9"])
        v = judge.parse_verdict(json.dumps(reply), mode="audit", offered_lessons=["L-1", "L-2"])
        self.assertEqual(v.lessons, ["L-1"])
        self.assertIn("unknown_lesson:L-9", v.flags)
        only_bad = judge.parse_verdict(json.dumps(dict(AUDIT_FULL, lessons=["L-9"])), mode="audit",
                                       offered_lessons=["L-1", "L-2"])
        self.assertEqual(only_bad.lessons, "abstain")

    def test_a_null_boolean_is_an_abstention_not_a_type_error(self):
        # R139: `null` is the prompt's abstention -- None, flagged abstain:<field>.
        reply = dict(AUDIT_FULL, is_mistake=None, is_decision_point=None)
        v = judge.parse_verdict(json.dumps(reply), mode="audit", offered_lessons=["L-1", "L-2"])
        for f in ("is_mistake", "is_decision_point"):
            self.assertIsNone(getattr(v, f))
            self.assertIn(f"abstain:{f}", v.flags)
            self.assertNotIn(f"invalid:{f}", v.flags)
        c = judge.parse_verdict(json.dumps({"is_correction": None, "origin_uuid": None,
                                            "is_decision_point": True, "lessons": [],
                                            "detectability": None, "quote": ""}),
                                mode="correction")
        self.assertIsNone(c.is_correction)
        self.assertIn("abstain:is_correction", c.flags)
        typed = judge.parse_verdict(json.dumps(dict(AUDIT_FULL, is_mistake="yes")), mode="audit",
                                    offered_lessons=["L-1", "L-2"])
        self.assertIn("invalid:is_mistake", typed.flags)  # a string is still a type error
        self.assertIsNone(typed.is_mistake)
        for mode, fields in (("audit", ("is_decision_point", "is_mistake")),
                             ("correction", ("is_correction", "is_decision_point"))):
            prompt = _rendered_template(mode)
            for f in fields:  # the schema offers the abstention it is parsed as
                self.assertIn(f'"{f}": true or false or null', prompt, (mode, f))


# --- judge(): votes and majority -------------------------------------------------------------------


class _FakeComplete:
    def __init__(self, replies):
        self.replies = list(replies)
        self.calls = []
        self.lock = threading.Lock()

    def __call__(self, prompt, log_path=None):
        with self.lock:
            self.calls.append((prompt, log_path))
            return self.replies[len(self.calls) - 1]


def _doc_input(mode="audit", lessons=()):
    item = {"id": "doc/1", "decision": "Nothing reads the table.", "decision_ts": None,
            "document": {"sha": "b" * 40, "path": "docs/x.md",
                         "excerpts": ["Intro. Nothing reads the table. Later the sweep reads it."]}}
    if mode == "correction":
        item["correction"] = "False as written: the sweep reads it."
    return judge.build_input(item, None, list(lessons), mode)


class MajorityTests(unittest.TestCase):
    def test_majority_keeps_all_votes_and_the_disagreement(self):
        base = {"is_decision_point": True, "is_mistake": True, "lesson_outcomes": {},
                "evidence_present_before": "yes", "evidence_used": "no",
                "quote": "Later the sweep reads it."}
        replies = [json.dumps(dict(base, lessons="uncovered", detectability="in-trace")),
                   json.dumps(dict(base, lessons=[], detectability="in-trace")),
                   json.dumps(dict(base, lessons="abstain", detectability="obtainable",
                                   is_mistake=False))]
        fake = _FakeComplete(replies)
        with tempfile.TemporaryDirectory() as tmp:
            out = judge.judge(_doc_input(), votes=3, complete=fake, log_dir=pathlib.Path(tmp))
        self.assertEqual(len(fake.calls), 3)
        self.assertEqual(len({str(lp) for _p, lp in fake.calls}), 3)  # its own log_path each
        self.assertEqual(len(out["votes"]), 3)
        self.assertEqual(sorted(v["raw"] for v in out["votes"]), sorted(replies))
        self.assertEqual(out["majority"]["detectability"], "in-trace")
        self.assertIs(out["majority"]["is_mistake"], True)
        self.assertEqual(out["majority"]["lessons"], "abstain")  # three different answers
        self.assertEqual(out["disagreement"]["detectability"], 2)
        self.assertEqual(out["disagreement"]["lessons"], 3)
        self.assertEqual(out["disagreement"]["evidence_used"], 1)
        self.assertIn("lessons", out["split_fields"])
        self.assertNotIn("evidence_used", out["split_fields"])

    def test_votes_run_at_most_three_at_a_time(self):
        state = {"active": 0, "peak": 0}
        lock = threading.Lock()
        reply = json.dumps({"is_decision_point": True, "is_mistake": False, "lessons": [],
                            "lesson_outcomes": {}, "detectability": None,
                            "evidence_present_before": "yes", "evidence_used": "yes",
                            "quote": "Nothing reads the table."})

        def slow(prompt, log_path=None):
            with lock:
                state["active"] += 1
                state["peak"] = max(state["peak"], state["active"])
            time.sleep(0.05)
            with lock:
                state["active"] -= 1
            return reply

        out = judge.judge(_doc_input(), votes=6, complete=slow)
        self.assertEqual(len(out["votes"]), 6)
        self.assertEqual(state["peak"], 3)  # R122: at most 3 concurrent calls, and they do overlap

    def test_a_failed_attempt_is_reissued_at_most_twice_and_every_attempt_kept(self):
        # R137, pre-registered: call_failed / unparseable / tool_call is re-issued, up to 3
        # attempts per vote; the vote is the first attempt that is none of those.
        good = json.dumps({"is_decision_point": True, "is_mistake": True, "lessons": "uncovered",
                           "lesson_outcomes": {}, "detectability": "obtainable",
                           "evidence_present_before": "no", "evidence_used": "no",
                           "quote": "Nothing reads the table."})
        boom = RuntimeError("codex exec exit 1")
        tool = judge.ToolCallError({"item:command_execution": 1})
        cases = {  # name: (what each successive call does, failures recorded, vote is_mistake)
            "fails twice, then answers": ([boom, boom, good],
                                          ["call_failed", "call_failed", None], True),
            "fails every time": ([boom, boom, boom, good], ["call_failed"] * 3, None),
            "unparseable, then answers": (["no json here", good], ["unparseable", None], True),
            "a tool call, then answers": ([tool, good], ["tool_call", None], True),
            # A parseable reply with a flagged field is a vote, not a failure: never re-issued.
            "missing a field": ([json.dumps({"is_mistake": True}), good], [None], True),
        }
        for name, (script, failures, is_mistake) in cases.items():
            with self.subTest(name), tempfile.TemporaryDirectory() as tmp:
                calls = []

                def scripted(prompt, log_path=None, script=script, calls=calls):
                    calls.append(log_path)
                    step = script[len(calls) - 1]
                    if isinstance(step, Exception):
                        raise step
                    return step

                out = judge.judge(_doc_input(), votes=1, complete=scripted,
                                  log_dir=pathlib.Path(tmp))
                vote = out["votes"][0]
                attempts = vote.get("attempts") or []
                self.assertEqual(len(calls), len(failures))
                self.assertEqual([a["failure"] for a in attempts], failures)
                self.assertEqual(len({str(p) for p in calls}), len(calls))  # its own log each
                self.assertIs(vote["is_mistake"], is_mistake)
                if failures[-1] is not None:  # a vote whose every attempt failed stays failed
                    self.assertTrue(any(f.startswith("call_failed:") for f in vote["flags"]))

    def test_lesson_outcomes_take_a_strict_majority_per_lesson(self):
        # R135(b), the A1.4 transfer input: 2 of 3 `missed` gives `missed`; an applied / missed
        # / absent split for one id omits that id.
        lessons = [{"id": i, "source": "s", "status": "dated", "anchor": "a", "first_sentence": "b"}
                   for i in ("L-1", "L-2")]
        base = {"is_decision_point": True, "is_mistake": True, "lessons": ["L-1"],
                "detectability": "obtainable", "evidence_present_before": "yes",
                "evidence_used": "no", "quote": "Nothing reads the table."}
        fake = _FakeComplete([
            json.dumps(dict(base, lesson_outcomes={"L-1": "missed", "L-2": "applied"})),
            json.dumps(dict(base, lesson_outcomes={"L-1": "missed", "L-2": "missed"})),
            json.dumps(dict(base, lesson_outcomes={"L-1": "applied"}))])
        out = judge.judge(_doc_input(lessons=lessons), votes=3, complete=fake)
        self.assertEqual(out["majority"]["lesson_outcomes"], {"L-1": "missed"})
        self.assertEqual(out["disagreement"]["lesson_outcomes"], 3)

    def test_judge_checks_a_quote_against_the_pre_evidence_never_the_prompt(self):
        # R135(a): the prompt SHOWS the correction, so a quote of it is found in the prompt; it
        # is not pre-decision evidence, so the vote abstains. A context quote keeps its lessons.
        lessons = [{"id": "L-1", "source": "s", "status": "dated", "anchor": "a",
                    "first_sentence": "b"}]
        item = _doc_input("correction", lessons=lessons)
        base = {"is_correction": True, "origin_uuid": None, "is_decision_point": True,
                "lessons": ["L-1"], "detectability": "in-trace"}
        for quote, verified in (("False as written: the sweep reads it.", False),
                                ("Later the sweep reads it.", True)):
            with self.subTest(quote=quote):
                self.assertIn(quote, item["prompt"])
                fake = _FakeComplete([json.dumps(dict(base, quote=quote))])
                vote = judge.judge(item, votes=1, complete=fake)["votes"][0]
                if verified:
                    self.assertEqual((vote["lessons"], vote["detectability"]), (["L-1"], "in-trace"))
                    self.assertNotIn("quote_not_verbatim", vote["flags"])
                else:
                    self.assertEqual((vote["lessons"], vote["detectability"]), ("abstain", None))
                    self.assertIn("quote_not_verbatim", vote["flags"])
                    self.assertIs(vote["is_correction"], True)  # the rest of the answer stands



# --- R122: the Codex channel (built, never called) -------------------------------------------------


_CLEAN_EVENTS = [  # a --json stream with no tool, exec or file-read event (codex-cli 0.154.0 shapes)
    {"type": "thread.started", "thread_id": "t-1"}, {"type": "turn.started"},
    {"type": "item.started", "item": {"id": "i0", "type": "reasoning", "text": "r"}},
    {"type": "item.updated", "item": {"id": "i0", "type": "reasoning", "text": "r2"}},
    {"type": "item.completed", "item": {"id": "i1", "type": "agent_message", "text": "a"}},
    {"type": "item.completed", "item": {"id": "i2", "type": "error", "message": "fallback"}},
    {"type": "error", "message": "Reconnecting... 2/5"},
    {"type": "turn.completed", "usage": {"input_tokens": 1, "output_tokens": 1}},
]
_TOOL_EVENT = {"type": "item.started",
               "item": {"id": "i9", "type": "command_execution", "command": "cat answers.md"}}


class _FakeRunner:
    """Stands in for `subprocess.run` under CodexChannel and starts no process. It records every
    argv with its stdin and env, and answers `codex --version`, the precondition's `test -e`
    (0 for a path the jail binds back or one listed in `visible`, else 1 -- a jail whose binds
    work), and `codex exec` (writing `reply` to its `-o` file and `events` as its --json
    stdout)."""

    def __init__(self, visible=(), version="codex-cli 0.154.0", reply="{}", events=None,
                 bound_visible=True):
        self.visible, self.version, self.reply = {str(p) for p in visible}, version, reply
        self.events = _CLEAN_EVENTS if events is None else events
        self.bound_visible = bound_visible
        self.calls, self.workdir_was_empty = [], []

    def __call__(self, argv, input=None, env=None, **_kw):
        argv = list(argv)
        self.calls.append({"argv": argv, "input": input, "env": dict(env or {})})
        cmd = argv[argv.index("--") + 1:] if argv[0] == "bwrap" else argv
        bound = {argv[i + 1] for i, a in enumerate(argv) if a == "--bind"}
        if cmd[:2] == ["codex", "--version"]:
            return subprocess.CompletedProcess(argv, 0, self.version + "\n", "")
        if cmd[:2] == ["codex", "sandbox"]:
            seen = cmd[-1] in self.visible or (self.bound_visible and cmd[-1] in bound)
            return subprocess.CompletedProcess(argv, 0 if seen else 1, "", "")
        if cmd[:2] == ["codex", "exec"]:
            work = pathlib.Path(cmd[cmd.index("-C") + 1])
            self.workdir_was_empty.append(list(work.iterdir()) == [])
            pathlib.Path(cmd[cmd.index("-o") + 1]).write_text(self.reply)
            out = "".join(json.dumps(e) + "\n" for e in self.events)
            return subprocess.CompletedProcess(argv, 0, out, "codex stderr")
        raise AssertionError(f"unexpected command {argv!r}")

    def ran(self, word):
        return [c for c in self.calls if word in c["argv"]]


def _home_dir(tmp):
    """A stand-in home: `.codex/auth.json` is a LINK to a file elsewhere (LOAD-BEARING: only a
    chained link tells the resolved file, which the jail binds, from the link path). Its content
    is a placeholder and is never read."""
    home = pathlib.Path(tmp) / "home"
    real = pathlib.Path(tmp) / "store" / "auth-real.json"
    real.parent.mkdir(parents=True)
    real.write_text("placeholder")
    (home / ".codex").mkdir(parents=True)
    (home / ".codex" / "auth.json").symlink_to(real)
    return home, real


class ChannelTests(unittest.TestCase):
    def test_the_codex_home_pins_the_campaign_model_and_links_only_auth(self):
        gs = judge._gs()  # loaded BY PATH from the rule-tell campaign (R122)
        self.assertEqual((gs.CODEX_MODEL, gs.CODEX_EFFORT), ("gpt-6-astra", "medium"))
        home = judge.new_codex_home(gs.CODEX_MODEL, gs.CODEX_EFFORT)
        try:
            self.assertEqual(sorted(p.name for p in home.iterdir()), ["auth.json", "config.toml"])
            self.assertTrue((home / "auth.json").is_symlink())  # the link, never its content
            self.assertEqual(os.readlink(home / "auth.json"),
                             os.path.realpath(pathlib.Path.home() / ".codex" / "auth.json"))
            # R133(d): subscription login only, and no in-app update.
            self.assertEqual((home / "config.toml").read_text(),
                             'model = "gpt-6-astra"\nmodel_reasoning_effort = "medium"\n'
                             'forced_login_method = "chatgpt"\n\n[features]\n'
                             'in_app_updates = false\n')
        finally:
            shutil.rmtree(home)

    def test_a_vote_goes_on_stdin_inside_the_jail_with_three_paths_bound(self):
        # R133(a)-(e), with a fake runner: the argv, the bind list, stdin and env are exactly
        # what a real vote would get, and no process starts.
        keys = {"OPENAI_API_KEY": "k1", "CODEX_API_KEY": "k2", "OPENAI_BASE_URL": "k3"}
        with tempfile.TemporaryDirectory() as tmp, mock.patch.dict(os.environ, keys):
            home, real = _home_dir(tmp)
            fake = _FakeRunner(reply='{"ok": 1}')
            ch = judge.CodexChannel(runner=fake, home_dir=home, uid=4242)
            try:
                self.assertEqual((ch.model, ch.effort, ch.version),
                                 ("gpt-6-astra", "medium", "codex-cli 0.154.0"))
                self.assertEqual(os.readlink(ch.home / "auth.json"), str(real))
                prompt = "THE RENDERED PROMPT\nsecond line"
                log = pathlib.Path(tmp) / "v1.log"
                self.assertEqual(ch.complete(prompt, log), '{"ok": 1}')
                ex = fake.ran("exec")
                self.assertEqual(len(ex), 1)
                argv = ex[0]["argv"]
                sep = argv.index("--")
                cmd = argv[sep + 1:]
                work = cmd[cmd.index("-C") + 1] if "-C" in cmd else "?"
                self.assertEqual(argv[:sep], [
                    "bwrap", "--die-with-parent", "--ro-bind", "/", "/", "--dev", "/dev",
                    "--proc", "/proc", "--tmpfs", str(home), "--tmpfs", "/tmp",
                    "--tmpfs", "/run/user/4242",
                    "--bind", str(ch.home), str(ch.home), "--bind", str(real), str(real),
                    "--bind", work, work])
                self.assertEqual(cmd, [
                    "codex", "exec", "--skip-git-repo-check", "--sandbox", "read-only", "-c",
                    "approval_policy=never", "--ephemeral", "--json", "--strict-config",
                    "-o", str(pathlib.Path(work) / "last.txt"), "-C", work, "-"])
                self.assertEqual(ex[0]["input"], prompt)  # the whole user message, on stdin
                self.assertFalse(any("RENDERED PROMPT" in a or "task.md" in a for a in argv))
                self.assertEqual(fake.workdir_was_empty, [True])
                self.assertFalse(pathlib.Path(work).exists())
                env = ex[0]["env"]
                self.assertFalse(set(keys) & set(env))  # subscription only
                self.assertEqual(env.get("CODEX_HOME"), str(ch.home))
                self.assertIn('"turn.completed"', log.read_text())  # the --json stream is logged
            finally:
                ch.close()
            self.assertFalse(ch.home.exists())

    def test_the_jail_precondition_refuses_a_visible_answer_path(self):
        with tempfile.TemporaryDirectory() as tmp:
            home, _real = _home_dir(tmp)
            fake = _FakeRunner()
            ch = judge.CodexChannel(runner=fake, home_dir=home, uid=4242)
            ch.close()
            hidden = [str(judge.REPO_ROOT), str(judge.REPO_ROOT / judge.RTD_DOC),
                      str(home / ".claude"), str(home / ".claude-sdd"), str(home / ".claude-kat"),
                      "/tmp/claude-4242"]
            rows = ch.precondition
            self.assertEqual([r["path"] for r in rows][:6], hidden)
            self.assertEqual([r["rc"] for r in rows], [1] * 6 + [0])  # the workdir is visible
            pre = fake.ran("sandbox")
            self.assertEqual(len(pre), 7)
            for c in pre:  # inside the SAME jail a vote runs in; a local command, not a model call
                sep = c["argv"].index("--")
                self.assertEqual(c["argv"][:15], [
                    "bwrap", "--die-with-parent", "--ro-bind", "/", "/", "--dev", "/dev",
                    "--proc", "/proc", "--tmpfs", str(home), "--tmpfs", "/tmp",
                    "--tmpfs", "/run/user/4242"])
                self.assertEqual(c["argv"].count("--bind"), 3)
                self.assertEqual(c["argv"][sep + 1:sep + 8],
                                 ["codex", "sandbox", "-c", "sandbox_mode=read-only", "--", "test",
                                  "-e"])
            self.assertEqual(fake.ran("exec"), [])
            for path in hidden:
                with self.subTest(visible=path):
                    f = _FakeRunner(visible=[path])
                    with self.assertRaisesRegex(judge.JailError, re.escape(path)):
                        judge.CodexChannel(runner=f, home_dir=home, uid=4242)
                    self.assertEqual(f.ran("exec"), [])
            blind = _FakeRunner(bound_visible=False)  # a jail that cannot see its own workdir
            with self.assertRaisesRegex(judge.JailError, "visible"):
                judge.CodexChannel(runner=blind, home_dir=home, uid=4242)

    def test_a_vote_whose_stream_shows_a_tool_event_is_a_failed_tool_call_vote(self):
        stream = "".join(json.dumps(e) + "\n" for e in _CLEAN_EVENTS) + "\n"
        self.assertEqual(judge.tool_events(stream), {})
        for ev, kind in (
                (_TOOL_EVENT, "item:command_execution"),
                ({"type": "item.completed", "item": {"type": "file_change"}}, "item:file_change"),
                ({"type": "item.completed", "item": {"type": "mcp_tool_call"}}, "item:mcp_tool_call"),
                ({"type": "item.completed", "item": {"type": "collab_tool_call"}},
                 "item:collab_tool_call"),
                ({"type": "item.started", "item": {"type": "web_search"}}, "item:web_search"),
                ({"type": "item.updated", "item": {"type": "todo_list"}}, "item:todo_list"),
                ({"type": "item.completed", "item": {"type": "a_future_kind"}}, "item:a_future_kind"),
                ({"type": "item.completed"}, "item:None"),
                ({"type": "a.future.event"}, "event:a.future.event")):
            with self.subTest(kind=kind):  # an unknown kind fails closed
                self.assertEqual(judge.tool_events(stream + json.dumps(ev) + "\n"), {kind: 1})
        self.assertEqual(judge.tool_events(stream + "not a json line\n"), {"unparsed_line": 1})
        reply = json.dumps({"is_decision_point": True, "is_mistake": True, "lessons": "uncovered",
                            "lesson_outcomes": {}, "detectability": "obtainable",
                            "evidence_present_before": "no", "evidence_used": "no",
                            "quote": "Nothing reads the table."})
        with tempfile.TemporaryDirectory() as tmp:
            home, _real = _home_dir(tmp)
            for events, tool in ((_CLEAN_EVENTS, False), (_CLEAN_EVENTS + [_TOOL_EVENT], True)):
                ch = judge.CodexChannel(runner=_FakeRunner(reply=reply, events=events),
                                        home_dir=home, uid=4242)
                try:
                    out = judge.judge(_doc_input(), votes=1, complete=ch.complete,
                                      log_dir=pathlib.Path(tmp) / "logs")
                finally:
                    ch.close()
                vote = out["votes"][0]
                with self.subTest(tool=tool):
                    if tool:  # recorded as a failed vote, never dropped, and re-issued (R137)
                        self.assertIn('tool_call:{"item:command_execution": 1}', vote["flags"])
                        self.assertIsNone(vote["is_mistake"])
                        self.assertIsNone(out["majority"]["is_mistake"])
                        self.assertEqual([a["failure"] for a in vote.get("attempts") or []],
                                         ["tool_call"] * 3)
                    else:
                        self.assertEqual(vote["flags"], [])
                        self.assertIs(out["majority"]["is_mistake"], True)

    def test_the_header_describes_the_channel_from_the_argv_it_builds(self):
        with tempfile.TemporaryDirectory() as tmp:
            home, _real = _home_dir(tmp)
            ch = judge.CodexChannel(runner=_FakeRunner(), home_dir=home, uid=4242)
            ch.close()
            live = ch.describe()
        dry = judge.gate_header(REPO_ROOT, REPO_ROOT / judge.RTD_DOC,
                                REPO_ROOT / judge.CONTROLS_DOC, 3, True, version="v")["channel"]
        self.assertEqual(live, dry)  # the live description is the dry one, with its paths labelled
        for part in ("--tmpfs <home> --tmpfs /tmp --tmpfs /run/user/<uid>",
                     "--bind <CODEX_HOME> <CODEX_HOME> --bind <auth.json target> "
                     "<auth.json target> --bind <workdir> <workdir> --",
                     "codex exec --skip-git-repo-check --sandbox read-only -c approval_policy=never "
                     "--ephemeral --json --strict-config -o <workdir>/last.txt -C <workdir> - ",
                     "OPENAI_API_KEY/CODEX_API_KEY/OPENAI_BASE_URL", 'forced_login_method = "chatgpt"'):
            self.assertIn(part, dry)
        self.assertNotIn(tmp, live)
        with mock.patch.object(judge, "_CODEX_EXEC_FLAGS", judge._CODEX_EXEC_FLAGS + ("--zz-probe",)):
            probed = judge.gate_header(REPO_ROOT, REPO_ROOT / judge.RTD_DOC,
                                       REPO_ROOT / judge.CONTROLS_DOC, 3, True, version="v")
        self.assertIn("--strict-config --zz-probe -o", probed["channel"])  # derived, not written



# --- R120 / the gate -------------------------------------------------------------------------------


def _result(kind, rid, mode, *, td=None, peer_yes=False, detectability=None, is_mistake=None):
    return {"id": rid, "case": rid.split("/")[0], "kind": kind, "mode": mode,
            "expected": {"text_detectable": td, "peer_yes": peer_yes,
                         "lesson": "uncovered" if mode == "correction" else None},
            "majority": {"detectability": detectability, "is_mistake": is_mistake,
                         "lessons": ["wrong-lesson"]}}


def _gate_results(agree=16, peer_yes=3, yes=6, fires=5):
    res = []
    for i in range(21):
        res.append(_result("rtd", f"RTD-{i + 1}/correction", "correction", td="partial",
                           detectability="obtainable" if i < agree else "external"))
    yes_cases = ("RTD-3", "RTD-8", "RTD-9", "RTD-10", "RTD-15", "RTD-17", "RTD-18", "RTD-20")
    for i, case in enumerate(yes_cases):
        is_peer = i < 4  # the spec's peer x yes cell is exactly the first four
        flag = (i < peer_yes) if is_peer else (i - 4 < yes - peer_yes)
        res.append(_result("rtd", f"{case}/audit", "audit", td="yes", peer_yes=is_peer,
                           is_mistake=flag))
    for i in range(52):
        res.append(_result("control", f"CTL-{i}/audit", "audit", is_mistake=i < fires))
    return res


class GateTests(unittest.TestCase):
    def test_gate_thresholds_match_the_spec(self):
        self.assertEqual(judge.GATE_DETECTABILITY_MIN, (16, 21))
        self.assertEqual(judge.GATE_PEER_YES_MIN, (3, 4))
        self.assertEqual(judge.GATE_YES_MIN, (6, 8))
        self.assertEqual(judge.GATE_CONTROL_FIRE_MAX, (5, 52))
        self.assertEqual(judge.TEXT_DETECTABLE_TO_DETECTABILITY,
                         {"yes": "in-trace", "partial": "obtainable", "no": "external"})

    def test_the_gate_score_applies_each_threshold_at_its_boundary(self):
        self.assertTrue(judge.score_gate(_gate_results())["passed"])
        for kwargs in ({"agree": 15}, {"peer_yes": 2, "yes": 6}, {"yes": 5}, {"fires": 6}):
            with self.subTest(**kwargs):
                s = judge.score_gate(_gate_results(**kwargs))
                self.assertFalse(s["passed"])
        s = judge.score_gate(_gate_results())
        self.assertEqual(s["checks"]["detectability"]["count"], 16)
        self.assertEqual(s["checks"]["control_fires"]["count"], 5)
        # Lesson agreement is REPORTED, never a pass condition (R120 / R109 revised): every
        # correction majority names a wrong lesson against an expected `uncovered`, so agreement
        # is 0 of 21 scoreable, and the gate still passes.
        self.assertEqual(s["lesson_assignment"]["agree"], 0)
        self.assertEqual(s["lesson_assignment"]["scoreable"], 21)
        self.assertTrue(s["passed"])

    def test_the_gate_score_refuses_a_population_of_the_wrong_size(self):
        res = _gate_results()[:-1]  # 51 controls
        with self.assertRaises(ValueError):
            judge.score_gate(res)

    def test_the_leak_scan_reports_each_kind_of_leak(self):
        # R135(c), a positive control per branch, beside a clean input it must not report: the
        # dry gate's "0 of 81" is a claim only a detector seen firing can make.
        def entry(text):
            return {"id": "L-1", "source": "s", "status": "dated", "anchor": text,
                    "first_sentence": ""}

        items = [{"id": "clean", "must_not_contain": ["absent everywhere"]},
                 {"id": "gate-id", "must_not_contain": []},
                 {"id": "in-evidence", "must_not_contain": ["the negative, verbatim"]},
                 {"id": "in-index", "must_not_contain": ["the other negative"]}]
        inputs = [{"prompt": "fine", "pre_evidence": "fine", "lessons": [entry("fine")]},
                  {"prompt": "see RTD-1 here", "pre_evidence": "", "lessons": []},
                  {"prompt": "", "pre_evidence": "x the negative,\n verbatim y", "lessons": []},
                  {"prompt": "", "pre_evidence": "", "lessons": [entry("the other negative")]}]
        with tempfile.TemporaryDirectory() as tmp:
            leaky = pathlib.Path(tmp) / "template.md"
            leaky.write_text("a template naming rule-tell\n")
            with mock.patch.object(judge, "PROMPT_PATH", leaky):
                scan = judge._leak_scan(items, inputs)
        self.assertEqual(scan["gate_id_tokens_by_item"], {"gate-id": 1})
        self.assertEqual(scan["own_correction_in_evidence_or_index"], ["in-evidence", "in-index"])
        self.assertEqual(scan["gate_id_tokens_in_template"], 1)
        self.assertEqual(judge._leak_scan(items[:1], inputs[:1])["gate_id_tokens_in_template"], 0)

    def test_the_real_documents_yield_21_correction_and_60_audit_items(self):
        probe = subprocess.run(["git", "-C", str(REPO_ROOT), "cat-file", "-e",
                                "78f7662cab533dc642cf217d1b645d722479ea39^{commit}"],
                               capture_output=True)
        if probe.returncode != 0:  # a shallow clone lacks the corpus SHAs; nothing else skips
            self.skipTest("the RTD corpus commit is not in this clone")
        items = judge.gate_items(REPO_ROOT)
        modes = [(i["kind"], i["mode"]) for i in items]
        self.assertEqual(modes.count(("rtd", "correction")), 21)
        self.assertEqual(modes.count(("rtd", "audit")), 8)
        self.assertEqual(modes.count(("control", "audit")), 52)
        peer_yes = sorted(i["case"] for i in items if i["mode"] == "audit" and i["kind"] == "rtd"
                          and i["expected"]["peer_yes"])
        self.assertEqual(peer_yes, ["RTD-10", "RTD-3", "RTD-8", "RTD-9"])
        self.assertEqual(len({i["id"] for i in items}), 81)
        self.assertEqual(judge.check_population(items),
                         {"correction": 21, "audit_rtd": 8, "peer_yes": 4, "controls": 52})
        judge.check_no_law(items)  # exactly RTD-8/9/15 expect `uncovered`
        flipped = [dict(i, expected=dict(i["expected"], lesson=None))
                   if i["id"] == "RTD-9/correction" else i for i in items]
        with self.assertRaises(ValueError):
            judge.check_no_law(flipped)
        rtd8 = next(i for i in items if i["id"] == "RTD-8/audit")
        # RTD-8's positive field quotes its falsifier in a SECOND fenced block: the decision is
        # the first; the falsifier is context, and must be in it.
        self.assertTrue(rtd8["decision"].startswith("It is not codescout's table"))
        self.assertIn("A referenced row survives the prune.", "\n".join(rtd8["document"]["excerpts"]))
        for it in items:
            if it["kind"] != "rtd":
                continue
            doc = it["document"]
            blob = subprocess.run(["git", "-C", str(REPO_ROOT), "show", f"{doc['sha']}:{doc['path']}"],
                                  capture_output=True, text=True, check=True).stdout
            self.assertIn(it["decision"], blob, it["id"])
            if it["source"]["side"] == "-":  # in place: the positive is gone at the correction
                after = subprocess.run(
                    ["git", "-C", str(REPO_ROOT), "show", f"{it['source']['correction_sha']}:{doc['path']}"],
                    capture_output=True, text=True, check=True).stdout
                self.assertNotIn(it["decision"], after, it["id"])


def _fixture_gate_repo(tmp):
    """A synthetic corpus in the RTD / controls formats. doc.md's positive ONE is replaced in
    place at c2 (so it is ABSENT at the correction sha), and positive TWO is corrected by an
    APPENDED retraction at c3 (so, read at c3, its context would carry its own correction).

    LOAD-BEARING for R140/R144: operator-rules.md first appears at c1, so a control originating
    at c0 (CTL1-1) predates it -- its undated set must come from the reference sha (the tree),
    since `undated_lessons` raises at c0. `## Gamma note` exists in CLAUDE.md at c1 and c2 only,
    and CTLX-1's text B names c2 as its origin, so Gamma is a dated lesson at CTLX-1's origin
    (c2) and absent at the tree (c3): only a freeze at the ORIGIN lists it. (B's origin is the
    fixture's metadata; its text itself exists from c0 on.)"""
    root = pathlib.Path(tmp) / "repo"
    _init_repo(root)
    claude_md = ("# P\n\n## Alpha rules\n\n- **Always state the window.** Because.\n"
                 "\n## Beta\n\nA quiet control sentence that is fine.\n"
                 "\nAnother control line, also fine.\n")
    gamma = "\n## Gamma note\n\nA passing remark.\n"
    _write(root, "CLAUDE.md", claude_md)
    c0 = _commit(root, "base", 1)
    _write(root, "docs/trackers/operator-rules.md", _OP_RULES)
    _write(root, "CLAUDE.md", claude_md + gamma)
    _write(root, "docs/sub/doc.md", "Intro paragraph.\n\nPOSITIVE ONE: all nine rows are closed.\n\n"
                                    "POSITIVE TWO: nothing reads the table.\n\nClosing paragraph.\n")
    c1 = _commit(root, "publish", 2)
    _write(root, "docs/sub/doc.md", "Intro paragraph.\n\nNEGATIVE ONE: seven of nine rows are closed.\n\n"
                                    "POSITIVE TWO: nothing reads the table.\n\nClosing paragraph.\n")
    c2 = _commit(root, "correct in place", 3)
    _write(root, "CLAUDE.md", claude_md)
    _write(root, "docs/sub/doc.md", "Intro paragraph.\n\nNEGATIVE ONE: seven of nine rows are closed.\n\n"
                                    "POSITIVE TWO: nothing reads the table.\n\nClosing paragraph.\n\n"
                                    "NEGATIVE TWO: the sweep reads the table.\n")
    c3 = _commit(root, "append a retraction", 4)
    rtd = f"""# Synthetic RTD

## Case format

**Three cases (RTD-2) have no law in the corpus that names their tell** (synthetic).

## Cases

### Case RTD-1 — replaced in place

- **rule:** `CLAUDE.md` § *Alpha rules* — *"state the window"*.
- **tell:** Does it?
- **positive:**

```
POSITIVE ONE: all nine rows are closed.
```

- **negative:**

```
NEGATIVE ONE: seven of nine rows are closed.
```

- **source:** `{c2[:8]}`, `docs/sub/doc.md`, `-` / `+` (in place).
- **caught_by:** `peer`.
- **text_detectable:** `yes` — synthetic.

### Case RTD-2 — appended

- **rule:** no law in the corpus names this tell.
- **tell:** Does it?
- **positive:**

```
POSITIVE TWO: nothing reads the table.
```

- **negative:**

```
NEGATIVE TWO: the sweep reads the table.
```

- **source:** positive from `{c1[:8]}` (`+` side); negative from `{c3[:8]}`. **Appended.**
- **caught_by:** `measurement`.
- **text_detectable:** `partial`.

## Candidates examined that yielded no case

Nothing.
"""
    controls = f"""# Synthetic controls

**Tree and instant:** built at `{c3}`, branch `x`.

## Passages for the RTD-1 prompt

### Passage CTL1-1 — a control

- **for_prompt:** RTD-1
- **text:**

```
A quiet control sentence that is fine.
```

- **source:** `CLAUDE.md:9`
- **why_it_resembles:** near-miss.
- **never_corrected:** needle `quiet control` — one commit, `{c0[:8]}` 2026-01-01.

### Passage CTL1-2 — a known positive

- **for_prompt:** RTD-1
- **text:**

```
POSITIVE TWO: nothing reads the table.
```

- **source:** `docs/sub/doc.md:5`
- **why_it_resembles:** full-shape.
- **never_corrected:** withheld — see § *Answer key*.

### Passage CTLX-1 — a pair

- **for_prompt:** contradiction
- **text (A):**

```
- **Always state the window.** Because.
```

- **text (B):**

```
Another control line, also fine.
```

- **source:** `CLAUDE.md:5` and `CLAUDE.md:11`
- **why_it_resembles:** elaboration.
- **never_corrected:** A, needle `state the window` — one commit, `{c0[:8]}` 2026-01-01. B, needle
  `Another control` — one commit, `{c2[:8]}` 2026-01-03.

## Answer key

| id | section | case | blob |
|---|---|---|---|
| `CTL1-2` | RTD-1 | Case RTD-2 | `{c1[:8]}` |

## Status

- done
"""
    _write(root, "eval/rtd.md", rtd)
    _write(root, "eval/controls.md", controls)
    return root, {"c0": c0, "c1": c1, "c2": c2, "c3": c3}


class GateItemTests(unittest.TestCase):
    def test_each_positive_is_read_from_the_pre_correction_blob(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, c = _fixture_gate_repo(tmp)
            items = judge.gate_items(root, rtd_doc=root / "eval/rtd.md",
                                     controls_doc=root / "eval/controls.md")
            by_id = {i["id"]: i for i in items}
            self.assertEqual(sorted(by_id), ["CTL1-1/audit", "CTLX-1/audit", "RTD-1/audit",
                                             "RTD-1/correction", "RTD-2/correction"])
            one = by_id["RTD-1/correction"]
            self.assertEqual(one["document"]["sha"], c["c1"])  # c2^: the positive is gone at c2
            self.assertEqual(one["decision"], "POSITIVE ONE: all nine rows are closed.")
            self.assertEqual(one["correction"], "NEGATIVE ONE: seven of nine rows are closed.")
            self.assertEqual(one["lesson_freeze_sha"], c["c1"])
            self.assertEqual(join.utc(one["lesson_decision_ts"]), join.utc(_day(2)))
            two = by_id["RTD-2/correction"]
            self.assertEqual(two["document"]["sha"], c["c1"])  # the positive's own + side
            joined = "\n".join(two["document"]["excerpts"])
            self.assertIn("POSITIVE TWO", joined)
            self.assertNotIn("NEGATIVE TWO", joined)
            # The structural guard is wired: build_input refuses an RTD item whose context
            # carries its own correction.
            self.assertEqual(two["must_not_contain"], ["NEGATIVE TWO: the sweep reads the table."])
            self.assertEqual(one["must_not_contain"], ["NEGATIVE ONE: seven of nine rows are closed."])
            self.assertNotIn("correction", by_id["RTD-1/audit"])
            self.assertEqual(by_id["RTD-1/audit"]["expected"]["peer_yes"], True)
            self.assertEqual(by_id["RTD-2/correction"]["expected"]["lesson"], "uncovered")
            pair = by_id["CTLX-1/audit"]
            self.assertIn("Always state the window.", pair["decision"])
            self.assertIn("Another control line", pair["decision"])
            # R140/R144: a control's repo lessons are frozen at its LATEST introducing commit
            # (CTLX-1: c0 for A, c2 for B), never at the controls' tree (c3); its undated set's
            # reference sha is the tree, for every control.
            self.assertEqual(pair["lesson_freeze_sha"], c["c2"])
            self.assertEqual(join.utc(pair["lesson_decision_ts"]), join.utc(_day(3)))
            self.assertEqual(by_id["CTL1-1/audit"]["lesson_freeze_sha"], c["c0"])
            for cid in ("CTL1-1/audit", "CTLX-1/audit"):
                self.assertEqual(by_id[cid]["lesson_undated_sha"], c["c3"])
            self.assertNotIn("lesson_undated_sha", one)  # RTD items: unchanged (R144 item 3)
            for it in items:  # every gate input builds, and none refuses as a leak
                judge.build_input(it, None, [], it["mode"])

    def test_the_dry_gate_makes_no_complete_call(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, _c = _fixture_gate_repo(tmp)
            counter = _FakeComplete([])
            out = pathlib.Path(tmp) / "dry.txt"
            with mock.patch.object(run.judge, "_codex_version", return_value="codex-cli test"):
                rc = run.main(["gate", "--dry", "--repo", str(root),
                               "--rtd-doc", str(root / "eval/rtd.md"),
                               "--controls-doc", str(root / "eval/controls.md"),
                               "--global-claude-md", str(_global_claude_md(tmp)),
                               "--any-population", "--out", str(out)], complete=counter)
            self.assertEqual(rc, 0)
            self.assertEqual(counter.calls, [])
            text = out.read_text()
            for rid in ("RTD-1/correction", "RTD-1/audit", "RTD-2/correction", "CTL1-1/audit",
                        "CTLX-1/audit"):
                self.assertIn(rid, text)
            self.assertIn("codex-cli test", text)
            self.assertIn("model: gpt-6-astra", text)
            self.assertIn("effort: medium", text)
            self.assertIn("total prompt chars", text)
            self.assertIn("complete() calls: 0", text)
    def test_a_live_gate_on_a_fake_channel_counts_every_call_and_reports(self):
        # Live code path, fake channel: no model call. A live run on the real channel without a
        # log dir is refused before any channel is built.
        with tempfile.TemporaryDirectory() as tmp:
            root, _c = _fixture_gate_repo(tmp)
            kwargs = dict(repo=root, rtd_doc=root / "eval/rtd.md",
                          controls_doc=root / "eval/controls.md",
                          global_claude_md=_global_claude_md(tmp), population=None)
            with self.assertRaises(ValueError):
                judge.run_gate(dry=False, complete=None, log_dir=None, **kwargs)
            self.assertIsNone(judge._CHANNEL)
            # LOAD-BEARING: the spec's votes and population, so the R138 refusals (which run
            # later) admit this input and only the log_dir guard can refuse it. With population
            # None, R138 refused it first and the log_dir guard went untested (mutant Q5
            # survived, fix round 1). A channel built here means the guard is gone.
            never = mock.Mock(side_effect=AssertionError("a channel was built with no log dir"))
            with mock.patch.object(judge, "CodexChannel", never), \
                    self.assertRaisesRegex(ValueError, "needs log_dir"):
                judge.run_gate(dry=False, complete=None, log_dir=None,
                               **dict(kwargs, population=judge.GATE_POPULATION))
            self.assertIsNone(judge._CHANNEL)
            reply = json.dumps({"is_correction": True, "origin_uuid": None, "is_decision_point": True,
                                "is_mistake": True, "lessons": "uncovered", "lesson_outcomes": {},
                                "detectability": "obtainable", "evidence_present_before": "no",
                                "evidence_used": "unknown", "quote": "a claim quoted here"})
            fake = _FakeComplete([reply] * 15)
            with mock.patch.object(judge, "_codex_version", return_value="codex-cli test"):
                res = judge.run_gate(dry=False, complete=fake, log_dir=pathlib.Path(tmp) / "logs",
                                     **kwargs)
            self.assertEqual(res["calls"], 15)  # 5 items x 3 votes
            self.assertEqual(len(fake.calls), 15)
            self.assertEqual(len({str(lp) for _p, lp in fake.calls}), 15)
            text = judge.format_gate(res)
            self.assertIn("PASSED: False", text)  # 1 correction item agrees, far below 16
            self.assertIn("| RTD-2/correction | obtainable | obtainable | None |", text)
            self.assertIn("complete() calls: 15", text)

    def _gate_kwargs(self, root, tmp, **extra):
        return dict(repo=root, rtd_doc=root / "eval/rtd.md", controls_doc=root / "eval/controls.md",
                    global_claude_md=_global_claude_md(tmp), **extra)

    def test_a_live_gate_refuses_to_start_while_the_leak_scan_reports_anything(self):
        # R135(d): the detector is a gate. Each of the scan's three findings refuses alone.
        clean = {"own_correction_in_evidence_or_index": [], "gate_id_tokens_by_item": {},
                 "gate_id_tokens_in_template": 0}
        with tempfile.TemporaryDirectory() as tmp:
            root, _c = _fixture_gate_repo(tmp)
            kwargs = self._gate_kwargs(root, tmp, population=None,
                                       log_dir=pathlib.Path(tmp) / "logs")
            for finding in ({"own_correction_in_evidence_or_index": ["RTD-1/correction"]},
                            {"gate_id_tokens_by_item": {"CTL1-1/audit": 1}},
                            {"gate_id_tokens_in_template": 1}):
                with self.subTest(finding=sorted(finding)), \
                        mock.patch.object(judge, "_leak_scan", return_value=dict(clean, **finding)), \
                        mock.patch.object(judge, "_codex_version", return_value="codex-cli test"):
                    fake = _FakeComplete([])
                    with self.assertRaises(judge.LeakageError):
                        judge.run_gate(dry=False, complete=fake, **kwargs)
                    self.assertEqual(fake.calls, [])
                    dry = judge.run_gate(dry=True, complete=fake, **kwargs)  # a dry run reports it
                    self.assertEqual(dry["leak_scan"], dict(clean, **finding))

    def test_a_live_gate_on_the_codex_channel_refuses_non_spec_parameters(self):
        # R138: 3 votes, the spec's population and codex-cli 0.154.0, or no live gate; the flags
        # stay for dry runs and for fixtures (a `complete` injected).
        with tempfile.TemporaryDirectory() as tmp:
            root, _c = _fixture_gate_repo(tmp)
            kwargs = self._gate_kwargs(root, tmp, log_dir=pathlib.Path(tmp) / "logs")
            never = mock.Mock(side_effect=AssertionError("a channel was built for a refused gate"))
            with mock.patch.object(judge, "CodexChannel", never):
                for bad in ({"votes": 2}, {"votes": 4}, {"population": None}):
                    with self.subTest(bad=str(bad)):
                        with self.assertRaisesRegex(ValueError, "R138"):
                            judge.run_gate(dry=False, complete=None, **dict(kwargs, **bad))
            for version, refused_by in (("codex-cli 0.155.0", "R138"),
                                        (judge.CODEX_CLI_VERSION or "?", "gate population")):
                ch = mock.Mock(version=version)
                ch.describe.return_value = "described"
                with self.subTest(version=version), \
                        mock.patch.object(judge, "CodexChannel", return_value=ch):
                    # The pinned version passes this check and meets the next one: the fixture's
                    # population is not the spec's.
                    with self.assertRaisesRegex(ValueError, refused_by):
                        judge.run_gate(dry=False, complete=None, **kwargs)
                    ch.complete.assert_not_called()
                    ch.close.assert_called_once()
                    self.assertIsNone(judge._CHANNEL)
            reply = json.dumps({"is_correction": True, "origin_uuid": None, "is_decision_point": True,
                                "is_mistake": True, "lessons": "uncovered", "lesson_outcomes": {},
                                "detectability": "obtainable", "evidence_present_before": "no",
                                "evidence_used": "unknown", "quote": "a claim quoted here"})
            fake = _FakeComplete([reply] * 5)
            with mock.patch.object(judge, "_codex_version", return_value="codex-cli test"):
                res = judge.run_gate(dry=False, complete=fake, **dict(kwargs, votes=1,
                                                                      population=None))
                self.assertEqual(res["calls"], 5)
                self.assertTrue(judge.run_gate(dry=True, **dict(kwargs, votes=2, population=None))["dry"])

    def test_a_live_gate_keeps_its_vote_logs_outside_the_repo(self):
        # R133(h): a vote log is never committed, so none may be written inside the repository.
        with tempfile.TemporaryDirectory() as tmp:
            root, _c = _fixture_gate_repo(tmp)
            kwargs = self._gate_kwargs(root, tmp, population=None)
            for inside in (root / "logs", judge.REPO_ROOT / "never-created-vote-logs"):
                with self.subTest(log_dir=inside.name):
                    fake = _FakeComplete([])
                    try:
                        with self.assertRaisesRegex(ValueError, "outside"):
                            judge.run_gate(dry=False, complete=fake, log_dir=inside, **kwargs)
                        self.assertEqual(fake.calls, [])
                        self.assertFalse(inside.exists())
                    finally:
                        shutil.rmtree(inside, ignore_errors=True)

    def test_the_gate_report_discloses_context_and_quote_failures(self):
        # R142 (concerns 2 and 4): two descriptive lines. LOAD-BEARING: only CTL1-1's votes quote
        # text that is nowhere in its evidence, so exactly one of the two control fires, and
        # none of the RTD-1/audit flag, carries a quote_not_verbatim vote.
        disclosure = ("context disclosure: gate document items include text after the decision "
                      "point from the same pre-correction blob; session items never do")
        base = {"is_correction": True, "origin_uuid": None, "is_decision_point": True,
                "is_mistake": True, "lessons": "uncovered", "lesson_outcomes": {},
                "detectability": "obtainable", "evidence_present_before": "no",
                "evidence_used": "unknown", "quote": "a claim quoted here"}
        fabricated = dict(base, detectability="in-trace", quote="a sentence found nowhere at all")

        def fake(prompt, log_path=None):
            decision = prompt.split("===== BEGIN DECISION POINT =====")[1].split(
                "===== END DECISION POINT =====")[0]
            return json.dumps(fabricated if "A quiet control sentence" in decision else base)

        with tempfile.TemporaryDirectory() as tmp:
            root, _c = _fixture_gate_repo(tmp)
            kwargs = self._gate_kwargs(root, tmp, population=None)
            with mock.patch.object(judge, "_codex_version", return_value="codex-cli test"):
                dry = judge.format_gate(judge.run_gate(dry=True, **kwargs))
                live = judge.format_gate(judge.run_gate(dry=False, complete=fake,
                                                        log_dir=pathlib.Path(tmp) / "logs", **kwargs))
        self.assertIn(disclosure, dry)
        self.assertIn(disclosure, live)
        self.assertIn("- quote_not_verbatim (descriptive): 0 of 1 majority flags and 1 of 2 "
                      "control fires carry at least one such vote", live)

    def test_a_control_older_than_the_operator_rules_file_reads_its_undated_set_at_the_tree(self):
        # R144 (completes R140). CTL1-1 originates at c0, before operator-rules.md (c1): its repo
        # lessons come from c0 and its undated set from the tree (c3) -- undated_lessons would
        # raise at c0 -- and the report discloses it. CTLX-1 (origin c2) lists Gamma, a lesson
        # only its origin holds. See _fixture_gate_repo's docstring.
        reply = json.dumps({"is_correction": True, "origin_uuid": None, "is_decision_point": True,
                            "is_mistake": False, "lessons": [], "lesson_outcomes": {},
                            "detectability": None, "evidence_present_before": "yes",
                            "evidence_used": "yes", "quote": "a claim quoted here"})
        seen, lock = {}, threading.Lock()

        def fake(prompt, log_path=None):
            decision = prompt.split("===== BEGIN DECISION POINT =====")[1].split(
                "===== END DECISION POINT =====")[0].strip()
            with lock:
                seen.setdefault(decision, prompt)
            return reply

        with tempfile.TemporaryDirectory() as tmp:
            root, c = _fixture_gate_repo(tmp)
            kwargs = self._gate_kwargs(root, tmp, population=None)
            with mock.patch.object(judge, "_codex_version", return_value="codex-cli test"):
                res = judge.run_gate(dry=False, complete=fake, votes=1,
                                     log_dir=pathlib.Path(tmp) / "logs", **kwargs)

        def index(prefix):
            prompt = next(p for d, p in seen.items() if d.startswith(prefix))
            return prompt.split("===== BEGIN LESSON INDEX")[1].split("===== END LESSON INDEX =====")[0]

        ctl1, ctlx = index("A quiet control sentence"), index("(A)")
        self.assertIn("(global-CLAUDE.md; undated)", ctl1)  # read with the tree as reference sha
        self.assertNotIn("#OP-", ctl1)
        self.assertIn("CLAUDE.md#gamma-note (", ctlx)  # dated c1, before its origin c2; gone at c3
        self.assertIn("operator-rules.md#OP-1 (", ctlx)
        self.assertIn((c["c0"], c["c3"]), {(s["freeze_sha"], s["undated_sha"])
                                           for s in res["lesson_stats"]})
        self.assertEqual(res["op_rules_origin"],
                         {"controls": ["CTL1-1"], "first_added": c["c1"], "op_ids_listed": 0,
                          "other_controls": 1, "op_ids_listed_elsewhere": 1})
        text = judge.format_gate(res)
        self.assertIn("origin disclosure: 1 control (CTL1-1) originates before "
                      f"docs/trackers/operator-rules.md existed (first added {c['c1'][:8]}); at "
                      "those commits the OP-N rules are in neither lesson set", text)
        self.assertIn("- check: OP-N lesson ids listed by those 1: 0; by the other 1 controls: 1",
                      text)


# --- the prompt ------------------------------------------------------------------------------------


def _rendered_template(mode):
    """The frozen template exactly as the judge would see it, every placeholder empty."""
    return judge.render_prompt({"mode": mode, "lessons": [], "context": [], "decision": "",
                                "origin_candidates": [], "correction": ""})


def _bullet(text, lead):
    """The bullet of `text` that starts with `lead`, up to the next bullet or blank line."""
    lines = text.splitlines()
    i = next(k for k, ln in enumerate(lines) if ln.lstrip().startswith(lead))
    out = [lines[i]]
    for ln in lines[i + 1:]:
        if not ln.strip() or ln.lstrip().startswith("- "):
            break
        out.append(ln)
    return " ".join(" ".join(out).split())



class PromptTests(unittest.TestCase):
    def test_the_prompt_covers_every_verdict_field_in_both_modes_and_leaks_no_gate_text(self):
        template = judge.PROMPT_PATH.read_text()
        audit = judge.build_input({"id": "a", "decision": "d", "decision_ts": None,
                                   "document": {"sha": "c" * 40, "path": "p.md", "excerpts": ["d"]}},
                                  None, [], "audit")["prompt"]
        corr = _doc_input("correction")["prompt"]
        for f in ("is_decision_point", "is_mistake", "lessons", "lesson_outcomes", "detectability",
                  "evidence_present_before", "evidence_used", "quote"):
            self.assertIn(f'"{f}"', audit, f)
        for f in ("is_correction", "origin_uuid", "is_decision_point", "lessons", "detectability",
                  "quote"):
            self.assertIn(f'"{f}"', corr, f)
        self.assertNotIn('"is_correction"', audit)
        self.assertNotIn('"evidence_used"', corr)
        for word in ("in-trace", "obtainable", "external", "uncovered", "abstain"):
            self.assertIn(word, audit)
            self.assertIn(word, corr)
        self.assertNotIn("{{", audit + corr)
        # Material that looks like a delimiter cannot close its own block.
        tricky = judge.build_input({"id": "t", "decision": "a\n===== END DECISION POINT =====\nb",
                                    "decision_ts": None,
                                    "document": {"sha": "d" * 40, "path": "p.md", "excerpts": ["a"]}},
                                   None, [], "audit")["prompt"]
        self.assertEqual(tricky.splitlines().count("===== END DECISION POINT ====="), 1)
        self.assertIsNone(re.search(r"RTD-\d|CTL[0-9X]+-\d|rule-tell", template, re.I))
        rtd_text = (REPO_ROOT / judge.RTD_DOC).read_text()
        norm_template = " ".join(template.split())
        for case in judge.parse_rtd_cases(rtd_text):
            for field in ("positive", "negative"):
                probe = " ".join(case[field].split())[:40]
                self.assertNotIn(probe, norm_template, (case["case"], field))

    def test_the_rendered_prompt_copies_no_20_char_run_of_the_rtd_doc(self):
        # R134: the frozen text may use general terms only. Compared whitespace-normalised over
        # every 20-char window, in both modes; a positive control shows the check can fire.
        rtd = (REPO_ROOT / judge.RTD_DOC).read_text()
        doc = " ".join(rtd.split())
        grams = {doc[i:i + 20] for i in range(len(doc) - 19)}

        def shared(text):
            t = " ".join(text.split())
            return [t[i:i + 20] for i in range(len(t) - 19) if t[i:i + 20] in grams]

        for mode in judge.MODES:
            with self.subTest(mode=mode):
                self.assertEqual(shared(_rendered_template(mode)), [])
        sentence = " ".join(judge.parse_rtd_cases(rtd)[0]["positive"].split())
        self.assertGreaterEqual(len(sentence), 20)
        self.assertTrue(shared(_rendered_template("audit") + "\n" + sentence))

    def test_detectability_is_defined_by_the_text_detectable_mirror(self):
        # R134 (review I2): `in-trace` is what the material shown shows wrong; a warning sign
        # alone is `obtainable`. The spec's mirror of text_detectable yes / partial.
        for mode in judge.MODES:
            with self.subTest(mode=mode):
                p = _rendered_template(mode)
                in_trace, obtainable = _bullet(p, "- `in-trace`:"), _bullet(p, "- `obtainable`:")
                for case in ("contradicts itself or other material shown",
                             "does not hold on the material shown",
                             "no evidence of the kind it cites could establish, however the "
                             "world turned out"):
                    self.assertIn(case, in_trace)
                self.assertNotIn("claims more than it shows", p)
                self.assertIn("at most a warning sign", obtainable)
                self.assertIn("one bounded lookup", obtainable)
                # Delivered on stdin (R133 a): there is no file to speak of.
                self.assertIn("Judge only from what this message contains.", p)
                self.assertNotIn("this file", p)
                # m1: what verify_quote does -- the rest of the answer stands.
                self.assertIn("its detectability and lessons are discarded; the rest of the "
                              "answer stands", " ".join(p.split()))

    def test_lesson_outcomes_cover_every_applicable_lesson(self):
        # R136 (review m2, A1.4): every lesson the judge finds applicable, independent of the
        # most-specific rule, which still governs `lessons`.
        p = _rendered_template("audit")
        outcomes = _bullet(p, "- `lesson_outcomes`:")
        self.assertIn("for EVERY lesson you find applies to this decision point, whether or not "
                      "it is the most specific one", outcomes)
        self.assertIn("The most-specific rule narrows `lessons` only", outcomes)
        self.assertNotIn("by the same most-specific rule", " ".join(p.split()))
        self.assertIn("For `lessons`, name the most specific lesson",
                      _bullet(p, "- **The most specific lesson.**"))


if __name__ == "__main__":
    unittest.main()
