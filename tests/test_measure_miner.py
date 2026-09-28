"""Task 8: Stage 2 correction miner tests. See .superpowers/sdd/2026-09-26-system1-base-rate-measurement/task-8-context.md
(rulings R88-R95) for the authoritative spec; the brief names four required test names verbatim,
matched exactly below.

Modules are loaded by path via importlib (never a real package import), the shape of
tests/test_measure_join.py. Fixtures build events.db by calling join._create_schema on a real
temp sqlite FILE (candidates() opens its own read-only connection by path, so ":memory:" will
not do) and inserting rows directly; git fixtures are real temp repos built with subprocess.
"""
import importlib.util
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

def _git(repo, *args):
    return subprocess.run(
        ["git", "-C", str(repo), *args], capture_output=True, text=True, check=True,
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
    """Like _write, but for raw bytes -- used to build a file whose git diff contains a byte
    sequence that is not valid UTF-8, reproducing the real trigger for the _git() /
    _blame_shas() `errors="replace"` fix: a real commit in this repo's own history
    (176015f2, "feat(run_command): compact short cargo test output; fix the stderr tail")
    has a diff carrying byte 0x93 at position 32879, which crashed miner.candidates() under
    strict-UTF-8 subprocess decoding before that fix."""
    p = repo_dir / relpath
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_bytes(content_bytes)


def _commit(repo_dir, message, session_id=None):
    full_message = message if session_id is None else f"{message}\n\nSession-Id: {session_id}\n"
    _git(repo_dir, "add", "-A")
    _git(repo_dir, "commit", "-q", "-m", full_message)
    return _git(repo_dir, "rev-parse", "HEAD").strip()


def _commit_ts(repo_dir, sha):
    raw = _git(repo_dir, "log", "-1", "--format=%cI", sha).strip()
    return join._fmt_ts(raw)


def _build_blame_fixture_repo(repo_dir, c_session_id):
    """Commit A (S1) writes line L in doc.md. Commit B (S2) touches ONLY other.md -- B is C's
    PARENT but never touched L, so a mutant that reads the parent's trailer instead of blaming
    would report B's session (S2) as the antecedent regardless of who C is, which is wrong in
    BOTH variants below. Commit C rewrites L with a marker ("corrected"), authored by
    `c_session_id`."""
    _init_repo(repo_dir)
    _write(repo_dir, "doc.md", "Original claim.\n")
    sha_a = _commit(repo_dir, "docs: write the original claim", session_id="S1")
    _write(repo_dir, "other.md", "unrelated content\n")
    _commit(repo_dir, "docs: unrelated edit", session_id="S2")
    _write(repo_dir, "doc.md", "Corrected claim.\n")
    sha_c = _commit(repo_dir, "docs: corrected the claim", session_id=c_session_id)
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
            self.assertEqual(interrupt_cand.text, "actually, stop and reconsider")
            self.assertEqual(interrupt_cand.origin_hint, {"uuid": "a1", "ts": _ts(0)})

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

    def test_ts_tie_between_assistant_text_rows_is_broken_by_rowid(self):
        with tempfile.TemporaryDirectory() as td:
            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            sid = "profileA/sess-8"
            tie_ts = _ts(0)
            # Same ts, inserted in this order -- a_late gets the higher rowid, so it is
            # "latest" under the (ts, rowid) tie-break and must win over a_early.
            _insert_turn(conn, sid=sid, uuid="a_early", ts=tie_ts, kind="assistant_text", text="c1")
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
            self.assertEqual(cands_self[0].origin_hint["sha"], sha_a_1)

            cands_peer, sha_a_2, sha_c_2 = self._build_and_run(td, "S2")
            self.assertEqual(len(cands_peer), 1)
            self.assertEqual(cands_peer[0].corrector, "peer-session")
            self.assertEqual(cands_peer[0].source, "correction_commit")
            self.assertEqual(cands_peer[0].cid, f"commit:testrepo:{sha_c_2}:{sha_a_2}")


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
            _write(repo_dir, "doc.md", "base\n")
            _commit(repo_dir, "docs: base", session_id="S1")
            _git(repo_dir, "checkout", "-q", "-b", "feature")
            _write(repo_dir, "b.md", "on feature\n")
            _commit(repo_dir, "docs: feature change", session_id="S1")
            _git(repo_dir, "checkout", "-q", "-")
            _write(repo_dir, "a.md", "on trunk\n")
            _commit(repo_dir, "docs: trunk change", session_id="S1")
            _git(repo_dir, "merge", "--no-ff", "-q", "-m",
                 "Merge feature\n\nSession-Id: S1\n", "feature")
            sha_merge = _git(repo_dir, "rev-parse", "HEAD").strip()

            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            _seed_turns_for_sids(conn, ["S1"])
            _insert_commit(conn, repo="testrepo", sha=sha_merge, ts=_commit_ts(repo_dir, sha_merge),
                            session_id="S1", subject="Merge feature")
            conn.commit()
            conn.close()

            stats = {}
            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir}, stats=stats)
            self.assertEqual(len(cands), 0)
            self.assertEqual(stats.get("commit_merge"), 1)

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
            sha = _commit(repo_dir, "docs: a change", session_id="S-abc-123")

            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            _insert_commit(conn, repo="testrepo", sha=sha, ts=_commit_ts(repo_dir, sha),
                            session_id="S-abc-123", subject="docs: a change")
            conn.commit()
            row = conn.execute("SELECT session_id FROM commits WHERE sha = ?", (sha,)).fetchone()
            conn.close()

            self.assertEqual(miner.session_id_of(repo_dir, sha), row[0])
class NonUtf8GitOutputDoesNotRaise(unittest.TestCase):
    def test_a_diff_containing_an_invalid_utf8_byte_does_not_raise(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            _write(repo_dir, "doc.md", "Original claim.\n")
            sha_a = _commit(repo_dir, "docs: write the original claim", session_id="S1")

            # A lone 0x93 is invalid as a UTF-8 start byte (it is a Windows-1252-style smart
            # quote), so this line's git diff cannot be decoded under Python's default strict
            # UTF-8 text mode -- exactly the class of byte the real trigger commit carried.
            _write_bytes(repo_dir, "doc.md", b"Corrected claim \x93 with an invalid byte.\n")
            sha_c = _commit(repo_dir, "docs: corrected the claim", session_id="S2")

            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            _seed_turns_for_sids(conn, ["S1", "S2"])
            _insert_commit(conn, repo="testrepo", sha=sha_c, ts=_commit_ts(repo_dir, sha_c),
                            session_id="S2", subject="docs: corrected the claim")
            conn.commit()
            conn.close()

            # Must not raise UnicodeDecodeError -- and the blame attribution must still work
            # on everything ELSE in the (now U+FFFD-patched) diff text.
            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir})
            self.assertEqual(len(cands), 1)
            self.assertEqual(cands[0].source, "correction_commit")
            self.assertEqual(cands[0].corrector, "peer-session")
            self.assertEqual(cands[0].cid, f"commit:testrepo:{sha_c}:{sha_a}")

    def test_blame_porcelain_output_with_an_invalid_utf8_summary_line_does_not_raise(self):
        with tempfile.TemporaryDirectory() as td:
            repo_dir = pathlib.Path(td) / "repo"
            _init_repo(repo_dir)
            # The antecedent commit's OWN subject carries the invalid byte -- git blame
            # --porcelain echoes a commit's summary line verbatim, so this exercises
            # _blame_shas's independent errors="replace" fix rather than _commit_diff's.
            _write(repo_dir, "doc.md", "Original claim.\n")
            _git(repo_dir, "add", "-A")
            msg = b"docs: write the original claim \x93 with a bad byte\n\nSession-Id: S1\n"
            subprocess.run(["git", "-C", str(repo_dir), "commit", "-q", "-F", "-"],
                            input=msg, check=True)
            sha_a = _git(repo_dir, "rev-parse", "HEAD").strip()

            _write(repo_dir, "doc.md", "Corrected claim.\n")
            sha_c = _commit(repo_dir, "docs: corrected the claim", session_id="S2")

            db_path = pathlib.Path(td) / "events.db"
            conn = _new_events_db(db_path)
            _seed_turns_for_sids(conn, ["S1", "S2"])
            _insert_commit(conn, repo="testrepo", sha=sha_c, ts=_commit_ts(repo_dir, sha_c),
                            session_id="S2", subject="docs: corrected the claim")
            conn.commit()
            conn.close()

            cands = miner.candidates(str(db_path), repos={"testrepo": repo_dir})
            self.assertEqual(len(cands), 1)
            self.assertEqual(cands[0].cid, f"commit:testrepo:{sha_c}:{sha_a}")


if __name__ == "__main__":
    unittest.main()
