"""Task 8: Stage 2 correction miner tests. See .superpowers/sdd/2026-09-26-system1-base-rate-measurement/task-8-context.md
(rulings R88-R95) for the authoritative spec; the brief names four required test names verbatim,
matched exactly below.

Modules are loaded by path via importlib (never a real package import), the shape of
tests/test_measure_join.py. Fixtures build events.db by calling join._create_schema on a real
temp sqlite FILE (candidates() opens its own read-only connection by path, so ":memory:" will
not do) and inserting rows directly; git fixtures are real temp repos built with subprocess.
"""
import importlib.util
import os
import pathlib
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from datetime import datetime, timedelta, timezone

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
MEASURE = REPO_ROOT / "scripts" / "measure"
sys.path.insert(0, str(MEASURE))


def _load(name):
    spec = importlib.util.spec_from_file_location(f"measure_{name}", MEASURE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


join = _load("join")
miner = _load("miner")

BASE = datetime(2026, 9, 26, 10, 0, 0, tzinfo=timezone.utc)


def _ts(offset_seconds=0):
    dt = BASE + timedelta(seconds=offset_seconds)
    return dt.strftime("%Y-%m-%dT%H:%M:%S.") + f"{dt.microsecond // 1000:03d}Z"


# --- events.db fixture helpers -----------------------------------------------------------

def _new_events_db(path):
    conn = sqlite3.connect(str(path))
    join._create_schema(conn)
    return conn


def _insert_turn(conn, *, sid, uuid, ts, kind, text="", role="assistant", agent_path=None):
    conn.execute(
        "INSERT INTO turns (sid, agent_path, uuid, ts, role, kind, text, message_id, tokens) "
        "VALUES (?, ?, ?, ?, ?, ?, ?, NULL, NULL)",
        (sid, agent_path, uuid, ts, role, kind, text),
    )


def _insert_commit(conn, *, repo, sha, ts, session_id, subject):
    conn.execute(
        "INSERT INTO commits (repo, sha, ts, session_id, subject, files_json) "
        "VALUES (?, ?, ?, ?, ?, ?)",
        (repo, sha, ts, session_id, subject, "[]"),
    )


def _seed_turns_for_sids(conn, bare_sids, profile="profileX"):
    """One assistant_text turn per bare sid, so sid_by_bare (R92) has an entry to resolve a
    correcting commit's Session-Id against. Carries no prompt/interrupt, so it never itself
    becomes an operator candidate."""
    for i, bare in enumerate(bare_sids):
        _insert_turn(conn, sid=f"{profile}/{bare}", uuid=f"seed-{bare}", ts=_ts(-1000 - i),
                     kind="assistant_text", text="seed")


# --- temp git repo fixture helpers --------------------------------------------------------

def _git(repo, *args, env=None):
    return subprocess.run(
        ["git", "-C", str(repo), *args], capture_output=True, text=True, check=True, env=env,
    ).stdout


def _init_repo(repo_dir):
    repo_dir.mkdir(parents=True, exist_ok=True)
    _git(repo_dir, "init", "-q")
    _git(repo_dir, "config", "user.name", "Test User")
    _git(repo_dir, "config", "user.email", "test@example.com")
    return repo_dir


def _write(repo_dir, relpath, content):
    p = repo_dir / relpath
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(content)


def _write_bytes(repo_dir, relpath, content_bytes):
    """Like _write, but for raw bytes: a file whose CONTENT is not valid UTF-8. That is the real
    trigger for the `errors="replace"` fix (176015f2's diff carries byte 0x93). The byte must be
    in file content, never in a commit message: git transcodes a non-UTF-8 message at commit
    time (0x93 is stored as C2 93), so a message byte never reaches the decoder."""
    p = repo_dir / relpath
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_bytes(content_bytes)


def _commit(repo_dir, message, session_id=None, *, date=None, committer_date=None):
    """`date` sets both the author and committer dates; `committer_date` then overrides the
    committer's alone, so the two can differ (R110)."""
    full_message = message if session_id is None else f"{message}\n\nSession-Id: {session_id}\n"
    env = None
    if date is not None:
        env = dict(os.environ, GIT_AUTHOR_DATE=date, GIT_COMMITTER_DATE=committer_date or date)
    _git(repo_dir, "add", "-A")
    _git(repo_dir, "commit", "-q", "-m", full_message, env=env)
    return _git(repo_dir, "rev-parse", "HEAD").strip()


def _commit_ts(repo_dir, sha):
    raw = _git(repo_dir, "log", "-1", "--format=%cI", sha).strip()
    return join._fmt_ts(raw)


_DATE_A = "2026-09-20T08:00:00+00:00"
_DATE_B = "2026-09-21T08:00:00+00:00"
_DATE_C = "2026-09-22T08:00:00+00:00"


def _build_blame_fixture_repo(repo_dir, c_session_id):
    """Commit A (S1) writes line L in doc.md, B (S2) touches only other.md, and C rewrites L
    with a marker ("corrected"), authored by `c_session_id`. Each gets its own day, so the
    antecedent's ts (A) and the correcting commit's ts (C) are distinguishable."""
    _init_repo(repo_dir)
    _write(repo_dir, "doc.md", "Original claim.\n")
    sha_a = _commit(repo_dir, "docs: write the original claim", session_id="S1", date=_DATE_A)
    _write(repo_dir, "other.md", "unrelated content\n")
    # B is C's PARENT but never touched L, so the parent differs from the antecedent: a mutant
    # reading the parent's trailer (S2) instead of blaming is wrong in BOTH variants. Keep B.
    _commit(repo_dir, "docs: unrelated edit", session_id="S2", date=_DATE_B)
    _write(repo_dir, "doc.md", "Corrected claim.\n")
    sha_c = _commit(repo_dir, "docs: corrected the claim", session_id=c_session_id, date=_DATE_C)
    return sha_a, sha_c


class OperatorCandidateFiltersOutNonPromptKinds(unittest.TestCase):
    def test_a_meta_or_tool_result_user_entry_never_becomes_an_operator_candidate(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid = "profileA/sess-1"
            _insert_turn(conn, sid=sid, uuid="a1", ts=_ts(0), kind="assistant_text",
                         text="I made a claim.")
            # Correction-shaped text on kinds that must NEVER become operator candidates --
            # this is the positive member that stops the absence check from being monotone.
            _insert_turn(conn, sid=sid, uuid="m1", ts=_ts(1), kind="meta",
                         role="user", text="no, that's wrong")
            _insert_turn(conn, sid=sid, uuid="tr1", ts=_ts(2), kind="tool_result",
                         role="user", text="no, that's wrong")
            _insert_turn(conn, sid=sid, uuid="d1", ts=_ts(3), kind="delegation",
                         role="user", text="no, that's wrong")
            _insert_turn(conn, sid=sid, uuid="p1", ts=_ts(4), kind="prompt",
                         role="user", text="Actually, no, that's wrong.")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={})
            self.assertEqual(len(cands), 1)
            self.assertEqual(cands[0].cid, f"op:{sid}:p1")
            self.assertEqual(cands[0].source, "operator_message")
            self.assertEqual(cands[0].corrector, "operator")
            self.assertEqual(cands[0].origin_hint, {"uuid": "a1", "ts": _ts(0)})


class OperatorOriginSelection(unittest.TestCase):
    def test_interrupt_becomes_operator_interrupt_with_next_prompt_text(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid = "profileA/sess-2"
            _insert_turn(conn, sid=sid, uuid="a1", ts=_ts(0), kind="assistant_text", text="claim")
            _insert_turn(conn, sid=sid, uuid="i1", ts=_ts(1), kind="interrupt", role="user", text="")
            _insert_turn(conn, sid=sid, uuid="p1", ts=_ts(2), kind="prompt", role="user",
                         text="actually, stop and reconsider")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={})
            self.assertEqual(len(cands), 2)
            by_cid = {c.cid: c for c in cands}
            interrupt_cand = by_cid[f"op:{sid}:i1"]
            self.assertEqual(interrupt_cand.source, "operator_interrupt")
            self.assertEqual(interrupt_cand.corrector, "operator")
            self.assertEqual(interrupt_cand.detected_ts, _ts(1))
            self.assertEqual(interrupt_cand.text, "actually, stop and reconsider")
            self.assertEqual(interrupt_cand.origin_hint, {"uuid": "a1", "ts": _ts(0)})

    def test_interrupt_text_is_the_next_prompt_not_the_next_row(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid = "profileA/sess-2c"
            _insert_turn(conn, sid=sid, uuid="a1", ts=_ts(0), kind="assistant_text", text="claim")
            _insert_turn(conn, sid=sid, uuid="i1", ts=_ts(1), kind="interrupt", role="user", text="")
            # Both non-empty and both BEFORE the next prompt: a "next row" rule reads one of
            # them. Empty text here would let that mutant pass on the no-prompt default "".
            _insert_turn(conn, sid=sid, uuid="m1", ts=_ts(2), kind="meta", role="user",
                         text="meta row text")
            _insert_turn(conn, sid=sid, uuid="tr1", ts=_ts(3), kind="tool_result", role="user",
                         text="tool result text")
            _insert_turn(conn, sid=sid, uuid="p1", ts=_ts(4), kind="prompt", role="user",
                         text="the prompt that follows")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={})
            by_cid = {c.cid: c for c in cands}
            self.assertEqual(set(by_cid), {f"op:{sid}:i1", f"op:{sid}:p1"})
            self.assertEqual(by_cid[f"op:{sid}:i1"].text, "the prompt that follows")

    def test_interrupt_with_no_following_prompt_has_empty_text(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid = "profileA/sess-3"
            _insert_turn(conn, sid=sid, uuid="a1", ts=_ts(0), kind="assistant_text", text="claim")
            _insert_turn(conn, sid=sid, uuid="i1", ts=_ts(1), kind="interrupt", role="user", text="")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={})
            self.assertEqual(len(cands), 1)
            self.assertEqual(cands[0].text, "")

    def test_first_prompt_with_no_earlier_assistant_text_is_not_a_candidate(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid = "profileA/sess-4"
            _insert_turn(conn, sid=sid, uuid="p1", ts=_ts(0), kind="prompt", role="user", text="hi")
            conn.commit()
            conn.close()

            stats = {}
            cands = miner.candidates(str(db_path), repos={}, stats=stats)
            self.assertEqual(len(cands), 0)
            self.assertEqual(stats.get("operator_no_prior_assistant_text"), 1)

    def test_origin_is_the_latest_assistant_text_strictly_before_not_a_later_one(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid = "profileA/sess-5"
            _insert_turn(conn, sid=sid, uuid="a1", ts=_ts(0), kind="assistant_text", text="claim1")
            _insert_turn(conn, sid=sid, uuid="p1", ts=_ts(1), kind="prompt", role="user", text="no")
            # LATER than the prompt -- must never be picked as its origin.
            _insert_turn(conn, sid=sid, uuid="a2", ts=_ts(2), kind="assistant_text", text="claim2")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={})
            self.assertEqual(len(cands), 1)
            self.assertEqual(cands[0].origin_hint["uuid"], "a1")

    def test_origin_ignores_assistant_thinking_and_tool_use_between_it_and_the_prompt(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid = "profileA/sess-6"
            _insert_turn(conn, sid=sid, uuid="a1", ts=_ts(0), kind="assistant_text", text="claim")
            # NULL text by design (per the measured shapes); neither kind may become the origin.
            _insert_turn(conn, sid=sid, uuid="th1", ts=_ts(1), kind="assistant_thinking", text=None)
            _insert_turn(conn, sid=sid, uuid="tu1", ts=_ts(2), kind="tool_use", text="")
            _insert_turn(conn, sid=sid, uuid="p1", ts=_ts(3), kind="prompt", role="user", text="no")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={})
            self.assertEqual(len(cands), 1)
            self.assertEqual(cands[0].origin_hint["uuid"], "a1")

    def test_origin_never_crosses_sessions(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid1 = "profileA/sess-7a"
            sid2 = "profileA/sess-7b"
            _insert_turn(conn, sid=sid1, uuid="a1", ts=_ts(0), kind="assistant_text", text="claim1")
            # LATER ts than sid1's own assistant_text, but a different session -- must never
            # be picked as sid1's origin.
            _insert_turn(conn, sid=sid2, uuid="a2", ts=_ts(1), kind="assistant_text", text="claim2")
            _insert_turn(conn, sid=sid1, uuid="p1", ts=_ts(2), kind="prompt", role="user", text="no")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={})
            self.assertEqual(len(cands), 1)
            self.assertEqual(cands[0].sid, sid1)
            self.assertEqual(cands[0].origin_hint["uuid"], "a1")

    def test_a_later_sorting_session_does_not_inherit_an_earlier_sessions_assistant_text(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            # sess-a sorts FIRST and holds only an assistant_text. sess-b sorts SECOND, so a
            # per-sid state that is not reset would hand a1 to sess-b's p1. Swapping the two
            # names makes this test vacuous: the leaked value would never reach the prompt.
            sid_a = "p/sess-a"
            sid_b = "p/sess-b"
            _insert_turn(conn, sid=sid_a, uuid="a1", ts=_ts(0), kind="assistant_text", text="claim")
            _insert_turn(conn, sid=sid_b, uuid="p1", ts=_ts(1), kind="prompt", role="user", text="hi")
            # Positive member: a later prompt in sess-b, after its own assistant_text.
            _insert_turn(conn, sid=sid_b, uuid="a2", ts=_ts(2), kind="assistant_text", text="c2")
            _insert_turn(conn, sid=sid_b, uuid="p2", ts=_ts(3), kind="prompt", role="user", text="no")
            conn.commit()
            conn.close()

            stats = {}
            cands = miner.candidates(str(db_path), repos={}, stats=stats)
            self.assertEqual([c.cid for c in cands], [f"op:{sid_b}:p2"])
            self.assertEqual(cands[0].origin_hint, {"uuid": "a2", "ts": _ts(2)})
            self.assertEqual(stats.get("operator_no_prior_assistant_text"), 1)

    def test_origin_is_never_a_subagent_assistant_text(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid = "profileA/sess-7c"
            _insert_turn(conn, sid=sid, uuid="a1", ts=_ts(0), kind="assistant_text", text="claim")
            # A SUBAGENT's assistant_text, later than a1 and still before the prompt: the
            # latest assistant_text by ts, so only the top-level filter keeps it out.
            _insert_turn(conn, sid=sid, uuid="sa1", ts=_ts(1), kind="assistant_text",
                         text="subagent claim", agent_path="agentA/sub1")
            _insert_turn(conn, sid=sid, uuid="p1", ts=_ts(2), kind="prompt", role="user", text="no")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={})
            self.assertEqual(len(cands), 1)
            self.assertEqual(cands[0].origin_hint, {"uuid": "a1", "ts": _ts(0)})

    def test_ts_tie_between_assistant_text_rows_is_broken_by_rowid(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid = "profileA/sess-8"
            tie_ts = _ts(0)
            # Same ts. z_early is inserted FIRST (lower rowid) but sorts LAST by uuid, so rowid
            # order and uuid order disagree: a uuid tie-break picks z_early. Dropping the
            # tie-break also picks it, because SQLite scans the (sid, uuid) autoindex here
            # (measured with EXPLAIN QUERY PLAN). Inserting in uuid order makes this vacuous.
            _insert_turn(conn, sid=sid, uuid="z_early", ts=tie_ts, kind="assistant_text", text="c1")
            _insert_turn(conn, sid=sid, uuid="a_late", ts=tie_ts, kind="assistant_text", text="c2")
            _insert_turn(conn, sid=sid, uuid="p1", ts=_ts(1), kind="prompt", role="user", text="no")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={})
            self.assertEqual(len(cands), 1)
            self.assertEqual(cands[0].origin_hint["uuid"], "a_late")

    def test_subagent_prompt_never_becomes_a_candidate(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid = "profileA/sess-9"
            _insert_turn(conn, sid=sid, uuid="a1", ts=_ts(0), kind="assistant_text", text="claim")
            _insert_turn(conn, sid=sid, uuid="sp1", ts=_ts(1), kind="prompt", role="user",
                         text="no", agent_path="agentA/sub1")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={})
            self.assertEqual(len(cands), 0)
class OperatorRejection(unittest.TestCase):
    """R105, the miner half: a `rejection` turn is an operator kind beside prompt and interrupt.
    Task 14 makes join emit it, so these fixtures insert the rows directly."""

    def test_a_rejection_turn_becomes_an_operator_rejection_candidate(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid = "profileA/sess-r1"
            _insert_turn(conn, sid=sid, uuid="a1", ts=_ts(0), kind="assistant_text", text="claim1")
            _insert_turn(conn, sid=sid, uuid="a2", ts=_ts(1), kind="assistant_text", text="claim2")
            _insert_turn(conn, sid=sid, uuid="r1", ts=_ts(2), kind="rejection", role="user",
                         text="rejected: use the other file")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={})
            self.assertEqual(cands, [miner.Candidate(
                cid=f"op:{sid}:r1", sid=sid, detected_ts=_ts(2), source="operator_rejection",
                corrector="operator", origin_hint={"uuid": "a2", "ts": _ts(1)},
                text="rejected: use the other file",
            )])

    def test_a_subagent_rejection_never_becomes_a_candidate(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid = "profileA/sess-r2"
            _insert_turn(conn, sid=sid, uuid="a1", ts=_ts(0), kind="assistant_text", text="claim")
            _insert_turn(conn, sid=sid, uuid="sr1", ts=_ts(1), kind="rejection", role="user",
                         text="rejected in a subagent", agent_path="agentA/sub1")
            # Positive member: a top-level rejection in the same session IS a candidate, so an
            # empty result cannot pass this test.
            _insert_turn(conn, sid=sid, uuid="r1", ts=_ts(2), kind="rejection", role="user",
                         text="rejected at top level")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={})
            self.assertEqual([c.cid for c in cands], [f"op:{sid}:r1"])


class OperatorGrouping(unittest.TestCase):
    def test_two_corrections_of_one_origin_group_once(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid = "profileA/sess-10"
            _insert_turn(conn, sid=sid, uuid="a1", ts=_ts(0), kind="assistant_text", text="claim1")
            _insert_turn(conn, sid=sid, uuid="p1", ts=_ts(1), kind="prompt", role="user", text="no1")
            _insert_turn(conn, sid=sid, uuid="p2", ts=_ts(2), kind="prompt", role="user", text="no2")
            _insert_turn(conn, sid=sid, uuid="a2", ts=_ts(3), kind="assistant_text", text="claim2")
            _insert_turn(conn, sid=sid, uuid="p3", ts=_ts(4), kind="prompt", role="user", text="no3")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={})
            self.assertEqual(len(cands), 3)
            groups = miner.group_by_origin(cands)
            self.assertEqual(len(groups), 2)
            sizes = sorted(len(g) for g in groups)
            self.assertEqual(sizes, [1, 2])
            group_of_two = next(g for g in groups if len(g) == 2)
            self.assertEqual({c.cid for c in group_of_two}, {f"op:{sid}:p1", f"op:{sid}:p2"})
            group_of_one = next(g for g in groups if len(g) == 1)
            self.assertEqual(group_of_one[0].cid, f"op:{sid}:p3")


class CommitBlameAttribution(unittest.TestCase):
    def _build_and_run(self, td, c_session_id):
        repo_dir = pathlib.Path(td) / f"repo-{c_session_id}"
        sha_a, sha_c = _build_blame_fixture_repo(repo_dir, c_session_id)
        commit_ts = _commit_ts(repo_dir, sha_c)
        db_path = pathlib.Path(td) / f"events-{c_session_id}.db"
        conn = _new_events_db(db_path)
        _seed_turns_for_sids(conn, ["S1", "S2"])
        _insert_commit(conn, repo="testrepo", sha=sha_c, ts=commit_ts,
                        session_id=c_session_id, subject="docs: corrected the claim")
        conn.commit()
        conn.close()
        cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir})
        return cands, sha_a, sha_c

    def test_blame_at_parent_names_the_origin_session(self):
        with tempfile.TemporaryDirectory() as td:
            cands_self, sha_a_1, sha_c_1 = self._build_and_run(td, "S1")
            self.assertEqual(len(cands_self), 1)
            self.assertEqual(cands_self[0].corrector, "self")
            self.assertEqual(cands_self[0].source, "correction_commit")
            self.assertEqual(cands_self[0].cid, f"commit:testrepo:{sha_c_1}:{sha_a_1}")
            # R92: the turns.sid copy_id, never the bare Session-Id.
            self.assertEqual(cands_self[0].sid, "profileX/S1")
            # R92: detected_ts is commits.ts (C's day); the origin ts is A's day in join's R50
            # format. The two differ only because the fixture dates each commit a day apart.
            self.assertEqual(cands_self[0].detected_ts, "2026-09-22T08:00:00.000Z")
            self.assertEqual(cands_self[0].origin_hint,
                             {"sha": sha_a_1, "ts": "2026-09-20T08:00:00.000Z"})

            cands_peer, sha_a_2, sha_c_2 = self._build_and_run(td, "S2")
            self.assertEqual(len(cands_peer), 1)
            self.assertEqual(cands_peer[0].corrector, "peer-session")
            self.assertEqual(cands_peer[0].source, "correction_commit")
            self.assertEqual(cands_peer[0].cid, f"commit:testrepo:{sha_c_2}:{sha_a_2}")
            self.assertEqual(cands_peer[0].sid, "profileX/S2")

    def test_origin_ts_is_the_antecedents_author_date_not_its_committer_date(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "Original claim.\n")
            # R110: committer date != author date, as after a rebase restamps the commit.
            sha_a = _commit(repo_dir, "docs: write the original claim", session_id="S1",
                            date=_DATE_A, committer_date="2026-09-25T08:00:00+00:00")
            _write(repo_dir, "doc.md", "Corrected claim.\n")
            sha_c = _commit(repo_dir, "docs: corrected the claim", session_id="S2", date=_DATE_C)

            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            _seed_turns_for_sids(conn, ["S1", "S2"])
            _insert_commit(conn, repo="testrepo", sha=sha_c, ts=_commit_ts(repo_dir, sha_c),
                            session_id="S2", subject="docs: corrected the claim")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir})
            self.assertEqual(len(cands), 1)
            self.assertEqual(cands[0].origin_hint, {"sha": sha_a, "ts": "2026-09-20T08:00:00.000Z"})


class CommitReviewClassification(unittest.TestCase):
    def test_review_commit_is_classified_review(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "Some claim.\n")
            sha_a = _commit(repo_dir, "docs: write claim", session_id="S1")

            # "review" appears only in the BODY, never the subject.
            _write(repo_dir, "doc.md", "Some claim (v2).\n")
            _git(repo_dir, "add", "-A")
            _git(repo_dir, "commit", "-q", "-m",
                 "docs: tweak wording\n\nReview: this line was checked and adjusted.\n\n"
                 "Session-Id: S2\n")
            sha_rev = _git(repo_dir, "rev-parse", "HEAD").strip()

            # Subject contains "preview" (must NOT match \breview) and no other selector --
            # not examined at all, so it must contribute zero candidates.
            _write(repo_dir, "doc.md", "Preview of upcoming work.\n")
            sha_prev = _commit(repo_dir, "docs: preview of upcoming work", session_id="S3")

            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            _seed_turns_for_sids(conn, ["S1", "S2", "S3"])
            _insert_commit(conn, repo="testrepo", sha=sha_rev, ts=_commit_ts(repo_dir, sha_rev),
                            session_id="S2", subject="docs: tweak wording")
            _insert_commit(conn, repo="testrepo", sha=sha_prev, ts=_commit_ts(repo_dir, sha_prev),
                            session_id="S3", subject="docs: preview of upcoming work")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir})
            self.assertEqual(len(cands), 1)
            self.assertEqual(cands[0].source, "review_commit")
            self.assertEqual(cands[0].corrector, "review")
            self.assertEqual(cands[0].cid, f"commit:testrepo:{sha_rev}:{sha_a}")
def _db_with_commit_rows(td, repo_dir, rows, bare_sids):
    """events.db with `bare_sids` seeded under profileX and one commits row per (sha, session_id,
    subject) in `rows`, each stamped with that commit's own ts."""
    db_path = pathlib.Path(td) / "events.db"
    conn = _new_events_db(db_path)
    _seed_turns_for_sids(conn, bare_sids)
    for sha, session_id, subject in rows:
        _insert_commit(conn, repo="testrepo", sha=sha, ts=_commit_ts(repo_dir, sha),
                       session_id=session_id, subject=subject)
    conn.commit()
    conn.close()
    return db_path


class CommitSelection(unittest.TestCase):
    def test_a_capitalised_correction_stem_in_the_subject_selects_the_commit(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "Original claim.\n")
            sha_a = _commit(repo_dir, "docs: write the original claim", session_id="S1")
            _write(repo_dir, "doc.md", "Revised claim.\n")
            # "Correcting" is a R90(iii) stem but no MARKER_RE word, and it is capitalised, so
            # only the stems' re.I selects this commit. A lower-case stem cannot test re.I.
            subject = "docs: Correcting the claim"
            sha_c = _commit(repo_dir, subject, session_id="S2")
            db_path = _db_with_commit_rows(td, repo_dir, [(sha_c, "S2", subject)], ["S1", "S2"])

            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir})
            self.assertEqual([c.cid for c in cands], [f"commit:testrepo:{sha_c}:{sha_a}"])
            self.assertEqual(cands[0].source, "correction_commit")

    def test_a_marker_word_only_in_the_trailer_block_does_not_select_the_commit(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "Some claim.\n")
            _commit(repo_dir, "docs: write the claim", session_id="S1")
            _write(repo_dir, "doc.md", "Some claim (v2).\n")
            # The marker "corrected" sits in the TRAILER block only (no blank line between the
            # two trailer lines), so stripping trailers leaves nothing to select this commit.
            _git(repo_dir, "add", "-A")
            _git(repo_dir, "commit", "-q", "-m",
                 "docs: tweak wording\n\nNote: corrected upstream\nSession-Id: S2\n")
            sha_t = _git(repo_dir, "rev-parse", "HEAD").strip()
            _write(repo_dir, "doc.md", "Some claim (v3).\n")
            # Positive member: the same marker in a body paragraph DOES select.
            sha_b = _commit(repo_dir, "docs: tweak wording again\n\nThis corrected the wording.",
                            session_id="S2")
            db_path = _db_with_commit_rows(
                td, repo_dir,
                [(sha_t, "S2", "docs: tweak wording"), (sha_b, "S2", "docs: tweak wording again")],
                ["S1", "S2"])

            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir})
            self.assertEqual([c.cid for c in cands], [f"commit:testrepo:{sha_b}:{sha_t}"])

    def test_review_takes_precedence_over_retraction(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "The claim holds.\n")
            sha_a = _commit(repo_dir, "docs: write the claim", session_id="S1")
            # A pure addition with a NOTE_RE note (a retraction by itself) AND "review" in a
            # body paragraph: both rules hold, and R90 says review wins.
            _write(repo_dir, "doc.md",
                   "The claim holds.\n*Corrected 2026-09-26: this was wrong.*\n")
            sha_c = _commit(repo_dir, "docs: annotate the claim\n\nFound in review.",
                            session_id="S2")
            db_path = _db_with_commit_rows(
                td, repo_dir, [(sha_c, "S2", "docs: annotate the claim")], ["S1", "S2"])

            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir})
            self.assertEqual([c.cid for c in cands], [f"commit:testrepo:{sha_c}:{sha_a}"])
            self.assertEqual(cands[0].source, "review_commit")
            self.assertEqual(cands[0].corrector, "review")


class CommitPureAdditionRetraction(unittest.TestCase):
    def test_pure_addition_with_a_note_re_note_is_a_retraction_of_the_annotated_line(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "The claim holds.\n")
            sha_a = _commit(repo_dir, "docs: write the claim", session_id="S1")
            _write(repo_dir, "doc.md",
                   "The claim holds.\n*Corrected 2026-09-26: this was wrong.*\n")
            sha_note = _commit(repo_dir, "docs: annotate a correction", session_id="S2")

            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            _seed_turns_for_sids(conn, ["S1", "S2"])
            _insert_commit(conn, repo="testrepo", sha=sha_note, ts=_commit_ts(repo_dir, sha_note),
                            session_id="S2", subject="docs: annotate a correction")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir})
            self.assertEqual(len(cands), 1)
            self.assertEqual(cands[0].source, "retraction")
            self.assertEqual(cands[0].corrector, "peer-session")
            self.assertEqual(cands[0].cid, f"commit:testrepo:{sha_note}:{sha_a}")
            self.assertEqual(cands[0].origin_hint["sha"], sha_a)

    def test_pure_addition_without_a_note_yields_no_antecedent(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "The claim holds.\n")
            _commit(repo_dir, "docs: write the claim", session_id="S1")
            _write(repo_dir, "doc.md", "The claim holds.\nAn unrelated addition.\n")
            # Subject alone makes this EXAMINED (matches the correction-subject regex), but
            # the added line carries no NOTE_RE marker, so blame never runs for this hunk.
            sha_add = _commit(repo_dir, "docs: correct the formatting", session_id="S2")

            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            _seed_turns_for_sids(conn, ["S1", "S2"])
            _insert_commit(conn, repo="testrepo", sha=sha_add, ts=_commit_ts(repo_dir, sha_add),
                            session_id="S2", subject="docs: correct the formatting")
            conn.commit()
            conn.close()

            stats = {}
            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir}, stats=stats)
            self.assertEqual(len(cands), 0)
            self.assertEqual(stats.get("no_antecedent"), 1)


class CommitDropCounters(unittest.TestCase):
    def test_commit_session_not_in_corpus_is_dropped_and_counted(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "Original claim.\n")
            _commit(repo_dir, "docs: write claim", session_id="S1")
            _write(repo_dir, "doc.md", "Corrected claim.\n")
            sha_c = _commit(repo_dir, "docs: corrected the claim", session_id="S-NOWHERE")

            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            # Deliberately no turns row for "S-NOWHERE".
            _seed_turns_for_sids(conn, ["S1"])
            _insert_commit(conn, repo="testrepo", sha=sha_c, ts=_commit_ts(repo_dir, sha_c),
                            session_id="S-NOWHERE", subject="docs: corrected the claim")
            conn.commit()
            conn.close()

            stats = {}
            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir}, stats=stats)
            self.assertEqual(len(cands), 0)
            self.assertEqual(stats.get("commit_session_not_in_corpus"), 1)

    def test_an_untrailered_antecedent_is_dropped_and_counted(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "Original claim.\n")
            _commit(repo_dir, "docs: write claim", session_id=None)  # no trailer at all
            _write(repo_dir, "doc.md", "Corrected claim.\n")
            sha_c = _commit(repo_dir, "docs: corrected the claim", session_id="S2")

            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            _seed_turns_for_sids(conn, ["S2"])
            _insert_commit(conn, repo="testrepo", sha=sha_c, ts=_commit_ts(repo_dir, sha_c),
                            session_id="S2", subject="docs: corrected the claim")
            conn.commit()
            conn.close()

            stats = {}
            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir}, stats=stats)
            self.assertEqual(len(cands), 0)
            self.assertEqual(stats.get("antecedent_untrailered"), 1)

    def test_a_merge_commit_is_skipped_and_counted(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "Original claim.\n")
            _commit(repo_dir, "docs: write the claim", session_id="S1")
            _git(repo_dir, "checkout", "-q", "-b", "feature")
            _write(repo_dir, "doc.md", "Corrected claim.\n")
            _commit(repo_dir, "docs: feature change", session_id="S2")
            _git(repo_dir, "checkout", "-q", "-")
            _write(repo_dir, "a.md", "on trunk\n")
            _commit(repo_dir, "docs: trunk change", session_id="S1")
            # The merge WOULD be a candidate if it were examined: its subject carries a marker,
            # and against its first parent it rewrites S1's line. So only the skip keeps it
            # out; an unselectable merge would pass a "counted but not skipped" mutant.
            _git(repo_dir, "merge", "--no-ff", "-q", "-m",
                 "Merge feature: corrected the claim\n\nSession-Id: S2\n", "feature")
            sha_merge = _git(repo_dir, "rev-parse", "HEAD").strip()
            db_path = _db_with_commit_rows(
                td, repo_dir, [(sha_merge, "S2", "Merge feature: corrected the claim")],
                ["S1", "S2"])

            stats = {}
            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir}, stats=stats)
            self.assertEqual(cands, [])
            self.assertEqual(stats, {"commit_merge": 1})

    def test_an_antecedent_from_a_spec_excluded_session_is_dropped_and_counted(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            excluded = sorted(join.SPEC_EXCLUDED_SIDS)[0]  # the measuring session itself
            _write(repo_dir, "doc.md", "Claim one.\n")
            _commit(repo_dir, "docs: write claim one", session_id=excluded)
            _write(repo_dir, "doc.md", "Claim one.\nClaim two.\n")
            sha_b = _commit(repo_dir, "docs: write claim two", session_id="S1")
            # One hunk rewrites both lines, so blame names both antecedents. Only the excluded
            # one is dropped; S1's line is the positive member.
            _write(repo_dir, "doc.md", "Corrected one.\nCorrected two.\n")
            sha_c = _commit(repo_dir, "docs: corrected both claims", session_id="S2")
            db_path = _db_with_commit_rows(
                td, repo_dir, [(sha_c, "S2", "docs: corrected both claims")], ["S1", "S2"])

            stats = {}
            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir}, stats=stats)
            self.assertEqual([c.cid for c in cands], [f"commit:testrepo:{sha_c}:{sha_b}"])
            self.assertEqual(stats, {"antecedent_excluded": 1})

    def test_a_correcting_session_id_in_two_profiles_is_ambiguous_and_counted(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "Original claim.\n")
            _commit(repo_dir, "docs: write claim", session_id="S1")
            _write(repo_dir, "doc.md", "Corrected claim.\n")
            sha_c = _commit(repo_dir, "docs: corrected the claim", session_id="S2")
            db_path = _db_with_commit_rows(
                td, repo_dir, [(sha_c, "S2", "docs: corrected the claim")], ["S1", "S2"])
            conn = sqlite3.connect(str(db_path))
            # The same bare sid S2 under a SECOND profile: two turns.sid copy_ids match it.
            _seed_turns_for_sids(conn, ["S2"], profile="profileY")
            conn.commit()
            conn.close()

            stats = {}
            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir}, stats=stats)
            self.assertEqual(cands, [])
            self.assertEqual(stats, {"commit_session_ambiguous": 1})

    def test_an_untrailered_correcting_commit_is_dropped_and_counted(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "Original claim.\n")
            _commit(repo_dir, "docs: write claim", session_id="S1")
            _write(repo_dir, "doc.md", "Corrected claim.\n")
            sha_c = _commit(repo_dir, "docs: corrected the claim", session_id=None)  # no trailer
            db_path = _db_with_commit_rows(
                td, repo_dir, [(sha_c, None, "docs: corrected the claim")], ["S1"])

            stats = {}
            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir}, stats=stats)
            self.assertEqual(cands, [])
            self.assertEqual(stats, {"commit_untrailered": 1})

    def test_a_failed_git_blame_is_counted_and_never_read_as_no_antecedent(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            # A gitlink (mode 160000, a submodule pointer): `git diff` shows it as a one-line
            # hunk, but `git blame` of it at the parent exits 128 ("no such path"). This
            # reaches R102 through candidates() with no fault injection. The index is written
            # directly and never `git add -A`, which would stage the gitlink's deletion.
            _git(repo_dir, "update-index", "--add", "--cacheinfo", f"160000,{'1' * 40},sub")
            _git(repo_dir, "commit", "-q", "-m", "chore: add the sub pointer\n\nSession-Id: S1\n")
            _git(repo_dir, "update-index", "--cacheinfo", f"160000,{'2' * 40},sub")
            _git(repo_dir, "commit", "-q", "-m",
                 "chore: corrected the sub pointer\n\nSession-Id: S2\n")
            sha_c = _git(repo_dir, "rev-parse", "HEAD").strip()
            db_path = _db_with_commit_rows(
                td, repo_dir, [(sha_c, "S2", "chore: corrected the sub pointer")], ["S1", "S2"])

            stats = {}
            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir}, stats=stats)
            self.assertEqual(cands, [])
            self.assertEqual(stats, {"blame_git_failures": 1})

    def test_a_commit_whose_repo_is_missing_from_repos_is_skipped_and_counted(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            _seed_turns_for_sids(conn, ["S1"])
            _insert_commit(conn, repo="ghost-repo", sha="d" * 40, ts=_ts(0),
                            session_id="S1", subject="docs: corrected the claim")
            conn.commit()
            conn.close()

            stats = {}
            cands = miner.candidates(str(db_path), repos={}, stats=stats)
            self.assertEqual(len(cands), 0)
            self.assertEqual(stats.get("commit_repo_missing"), 1)


class SessionIdOfMatchesCommitsTable(unittest.TestCase):
    def test_session_id_of_equals_commits_session_id_for_the_fixtures_commit(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "content\n")
            # "Session-Id:" appears MID-line here, so git's trailer parser -- which join and the
            # miner both read since Task 14 (R110) -- reads S-abc-123, while an unanchored
            # search reads S-MIDLINE. A prose line STARTING "Session-Id:" no longer splits the
            # two: tests/test_measure_join.py::OneSessionIdDefinition pins that case.
            sha_1 = _commit(repo_dir, "docs: a change\n\nThe old Session-Id: S-MIDLINE is gone.",
                            session_id="S-abc-123")
            _write(repo_dir, "other.md", "more\n")
            sha_2 = _commit(repo_dir, "docs: an untrailered change")  # no trailer: both None

            # The commits rows are PRODUCED by join (_build_commits -> _run_git_log), never
            # typed in by hand, so this compares the two real readers.
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            counts = {}
            join._build_commits(conn, {"repos": {"testrepo": sha_2}, "bounds": {}},
                                {"testrepo": repo_dir}, counts)
            conn.commit()
            rows = dict(conn.execute("SELECT sha, session_id FROM commits").fetchall())
            conn.close()

            self.assertEqual(rows, {sha_1: "S-abc-123", sha_2: None})
            for sha, db_session_id in rows.items():
                self.assertEqual(miner.session_id_of(repo_dir, sha), db_session_id, sha)

    def test_session_id_of_reads_the_trailer_not_an_earlier_prose_line(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "content\n")
            # R110: a prose paragraph STARTING "Session-Id: WRONG" sits ABOVE the real trailer
            # block. A first-match body regex reads WRONG; git's trailer parser reads S-REAL.
            sha = _commit(repo_dir,
                          "docs: a change\n\nSession-Id: WRONG was the value a draft named.",
                          session_id="S-REAL")
            self.assertEqual(miner.session_id_of(repo_dir, sha), "S-REAL")


class NonUtf8GitOutputDoesNotRaise(unittest.TestCase):
    def _candidates_or_fail(self, db_path, repo_dir):
        """Both fixtures put byte 0x93 in FILE CONTENT (see _write_bytes for why never in a
        message), and a UnicodeDecodeError becomes an assertion failure, not an error."""
        try:
            return miner.candidates(str(db_path), repos={"testrepo": repo_dir})
        except UnicodeDecodeError as e:
            self.fail(f"candidates() raised on a non-UTF-8 git output byte: {e!r}")

    def test_a_diff_containing_an_invalid_utf8_byte_does_not_raise(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "Original claim.\n")
            sha_a = _commit(repo_dir, "docs: write the original claim", session_id="S1")

            # 0x93 on the ADDED line: the diff (_git) carries it, while blame at the parent
            # reads A's clean line, so this case exercises _git's decoding alone.
            _write_bytes(repo_dir, "doc.md", b"Corrected claim \x93 with an invalid byte.\n")
            sha_c = _commit(repo_dir, "docs: corrected the claim", session_id="S2")
            db_path = _db_with_commit_rows(
                td, repo_dir, [(sha_c, "S2", "docs: corrected the claim")], ["S1", "S2"])

            cands = self._candidates_or_fail(db_path, repo_dir)
            self.assertEqual(len(cands), 1)
            self.assertEqual(cands[0].source, "correction_commit")
            self.assertEqual(cands[0].corrector, "peer-session")
            self.assertEqual(cands[0].cid, f"commit:testrepo:{sha_c}:{sha_a}")

    def test_blame_porcelain_output_with_an_invalid_utf8_content_line_does_not_raise(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            # 0x93 on the antecedent's line that C REMOVES: `git blame --porcelain` echoes the
            # blamed line's content raw, so _blame_shas must decode it. (The diff carries it
            # too, on the "-" line, so strict _git would also fail here.)
            _write_bytes(repo_dir, "doc.md", b"Original claim \x93 quoted.\n")
            sha_a = _commit(repo_dir, "docs: write the original claim", session_id="S1")
            _write(repo_dir, "doc.md", "Corrected claim.\n")
            sha_c = _commit(repo_dir, "docs: corrected the claim", session_id="S2")
            db_path = _db_with_commit_rows(
                td, repo_dir, [(sha_c, "S2", "docs: corrected the claim")], ["S1", "S2"])

            cands = self._candidates_or_fail(db_path, repo_dir)
            self.assertEqual(len(cands), 1)
            self.assertEqual(cands[0].cid, f"commit:testrepo:{sha_c}:{sha_a}")
            self.assertEqual(cands[0].corrector, "peer-session")


if __name__ == "__main__":
    unittest.main()
