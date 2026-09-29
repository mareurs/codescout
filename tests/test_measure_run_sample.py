"""Task 5 of the labelled sample: run.py frame / draw / render / estimate / export.

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_run_sample.py -v

Every transcript here is SYNTHETIC (tests/measure_corpus_fixture.py); nothing reads a real corpus.
The corpus has 6 sessions -> 33 units: 18 routine, 12 substantive top-level, 3 substantive
hand-backs (sessions 0-2 have one subagent file each). Fixture details that carry a guard are
annotated on their own line with what breaks if they go.
"""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
import pathlib
import random
import stat
import sys
import tempfile
import unittest
from unittest import mock

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
MEASURE = REPO_ROOT / "scripts" / "measure"
sys.path.insert(0, str(MEASURE))
sys.path.insert(0, str(REPO_ROOT / "tests"))


def _load(name):
    spec = importlib.util.spec_from_file_location(f"measure_{name}", MEASURE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


run = _load("run")
sampler = _load("sampler")
packet = _load("packet")
label = _load("label")
import measure_corpus_fixture as fx  # noqa: E402

CORPUS_ID = "corpus-2026-09-29"
# ghp_ + exactly 36 alphanumerics: the shape packet._TOKEN_RE refuses.
TOKEN = "ghp_" + "A1" * 18
NOTE = "NOTE-SENTINEL-do-not-export"
LABELLED_AT = "2031-01-02T03:04:05Z"  # a time of day no source timestamp has
OUTLIER_SECONDS = 7777.777  # the max of the seconds list: never a median, so it must never be printed
COUNTS = {"substantive": {"top": 12, "handback": 3}, "routine": {"top": 18}}
# The exported record of the seed-11 main draw (6 substantive + 4 routine), pinned as literals: case id,
# packet sha256 (deterministic for the fixture), stratum, kind, in draw.json's `cases` order.
EXPORT_CASES = [
    ("9cc8493506", "5e60b00766f032e061fc5dc049ad4603a43da19f8822d210ba2ba6b4d7943a0f", "substantive", "top"),
    ("d6a1394c31", "c560d140225cb1e1ff3eee825a74b76170ec59ec14e6027a2d8234fc4271d1da", "substantive", "top"),
    ("77ba2592f7", "95290a93da6dbb2df60d304f1ed96a34590510c21a3fdd31e258a308a046e73b", "substantive", "handback"),
    ("1a64a9afdb", "5753597682f3477ae706c96fb8833b97567830a6480f8417d92c080fd06b13bc", "substantive", "top"),
    ("b837168a2c", "a9cfffbd82fa2fd88466938b271c4cd5ec9a7e9b3a9ea24d2826d54fe8b92aac", "substantive", "top"),
    ("0761ce7ec5", "43e3cf4b7402df86ab248ba74df75dd033610b5ebf20b73eb46104695571e2e8", "substantive", "top"),
    ("d6ec25c96f", "cf449453976052933cea49034560e13e2514a97a8e4d7435a72545c451934f6e", "routine", "top"),
    ("30fc2c09dd", "ac07955b317ac1f061caf74da091cfec4094ba144819a75a93e2de0c6b54118b", "routine", "top"),
    ("3c9284abec", "7786e4d6400ece5ae77a970ebeba3e230b6a3d3ff3d26c51e6050b999f3e57c5", "routine", "top"),
    ("812c9a7436", "642d50b4c8f65ebe86a1f78bb2c61f07fd8bcc0c40188e76853c079c6b71f6b4", "routine", "top"),
]
# write_labels(sub_hits=4, rou_hits=1) on that draw, in the order it writes them:
# (case id, sha256, labels, delivery, recall, seconds)
V, Q, N, S = ["verify"], "quiet", ["none"], "silent"
EXPORT_LABELS = [
    ("77ba2592f7", "95290a93da6dbb2df60d304f1ed96a34590510c21a3fdd31e258a308a046e73b", V, Q, "y", 900.123),
    ("9cc8493506", "5e60b00766f032e061fc5dc049ad4603a43da19f8822d210ba2ba6b4d7943a0f", V, Q, "n", 901.123),
    ("1a64a9afdb", "5753597682f3477ae706c96fb8833b97567830a6480f8417d92c080fd06b13bc", V, Q, "n", 902.123),
    ("d6a1394c31", "c560d140225cb1e1ff3eee825a74b76170ec59ec14e6027a2d8234fc4271d1da", V, Q, "n", 903.123),
    ("0761ce7ec5", "43e3cf4b7402df86ab248ba74df75dd033610b5ebf20b73eb46104695571e2e8", N, S, "n", 904.123),
    ("b837168a2c", "a9cfffbd82fa2fd88466938b271c4cd5ec9a7e9b3a9ea24d2826d54fe8b92aac", N, S, "n", 905.123),
    ("3c9284abec", "7786e4d6400ece5ae77a970ebeba3e230b6a3d3ff3d26c51e6050b999f3e57c5", V, Q, "n", 906.123),
    ("812c9a7436", "642d50b4c8f65ebe86a1f78bb2c61f07fd8bcc0c40188e76853c079c6b71f6b4", N, S, "n", 907.123),
    ("d6ec25c96f", "cf449453976052933cea49034560e13e2514a97a8e4d7435a72545c451934f6e", N, S, "n", 908.123),
    ("30fc2c09dd", "ac07955b317ac1f061caf74da091cfec4094ba144819a75a93e2de0c6b54118b", N, S, "n", 7777.777),
]
LABEL_FIELDS = ("case_id", "packet_sha256", "labels", "delivery", "recall", "seconds")


def sid_for(i):
    return f"5b1f0c2e-{i:04d}-4222-8333-444455556666"  # UUID-shaped: a leaked session id is only scrubbed as one


class Seq:
    def __init__(self, i):
        self.i, self.n, self.entries = i, 0, []

    def _ids(self):
        self.n += 1
        return f"{self.i:04d}aaaa-0000-4000-8000-{self.n:012d}", f"2026-09-20T10:{self.i % 60:02d}:{self.n:02d}Z"

    def user(self, text):
        u, t = self._ids()
        self.entries.append(fx.user_prompt(u, t, text))

    def asst(self, mid, text=None, tools=(), stop=None):
        u, t = self._ids()
        self.entries.append(fx.assistant(u, t, mid, text=text, tool_uses=tools, stop=stop))
        return [f"{u}-tu{k}" for k in range(len(tools))]

    def result(self, tid, content):
        u, t = self._ids()
        self.entries.append(fx.tool_result(u, t, tid, content))


def session(i, token=False):
    s = Seq(i)
    s.user(f"please work on task {i}")
    for j in range(3):  # routine: no claim marker, no tool, no end_turn
        s.asst(f"m{i}r{j}", "Reading the module.")
    (tu,) = s.asst(f"m{i}e", "Applying the change.", tools=[("edit_file", {"path": "a.rs"})])  # substantive: edit
    s.result(tu, "ok")
    s.asst(f"m{i}c", "Fixed and verified." + (f" {TOKEN}" if token else ""), stop="end_turn")  # substantive: claim
    subs = {}
    if i < 3:
        h = Seq(100 + i)
        h.user("search the tree")
        h.asst(f"h{i}a", "Searching.")
        h.asst(f"h{i}b", "Report: done.", stop="end_turn")  # the LAST text message of the file = the hand-back
        subs = {f"agent-{i}": h.entries}
    return {"sid": sid_for(i), "profile": "work", "slug": "proj", "entries": s.entries, "subagents": subs}


def make_sessions(token_session=None):
    return [session(i, token=(i == token_session)) for i in range(6)]


def cli(*argv):
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        rc = run.main([str(a) for a in argv])
    return rc, out.getvalue(), err.getvalue()


def read(path):
    return pathlib.Path(path).read_text(encoding="utf-8")


def mode(path):
    return stat.S_IMODE(os.stat(path).st_mode)


def write_labels(set_dir, sub_hits, rou_hits, drop_sub=0, keep_routine=None):
    """Scripted operator records, straight into labels.jsonl. Substantive cases first, then routine, each
    in draw order; the first `sub_hits` (`rou_hits`) of a stratum are hits (verify/quiet), the rest
    none/silent. The first substantive case is recall-flagged. drop_sub leaves that many substantive
    cases unlabelled; keep_routine limits the routine ones labelled."""
    set_dir = pathlib.Path(set_dir)
    draw = json.loads(read(set_dir / "draw.json"))
    key = json.loads(read(set_dir / "key.json"))
    sha = {c["case_id"]: c["sha256"] for c in draw["cases"]}
    subs = [c for c in draw["order"] if key[c]["stratum"] == "substantive"]
    rous = [c for c in draw["order"] if key[c]["stratum"] == "routine"]
    chosen = subs[:len(subs) - drop_sub] + (rous if keep_routine is None else rous[:keep_routine])
    recs = []
    for n, cid in enumerate(chosen):
        st = key[cid]["stratum"]
        pos = (subs if st == "substantive" else rous).index(cid)
        hit = pos < (sub_hits if st == "substantive" else rou_hits)
        recs.append({"case_id": cid, "packet_sha256": sha[cid],
                     "labels": ["verify"] if hit else ["none"], "delivery": "quiet" if hit else "silent",
                     "note": f"{NOTE} {cid}", "recall": "y" if (st == "substantive" and pos == 0) else "n",
                     "seconds": 900.123 + n, "labelled_at": LABELLED_AT})
    recs[-1]["seconds"] = OUTLIER_SECONDS
    with open(set_dir / "labels.jsonl", "w", encoding="utf-8") as f:
        for r in recs:
            f.write(json.dumps(r) + "\n")
    return recs


class Base(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = pathlib.Path(tmp.name)
        self.corpus = fx.build_corpus(self.root / CORPUS_ID, make_sessions())

    def draw(self, name, seed, sub, rou, *extra):
        set_dir = self.root / name
        rc, out, err = cli("draw", "--corpus", self.corpus, "--set", set_dir, "--seed", seed,
                           "--substantive", sub, "--routine", rou, *extra)
        self.assertEqual((rc, err), (0, ""), out)
        return set_dir

    def rendered(self, name="main", seed=11, sub=6, rou=4):
        set_dir = self.draw(name, seed, sub, rou)
        rc, out, err = cli("render", "--corpus", self.corpus, "--set", set_dir)
        self.assertEqual((rc, err), (0, ""), out)
        return set_dir

    def frame_file(self):
        p = self.root / "frame.json"
        rc, _, err = cli("frame", "--corpus", self.corpus, "--out", p)
        self.assertEqual((rc, err), (0, ""))
        return p


class Frame(Base):
    def test_frame_writes_counts_and_code_hashes(self):
        out_path = self.root / "sub" / "frame.json"
        rc, out, err = cli("frame", "--corpus", self.corpus, "--out", out_path)
        self.assertEqual((rc, err), (0, ""))
        rec = json.loads(read(out_path))
        self.assertEqual(rec, {
            "corpus_id": CORPUS_ID, "counts": COUNTS, "n_units": 33,
            "code_sha256": {"sampler.py": hashlib.sha256((MEASURE / "sampler.py").read_bytes()).hexdigest(),
                            "packet.py": hashlib.sha256((MEASURE / "packet.py").read_bytes()).hexdigest()}})
        self.assertEqual(json.loads(out), COUNTS)  # what is printed is the counts, and only them

    def test_frame_refuses_an_existing_out_file_and_an_empty_corpus(self):
        p = self.root / "frame.json"
        p.write_text("first evidence")
        rc, out, err = cli("frame", "--corpus", self.corpus, "--out", p)
        self.assertEqual(rc, 1)
        self.assertIn("exists", err)
        self.assertEqual(p.read_text(), "first evidence")
        empty = self.root / "empty-corpus"
        empty.mkdir()
        rc, out, err = cli("frame", "--corpus", empty, "--out", self.root / "f2.json")
        self.assertEqual(rc, 1)
        self.assertIn("no units", err)
        self.assertFalse((self.root / "f2.json").exists())


class Draw(Base):
    def test_draw_writes_private_key_and_draw_json(self):
        set_dir = self.draw("main", 11, 6, 4)
        key = json.loads(read(set_dir / "key.json"))
        draw = json.loads(read(set_dir / "draw.json"))
        self.assertEqual(mode(set_dir), 0o700)
        self.assertEqual(mode(set_dir / "key.json"), 0o600)
        self.assertEqual(mode(set_dir / "draw.json"), 0o600)
        # key.json: exactly the drawn units, substantive stratum first (the sampler's order)
        units = sampler.frame(self.corpus, excluded_sids=set())
        drawn = sampler.draw(units, {"substantive": 6, "routine": 4}, 11)
        self.assertEqual(list(key), [packet.case_id_for(u, 11) for u in drawn])
        for cid, u in zip(key, drawn):
            self.assertEqual(key[cid], {"case_key": u.case_key, "stratum": u.stratum, "kind": u.kind,
                                        "copy_id": u.copy_id, "reasons": list(u.reasons)})
        self.assertEqual(sum(v["stratum"] == "substantive" for v in key.values()), 6)
        self.assertEqual(sum(v["stratum"] == "routine" for v in key.values()), 4)
        # draw.json: set_id, seed, cases without hashes yet, order = a seed+1 shuffle
        self.assertEqual(set(draw), {"set_id", "seed", "cases", "order"})
        self.assertEqual(draw["set_id"], "main")
        self.assertEqual(draw["seed"], 11)
        self.assertEqual(draw["cases"], [{"case_id": cid, "sha256": None} for cid in key])
        expected = list(key)
        random.Random(12).shuffle(expected)  # seed + 1, computed here by the stdlib, not by run.py
        self.assertEqual(draw["order"], expected)
        self.assertNotEqual(draw["order"], list(key))  # non-vacuous: the shuffle moved something

    def test_draw_prints_counts_only(self):
        set_dir = self.root / "main"
        rc, out, err = cli("draw", "--corpus", self.corpus, "--set", set_dir, "--seed", 11,
                           "--substantive", 6, "--routine", 4)
        self.assertEqual((rc, out, err), (0, "drawn substantive=6 routine=4 excluded=0\n", ""))

    def test_pilot_order_is_pinned(self):
        set_dir = self.draw("pilot", 3, 2, 1)
        draw = json.loads(read(set_dir / "draw.json"))
        self.assertEqual(draw["order"], PILOT_ORDER)

    def test_draw_refuses_existing_or_in_repo_dir(self):
        existing = self.root / "taken"
        existing.mkdir()
        (existing / "marker.txt").write_text("mine")  # no key.json here: only the mkdir refusal can stop this
        rc, out, err = cli("draw", "--corpus", self.corpus, "--set", existing, "--seed", 1,
                           "--substantive", 2, "--routine", 1)
        self.assertEqual(rc, 1)
        self.assertIn("already exists", err)
        self.assertEqual(sorted(p.name for p in existing.iterdir()), ["marker.txt"])
        fake_repo = self.root / "fakerepo"  # a stand-in repo: a guard mutated away cannot litter the shared checkout
        inrepo = fake_repo / "sets" / "never-created"  # absent: only the repo refusal can stop this
        with mock.patch.object(run.archive, "REPO_ROOT", fake_repo):
            rc, out, err = cli("draw", "--corpus", self.corpus, "--set", inrepo, "--seed", 1,
                               "--substantive", 2, "--routine", 1)
        self.assertEqual(rc, 1)
        self.assertIn("inside the repository", err)
        self.assertFalse(inrepo.exists())
        self.assertFalse(fake_repo.exists())
        # positive control: a fresh directory outside the repo is accepted with the same arguments
        self.draw("fresh", 1, 2, 1)

    def test_draw_refuses_an_unsafe_set_name_and_writes_nothing_on_a_short_pool(self):
        rc, _, err = cli("draw", "--corpus", self.corpus, "--set", self.root / ".hidden", "--seed", 1,
                         "--substantive", 1, "--routine", 1)
        self.assertEqual(rc, 1)
        self.assertIn("set name", err)
        big = self.root / "toobig"
        rc, _, err = cli("draw", "--corpus", self.corpus, "--set", big, "--seed", 1,
                         "--substantive", 16, "--routine", 1)  # 15 substantive exist
        self.assertEqual(rc, 1)
        self.assertIn("15 candidates", err)
        self.assertFalse(big.exists())

    def test_main_draw_excludes_pilot_units(self):
        pilot = self.draw("pilot", 5, 3, 2)
        pilot_keys = {v["case_key"] for v in json.loads(read(pilot / "key.json")).values()}
        # positive control: the SAME seed and sizes without --exclude-set redraw the pilot's units
        same = self.draw("same", 5, 3, 2)
        self.assertEqual({v["case_key"] for v in json.loads(read(same / "key.json")).values()}, pilot_keys)
        main = self.root / "main"
        rc, out, err = cli("draw", "--corpus", self.corpus, "--set", main, "--seed", 5, "--substantive", 3,
                           "--routine", 2, "--exclude-set", pilot)
        self.assertEqual((rc, err), (0, ""))
        self.assertEqual(out, "drawn substantive=3 routine=2 excluded=5\n")
        main_keys = {v["case_key"] for v in json.loads(read(main / "key.json")).values()}
        self.assertEqual(len(main_keys), 5)
        self.assertEqual(main_keys & pilot_keys, set())
        # boundary on the pool itself: 15 - 3 = 12 substantive remain, 12 is drawable and 13 is not
        rc, _, err = cli("draw", "--corpus", self.corpus, "--set", self.root / "all12", "--seed", 5,
                         "--substantive", 12, "--routine", 0, "--exclude-set", pilot)
        self.assertEqual((rc, err), (0, ""))
        rc, _, err = cli("draw", "--corpus", self.corpus, "--set", self.root / "all13", "--seed", 5,
                         "--substantive", 13, "--routine", 0, "--exclude-set", pilot)
        self.assertEqual(rc, 1)
        self.assertIn("12 candidates", err)
        rc, _, err = cli("draw", "--corpus", self.corpus, "--set", self.root / "nokey", "--seed", 5,
                         "--substantive", 1, "--routine", 0, "--exclude-set", self.root / "no-such-set")
        self.assertEqual(rc, 1)
        self.assertIn("exclude", err)

    def test_draw_refuses_an_exclude_set_from_another_frame(self):
        # a pilot drawn from a DIFFERENT frame (other session id -> other case_keys): disjointness would be fiction
        foreign_corpus = fx.build_corpus(self.root / "foreign", [dict(session(0), sid=sid_for(50))])
        foreign = self.root / "foreign-set"
        rc, _, err = cli("draw", "--corpus", foreign_corpus, "--set", foreign, "--seed", 5, "--substantive", 2,
                         "--routine", 1)
        self.assertEqual((rc, err), (0, ""))
        rc, out, err = cli("draw", "--corpus", self.corpus, "--set", self.root / "m1", "--seed", 5,
                           "--substantive", 2, "--routine", 1, "--exclude-set", foreign)
        self.assertEqual(rc, 1)
        self.assertEqual(err, "error: --exclude-set holds 3 case_key(s) that are not in this frame; the pilot must "
                              "come from the same frame, or disjointness is not guaranteed\n")
        self.assertFalse((self.root / "m1").exists())
        # ONE foreign key among real ones is enough to refuse
        pilot = self.draw("pilot", 5, 3, 2)
        key = json.loads(read(pilot / "key.json"))
        key["ffffffffff"] = {"case_key": "work/nope|transcripts/x.jsonl|mX"}
        (pilot / "key.json").write_text(json.dumps(key))
        rc, out, err = cli("draw", "--corpus", self.corpus, "--set", self.root / "m2", "--seed", 6,
                           "--substantive", 2, "--routine", 1, "--exclude-set", pilot)
        self.assertEqual(rc, 1)
        self.assertIn("holds 1 case_key(s) that are not in this frame", err)
        self.assertFalse((self.root / "m2").exists())

    def test_draw_refuses_a_malformed_exclude_set(self):
        cases = {"bad-json": "{not json", "missing-field": json.dumps({"aaaaaaaaaa": {"stratum": "routine"}}),
                 "not-a-map": json.dumps(["x"]), "entry-not-object": json.dumps({"aaaaaaaaaa": 3})}
        for name, text in cases.items():
            with self.subTest(name):
                bad = self.root / f"bad-{name}"
                bad.mkdir()
                (bad / "key.json").write_text(text)
                rc, out, err = cli("draw", "--corpus", self.corpus, "--set", self.root / f"m-{name}", "--seed", 5,
                                   "--substantive", 1, "--routine", 0, "--exclude-set", bad)
                self.assertEqual(rc, 1)
                self.assertEqual(err, f"error: --exclude-set {bad}: key.json is malformed "
                                      f"(not a case_id -> case_key map)\n")
                self.assertFalse((self.root / f"m-{name}").exists())


# Pinned after observation (see the report): the pilot draw (seed 3, 2 substantive + 1 routine) shuffled by
# random.Random(4). A literal, not a recomputation.
PILOT_ORDER = ["6b4267295b", "8c61c89952", "b9ba446c23"]


class Render(Base):
    def test_render_builds_every_packet_and_fills_hashes(self):
        set_dir = self.draw("main", 11, 6, 4)
        before = json.loads(read(set_dir / "draw.json"))
        rc, out, err = cli("render", "--corpus", self.corpus, "--set", set_dir)
        self.assertEqual((rc, out, err), (0, "rendered 10 packets\n", ""))  # counts only
        draw = json.loads(read(set_dir / "draw.json"))
        self.assertEqual(draw["order"], before["order"])
        self.assertEqual([c["case_id"] for c in draw["cases"]], [c["case_id"] for c in before["cases"]])
        self.assertEqual(mode(set_dir / "packets"), 0o700)
        files = sorted(p.name for p in (set_dir / "packets").iterdir())
        self.assertEqual(files, sorted(f"{c['case_id']}.md" for c in draw["cases"]))
        for c in draw["cases"]:
            p = set_dir / "packets" / f"{c['case_id']}.md"
            self.assertEqual(hashlib.sha256(p.read_bytes()).hexdigest(), c["sha256"])
            self.assertEqual(mode(p), 0o600)
        self.assertEqual(label.verify(set_dir), {"ok": 10, "mismatch": []})
        # the printed output carries no packet text (positive control: the text IS in the packets)
        all_text = "".join(read(p) for p in (set_dir / "packets").iterdir())
        for phrase in ("Reading the module.", "Fixed and verified."):
            self.assertIn(phrase, all_text)
            self.assertNotIn(phrase, out + err)

    def test_render_refuses_a_set_that_is_already_rendered(self):
        set_dir = self.rendered()
        draw_bytes = (set_dir / "draw.json").read_bytes()
        packets = {p.name: p.read_bytes() for p in (set_dir / "packets").iterdir()}
        rc, out, err = cli("render", "--corpus", self.corpus, "--set", set_dir)
        self.assertEqual(rc, 1)
        self.assertIn("already rendered", err)
        self.assertEqual((set_dir / "draw.json").read_bytes(), draw_bytes)
        self.assertEqual({p.name: p.read_bytes() for p in (set_dir / "packets").iterdir()}, packets)

    def test_render_aborts_on_token(self):
        corpus = fx.build_corpus(self.root / "token-corpus", make_sessions(token_session=2))
        set_dir = self.root / "tok"
        rc, _, err = cli("draw", "--corpus", corpus, "--set", set_dir, "--seed", TOKEN_SEED,
                         "--substantive", 15, "--routine", 2)  # all 15 substantive: the token unit is in
        self.assertEqual((rc, err), (0, ""))
        (unit,) = [u for u in sampler.frame(corpus, excluded_sids=set()) if u.message_id == "m2c"]
        token_cid = packet.case_id_for(unit, TOKEN_SEED)
        draw_bytes = (set_dir / "draw.json").read_bytes()
        draw = json.loads(draw_bytes)
        # precondition: at least one packet is built BEFORE the token one, so an abort that
        # wrote as it went would leave files behind
        self.assertGreater([c["case_id"] for c in draw["cases"]].index(token_cid), 0)
        rc, out, err = cli("render", "--corpus", corpus, "--set", set_dir)
        self.assertEqual(rc, 1)
        self.assertIn(token_cid, err)
        self.assertNotIn("ghp_", out + err)
        self.assertNotIn("Fixed and verified", out + err)
        self.assertEqual((set_dir / "draw.json").read_bytes(), draw_bytes)
        packets = set_dir / "packets"
        self.assertEqual(sorted(packets.iterdir()) if packets.exists() else [], [])
        # positive control: the identical draw over the token-free corpus renders
        rc, out, err = cli("render", "--corpus", self.corpus, "--set", self.draw_same(TOKEN_SEED, 15, 2))
        self.assertEqual((rc, err), (0, ""))

    def draw_same(self, seed, sub, rou):
        return self.draw("clean", seed, sub, rou)

    def test_render_fills_draw_json_atomically(self):
        set_dir = self.draw("main", 11, 6, 4)
        draw_bytes = (set_dir / "draw.json").read_bytes()
        real_replace = os.replace

        def failing(src, dst, *a, **k):
            if pathlib.Path(dst).name == "draw.json":
                raise OSError("simulated crash during the draw.json swap")
            return real_replace(src, dst, *a, **k)

        with mock.patch.object(os, "replace", side_effect=failing):
            with self.assertRaises(OSError):
                cli("render", "--corpus", self.corpus, "--set", set_dir)
        self.assertEqual((set_dir / "draw.json").read_bytes(), draw_bytes)  # neither torn nor half-filled
        self.assertEqual([p.name for p in set_dir.iterdir() if p.name.endswith(".tmp")], [])
        # and the set is still renderable afterwards
        rc, out, err = cli("render", "--corpus", self.corpus, "--set", set_dir)
        self.assertEqual((rc, err), (0, ""))

    def test_render_refuses_a_unit_that_no_longer_classifies_as_drawn(self):
        set_dir = self.draw("main", 11, 6, 4)
        key = json.loads(read(set_dir / "key.json"))
        cid = next(iter(key))
        key[cid]["stratum"] = "routine"  # the sampler would now place this unit elsewhere
        (set_dir / "key.json").write_text(json.dumps(key))
        rc, out, err = cli("render", "--corpus", self.corpus, "--set", set_dir)
        self.assertEqual(rc, 1)
        self.assertIn(cid, err)
        self.assertIn("no longer classifies", err)
        self.assertFalse((set_dir / "packets").exists())


# Seed for the token-corpus draw: chosen so the token unit is NOT the first case in draw.json (asserted).
TOKEN_SEED = 1


class Estimate(Base):
    def estimate(self, set_dir, name="result.json"):
        out = self.root / name
        frame = self.root / "frame.json"
        if not frame.exists():
            self.frame_file()
        rc, so, se = cli("estimate", "--set", set_dir, "--frame", frame, "--seed", 99, "--out", out)
        return rc, so, se, out

    def test_frame_draw_render_estimate_export_end_to_end(self):
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        rc, so, se, out = self.estimate(set_dir)
        self.assertEqual((rc, se), (0, ""))
        self.assertEqual(json.loads(so), json.loads(read(out)))  # stdout and RESULT.json agree
        res = json.loads(read(out))
        self.assertEqual(res["status"], "COMPLETE")
        self.assertEqual(res.get("corpus_id"), CORPUS_ID)
        self.assertEqual(res["drawn"], {"substantive": 6, "routine": 4})
        self.assertEqual(res["labelled"], {"substantive": 6, "routine": 4})
        allc = res["results"]["all_cases"]
        sub = allc["substantive"]
        self.assertEqual((sub["n"], sub["k"], sub["unresolved"]), (6, 4, 0))
        self.assertAlmostEqual(sub["rate"], 4 / 6)
        self.assertAlmostEqual(sub["decision"]["wilson"][0], 0.300, places=3)  # Wilson(4, 6), by hand: 0.6016 - 0.3016
        self.assertAlmostEqual(sub["decision"]["wilson"][1], 0.903, places=3)
        self.assertEqual(sub["decision"]["outcome"], "inconclusive")  # pinned: Wilson says go, the bootstrap disagrees
        self.assertEqual(sub["decision"]["guards"], ["interval_disagreement"])
        self.assertEqual((sub["decision"]["wilson_outcome"], sub["decision"]["boot_outcome"]), ("go", "inconclusive"))
        self.assertEqual(sub["by_kind"], {"handback": {"n": 1, "k": 1}, "top": {"n": 5, "k": 3}})
        self.assertEqual(sub["by_label"], {"verify": 4, "qualify": 0, "correct": 0, "none": 2, "unresolved": 0})
        self.assertEqual(sub["by_delivery"], {"silent": 2, "quiet": 4, "interrupt": 0})
        self.assertEqual(sub["by_recall"], {"y": {"n": 1, "k": 1}, "n": {"n": 5, "k": 3}})
        self.assertEqual(allc["routine"]["n"], 4)
        self.assertEqual(allc["routine"]["k"], 1)
        self.assertIsNotNone(allc["overall"])
        self.assertEqual(allc["overall"]["weights"], {"substantive": 15, "routine": 18})
        self.assertAlmostEqual(allc["overall"]["rate"], (4 / 6 * 15 + 1 / 4 * 18) / 33)
        self.assertAlmostEqual(allc["median_seconds"]["value"], 904.623)
        self.assertEqual(allc["median_seconds"]["population"],
                         "median labelling time in seconds over all labelled cases of both strata, n=10")
        self.assertEqual(allc["median_seconds"]["corpus_id"], CORPUS_ID)
        wo = res["results"]["without_recall_flagged"]
        self.assertEqual((wo["substantive"]["n"], wo["substantive"]["k"]), (5, 3))
        self.assertAlmostEqual(wo["median_seconds"]["value"], 905.123)
        self.assertEqual(wo["median_seconds"]["population"],
                         "median labelling time in seconds over all labelled cases of both strata, "
                         "recall-flagged cases excluded, n=9")
        self.assertEqual(wo["median_seconds"]["corpus_id"], CORPUS_ID)
        self.assertIsNone(res["results"]["self_agreement"])
        # and the export of the same set: pinned literals, not recomputed
        exp = self.root / "fakerepo" / "data"
        (self.root / "fakerepo").mkdir()
        with mock.patch.object(run.archive, "REPO_ROOT", self.root / "fakerepo"):
            rc, out, err = cli("export", "--set", set_dir, "--out-dir", exp)
        self.assertEqual((rc, out, err), (0, "exported 10 cases, 10 labels, 0 relabels\n", ""))
        self.assertEqual(json.loads(read(exp / "main-draw.json")), {
            "set_id": "main", "seed": 11,
            "cases": [{"case_id": c, "sha256": s, "stratum": st, "kind": k} for c, s, st, k in EXPORT_CASES]})
        self.assertEqual([json.loads(l) for l in read(exp / "main-labels.jsonl").splitlines()],
                         [dict(zip(LABEL_FIELDS, row)) for row in EXPORT_LABELS])

    def test_estimate_attaches_corpus_id_and_population_to_every_figure_block(self):
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        with open(set_dir / "relabels.jsonl", "w") as f:
            for r in [json.loads(l) for l in read(set_dir / "labels.jsonl").splitlines()][:3]:
                f.write(json.dumps(dict(r, delivery=r["delivery"])) + "\n")
        rc, so, se, out = self.estimate(set_dir)
        self.assertEqual((rc, se), (0, ""))
        res = json.loads(read(out))["results"]
        want = {("all_cases", "substantive"): "substantive stratum, n=6",
                ("all_cases", "routine"): "routine stratum, n=4",
                ("all_cases", "overall"): "overall, frame-weighted over substantive+routine strata, n=10",
                ("without_recall_flagged", "substantive"): "substantive stratum, recall-flagged cases excluded, n=5",
                ("without_recall_flagged", "routine"): "routine stratum, recall-flagged cases excluded, n=4",
                ("without_recall_flagged", "overall"):
                    "overall, frame-weighted over substantive+routine strata, recall-flagged cases excluded, n=9"}
        for (branch, block), population in want.items():
            self.assertEqual(res[branch][block].get("population"), population, (branch, block))
            self.assertEqual(res[branch][block].get("corpus_id"), CORPUS_ID, (branch, block))
        sa = res["self_agreement"]
        self.assertIsNotNone(sa)
        self.assertEqual(sa.get("population"), "relabelled cases of the main set, n=3")
        self.assertEqual(sa.get("corpus_id"), CORPUS_ID)

    def test_estimate_output_is_aggregates_only(self):
        set_dir = self.rendered()
        recs = write_labels(set_dir, sub_hits=4, rou_hits=1)
        with open(set_dir / "relabels.jsonl", "w") as f:
            for r in recs[:3]:
                f.write(json.dumps(r) + "\n")
        rc, so, se, out = self.estimate(set_dir)
        self.assertEqual((rc, se), (0, ""))
        key = json.loads(read(set_dir / "key.json"))
        private = read(set_dir / "key.json") + read(set_dir / "labels.jsonl")
        secrets = (list(key) + [v["case_key"] for v in key.values()] + [v["copy_id"] for v in key.values()]
                   + [v["copy_id"].split("/", 1)[1] for v in key.values()]  # the bare session ids
                   + [NOTE, LABELLED_AT, str(OUTLIER_SECONDS), "01-work", "proj"])
        published = so + read(out)
        for s in secrets:
            self.assertIn(s, private, s)  # positive control: every sentinel IS in the private files
            self.assertNotIn(s, published, s)
        sa = json.loads(so)["results"]["self_agreement"]
        self.assertIsNotNone(sa)
        self.assertEqual(sa["n"], 3)  # the relabel block is in the output

    def test_estimate_withholds_the_decision_unless_every_substantive_case_is_labelled(self):
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1, drop_sub=1)  # 5 of 6 substantive labelled
        rc, so, se, out = self.estimate(set_dir)
        self.assertEqual(rc, 0)
        self.assertIn("PARTIAL", se)
        res = json.loads(read(out))
        self.assertEqual(res["status"], "PARTIAL")
        self.assertEqual((res["drawn"]["substantive"], res["labelled"]["substantive"]), (6, 5))
        for branch in ("all_cases", "without_recall_flagged"):
            sub = res["results"][branch]["substantive"]
            self.assertNotIn("decision", sub)
            self.assertEqual(sub.get("decision_withheld"), "PARTIAL")
            self.assertIn("rate", sub)  # the figures themselves are still reported
        self.assertNotIn('"decision"', so)
        self.assertNotIn('"go"', so)
        # the other side of the equality: the full labelling of the same set is COMPLETE with a decision
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        rc, so2, se2, out2 = self.estimate(set_dir, "result2.json")
        self.assertEqual((rc, se2), (0, ""))
        full = json.loads(read(out2))
        self.assertEqual(full["status"], "COMPLETE")
        self.assertIn("decision", full["results"]["all_cases"]["substantive"])

    def test_an_unlabelled_routine_case_does_not_withhold_the_substantive_decision(self):
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1, keep_routine=2)
        rc, so, se, out = self.estimate(set_dir)
        self.assertEqual((rc, se), (0, ""))
        res = json.loads(read(out))
        self.assertEqual(res["status"], "COMPLETE")
        self.assertEqual((res["drawn"]["routine"], res["labelled"]["routine"]), (4, 2))
        self.assertEqual(res["results"]["all_cases"]["routine"]["population"], "routine stratum, n=2")

    def test_estimate_refuses_an_unrendered_set_and_labels_that_do_not_match_draw_json(self):
        set_dir = self.draw("main", 11, 6, 4)  # drawn, never rendered: draw.json has no hashes
        (set_dir / "labels.jsonl").write_text("")
        rc, so, se, out = self.estimate(set_dir)
        self.assertEqual(rc, 1)
        self.assertIn("not rendered", se)
        self.assertFalse(out.exists())
        rendered = self.rendered("main2", seed=12)
        recs = write_labels(rendered, sub_hits=4, rou_hits=1)
        recs[3]["packet_sha256"] = "0" * 64
        (rendered / "labels.jsonl").write_text("".join(json.dumps(r) + "\n" for r in recs))
        rc, so, se, out = self.estimate(rendered, "r2.json")
        self.assertEqual(rc, 1)
        self.assertIn("1 label record(s) do not match", se)
        self.assertFalse(out.exists())

    def test_estimate_refuses_to_overwrite_its_result(self):
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        rc, so, se, out = self.estimate(set_dir)
        self.assertEqual(rc, 0)
        before = read(out)
        rc, so, se, out = self.estimate(set_dir)
        self.assertEqual(rc, 1)
        self.assertIn("exists", se)
        self.assertEqual(read(out), before)


class SetGuards(Base):
    """The checks every command that opens an existing set shares (_load_set)."""

    def test_render_estimate_and_export_refuse_a_set_inside_the_repo(self):
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        frame = self.frame_file()
        with mock.patch.object(run.archive, "REPO_ROOT", self.root):  # the set is now "inside the repo"
            attempts = {
                "render": ("render", "--corpus", self.corpus, "--set", set_dir),
                "estimate": ("estimate", "--set", set_dir, "--frame", frame, "--seed", 1,
                             "--out", self.root / "r.json"),
                "export": ("export", "--set", set_dir, "--out-dir", self.root / "exp"),  # the out dir IS inside
            }
            for name, argv in attempts.items():
                rc, out, err = cli(*argv)
                self.assertEqual(rc, 1, name)
                self.assertIn("inside the repository", err, name)
        self.assertFalse((self.root / "r.json").exists())
        self.assertFalse((self.root / "exp").exists())

    def test_a_tampered_draw_json_is_refused_by_render(self):
        # a path-shaped id that key.json also holds: only the id's SHAPE can refuse it
        a = self.draw("shape", 11, 2, 1)
        draw, key = json.loads(read(a / "draw.json")), json.loads(read(a / "key.json"))
        key["../escape"] = key.pop(draw["cases"][0]["case_id"])
        draw["cases"][0]["case_id"] = "../escape"
        (a / "key.json").write_text(json.dumps(key))
        (a / "draw.json").write_text(json.dumps(draw))
        # a well-formed id that key.json lacks: only the membership can refuse it
        b = self.draw("member", 11, 2, 1)
        draw = json.loads(read(b / "draw.json"))
        draw["cases"][0]["case_id"] = "0123456789"
        (b / "draw.json").write_text(json.dumps(draw))
        for set_dir in (a, b):
            rc, out, err = cli("render", "--corpus", self.corpus, "--set", set_dir)
            self.assertEqual(rc, 1, set_dir.name)
            self.assertIn("draw.json names a case_id", err)
            self.assertFalse((set_dir / "packets").exists())

    def test_a_set_directory_that_is_not_a_set_is_refused(self):
        empty = self.root / "empty"
        empty.mkdir()
        rc, out, err = cli("render", "--corpus", self.corpus, "--set", empty)
        self.assertEqual(rc, 1)
        self.assertIn("is not a label set", err)

    def test_estimate_refuses_a_relabel_that_does_not_match_draw_json(self):
        set_dir = self.rendered()
        recs = write_labels(set_dir, sub_hits=4, rou_hits=1)
        (set_dir / "relabels.jsonl").write_text(json.dumps(dict(recs[0], packet_sha256="f" * 64)) + "\n")
        rc, out, err = cli("estimate", "--set", set_dir, "--frame", self.frame_file(), "--seed", 1,
                           "--out", self.root / "r.json")
        self.assertEqual(rc, 1)
        self.assertIn("1 label record(s) do not match", err)
        self.assertFalse((self.root / "r.json").exists())

    def test_export_refuses_an_unsafe_set_id(self):
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        draw = json.loads(read(set_dir / "draw.json"))
        draw["set_id"] = "../../escape"
        (set_dir / "draw.json").write_text(json.dumps(draw))
        repo = self.root / "fakerepo"
        repo.mkdir()
        with mock.patch.object(run.archive, "REPO_ROOT", repo):
            rc, out, err = cli("export", "--set", set_dir, "--out-dir", repo / "data")
        self.assertEqual(rc, 1)
        self.assertIn("not a plain file name", err)
        self.assertEqual(list(repo.iterdir()), [])

    def test_render_refuses_a_case_whose_unit_is_not_in_the_corpus(self):
        set_dir = self.draw("main", 11, 6, 4)
        other = fx.build_corpus(self.root / "other-corpus", [session(0)])  # sessions 1-5 are absent
        rc, out, err = cli("render", "--corpus", other, "--set", set_dir)
        self.assertEqual(rc, 1)
        self.assertIn("its unit is not in the corpus", err)
        self.assertFalse((set_dir / "packets").exists())

    def test_estimate_refuses_a_duplicate_label_instead_of_crashing(self):
        set_dir = self.rendered()
        recs = write_labels(set_dir, sub_hits=4, rou_hits=1)
        with open(set_dir / "labels.jsonl", "a") as f:
            f.write(json.dumps(recs[-1]) + "\n")  # a ROUTINE case labelled twice: the estimator's own refusal
        rc, out, err = cli("estimate", "--set", set_dir, "--frame", self.frame_file(), "--seed", 1,
                           "--out", self.root / "r.json")
        self.assertEqual(rc, 1)
        self.assertIn(f"duplicate label for case_id '{recs[-1]['case_id']}'", err)
        self.assertFalse((self.root / "r.json").exists())

    def test_export_writes_nothing_when_only_one_target_exists(self):
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        repo = self.root / "fakerepo"
        out_dir = repo / "data"
        out_dir.mkdir(parents=True)
        (out_dir / "main-labels.jsonl").write_text("earlier evidence")  # the SECOND file of the pair exists
        with mock.patch.object(run.archive, "REPO_ROOT", repo):
            rc, out, err = cli("export", "--set", set_dir, "--out-dir", out_dir)
        self.assertEqual(rc, 1)
        self.assertIn("is not a prefix of the new export; an exported label file only grows", err)
        self.assertEqual(sorted(p.name for p in out_dir.iterdir()), ["main-labels.jsonl"])  # no half-written pair
        self.assertEqual(read(out_dir / "main-labels.jsonl"), "earlier evidence")

    def test_a_key_json_entry_draw_json_does_not_hold_is_refused_at_load(self):
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        key = json.loads(read(set_dir / "key.json"))
        key["abcdef0123"] = dict(next(iter(key.values())))  # the set is bigger than the draw says
        (set_dir / "key.json").write_text(json.dumps(key))
        repo = self.root / "fakerepo"
        repo.mkdir()
        attempts = {"render": ("render", "--corpus", self.corpus, "--set", set_dir),
                    "estimate": ("estimate", "--set", set_dir, "--frame", self.frame_file(), "--seed", 1,
                                 "--out", self.root / "r.json"),
                    "export": ("export", "--set", set_dir, "--out-dir", repo / "data")}
        with mock.patch.object(run.archive, "REPO_ROOT", repo):
            for name, argv in attempts.items():
                rc, out, err = cli(*argv)
                self.assertEqual(rc, 1, name)
                self.assertIn("key.json holds 1 case(s) draw.json does not; the two must name the same cases", err, name)
        self.assertFalse((self.root / "r.json").exists())
        self.assertEqual(list(repo.iterdir()), [])

    def test_estimate_refuses_a_label_for_a_case_draw_json_does_not_hold(self):
        # p2: a null-hash label for a case_id in neither draw.json nor key.json -> was a KeyError crash
        set_dir = self.rendered()
        recs = write_labels(set_dir, sub_hits=4, rou_hits=1)
        frame = self.frame_file()
        for extra_sha in (None, "a" * 64):  # a real-looking hash does not help a stray case either
            with open(set_dir / "labels.jsonl", "a") as f:
                f.write(json.dumps(dict(recs[0], case_id="0123456789", packet_sha256=extra_sha)) + "\n")
            rc, out, err = cli("estimate", "--set", set_dir, "--frame", frame, "--seed", 1,
                               "--out", self.root / "r.json")
            self.assertEqual(rc, 1, extra_sha)
            self.assertEqual(err, "error: 1 label record(s) name a case_id that draw.json does not hold; "
                                  "refusing to estimate\n")
            self.assertFalse((self.root / "r.json").exists())
            (set_dir / "labels.jsonl").write_text("".join(json.dumps(r) + "\n" for r in recs))
        for stray_id in ("0123456789", ["0123456789"]):  # unknown, and unhashable
            with self.subTest(str(stray_id)):
                (set_dir / "labels.jsonl").write_text("".join(json.dumps(r) + "\n" for r in recs)
                                                      + json.dumps(dict(recs[0], case_id=stray_id)) + "\n")
                rc, out, err = cli("estimate", "--set", set_dir, "--frame", frame, "--seed", 1,
                                   "--out", self.root / "r-stray.json")
                self.assertEqual(rc, 1)
                self.assertIn("1 label record(s) name a case_id that draw.json does not hold", err)
        self.assertFalse((self.root / "r-stray.json").exists())
        (set_dir / "labels.jsonl").write_text("".join(json.dumps(r) + "\n" for r in recs))
        # the same stray record as a RELABEL is refused too
        (set_dir / "relabels.jsonl").write_text(json.dumps(dict(recs[0], case_id="0123456789")) + "\n")
        rc, out, err = cli("estimate", "--set", set_dir, "--frame", frame, "--seed", 1,
                           "--out", self.root / "r.json")
        self.assertEqual(rc, 1)
        self.assertIn("1 label record(s) name a case_id that draw.json does not hold", err)

    def test_estimate_refuses_a_null_or_missing_packet_hash(self):
        # p1: a null-hash label used to defeat the binding and then the PARTIAL gate
        set_dir = self.rendered()
        recs = write_labels(set_dir, sub_hits=4, rou_hits=1, drop_sub=1)  # 5 of 6 substantive labelled
        frame = self.frame_file()
        for name, mutate in (("null", lambda r: dict(r, packet_sha256=None)),
                             ("missing", lambda r: {k: v for k, v in r.items() if k != "packet_sha256"}),
                             ("wrong", lambda r: dict(r, packet_sha256="0" * 64))):
            with self.subTest(name):
                bad = [mutate(recs[0])] + recs[1:]
                (set_dir / "labels.jsonl").write_text("".join(json.dumps(r) + "\n" for r in bad))
                rc, out, err = cli("estimate", "--set", set_dir, "--frame", frame, "--seed", 1,
                                   "--out", self.root / f"r-{name}.json")
                self.assertEqual(rc, 1)
                self.assertEqual(err, "error: 1 label record(s) do not match draw.json's packet hashes; "
                                      "refusing to estimate\n")
                self.assertFalse((self.root / f"r-{name}.json").exists())

    def test_estimate_states_the_invariant_that_labels_cannot_outnumber_the_draw(self):
        set_dir = self.rendered()
        recs = write_labels(set_dir, sub_hits=4, rou_hits=1)
        with open(set_dir / "labels.jsonl", "a") as f:
            f.write(json.dumps(recs[0]) + "\n")  # a SUBSTANTIVE case labelled twice: 7 records for 6 cases
        rc, out, err = cli("estimate", "--set", set_dir, "--frame", self.frame_file(), "--seed", 1,
                           "--out", self.root / "r.json")
        self.assertEqual(rc, 1)
        self.assertEqual(err, "error: 7 substantive label records for 6 substantive cases drawn "
                              "(a case labelled twice?); refusing to estimate\n")
        self.assertFalse((self.root / "r.json").exists())

    def test_render_hides_an_unexpected_build_error_text(self):
        set_dir = self.draw("main", 11, 6, 4)
        secret = "work/" + sid_for(3)  # what packet.py's ValueError embeds: the unit's case_key

        with mock.patch.object(run.packet, "build_packet", side_effect=ValueError(f"unit {secret}|x is not a message")):
            rc, out, err = cli("render", "--corpus", self.corpus, "--set", set_dir)
        self.assertEqual(rc, 1)
        first = json.loads(read(set_dir / "draw.json"))["cases"][0]["case_id"]
        self.assertEqual(err, f"error: case {first}: the packet could not be built (ValueError); render aborted, "
                              f"nothing written\n")
        self.assertNotIn(sid_for(3), out + err)
        self.assertFalse((set_dir / "packets").exists())
        self.assertIsNone(json.loads(read(set_dir / "draw.json"))["cases"][0]["sha256"])



class Export(Base):
    def setUp(self):
        super().setUp()
        self.fake_repo = self.root / "fakerepo"
        self.out_dir = self.fake_repo / "docs" / "evals" / "data" / "labelled-sample"
        self.set_dir = self.rendered()
        self.recs = write_labels(self.set_dir, sub_hits=4, rou_hits=1)
        (self.set_dir / "relabel_pick.json").write_text('{"ids": ["PICK-SENTINEL"]}')
        self.fake_repo.mkdir()

    def export(self, out_dir=None):
        with mock.patch.object(run.archive, "REPO_ROOT", self.fake_repo):
            return cli("export", "--set", self.set_dir, "--out-dir", out_dir or self.out_dir)

    def test_export_omits_notes(self):
        rc, out, err = self.export()
        self.assertEqual((rc, err), (0, ""))
        self.assertEqual(out, "exported 10 cases, 10 labels, 0 relabels\n")
        files = sorted(p.name for p in self.out_dir.iterdir())
        self.assertEqual(files, ["main-draw.json", "main-labels.jsonl"])  # no key.json, pick, packets, notes
        key = json.loads(read(self.set_dir / "key.json"))
        private = "".join(read(self.set_dir / f) for f in ("key.json", "labels.jsonl", "relabel_pick.json", "draw.json"))
        packets = "".join(read(p) for p in (self.set_dir / "packets").iterdir())
        secrets = ([v["case_key"] for v in key.values()] + [v["copy_id"] for v in key.values()]
                   + [v["case_key"].rsplit("|", 1)[1] for v in key.values()]  # the message ids
                   + [NOTE, LABELLED_AT, "PICK-SENTINEL", "01-work", "proj",
                      '"note"', '"labelled_at"', '"case_key"', '"copy_id"', '"order"'])
        published = "".join(read(self.out_dir / f) for f in files)
        for s in secrets:
            self.assertIn(s, private, s)  # positive control: every sentinel IS in the private files
            self.assertNotIn(s, published, s)
        # packet text and the source's session ids / timestamps never reach the export either
        bare_sids = tuple(v["copy_id"].split("/", 1)[1] for v in key.values())
        source = "".join(p.read_text() for p in self.corpus.rglob("*.jsonl"))
        for s in ("Reading the module.", "Fixed and verified.", "T10:") + bare_sids:
            # positive control for each: it IS in the packets / the source transcripts / the private key
            self.assertTrue(s in packets or s in source or s in private, s)
            self.assertNotIn(s, published, s)
        self.assertIn("Reading the module.", packets)
        self.assertIn("Fixed and verified.", packets)
        self.assertIn("T10:", source)
        self.assertTrue(all(s in private for s in bare_sids))

    def test_export_contents(self):
        self.export()
        draw = json.loads(read(self.out_dir / "main-draw.json"))
        key = json.loads(read(self.set_dir / "key.json"))
        src = json.loads(read(self.set_dir / "draw.json"))
        self.assertEqual(set(draw), {"set_id", "seed", "cases"})
        self.assertEqual((draw["set_id"], draw["seed"]), ("main", 11))
        self.assertEqual(draw["cases"], [{"case_id": c["case_id"], "sha256": c["sha256"],
                                          "stratum": key[c["case_id"]]["stratum"], "kind": key[c["case_id"]]["kind"]}
                                         for c in src["cases"]])
        self.assertTrue(all(len(c["sha256"]) == 64 for c in draw["cases"]))
        rows = [json.loads(l) for l in read(self.out_dir / "main-labels.jsonl").splitlines()]
        self.assertEqual(len(rows), 10)
        for row, rec in zip(rows, self.recs):
            self.assertEqual(row, {k: rec[k] for k in ("case_id", "packet_sha256", "labels", "delivery",
                                                       "recall", "seconds")})

    def test_export_refuses_a_directory_outside_the_repo(self):
        outside = self.root / "elsewhere" / "data"
        rc, out, err = self.export(outside)
        self.assertEqual(rc, 1)
        self.assertIn("outside the repository", err)
        self.assertFalse(outside.exists())
        self.assertFalse((self.root / "elsewhere").exists())
        # the REAL repo root is what guards by default: a tmp dir is outside it
        rc, out, err = cli("export", "--set", self.set_dir, "--out-dir", self.root / "elsewhere2")
        self.assertEqual(rc, 1)
        self.assertIn("outside the repository", err)
        self.assertFalse((self.root / "elsewhere2").exists())
        # positive control: inside the (fake) repo the same set exports
        self.assertEqual(self.export()[0], 0)

    def test_export_is_rerunnable_and_idempotent(self):
        self.assertEqual(self.export()[0], 0)
        before = {p.name: p.read_bytes() for p in self.out_dir.iterdir()}
        rc, out, err = self.export()
        self.assertEqual((rc, out, err), (0, "exported 10 cases, 10 labels, 0 relabels\n", ""))
        self.assertEqual({p.name: p.read_bytes() for p in self.out_dir.iterdir()}, before)
        self.assertEqual(sorted(before), ["main-draw.json", "main-labels.jsonl"])  # no temp file left behind

    def test_first_export_without_labels_writes_the_draw_and_an_empty_labels_file(self):
        for how in ("absent", "empty"):
            with self.subTest(how):
                out_dir = self.fake_repo / how
                labels = self.set_dir / "labels.jsonl"
                saved = read(labels)
                if how == "absent":
                    labels.unlink()
                else:
                    labels.write_text("")
                rc, out, err = self.export(out_dir)
                self.assertEqual((rc, out, err), (0, "exported 10 cases, 0 labels, 0 relabels\n", ""))
                self.assertEqual(sorted(p.name for p in out_dir.iterdir()), ["main-draw.json", "main-labels.jsonl"])
                self.assertEqual(read(out_dir / "main-labels.jsonl"), "")
                self.assertEqual(len(json.loads(read(out_dir / "main-draw.json"))["cases"]), 10)
                labels.write_text(saved)

    def test_second_export_after_labelling_grows_the_first(self):
        lines = read(self.set_dir / "labels.jsonl").splitlines(keepends=True)
        (self.set_dir / "labels.jsonl").write_text("".join(lines[:4]))
        self.assertEqual(self.export()[0], 0)
        first = [json.loads(l) for l in read(self.out_dir / "main-labels.jsonl").splitlines()]
        draw_bytes = (self.out_dir / "main-draw.json").read_bytes()
        (self.set_dir / "labels.jsonl").write_text("".join(lines))
        rc, out, err = self.export()
        self.assertEqual((rc, out, err), (0, "exported 10 cases, 10 labels, 0 relabels\n", ""))
        second = [json.loads(l) for l in read(self.out_dir / "main-labels.jsonl").splitlines()]
        self.assertEqual((len(first), len(second)), (4, 10))
        self.assertEqual(second[:4], first)  # the old records are a prefix
        self.assertEqual([r["case_id"] for r in second], [row[0] for row in EXPORT_LABELS])
        self.assertEqual((self.out_dir / "main-draw.json").read_bytes(), draw_bytes)
        self.assertEqual(sorted(p.name for p in self.out_dir.iterdir()), ["main-draw.json", "main-labels.jsonl"])

    def test_re_export_with_a_changed_draw_is_refused(self):
        self.assertEqual(self.export()[0], 0)
        before = {p.name: p.read_bytes() for p in self.out_dir.iterdir()}
        draw = json.loads(read(self.set_dir / "draw.json"))
        draw["seed"] = 12  # any change to what the manifest would say
        (self.set_dir / "draw.json").write_text(json.dumps(draw))
        rc, out, err = self.export()
        self.assertEqual(rc, 1)
        self.assertEqual(err, f"error: {self.out_dir.resolve() / 'main-draw.json'} differs from the new draw manifest; "
                              f"the draw manifest is immutable once committed\n")
        self.assertEqual({p.name: p.read_bytes() for p in self.out_dir.iterdir()}, before)

    def test_re_export_that_drops_or_alters_an_exported_label_is_refused(self):
        self.assertEqual(self.export()[0], 0)
        before = {p.name: p.read_bytes() for p in self.out_dir.iterdir()}
        lines = read(self.set_dir / "labels.jsonl").splitlines(keepends=True)
        altered = json.loads(lines[2])
        altered["seconds"] = 1.5
        variants = {"dropped": "".join(lines[:9]), "altered": "".join(lines[:2]) + json.dumps(altered) + "\n"
                    + "".join(lines[3:]), "reordered": "".join([lines[1], lines[0]] + lines[2:])}
        for name, text in variants.items():
            with self.subTest(name):
                (self.set_dir / "labels.jsonl").write_text(text)
                rc, out, err = self.export()
                self.assertEqual(rc, 1)
                self.assertEqual(err, f"error: {self.out_dir.resolve() / 'main-labels.jsonl'} is not a prefix of the "
                                      f"new export; an exported label file only grows\n")
                self.assertEqual({p.name: p.read_bytes() for p in self.out_dir.iterdir()}, before)

    def test_re_export_that_drops_exported_relabels_is_refused_and_growth_is_allowed(self):
        lines = read(self.set_dir / "labels.jsonl").splitlines(keepends=True)
        (self.set_dir / "relabels.jsonl").write_text("".join(lines[:2]))
        self.assertEqual(self.export()[1], "exported 10 cases, 10 labels, 2 relabels\n")
        (self.set_dir / "relabels.jsonl").write_text("".join(lines[:3]))  # grows: fine
        self.assertEqual(self.export()[1], "exported 10 cases, 10 labels, 3 relabels\n")
        self.assertEqual(len(read(self.out_dir / "main-relabels.jsonl").splitlines()), 3)
        (self.set_dir / "relabels.jsonl").unlink()  # the exported file exists, the source has none
        rc, out, err = self.export()
        self.assertEqual(rc, 1)
        self.assertIn("main-relabels.jsonl is not a prefix of the new export; an exported label file only grows", err)
        self.assertEqual(len(read(self.out_dir / "main-relabels.jsonl").splitlines()), 3)

    def test_export_refuses_an_unrendered_draw(self):
        unrendered = self.draw("fresh", 12, 2, 1)
        with mock.patch.object(run.archive, "REPO_ROOT", self.fake_repo):
            rc, out, err = cli("export", "--set", unrendered, "--out-dir", self.out_dir)
        self.assertEqual(rc, 1)
        self.assertEqual(err, f"error: {unrendered} is not rendered (draw.json holds no packet hashes); "
                              f"run render first\n")
        self.assertFalse(self.out_dir.exists())

    def test_export_validates_the_values_it_commits(self):
        key = json.loads(read(self.set_dir / "key.json"))
        secret = next(iter(key.values()))["case_key"]  # contains a session id: must never be committed
        first = self.recs[0]
        cid = first["case_id"]
        bad = {  # name -> (record, expected message)
            "case_key in labels": (dict(first, labels=[secret]), f"case {cid}: field labels"),
            "case_key as case_id": (dict(first, case_id=secret), "record 0: field case_id"),
            "unknown case_id": (dict(first, case_id="0123456789"), "record 0: field case_id"),
            "case_id a list": (dict(first, case_id=[secret]), "record 0: field case_id"),
            "labels a dict": (dict(first, labels={"verify": secret}), f"case {cid}: field labels"),
            "wrong sha": (dict(first, packet_sha256="0" * 64), f"case {cid}: field packet_sha256"),
            "null sha": (dict(first, packet_sha256=None), f"case {cid}: field packet_sha256"),
            "unknown delivery": (dict(first, delivery=secret), f"case {cid}: field delivery"),
            "bad recall": (dict(first, recall=secret), f"case {cid}: field recall"),
            "bool recall": (dict(first, recall=True), f"case {cid}: field recall"),
            "nan seconds": (dict(first, seconds=float("nan")), f"case {cid}: field seconds"),
            "inf seconds": (dict(first, seconds=float("inf")), f"case {cid}: field seconds"),
            "negative seconds": (dict(first, seconds=-1), f"case {cid}: field seconds"),
            "string seconds": (dict(first, seconds=secret), f"case {cid}: field seconds"),
            "bool seconds": (dict(first, seconds=True), f"case {cid}: field seconds"),
            "labels not a list": (dict(first, labels="verify"), f"case {cid}: field labels"),
            "labels empty": (dict(first, labels=[]), f"case {cid}: field labels"),
            "none with verify": (dict(first, labels=["none", "verify"]), f"case {cid}: field labels"),
            "verify with silent": (dict(first, delivery="silent"), f"case {cid}: field labels"),
            "missing seconds": ({k: v for k, v in first.items() if k != "seconds"}, f"case {cid}: field seconds"),
        }
        original = read(self.set_dir / "labels.jsonl")
        rest = "".join(original.splitlines(keepends=True)[1:])
        for name, (rec, message) in bad.items():
            with self.subTest(name):
                (self.set_dir / "labels.jsonl").write_text(json.dumps(rec) + "\n" + rest)
                rc, out, err = self.export()
                self.assertEqual(rc, 1)
                self.assertIn(f"main-labels.jsonl: {message} is", err)
                self.assertIn("; nothing exported", err)
                self.assertNotIn(secret, out + err)
                self.assertNotIn(sid_for(0), out + err)
                self.assertFalse(self.out_dir.exists())  # nothing written
        # the same rules apply to relabels
        (self.set_dir / "labels.jsonl").write_text(original)
        (self.set_dir / "relabels.jsonl").write_text(json.dumps(dict(first, labels=[secret])) + "\n")
        rc, out, err = self.export()
        self.assertEqual(rc, 1)
        self.assertIn(f"main-relabels.jsonl: case {cid}: field labels is invalid", err)
        self.assertNotIn(secret, out + err)
        self.assertFalse(self.out_dir.exists())
        # positive control: without the bad record the same files export, whitelisted fields present
        (self.set_dir / "relabels.jsonl").unlink()
        (self.set_dir / "labels.jsonl").write_text(json.dumps(dict(first, seconds=0)) + "\n" + rest)  # 0 is a valid duration
        self.assertEqual(self.export()[0], 0)
        rows = [json.loads(l) for l in read(self.out_dir / "main-labels.jsonl").splitlines()]
        self.assertEqual(rows[0], dict(zip(LABEL_FIELDS, EXPORT_LABELS[0][:5] + (0,))))
        self.assertEqual(rows[1:], [dict(zip(LABEL_FIELDS, row)) for row in EXPORT_LABELS[1:]])

    def test_export_writes_are_atomic(self):
        lines = read(self.set_dir / "labels.jsonl").splitlines(keepends=True)
        (self.set_dir / "labels.jsonl").write_text("".join(lines[:4]))
        self.assertEqual(self.export()[0], 0)
        before = {p.name: p.read_bytes() for p in self.out_dir.iterdir()}
        (self.set_dir / "labels.jsonl").write_text("".join(lines))
        real_replace = os.replace

        def failing(src, dst, *a, **k):
            if pathlib.Path(dst).name == "main-labels.jsonl":
                raise OSError("simulated crash during the labels swap")
            return real_replace(src, dst, *a, **k)

        with mock.patch.object(os, "replace", side_effect=failing):
            with self.assertRaises(OSError):
                self.export()
        self.assertEqual({p.name: p.read_bytes() for p in self.out_dir.iterdir()}, before)  # whole, no .tmp

    def test_export_refuses_a_record_that_is_not_an_object(self):
        (self.set_dir / "labels.jsonl").write_text("[1, 2]\n")
        rc, out, err = self.export()
        self.assertEqual(rc, 1)
        self.assertEqual(err, "error: main-labels.jsonl: record 0 is not an object; nothing exported\n")
        self.assertFalse(self.out_dir.exists())


    def test_export_relabels_are_whitelisted_too(self):
        with open(self.set_dir / "relabels.jsonl", "w") as f:
            for r in self.recs[:3]:
                f.write(json.dumps(dict(r, note=f"{NOTE}-relabel")) + "\n")
        rc, out, err = self.export()
        self.assertEqual((rc, err), (0, ""))
        self.assertEqual(out, "exported 10 cases, 10 labels, 3 relabels\n")
        self.assertTrue((self.out_dir / "main-relabels.jsonl").is_file())
        text = read(self.out_dir / "main-relabels.jsonl")
        rows = [json.loads(l) for l in text.splitlines()]
        self.assertEqual(len(rows), 3)
        for row, rec in zip(rows, self.recs):
            self.assertEqual(row, {k: rec[k] for k in ("case_id", "packet_sha256", "labels", "delivery",
                                                       "recall", "seconds")})
        self.assertNotIn(NOTE, text)


if __name__ == "__main__":
    unittest.main()
