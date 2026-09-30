"""Task 5 of the labelled sample: run.py frame / draw / render / estimate / export.

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_run_sample.py -v

Every transcript here is SYNTHETIC (tests/measure_corpus_fixture.py); nothing reads a real corpus.
The corpus has 6 sessions -> 33 units: 18 routine, 12 substantive top-level, 3 substantive
hand-backs (sessions 0-2 have one subagent file each). Fixture details that carry a guard are
annotated on their own line with what breaks if they go.
"""
import atexit
import contextlib
import hashlib
import importlib.util
import io
import json
import os
import pathlib
import random
import shutil
import stat
import sys
import tempfile
import unittest
import uuid
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
SETP = "2026-09-29-labelled-"  # the shape of a real set directory name
MAIN = SETP + "main"
SETID_MESSAGE = ("draw.json's set_id must equal the set directory's name, which must look like "
                "2026-09-29-labelled-main and hold no session-id-shaped run; nothing exported")
MAIN_DRAW, MAIN_LABELS, MAIN_RELABELS = MAIN + "-draw.json", MAIN + "-labels.jsonl", MAIN + "-relabels.jsonl"
# ghp_ + exactly 36 alphanumerics: the shape packet._TOKEN_RE refuses.
TOKEN = "ghp_" + "A1" * 18
NOTE = "NOTE-SENTINEL-do-not-export"
LABELLED_AT = "2031-01-02T03:04:05Z"  # a time of day no source timestamp has
OUTLIER_SECONDS = 7777.777  # the max of the seconds list: never a median, so it must never be printed
COUNTS = {"substantive": {"top": 12, "handback": 3}, "routine": {"top": 18}}
# The exported record of the seed-11 main draw (6 substantive + 4 routine), pinned as literals: case id,
# packet sha256 (deterministic for the fixture), stratum, kind, in draw.json's `cases` order.
EXPORT_CASES = [
    ("9cc8493506", "430e653648753f6597d3e50082e34c1c4055ac55abb291bfb61f8bb63f2a8575", "substantive", "top"),
    ("d6a1394c31", "3123af9870be2a87a27acf6054d9361b5efe60ca1fbe8c9d3bca0d550ae11e8e", "substantive", "top"),
    ("77ba2592f7", "efc86666d0868416a0ce10ff27f2725ead80672cd8d31b560bdb4003b12187b2", "substantive", "handback"),
    ("1a64a9afdb", "57e81a664497d1700642d9e73708b14710e452457e2e2314bfc78db5ce60703c", "substantive", "top"),
    ("b837168a2c", "fc52191b68cd63951e316ae6db11bd582935ee1648acc375e8ed65ad2d79911c", "substantive", "top"),
    ("0761ce7ec5", "b195a24d7805e7a761e47e5b7535ee7ff676a7249e5a61d2775ce323874cee94", "substantive", "top"),
    ("d6ec25c96f", "5fbe09887577ec2923f988d7ddc4b0e8554ea2bdeaebe35a620d95ea25e23a91", "routine", "top"),
    ("30fc2c09dd", "776273d57d4f3812958034dd2d51f570dcb934d20d2839e4302c152d80a6bdd2", "routine", "top"),
    ("3c9284abec", "622d06adb833f76ec9030a5b6dafb2aa5f12c75e63c1fc72269fd7e54353e83c", "routine", "top"),
    ("812c9a7436", "d2d2ca3d9fbd046210803a3325dc6feb714d284ee0ed6d6e1dc9518ed8c6a919", "routine", "top"),
]
# write_labels(sub_hits=4, rou_hits=1) on that draw, in the order it writes them:
# (case id, sha256, labels, delivery, recall, seconds)
V, Q, N, S = ["verify"], "quiet", ["none"], "silent"
EXPORT_LABELS = [
    ("77ba2592f7", "efc86666d0868416a0ce10ff27f2725ead80672cd8d31b560bdb4003b12187b2", V, Q, "y", 900.123),
    ("9cc8493506", "430e653648753f6597d3e50082e34c1c4055ac55abb291bfb61f8bb63f2a8575", V, Q, "n", 901.123),
    ("1a64a9afdb", "57e81a664497d1700642d9e73708b14710e452457e2e2314bfc78db5ce60703c", V, Q, "n", 902.123),
    ("d6a1394c31", "3123af9870be2a87a27acf6054d9361b5efe60ca1fbe8c9d3bca0d550ae11e8e", V, Q, "n", 903.123),
    ("0761ce7ec5", "b195a24d7805e7a761e47e5b7535ee7ff676a7249e5a61d2775ce323874cee94", N, S, "n", 904.123),
    ("b837168a2c", "fc52191b68cd63951e316ae6db11bd582935ee1648acc375e8ed65ad2d79911c", N, S, "n", 905.123),
    ("3c9284abec", "622d06adb833f76ec9030a5b6dafb2aa5f12c75e63c1fc72269fd7e54353e83c", V, Q, "n", 906.123),
    ("812c9a7436", "d2d2ca3d9fbd046210803a3325dc6feb714d284ee0ed6d6e1dc9518ed8c6a919", N, S, "n", 907.123),
    ("d6ec25c96f", "5fbe09887577ec2923f988d7ddc4b0e8554ea2bdeaebe35a620d95ea25e23a91", N, S, "n", 908.123),
    ("30fc2c09dd", "776273d57d4f3812958034dd2d51f570dcb934d20d2839e4302c152d80a6bdd2", N, S, "n", 7777.777),
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


_AUTO_FRAMES = {}
_AUTO_DIR = tempfile.mkdtemp(prefix="measure-auto-frames-")
atexit.register(shutil.rmtree, _AUTO_DIR, ignore_errors=True)


def _auto_frame(corpus):
    """A frame file for `corpus`. `draw` requires --frame and checks it against the corpus; the many tests that are
    about something else get the frame `run.py frame` would have produced (byte-identical to Base.frame_file's).
    Keyed by the corpus path AND its files' names, sizes and mtimes, so a corpus rebuilt or changed at the same
    path never gets a stale frame."""
    root = pathlib.Path(corpus)
    stamp = tuple((str(p.relative_to(root)), p.stat().st_size, p.stat().st_mtime_ns)
                  for p in sorted(root.rglob("*")) if p.is_file()) if root.is_dir() else ()
    key = (str(corpus), stamp)
    if key not in _AUTO_FRAMES:
        path = pathlib.Path(_AUTO_DIR) / f"frame-{len(_AUTO_FRAMES)}.json"
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            run.main(["frame", "--corpus", corpus, "--out", str(path)])
        _AUTO_FRAMES[key] = path
    return _AUTO_FRAMES[key]


def cli(*argv, auto_frame=True):
    argv = [str(a) for a in argv]
    if (auto_frame and argv[:1] == ["draw"] and "--frame" not in argv and "--exclude-units" not in argv
            and "--corpus" in argv):
        argv += ["--frame", str(_auto_frame(argv[argv.index("--corpus") + 1]))]
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        rc = run.main(argv)
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
        set_dir = self.root / (SETP + name)
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
            "code_sha256": {name: hashlib.sha256((MEASURE / name).read_bytes()).hexdigest()
                            for name in ("sampler.py", "packet.py", "estimate.py", "run.py")}})
        self.assertEqual(json.loads(out), COUNTS)  # what is printed is the counts, and only them

    def test_the_kind_and_stratum_vocabularies_match_what_frame_produces(self):
        units = sampler.frame(self.corpus, excluded_sids=set())
        self.assertEqual({u.kind for u in units}, set(run.KINDS))
        self.assertEqual(run.STRATA, sampler.STRATA)
        self.assertEqual({u.stratum for u in units}, set(run.STRATA))

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
        self.assertEqual(set(draw), {"set_id", "seed", "frame_sha256", "cases", "order"})
        self.assertEqual(draw["set_id"], MAIN)
        self.assertEqual(draw["seed"], 11)
        self.assertEqual(draw["cases"], [{"case_id": cid, "sha256": None} for cid in key])
        expected = list(key)
        random.Random(12).shuffle(expected)  # seed + 1, computed here by the stdlib, not by run.py
        self.assertEqual(draw["order"], expected)
        self.assertNotEqual(draw["order"], list(key))  # non-vacuous: the shuffle moved something

    def test_draw_prints_counts_only(self):
        set_dir = self.root / (SETP + "main")
        rc, out, err = cli("draw", "--corpus", self.corpus, "--set", set_dir, "--seed", 11,
                           "--substantive", 6, "--routine", 4)
        self.assertEqual((rc, out, err), (0, "drawn substantive=6 routine=4 excluded=0 sessions_substantive=3 sessions_routine=4\n", ""))

    def test_pilot_order_is_pinned(self):
        set_dir = self.draw("pilot", 3, 2, 1)
        draw = json.loads(read(set_dir / "draw.json"))
        self.assertEqual(draw["order"], PILOT_ORDER)

    def test_draw_refuses_existing_or_in_repo_dir(self):
        existing = self.root / (SETP + "taken")
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
        big = self.root / (SETP + "toobig")
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
        main = self.root / (SETP + "main")
        rc, out, err = cli("draw", "--corpus", self.corpus, "--set", main, "--seed", 5, "--substantive", 3,
                           "--routine", 2, "--exclude-set", pilot)
        self.assertEqual((rc, err), (0, ""))
        self.assertEqual(out, "drawn substantive=3 routine=2 excluded=5 sessions_substantive=3 sessions_routine=2\n")
        main_keys = {v["case_key"] for v in json.loads(read(main / "key.json")).values()}
        self.assertEqual(len(main_keys), 5)
        self.assertEqual(main_keys & pilot_keys, set())
        # boundary on the pool itself: 15 - 3 = 12 substantive remain, 12 is drawable and 13 is not
        rc, _, err = cli("draw", "--corpus", self.corpus, "--set", self.root / (SETP + "all12"), "--seed", 5,
                         "--substantive", 12, "--routine", 0, "--exclude-set", pilot)
        self.assertEqual((rc, err), (0, ""))
        rc, _, err = cli("draw", "--corpus", self.corpus, "--set", self.root / (SETP + "all13"), "--seed", 5,
                         "--substantive", 13, "--routine", 0, "--exclude-set", pilot)
        self.assertEqual(rc, 1)
        self.assertIn("12 candidates", err)
        rc, _, err = cli("draw", "--corpus", self.corpus, "--set", self.root / (SETP + "nokey"), "--seed", 5,
                         "--substantive", 1, "--routine", 0, "--exclude-set", self.root / "no-such-set")
        self.assertEqual(rc, 1)
        self.assertIn("exclude", err)

    def test_draw_refuses_an_exclude_set_from_another_frame(self):
        # a pilot drawn from a DIFFERENT frame (other session id -> other case_keys): disjointness would be fiction
        foreign_corpus = fx.build_corpus(self.root / "foreign", [dict(session(0), sid=sid_for(50))])
        foreign = self.root / (SETP + "foreign-set")
        rc, _, err = cli("draw", "--corpus", foreign_corpus, "--set", foreign, "--seed", 5, "--substantive", 2,
                         "--routine", 1)
        self.assertEqual((rc, err), (0, ""))
        rc, out, err = cli("draw", "--corpus", self.corpus, "--set", self.root / (SETP + "m1"), "--seed", 5,
                           "--substantive", 2, "--routine", 1, "--exclude-set", foreign)
        self.assertEqual(rc, 1)
        self.assertEqual(err, "error: --exclude-set holds 3 case_key(s) that are not in this frame; the pilot must "
                              "come from the same frame, or disjointness is not guaranteed\n")
        self.assertFalse((self.root / (SETP + "m1")).exists())
        # ONE foreign key among real ones is enough to refuse
        pilot = self.draw("pilot", 5, 3, 2)
        key = json.loads(read(pilot / "key.json"))
        key["ffffffffff"] = {"case_key": "work/nope|transcripts/x.jsonl|mX"}
        (pilot / "key.json").write_text(json.dumps(key))
        rc, out, err = cli("draw", "--corpus", self.corpus, "--set", self.root / (SETP + "m2"), "--seed", 6,
                           "--substantive", 2, "--routine", 1, "--exclude-set", pilot)
        self.assertEqual(rc, 1)
        self.assertIn("holds 1 case_key(s) that are not in this frame", err)
        self.assertFalse((self.root / (SETP + "m2")).exists())

    def test_draw_refuses_a_malformed_exclude_set(self):
        cases = {"bad-json": "{not json", "missing-field": json.dumps({"aaaaaaaaaa": {"stratum": "routine"}}),
                 "not-a-map": json.dumps(["x"]), "entry-not-object": json.dumps({"aaaaaaaaaa": 3})}
        for name, text in cases.items():
            with self.subTest(name):
                bad = self.root / f"bad-{name}"
                bad.mkdir()
                (bad / "key.json").write_text(text)
                rc, out, err = cli("draw", "--corpus", self.corpus, "--set", self.root / f"{SETP}m-{name}", "--seed", 5,
                                   "--substantive", 1, "--routine", 0, "--exclude-set", bad)
                self.assertEqual(rc, 1)
                self.assertEqual(err, f"error: --exclude-set {bad}: key.json is malformed "
                                      f"(not a case_id -> case_key map)\n")
                self.assertFalse((self.root / f"{SETP}m-{name}").exists())


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
        set_dir = self.root / (SETP + "tok")
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
        seed = json.loads(read(pathlib.Path(set_dir) / "draw.json"))["seed"]  # estimate is bound to the drawn seed
        rc, so, se = cli("estimate", "--set", set_dir, "--frame", frame, "--seed", seed, "--out", out)
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
        self.assertEqual(json.loads(read(exp / MAIN_DRAW)), {
            "set_id": MAIN, "seed": 11,
            "cases": [{"case_id": c, "sha256": s, "stratum": st, "kind": k} for c, s, st, k in EXPORT_CASES]})
        self.assertEqual([json.loads(l) for l in read(exp / MAIN_LABELS).splitlines()],
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
        self.assertIn("must equal the set directory's name", err)
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
        self.assertEqual(err, f"error: duplicate label for case_id '{recs[-1]['case_id']}'; refusing to estimate\n")
        self.assertFalse((self.root / "r.json").exists())

    def test_export_writes_nothing_when_only_one_target_exists(self):
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        repo = self.root / "fakerepo"
        out_dir = repo / "data"
        out_dir.mkdir(parents=True)
        (out_dir / MAIN_LABELS).write_text("earlier evidence")  # the SECOND file of the pair exists
        with mock.patch.object(run.archive, "REPO_ROOT", repo):
            rc, out, err = cli("export", "--set", set_dir, "--out-dir", out_dir)
        self.assertEqual(rc, 1)
        self.assertIn("is not a prefix of the new export; an exported label file only grows", err)
        self.assertEqual(sorted(p.name for p in out_dir.iterdir()), [MAIN_LABELS])  # no half-written pair
        self.assertEqual(read(out_dir / MAIN_LABELS), "earlier evidence")

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

    def test_a_duplicated_case_entry_in_draw_json_is_refused_at_load(self):
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        draw = json.loads(read(set_dir / "draw.json"))
        draw["cases"].append(dict(draw["cases"][0]))  # the same case listed twice; key.json still agrees as a set
        (set_dir / "draw.json").write_text(json.dumps(draw))
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
                tail = "; nothing exported" if name == "export" else ""
                self.assertEqual(err, f"error: {set_dir}: draw.json lists a case more than once{tail}\n", name)
        self.assertFalse((self.root / "r.json").exists())

    def test_a_failing_corpus_read_never_quotes_the_exception_text(self):
        set_dir = self.draw("main", 11, 6, 4)
        secret = sid_for(4)
        boom = ValueError(f"cannot read work/{secret}")
        attempts = {"render": ("render", "--corpus", self.corpus, "--set", set_dir),
                    "draw": ("draw", "--corpus", self.corpus, "--set", self.root / (SETP + "d2"), "--seed", 1,
                             "--substantive", 1, "--routine", 1),
                    "frame": ("frame", "--corpus", self.corpus, "--out", self.root / "f2.json")}
        for name, argv in attempts.items():
            with self.subTest(name):
                with mock.patch.object(run.sampler, "frame", side_effect=boom):
                    rc, out, err = cli(*argv)
                self.assertEqual((rc, out), (1, ""))
                self.assertEqual(err, "error: the corpus could not be read (ValueError); nothing written\n")
                self.assertNotIn(secret, out + err)
        self.assertFalse((self.root / (SETP + "d2")).exists() or (self.root / "f2.json").exists())
        self.assertFalse((set_dir / "packets").exists())

    def test_estimate_refuses_an_orphan_or_duplicate_relabel(self):
        set_dir = self.rendered()
        recs = write_labels(set_dir, sub_hits=4, rou_hits=1, drop_sub=1)  # the last substantive case is unlabelled
        unlabelled = next(c for c in json.loads(read(set_dir / "draw.json"))["order"]
                          if c not in {r["case_id"] for r in recs} and
                          json.loads(read(set_dir / "key.json"))[c]["stratum"] == "substantive")
        sha = {c["case_id"]: c["sha256"] for c in json.loads(read(set_dir / "draw.json"))["cases"]}
        frame = self.frame_file()
        for name, relabels, message in (
                ("orphan", [dict(recs[0], case_id=unlabelled, packet_sha256=sha[unlabelled])],
                 f"orphan relabel for case_id '{unlabelled}'"),
                ("duplicate", [recs[0], recs[0]], f"duplicate relabel for case_id '{recs[0]['case_id']}'")):
            with self.subTest(name):
                (set_dir / "relabels.jsonl").write_text("".join(json.dumps(r) + "\n" for r in relabels))
                rc, out, err = cli("estimate", "--set", set_dir, "--frame", frame, "--seed", 1,
                                   "--out", self.root / f"r-{name}.json")
                self.assertEqual(rc, 1)
                self.assertIn(f"error: {message}", err)
                self.assertFalse((self.root / f"r-{name}.json").exists())

    def test_key_json_values_are_validated_at_load_by_render_estimate_and_export(self):
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        frame = self.frame_file()
        repo = self.root / "fakerepo"
        repo.mkdir()
        saved = read(set_dir / "key.json")
        key0 = json.loads(saved)
        cid = next(iter(key0))
        secret = key0[cid]["case_key"]  # a session id inside
        self.assertIn(secret, saved)  # positive control: the sentinel IS in the private file
        for field, values in (("stratum", [secret, "other", None, "top", "handback"]),
                              ("kind", [secret, "other", None, "substantive", "routine"])):
            for value in values:
                with self.subTest(field=field, value=str(value)[:12]):
                    key = json.loads(saved)
                    key[cid][field] = value
                    (set_dir / "key.json").write_text(json.dumps(key))
                    message = f"{set_dir}: key.json: case {cid}: field {field} is invalid"
                    attempts = {"render": (("render", "--corpus", self.corpus, "--set", set_dir), ""),
                                "estimate": (("estimate", "--set", set_dir, "--frame", frame, "--seed", 11,
                                              "--out", self.root / "r.json"), ""),
                                "export": (("export", "--set", set_dir, "--out-dir", repo / "data"),
                                           "; nothing exported")}
                    with mock.patch.object(run.archive, "REPO_ROOT", repo):
                        for name, (argv, tail) in attempts.items():
                            rc, out, err = cli(*argv)
                            self.assertEqual((rc, out, err), (1, "", f"error: {message}{tail}\n"), name)
        (set_dir / "key.json").write_text(saved)
        rc, out, err = cli("estimate", "--set", set_dir, "--frame", frame, "--seed", 11, "--out", self.root / "r.json")
        self.assertEqual((rc, err), (0, ""))  # restored: the same set estimates
        self.assertFalse((repo / "data").exists())

    def test_an_estimator_error_never_carries_a_value_out(self):
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        secret = "work/" + sid_for(2)
        with mock.patch.object(run.estimator, "estimate", side_effect=ValueError(f"case {secret} is odd")):
            rc, out, err = cli("estimate", "--set", set_dir, "--frame", self.frame_file(), "--seed", 11,
                               "--out", self.root / "r.json")
        self.assertEqual((rc, out), (1, ""))
        self.assertEqual(err, "error: the estimator refused the labels (ValueError); refusing to estimate\n")
        self.assertFalse((self.root / "r.json").exists())

    def test_the_seed_is_bounded_on_both_sides_at_draw_estimate_and_export(self):
        uuid_int = uuid.UUID(sid_for(3)).int
        for i, (seed, accepted) in enumerate(((0, True), (2 ** 32 - 1, True), (2 ** 32, False), (-1, False),
                                              (10 ** 400, False), (uuid_int, False))):
            with self.subTest(draw=str(seed)[:12]):
                target = self.root / f"{SETP}seed{i}"
                rc, out, err = cli("draw", "--corpus", self.corpus, "--set", target, "--seed", seed,
                                   "--substantive", 1, "--routine", 0)
                if accepted:
                    self.assertEqual((rc, err), (0, ""))
                    self.assertEqual(json.loads(read(target / "draw.json"))["seed"], seed)
                else:
                    self.assertEqual((rc, out, err), (1, "", "error: --seed must be an integer from 0 to 2**32 - 1\n"))
                    self.assertFalse(target.exists())
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        frame = self.frame_file()
        drawn = json.loads(read(set_dir / "draw.json"))
        for j, (seed, accepted) in enumerate(((0, True), (2 ** 32 - 1, True), (2 ** 32, False), (-1, False), (uuid_int, False))):
            with self.subTest(estimate=str(seed)[:12]):
                if accepted:  # estimate is bound to the drawn seed, so draw.json carries the seed under test
                    (set_dir / "draw.json").write_text(json.dumps(dict(drawn, seed=seed)))
                out_file = self.root / f"e{j}.json"
                rc, out, err = cli("estimate", "--set", set_dir, "--frame", frame, "--seed", seed, "--out", out_file)
                self.assertEqual(rc, 0 if accepted else 1)
                self.assertEqual(out_file.exists(), accepted)
                if not accepted:
                    self.assertEqual(err, "error: --seed must be an integer from 0 to 2**32 - 1\n")
        repo = self.root / "fakerepo"
        repo.mkdir()
        draw = json.loads(read(set_dir / "draw.json"))
        for seed in (0, 2 ** 32 - 1):
            with self.subTest(export=seed):
                draw["seed"] = seed
                (set_dir / "draw.json").write_text(json.dumps(draw))
                with mock.patch.object(run.archive, "REPO_ROOT", repo):
                    rc, out, err = cli("export", "--set", set_dir, "--out-dir", repo / f"x{seed}")
                self.assertEqual((rc, err), (0, ""))
                self.assertEqual(json.loads(read(repo / f"x{seed}" / f"{MAIN}-draw.json"))["seed"], seed)

    def test_set_names_at_draw_and_at_export(self):
        long_ok = "2026-09-29-" + "a" * 41  # 41 name characters: the limit
        accepted = ["2026-09-29-labelled-pilot", "2026-09-29-labelled-main", long_ok]
        refused = {"bare uuid": sid_for(3), "date-prefixed name embedding a uuid": "2026-09-29-x-" + sid_for(3),
                   "no date": "main", "short date": "2026-9-29-x", "uppercase": "2026-09-29-Labelled",
                   "name too long": "2026-09-29-" + "a" * 42, "name starts with a digit": "2026-09-29-9x",
                   "nothing after the date": "2026-09-29-", "underscore": "2026-09-29-a_b"}
        for name in accepted:
            with self.subTest(accepted=name[:30]):
                rc, out, err = cli("draw", "--corpus", self.corpus, "--set", self.root / name, "--seed", 1,
                                   "--substantive", 1, "--routine", 0)
                self.assertEqual((rc, err), (0, ""))
        for why, name in refused.items():
            with self.subTest(refused=why):
                rc, out, err = cli("draw", "--corpus", self.corpus, "--set", self.root / name, "--seed", 1,
                                   "--substantive", 1, "--routine", 0)
                self.assertEqual((rc, out), (1, ""))
                self.assertEqual(err, "error: set name is not allowed: it must look like 2026-09-29-labelled-main "
                                      "(a date, then a lowercase name) and hold no session-id-shaped run\n")
                self.assertFalse((self.root / name).exists())
                self.assertNotIn(sid_for(3), err)
        set_dir = self.rendered("exp")
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        repo = self.root / "fakerepo"
        repo.mkdir()
        bad = self.root / ("2026-09-29-x-" + sid_for(3))
        set_dir.rename(bad)
        draw = json.loads(read(bad / "draw.json"))
        draw["set_id"] = bad.name
        (bad / "draw.json").write_text(json.dumps(draw))
        with mock.patch.object(run.archive, "REPO_ROOT", repo):
            rc, out, err = cli("export", "--set", bad, "--out-dir", repo / "data")
        self.assertEqual((rc, out, err), (1, "", f"error: {SETID_MESSAGE}\n"))
        self.assertEqual(list(repo.iterdir()), [])
        # positive control: the same set under a good name exports, names carried into the file names
        good = bad.rename(self.root / (SETP + "renamed"))
        draw["set_id"] = good.name
        (good / "draw.json").write_text(json.dumps(draw))
        with mock.patch.object(run.archive, "REPO_ROOT", repo):
            rc, out, err = cli("export", "--set", good, "--out-dir", repo / "data")
        self.assertEqual((rc, err), (0, ""))
        self.assertEqual(sorted(p.name for p in (repo / "data").iterdir()),
                         [f"{good.name}-draw.json", f"{good.name}-labels.jsonl"])




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
        self.assertEqual(files, [MAIN_DRAW, MAIN_LABELS])  # no key.json, pick, packets, notes
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
        draw = json.loads(read(self.out_dir / MAIN_DRAW))
        key = json.loads(read(self.set_dir / "key.json"))
        src = json.loads(read(self.set_dir / "draw.json"))
        self.assertEqual(set(draw), {"set_id", "seed", "cases"})
        self.assertEqual((draw["set_id"], draw["seed"]), (MAIN, 11))
        self.assertEqual(draw["cases"], [{"case_id": c["case_id"], "sha256": c["sha256"],
                                          "stratum": key[c["case_id"]]["stratum"], "kind": key[c["case_id"]]["kind"]}
                                         for c in src["cases"]])
        self.assertTrue(all(len(c["sha256"]) == 64 for c in draw["cases"]))
        rows = [json.loads(l) for l in read(self.out_dir / MAIN_LABELS).splitlines()]
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
        self.assertEqual(sorted(before), [MAIN_DRAW, MAIN_LABELS])  # no temp file left behind

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
                self.assertEqual(sorted(p.name for p in out_dir.iterdir()), [MAIN_DRAW, MAIN_LABELS])
                self.assertEqual(read(out_dir / MAIN_LABELS), "")
                self.assertEqual(len(json.loads(read(out_dir / MAIN_DRAW))["cases"]), 10)
                labels.write_text(saved)

    def test_second_export_after_labelling_grows_the_first(self):
        lines = read(self.set_dir / "labels.jsonl").splitlines(keepends=True)
        (self.set_dir / "labels.jsonl").write_text("".join(lines[:4]))
        self.assertEqual(self.export()[0], 0)
        first = [json.loads(l) for l in read(self.out_dir / MAIN_LABELS).splitlines()]
        draw_bytes = (self.out_dir / MAIN_DRAW).read_bytes()
        (self.set_dir / "labels.jsonl").write_text("".join(lines))
        rc, out, err = self.export()
        self.assertEqual((rc, out, err), (0, "exported 10 cases, 10 labels, 0 relabels\n", ""))
        second = [json.loads(l) for l in read(self.out_dir / MAIN_LABELS).splitlines()]
        self.assertEqual((len(first), len(second)), (4, 10))
        self.assertEqual(second[:4], first)  # the old records are a prefix
        self.assertEqual([r["case_id"] for r in second], [row[0] for row in EXPORT_LABELS])
        self.assertEqual((self.out_dir / MAIN_DRAW).read_bytes(), draw_bytes)
        self.assertEqual(sorted(p.name for p in self.out_dir.iterdir()), [MAIN_DRAW, MAIN_LABELS])

    def test_re_export_with_a_changed_draw_is_refused(self):
        self.assertEqual(self.export()[0], 0)
        before = {p.name: p.read_bytes() for p in self.out_dir.iterdir()}
        draw = json.loads(read(self.set_dir / "draw.json"))
        draw["seed"] = 12  # any change to what the manifest would say
        (self.set_dir / "draw.json").write_text(json.dumps(draw))
        rc, out, err = self.export()
        self.assertEqual(rc, 1)
        self.assertEqual(err, f"error: {self.out_dir.resolve() / MAIN_DRAW} differs from the new draw manifest; "
                              f"the draw manifest is immutable once committed; nothing exported\n")
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
                self.assertEqual(err, f"error: {self.out_dir.resolve() / MAIN_LABELS} is not a prefix of the "
                                      f"new export; an exported label file only grows; nothing exported\n")
                self.assertEqual({p.name: p.read_bytes() for p in self.out_dir.iterdir()}, before)

    def test_re_export_that_drops_exported_relabels_is_refused_and_growth_is_allowed(self):
        lines = read(self.set_dir / "labels.jsonl").splitlines(keepends=True)
        (self.set_dir / "relabels.jsonl").write_text("".join(lines[:2]))
        self.assertEqual(self.export()[1], "exported 10 cases, 10 labels, 2 relabels\n")
        (self.set_dir / "relabels.jsonl").write_text("".join(lines[:3]))  # grows: fine
        self.assertEqual(self.export()[1], "exported 10 cases, 10 labels, 3 relabels\n")
        self.assertEqual(len(read(self.out_dir / MAIN_RELABELS).splitlines()), 3)
        (self.set_dir / "relabels.jsonl").unlink()  # the exported file exists, the source has none
        rc, out, err = self.export()
        self.assertEqual(rc, 1)
        self.assertIn(f"{MAIN_RELABELS} is not a prefix of the new export; an exported label file only grows", err)
        self.assertEqual(len(read(self.out_dir / MAIN_RELABELS).splitlines()), 3)

    def test_export_refuses_an_unrendered_draw(self):
        unrendered = self.draw("fresh", 12, 2, 1)
        with mock.patch.object(run.archive, "REPO_ROOT", self.fake_repo):
            rc, out, err = cli("export", "--set", unrendered, "--out-dir", self.out_dir)
        self.assertEqual(rc, 1)
        self.assertEqual(err, f"error: {unrendered} is not rendered (draw.json holds no packet hashes); "
                              f"run render first; nothing exported\n")
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
                self.assertIn(f"{MAIN_LABELS}: {message} is", err)
                self.assertIn("; nothing exported", err)
                self.assertNotIn(secret, out + err)
                self.assertNotIn(sid_for(0), out + err)
                self.assertFalse(self.out_dir.exists())  # nothing written
        # the same rules apply to relabels
        (self.set_dir / "labels.jsonl").write_text(original)
        (self.set_dir / "relabels.jsonl").write_text(json.dumps(dict(first, labels=[secret])) + "\n")
        rc, out, err = self.export()
        self.assertEqual(rc, 1)
        self.assertIn(f"{MAIN_RELABELS}: case {cid}: field labels is invalid", err)
        self.assertNotIn(secret, out + err)
        self.assertFalse(self.out_dir.exists())
        # positive control: without the bad record the same files export, whitelisted fields present
        (self.set_dir / "relabels.jsonl").unlink()
        (self.set_dir / "labels.jsonl").write_text(json.dumps(dict(first, seconds=0)) + "\n" + rest)  # 0 is a valid duration
        self.assertEqual(self.export()[0], 0)
        rows = [json.loads(l) for l in read(self.out_dir / MAIN_LABELS).splitlines()]
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
            if pathlib.Path(dst).name == MAIN_LABELS:
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
        self.assertEqual(err, f"error: {MAIN_LABELS}: record 0 is not an object; nothing exported\n")
        self.assertFalse(self.out_dir.exists())

    def test_export_validates_the_draw_manifest_values(self):
        key_path, draw_path, labels_path = (self.set_dir / n for n in ("key.json", "draw.json", "labels.jsonl"))
        saved = {p: read(p) for p in (key_path, draw_path, labels_path)}
        cid, good_sha = EXPORT_CASES[0][0], EXPORT_CASES[0][1]
        secret = json.loads(saved[key_path])[cid]["case_key"]  # holds a session id
        # positive control first: the untouched set exports, and seed / sha / stratum / kind ARE in the manifest
        rc, out, err = self.export(self.fake_repo / "control")
        self.assertEqual((rc, err), (0, ""))
        manifest = json.loads(read(self.fake_repo / "control" / MAIN_DRAW))
        self.assertEqual(manifest["seed"], 11)
        self.assertEqual(manifest["cases"][0], {"case_id": cid, "sha256": good_sha, "stratum": "substantive",
                                                "kind": "top"})
        bad_sha = {"case_key": secret, "uppercase": good_sha.upper(), "63 chars": good_sha[:63],
                   "65 chars": good_sha + "0", "not hex": "g" * 64, "an int": 7, "a 64-digit int": int("1" * 64), "a list": [good_sha]}
        cases = [("seed", "draw.json: field seed is invalid; nothing exported", "draw.json",
                  lambda d, k, v: d.update(seed=v), [secret, True, 11.0, None, [11], 2 ** 32, -1, 10 ** 400,
                                                     uuid.UUID(sid_for(3)).int])]
        cases += [("sha256", f"draw.json: case {cid}: field sha256 is invalid; nothing exported", "draw.json",
                   lambda d, k, v: d["cases"][0].update(sha256=v), list(bad_sha.values()))]
        cases += [("stratum", f"{self.set_dir}: key.json: case {cid}: field stratum is invalid; nothing exported",
                   "key.json",
                   lambda d, k, v: k[cid].update(stratum=v), [secret, "other", "", None, "top", "handback"])]
        cases += [("kind", f"{self.set_dir}: key.json: case {cid}: field kind is invalid; nothing exported",
                   "key.json",
                   lambda d, k, v: k[cid].update(kind=v), [secret, "other", "", None, "substantive", "routine"])]
        cases += [("set_id", SETID_MESSAGE, "draw.json",
                   lambda d, k, v: d.update(set_id=v), [secret, "../escape", ".hidden", "", ["main"], None, sid_for(3),
                                                    "2026-09-29-x-" + sid_for(3), "main", SETP + "other",
                                                    MAIN.upper()])]
        for field, message, _, mutate, values in cases:
            for value in values:
                with self.subTest(field=field, value=type(value).__name__ + str(value)[:12]):
                    draw, key = json.loads(saved[draw_path]), json.loads(saved[key_path])
                    mutate(draw, key, value)
                    draw_path.write_text(json.dumps(draw))
                    key_path.write_text(json.dumps(key))
                    # a label edited to carry the SAME bad hash, so only the manifest check can refuse it
                    recs = [json.loads(l) for l in saved[labels_path].splitlines()]
                    recs = [dict(r, packet_sha256=value) if field == "sha256" and r["case_id"] == cid else r
                            for r in recs]
                    labels_path.write_text("".join(json.dumps(r) + "\n" for r in recs))
                    rc, out, err = self.export()
                    self.assertEqual((rc, err), (1, f"error: {message}\n"))
                    self.assertNotIn(secret, out + err)
                    self.assertFalse(self.out_dir.exists())
        for p, text in saved.items():
            p.write_text(text)
        self.assertEqual(self.export()[0], 0)  # restored: exports again

    def test_export_seconds_are_bounded_on_both_sides(self):
        first, rest = self.recs[0], "".join(read(self.set_dir / "labels.jsonl").splitlines(keepends=True)[1:])
        for value, accepted in ((86400 * 7 - 1, True), (86400 * 7 - 0.5, True), (0, True),
                                (86400 * 7, False), (86400 * 7 + 0.5, False), (10 ** 400, False),
                                (int(sid_for(1).replace("-", ""), 16), False)):  # a UUID encoded as an int
            with self.subTest(value=str(value)[:12]):
                out_dir = self.fake_repo / f"s{len(str(value))}-{str(value)[:6]}"
                (self.set_dir / "labels.jsonl").write_text(json.dumps(dict(first, seconds=value)) + "\n" + rest)
                rc, out, err = self.export(out_dir)
                if accepted:
                    self.assertEqual((rc, err), (0, ""))
                    self.assertEqual(json.loads(read(out_dir / MAIN_LABELS).splitlines()[0])["seconds"], value)
                else:
                    self.assertEqual((rc, err), (1, f"error: {MAIN_LABELS}: case {first['case_id']}: field "
                                                    f"seconds is invalid; nothing exported\n"))
                    self.assertFalse(out_dir.exists())

    def test_export_refuses_duplicate_and_orphan_records(self):
        labels_path, relabels_path = self.set_dir / "labels.jsonl", self.set_dir / "relabels.jsonl"
        lines = read(labels_path).splitlines(keepends=True)
        cid0, cid_late = self.recs[0]["case_id"], self.recs[6]["case_id"]
        # positive control: a relabel of a labelled case exports
        relabels_path.write_text(lines[0])
        rc, out, err = self.export(self.fake_repo / "ok")
        self.assertEqual((rc, out, err), (0, "exported 10 cases, 10 labels, 1 relabels\n", ""))
        attempts = {
            "duplicate label": ("".join(lines) + lines[0], "", f"duplicate label for case_id '{cid0}'"),
            "orphan relabel": ("".join(lines[:4]), lines[6], f"orphan relabel for case_id '{cid_late}'"),
            "duplicate relabel": ("".join(lines), lines[0] + lines[0], f"duplicate relabel for case_id '{cid0}'"),
        }
        for name, (labels_text, relabels_text, message) in attempts.items():
            with self.subTest(name):
                labels_path.write_text(labels_text)
                relabels_path.write_text(relabels_text)
                rc, out, err = self.export()
                self.assertEqual(rc, 1)
                self.assertIn(f"error: {message}", err)
                self.assertIn("nothing exported", err)
                self.assertFalse(self.out_dir.exists())


    def test_export_relabels_are_whitelisted_too(self):
        with open(self.set_dir / "relabels.jsonl", "w") as f:
            for r in self.recs[:3]:
                f.write(json.dumps(dict(r, note=f"{NOTE}-relabel")) + "\n")
        rc, out, err = self.export()
        self.assertEqual((rc, err), (0, ""))
        self.assertEqual(out, "exported 10 cases, 10 labels, 3 relabels\n")
        self.assertTrue((self.out_dir / MAIN_RELABELS).is_file())
        text = read(self.out_dir / MAIN_RELABELS)
        rows = [json.loads(l) for l in text.splitlines()]
        self.assertEqual(len(rows), 3)
        for row, rec in zip(rows, self.recs):
            self.assertEqual(row, {k: rec[k] for k in ("case_id", "packet_sha256", "labels", "delivery",
                                                       "recall", "seconds")})
        self.assertNotIn(NOTE, text)


def corpus_text(root):
    """Every byte of every file under a corpus, for the positive controls of the absence assertions."""
    return "".join(p.read_text(encoding="utf-8") + str(p) for p in sorted(pathlib.Path(root).rglob("*")) if p.is_file())


class TokenBase(Base):
    """A corpus in which exactly one unit (session 2's last message, m2c) holds a token-shaped string."""

    def token_corpus(self):
        return fx.build_corpus(self.root / "token-corpus", make_sessions(token_session=2))

    def token_unit(self, corpus):
        (unit,) = [u for u in sampler.frame(corpus, excluded_sids=set()) if u.message_id == "m2c"]
        return unit



class Preflight(TokenBase):
    """`preflight` finds every packet refusal BEFORE registration: after it nothing may be redrawn."""



    def test_counts_one_token_refusal_prints_nothing_but_counts_and_writes_the_excluded_file_0600(self):
        corpus = self.token_corpus()
        unit = self.token_unit(corpus)
        out_file = self.root / "private" / "excluded.json"
        rc, out, err = cli("preflight", "--corpus", corpus, "--excluded-out", out_file)
        self.assertEqual((rc, err), (0, ""))
        self.assertEqual(json.loads(out), {
            "units": 33, "built": 32, "refused": 1, "by_type": {"TokenFound": 1},
            "by_stratum": {"substantive": 1}, "by_kind": {"top": 1}})
        # the file: exactly that unit's case_key, private
        self.assertEqual(json.loads(read(out_file)), [unit.case_key])
        self.assertEqual(mode(out_file), 0o600)
        # absence, with positive controls: every sentinel IS in the corpus (or the private file)
        text = corpus_text(corpus)
        for sentinel in (TOKEN, sid_for(2), "m2c", "Fixed and verified."):
            self.assertIn(sentinel, text, sentinel)
            self.assertNotIn(sentinel, out + err, sentinel)
        self.assertIn(unit.case_key, read(out_file))
        self.assertNotIn(unit.case_key, out + err)
        self.assertNotIn(unit.transcript, out + err)

    def test_a_clean_corpus_reports_no_refusal_and_writes_an_empty_list_on_request(self):
        out_file = self.root / "excluded.json"
        rc, out, err = cli("preflight", "--corpus", self.corpus, "--excluded-out", out_file)
        self.assertEqual((rc, err), (0, ""))
        self.assertEqual(json.loads(out), {"units": 33, "built": 33, "refused": 0, "by_type": {},
                                           "by_stratum": {}, "by_kind": {}})
        self.assertEqual(json.loads(read(out_file)), [])
        rc, out, err = cli("preflight", "--corpus", self.corpus)  # no --excluded-out: counts only, no file
        self.assertEqual((rc, err), (0, ""))
        self.assertEqual(json.loads(out)["refused"], 0)

    def test_every_exception_type_is_caught_per_unit_and_only_its_type_is_printed(self):
        real = packet.build_packet
        secret = "SECRET-MESSAGE-" + sid_for(4)

        def flaky(corpus, unit, case_id, cache=None):
            if unit.message_id in ("m4e", "m4c"):
                raise ValueError(secret)
            if unit.message_id == "m5e":
                raise KeyError(secret)
            return real(corpus, unit, case_id, cache=cache)

        out_file = self.root / "excluded.json"
        with mock.patch.object(run.packet, "build_packet", flaky):
            rc, out, err = cli("preflight", "--corpus", self.corpus, "--excluded-out", out_file)
        self.assertEqual((rc, err), (0, ""))
        self.assertEqual(json.loads(out), {
            "units": 33, "built": 30, "refused": 3, "by_type": {"KeyError": 1, "ValueError": 2},
            "by_stratum": {"substantive": 3}, "by_kind": {"top": 3}})
        self.assertNotIn(secret, out + err)
        self.assertNotIn(sid_for(4), out + err)
        # the file is sorted, whatever order the units were built in (m4e is built BEFORE m4c: it comes first in
        # its transcript), so the same corpus always registers the same bytes
        suffixes = [k.rsplit("|", 1)[1] for k in json.loads(read(out_file))]
        self.assertEqual(suffixes, ["m4c", "m4e", "m5e"])

    def test_units_are_built_grouped_by_transcript(self):
        real = packet.build_packet
        seen = []

        def spy(corpus, unit, case_id, cache=None):
            seen.append(unit.transcript)
            return real(corpus, unit, case_id, cache=cache)

        with mock.patch.object(run.packet, "build_packet", spy):
            rc, out, err = cli("preflight", "--corpus", self.corpus)
        self.assertEqual(rc, 0)
        self.assertEqual(len(seen), 33)  # positive control: every unit was built
        runs = [t for i, t in enumerate(seen) if i == 0 or t != seen[i - 1]]  # consecutive duplicates collapsed
        self.assertEqual(len(runs), len(set(runs)))  # so a transcript that reappears would show up twice
        self.assertEqual(len(set(seen)), 9)  # 6 sessions + 3 subagent files: the grouping had something to group

    def test_the_grouping_is_done_by_preflight_not_inherited_from_the_frame_order(self):
        # sampler.frame happens to yield a transcript's units together; feed preflight an INTERLEAVED frame
        # (by entry index first) so only its own sort can regroup them.
        units = sampler.frame(self.corpus, excluded_sids=set())
        interleaved = sorted(units, key=lambda u: (u.first_entry_index, u.transcript))
        firsts = [u.transcript for u in interleaved]
        self.assertGreater(sum(1 for i in range(1, len(firsts)) if firsts[i] != firsts[i - 1]), 9)  # control: it IS interleaved
        real = packet.build_packet
        seen = []

        def spy(corpus, unit, case_id, cache=None):
            seen.append(unit.transcript)
            return real(corpus, unit, case_id, cache=cache)

        with mock.patch.object(run, "_frame_units", lambda corpus: interleaved), \
                mock.patch.object(run.packet, "build_packet", spy):
            rc, out, err = cli("preflight", "--corpus", self.corpus)
        self.assertEqual(rc, 0)
        runs = [t for i, t in enumerate(seen) if i == 0 or t != seen[i - 1]]
        self.assertEqual(len(runs), len(set(runs)))
        self.assertEqual(len(seen), 33)


    def test_the_excluded_out_path_is_private_and_new(self):
        corpus = self.token_corpus()
        fake_repo = self.root / "fakerepo"
        inrepo = fake_repo / "excluded.json"
        with mock.patch.object(run.archive, "REPO_ROOT", fake_repo):
            rc, out, err = cli("preflight", "--corpus", corpus, "--excluded-out", inrepo)
        self.assertEqual((rc, out), (1, ""))
        self.assertIn("inside the repository", err)
        self.assertFalse(inrepo.exists())
        self.assertFalse(fake_repo.exists())
        existing = self.root / "already.json"
        existing.write_text("first evidence")
        rc, out, err = cli("preflight", "--corpus", corpus, "--excluded-out", existing)
        self.assertEqual((rc, out), (1, ""))
        self.assertIn("exists", err)
        self.assertEqual(existing.read_text(), "first evidence")
        # positive control: a fresh path outside the repo is accepted with the same corpus
        rc, out, err = cli("preflight", "--corpus", corpus, "--excluded-out", self.root / "fresh.json")
        self.assertEqual((rc, err), (0, ""))

    def test_an_unreadable_corpus_is_refused_by_type(self):
        empty = self.root / "empty-corpus"
        empty.mkdir()
        for corpus in (self.root / "no-such-corpus", empty):
            with self.subTest(corpus=corpus.name):
                rc, out, err = cli("preflight", "--corpus", corpus)
                self.assertEqual((rc, out), (1, ""))
                self.assertTrue("no units" in err or "could not be read" in err, err)


class ExcludeUnits(TokenBase):
    """`frame`/`draw` --exclude-units: the registered treatment of the units preflight found."""

    def excluded_file(self, corpus):
        path = self.root / "excluded.json"
        rc, out, err = cli("preflight", "--corpus", corpus, "--excluded-out", path)
        self.assertEqual((rc, err), (0, ""))
        return path

    def frame_with(self, corpus, excluded, name="frame.json"):
        path = self.root / name
        rc, out, err = cli("frame", "--corpus", corpus, "--out", path, "--exclude-units", excluded)
        self.assertEqual((rc, err), (0, ""), out)
        return path

    def test_frame_removes_the_units_before_counting_and_records_count_and_hash_only(self):
        corpus = self.token_corpus()
        unit = self.token_unit(corpus)
        excluded = self.excluded_file(corpus)
        frame = self.frame_with(corpus, excluded)
        rec = json.loads(read(frame))
        self.assertEqual(rec["counts"], {"substantive": {"top": 11, "handback": 3}, "routine": {"top": 18}})
        self.assertEqual(rec["n_units"], 32)
        self.assertEqual(rec["excluded_units"],
                         {"count": 1, "sha256": hashlib.sha256(excluded.read_bytes()).hexdigest()})
        for secret in (unit.case_key, sid_for(2), unit.transcript):
            self.assertIn(secret, read(excluded) + corpus_text(corpus), secret)  # positive control
            self.assertNotIn(secret, read(frame), secret)
        # the same corpus framed without the file counts the unit and records no exclusion
        plain = self.root / "plain.json"
        self.assertEqual(cli("frame", "--corpus", corpus, "--out", plain)[0], 0)
        self.assertEqual(json.loads(read(plain))["counts"]["substantive"]["top"], 12)
        self.assertNotIn("excluded_units", json.loads(read(plain)))

    def test_frame_refuses_a_malformed_stale_or_duplicated_file(self):
        corpus = self.token_corpus()
        unit = self.token_unit(corpus)
        shape = "--exclude-units: the file must be a JSON list of case_key strings"
        cases = {  # each case is refused by ITS OWN guard: the message names which
            "not json": ("this is not json", shape),
            "not a list": (json.dumps({"a": 1}), shape),
            "not strings": (json.dumps([1, 2]), shape),
            "nested": (json.dumps([["x"]]), shape),
            "duplicate": (json.dumps([unit.case_key, unit.case_key]),
                          "--exclude-units: the file lists a case_key more than once"),
            "key not in the frame": (json.dumps(["nobody|nowhere|m0"]),
                                     "--exclude-units holds 1 case_key(s) that are not in this frame; the file "
                                     "belongs to another corpus"),
        }
        for why, (text, message) in cases.items():
            with self.subTest(why=why):
                path = self.root / f"bad-{why.replace(' ', '-')}.json"
                path.write_text(text)
                out = self.root / f"f-{why.replace(' ', '-')}.json"
                rc, so, err = cli("frame", "--corpus", corpus, "--out", out, "--exclude-units", path)
                self.assertEqual((rc, so), (1, ""))
                self.assertEqual(err, f"error: {message}\n")
                self.assertFalse(out.exists())
                self.assertNotIn("nobody", err)  # a value of the file is never echoed
        rc, so, err = cli("frame", "--corpus", corpus, "--out", self.root / "f-missing.json",
                          "--exclude-units", self.root / "no-such-file.json")
        self.assertEqual((rc, so), (1, ""))
        self.assertIn("cannot be read", err)

    def test_draw_never_returns_an_excluded_unit_and_the_pool_shrinks_by_exactly_it(self):
        corpus = self.token_corpus()
        unit = self.token_unit(corpus)
        excluded = self.excluded_file(corpus)
        frame = self.frame_with(corpus, excluded)
        main = self.root / (SETP + "main")
        rc, out, err = cli("draw", "--corpus", corpus, "--set", main, "--seed", 7, "--substantive", 14,
                           "--routine", 2, "--frame", frame, "--exclude-units", excluded)
        self.assertEqual((rc, err), (0, ""), out)
        keys = {v["case_key"] for v in json.loads(read(main / "key.json")).values()}
        self.assertEqual(len(keys), 16)
        self.assertNotIn(unit.case_key, keys)
        # boundary on the pool: 15 - 1 = 14 substantive are drawable, 15 are not
        rc, out, err = cli("draw", "--corpus", corpus, "--set", self.root / (SETP + "all15"), "--seed", 7,
                           "--substantive", 15, "--routine", 0, "--frame", frame, "--exclude-units", excluded)
        self.assertEqual(rc, 1)
        self.assertIn("14 candidates", err)
        # positive control: without the exclusion the same draw of all 15 substantive DOES hold the unit
        rc, out, err = cli("draw", "--corpus", corpus, "--set", self.root / (SETP + "nofile"), "--seed", 7,
                           "--substantive", 15, "--routine", 0)
        self.assertEqual((rc, err), (0, ""))
        self.assertIn(unit.case_key, {v["case_key"] for v in json.loads(read(self.root / (SETP + "nofile") / "key.json")).values()})

    def test_draw_refuses_a_file_whose_hash_is_not_the_one_the_frame_registered(self):
        corpus = self.token_corpus()
        excluded = self.excluded_file(corpus)
        frame = self.frame_with(corpus, excluded)
        other = self.root / "other.json"
        other.write_text("[]\n")  # a valid list, but not the registered bytes
        target = self.root / (SETP + "main")
        rc, out, err = cli("draw", "--corpus", corpus, "--set", target, "--seed", 7, "--substantive", 2,
                           "--routine", 1, "--frame", frame, "--exclude-units", other)
        self.assertEqual((rc, out), (1, ""))
        self.assertIn("does not match the frame's registered exclusion", err)
        self.assertFalse(target.exists())
        # one byte of difference is enough: the registered file, appended to
        same_but = self.root / "same-but.json"
        same_but.write_bytes(excluded.read_bytes() + b" ")
        rc, out, err = cli("draw", "--corpus", corpus, "--set", target, "--seed", 7, "--substantive", 2,
                           "--routine", 1, "--frame", frame, "--exclude-units", same_but)
        self.assertEqual(rc, 1)
        self.assertFalse(target.exists())
        # positive control: the registered bytes are accepted
        rc, out, err = cli("draw", "--corpus", corpus, "--set", target, "--seed", 7, "--substantive", 2,
                           "--routine", 1, "--frame", frame, "--exclude-units", excluded)
        self.assertEqual((rc, err), (0, ""))

    def test_the_exclusion_flags_are_refused_when_the_frame_and_the_draw_disagree(self):
        corpus = self.token_corpus()
        excluded = self.excluded_file(corpus)
        recorded = self.frame_with(corpus, excluded)
        plain = self.root / "plain.json"
        self.assertEqual(cli("frame", "--corpus", corpus, "--out", plain)[0], 0)
        base = ("draw", "--corpus", corpus, "--seed", 7, "--substantive", 2, "--routine", 1)
        target = self.root / (SETP + "main")
        # the frame records no exclusion, but the draw brings one
        rc, out, err = cli(*base, "--set", target, "--frame", plain, "--exclude-units", excluded)
        self.assertEqual((rc, out), (1, ""))
        self.assertIn("records no exclusion", err)
        # the draw brings an exclusion without a frame to check it against: --frame is argparse-required
        err_buf = io.StringIO()
        with contextlib.redirect_stderr(err_buf), self.assertRaises(SystemExit) as cm:
            run.main([str(a) for a in (*base, "--set", target, "--exclude-units", excluded)])
        self.assertEqual(cm.exception.code, 2)
        self.assertIn("--frame", err_buf.getvalue())
        # the frame records an exclusion, but the draw does not apply it
        rc, out, err = cli(*base, "--set", target, "--frame", recorded)
        self.assertEqual((rc, out), (1, ""))
        self.assertIn("pass --exclude-units", err)
        self.assertFalse(target.exists())

    def test_pilot_and_main_share_the_exclusion_and_the_recovery_path_renders(self):
        corpus = self.token_corpus()
        # without the exclusion the token unit is in a full draw and render aborts (the problem)
        bad = self.root / (SETP + "bad")
        self.assertEqual(cli("draw", "--corpus", corpus, "--set", bad, "--seed", 7, "--substantive", 15,
                             "--routine", 0)[0], 0)
        rc, out, err = cli("render", "--corpus", corpus, "--set", bad)
        self.assertEqual(rc, 1)
        self.assertIn("token-shaped", err)
        # with it: pilot, then main excluding the pilot, both under the same registered file
        excluded = self.excluded_file(corpus)
        frame = self.frame_with(corpus, excluded)
        common = ("--corpus", corpus, "--seed", 7, "--frame", frame, "--exclude-units", excluded)
        pilot, main = self.root / (SETP + "pilot"), self.root / (SETP + "main")
        rc, out, err = cli("draw", *common, "--set", pilot, "--substantive", 4, "--routine", 1)
        self.assertEqual((rc, err), (0, ""))
        rc, out, err = cli("draw", *common, "--set", main, "--substantive", 10, "--routine", 3,
                           "--exclude-set", pilot)
        self.assertEqual((rc, err), (0, ""), out)
        keys = lambda d: {v["case_key"] for v in json.loads(read(d / "key.json")).values()}  # noqa: E731
        self.assertEqual(keys(pilot) & keys(main), set())
        self.assertNotIn(self.token_unit(corpus).case_key, keys(pilot) | keys(main))
        for d, n in ((pilot, 5), (main, 13)):
            rc, out, err = cli("render", "--corpus", corpus, "--set", d)
            self.assertEqual((rc, err), (0, ""))
            self.assertEqual(out, f"rendered {n} packets\n")


class EstimateBinding(Base):
    """`estimate` is bound to the registered seed and to the code the frame hashed."""

    def labelled(self):
        set_dir = self.rendered()
        write_labels(set_dir, sub_hits=4, rou_hits=1)
        return set_dir, self.frame_file()

    def run_estimate(self, set_dir, frame, seed, name="result.json"):
        out = self.root / name
        rc, so, se = cli("estimate", "--set", set_dir, "--frame", frame, "--seed", seed, "--out", out)
        return rc, so, se, out

    def test_the_seed_must_equal_the_drawn_seed_on_both_sides(self):
        set_dir, frame = self.labelled()  # drawn with seed 11
        for i, seed in enumerate((10, 12, 0, 99)):
            with self.subTest(seed=seed):
                rc, so, se, out = self.run_estimate(set_dir, frame, seed, f"bad{i}.json")
                self.assertEqual((rc, so), (1, ""))
                self.assertEqual(se, f"error: --seed {seed} is not the seed this set was drawn with (11); "
                                     f"estimate must use the registered seed\n")
                self.assertFalse(out.exists())
        rc, so, se, out = self.run_estimate(set_dir, frame, 11)
        self.assertEqual((rc, se), (0, ""))
        self.assertEqual(json.loads(read(out))["seed"], 11)

    def test_an_invalid_seed_in_draw_json_is_refused_without_echoing_it(self):
        set_dir, frame = self.labelled()
        draw = json.loads(read(set_dir / "draw.json"))
        leak = "work/" + sid_for(2)  # session-id-shaped: what a tampered draw.json could carry into a message
        (set_dir / "draw.json").write_text(json.dumps(dict(draw, seed=leak)))
        rc, so, se, out = self.run_estimate(set_dir, frame, 11)
        self.assertEqual((rc, so), (1, ""))
        self.assertEqual(se, "error: draw.json: field seed is invalid; refusing to estimate\n")
        self.assertNotIn(sid_for(2), se)
        self.assertFalse(out.exists())
        self.assertIn(leak, read(set_dir / "draw.json"))  # positive control: the value IS on disk


    def test_every_one_of_the_four_code_hashes_is_checked(self):
        set_dir, frame = self.labelled()
        good = json.loads(read(frame))
        self.assertEqual(sorted(good["code_sha256"]), ["estimate.py", "packet.py", "run.py", "sampler.py"])

        def drawn_against(path):
            """Make the set look as if it had been drawn against this (tampered) frame file: the frame binding
            is by bytes, so a frame frozen by other code is only reachable through a set bound to it."""
            draw = json.loads(read(set_dir / "draw.json"))
            draw["frame_sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
            (set_dir / "draw.json").write_text(json.dumps(draw))

        for i, name in enumerate(("sampler.py", "packet.py", "estimate.py", "run.py")):
            with self.subTest(tampered=name):
                bad = json.loads(json.dumps(good))
                bad["code_sha256"][name] = "0" * 64
                path = self.root / f"tampered{i}.json"
                path.write_text(json.dumps(bad))
                drawn_against(path)
                rc, so, se, out = self.run_estimate(set_dir, path, 11, f"t{i}.json")
                self.assertEqual((rc, so), (1, ""))
                self.assertEqual(se, f"error: the code changed since the frame was frozen ({name}); "
                                     f"re-run frame, or estimate with the code that was registered\n")
                self.assertFalse(out.exists())
        for why, code in (("missing key", {k: v for k, v in good["code_sha256"].items() if k != "run.py"}),
                          ("no hashes", None)):
            with self.subTest(why=why):
                bad = dict(good)
                if code is None:
                    del bad["code_sha256"]
                else:
                    bad["code_sha256"] = code
                path = self.root / f"missing{len(why)}.json"
                path.write_text(json.dumps(bad))
                drawn_against(path)
                rc, so, se, out = self.run_estimate(set_dir, path, 11, f"m{len(why)}.json")
                self.assertEqual((rc, so), (1, ""))
                self.assertIn("the code changed since the frame was frozen", se)
                self.assertFalse(out.exists())
        # positive control: the untouched frame file estimates
        drawn_against(frame)
        rc, so, se, out = self.run_estimate(set_dir, frame, 11, "ok.json")
        self.assertEqual((rc, se), (0, ""))

    def test_n_sessions_counts_distinct_sessions_per_stratum_and_carries_no_id(self):
        set_dir, frame = self.labelled()
        rc, so, se, out = self.run_estimate(set_dir, frame, 11)
        self.assertEqual((rc, se), (0, ""))
        res = json.loads(read(out))
        self.assertEqual(json.loads(so), res)  # stdout and RESULT.json carry the same figures
        allc = res["results"]["all_cases"]
        # 6 substantive units in 3 sessions (2, 4, 5); 4 routine units in 4 sessions (1, 2, 3, 5)
        self.assertEqual((allc["substantive"]["n"], allc["substantive"]["n_sessions"]), (6, 3))
        self.assertEqual((allc["routine"]["n"], allc["routine"]["n_sessions"]), (4, 4))
        self.assertNotIn("n_sessions", allc["overall"])
        wo = res["results"]["without_recall_flagged"]
        self.assertEqual((wo["substantive"]["n"], wo["substantive"]["n_sessions"]), (5, 3))  # h2b flagged; m2e keeps session 2
        # no session id, copy_id or case_key anywhere in what is published
        key = json.loads(read(set_dir / "key.json"))
        for secret in ([v["copy_id"] for v in key.values()] + [v["copy_id"].split("/", 1)[1] for v in key.values()]
                       + [v["case_key"] for v in key.values()]):
            self.assertIn(secret, read(set_dir / "key.json"), secret)  # positive control
            self.assertNotIn(secret, so + read(out), secret)

    def test_n_sessions_follows_the_labelled_units_not_the_drawn_ones(self):
        set_dir = self.rendered()
        recs = write_labels(set_dir, sub_hits=4, rou_hits=1, keep_routine=2)
        # flag BOTH session-2 substantive units (h2b, m2e) as recalled: that session drops from the second branch
        key = json.loads(read(set_dir / "key.json"))
        for r in recs:
            if key[r["case_id"]]["case_key"].split("|")[-1] in ("h2b", "m2e"):
                r["recall"] = "y"
        (set_dir / "labels.jsonl").write_text("".join(json.dumps(r) + "\n" for r in recs))
        rc, so, se, out = self.run_estimate(set_dir, self.frame_file(), 11)
        self.assertEqual((rc, se), (0, ""))
        res = json.loads(read(out))["results"]
        self.assertEqual(res["all_cases"]["substantive"]["n_sessions"], 3)
        self.assertEqual(res["without_recall_flagged"]["substantive"]["n"], 4)
        self.assertEqual(res["without_recall_flagged"]["substantive"]["n_sessions"], 2)
        # only the first 2 routine cases in draw order are labelled: sessions 5 and 3
        self.assertEqual((res["all_cases"]["routine"]["n"], res["all_cases"]["routine"]["n_sessions"]), (2, 2))

    def test_estimate_help_says_an_existing_out_is_never_overwritten(self):
        help_text = " ".join(run._parser()._subparsers._group_actions[0].choices["estimate"].format_help().split())
        self.assertIn("refuses to overwrite", help_text)
        self.assertIn("relabel", help_text)


class DrawSessions(Base):
    def test_draw_prints_the_distinct_session_count_of_each_stratum(self):
        set_dir = self.root / (SETP + "main")
        rc, out, err = cli("draw", "--corpus", self.corpus, "--set", set_dir, "--seed", 11,
                           "--substantive", 6, "--routine", 4)
        self.assertEqual((rc, err), (0, ""))
        self.assertEqual(out, "drawn substantive=6 routine=4 excluded=0 sessions_substantive=3 sessions_routine=4\n")
        # an all-strata draw: every unit in, so every session counts once however many of its units are drawn
        one = self.root / (SETP + "two")
        rc, out, err = cli("draw", "--corpus", self.corpus, "--set", one, "--seed", 11,
                           "--substantive", 15, "--routine", 18)
        self.assertEqual((rc, err), (0, ""))
        self.assertEqual(out, "drawn substantive=15 routine=18 excluded=0 sessions_substantive=6 sessions_routine=6\n")


class EndToEndThroughTheOperatorTool(Base):
    """A rendered set labelled THROUGH label.run_next (the operator loop), not hand-written records: a
    change to label.py's record schema must turn this red even when every hand-made fixture stays green."""

    def test_label_run_next_output_feeds_verify_summary_estimate_and_export(self):
        set_dir = self.rendered()
        draw, key = json.loads(read(set_dir / "draw.json")), json.loads(read(set_dir / "key.json"))
        order = draw["order"]
        subs = [c for c in order if key[c]["stratum"] == "substantive"]
        rous = [c for c in order if key[c]["stratum"] == "routine"]
        plan = {}  # case_id -> (labels letter, delivery letter, recall, note)
        for pos, cid in enumerate(subs):
            plan[cid] = ("v", "q", "y" if pos == 0 else "n", f"note {cid}") if pos < 4 else ("n", "s", "n", "")
        for pos, cid in enumerate(rous):
            plan[cid] = ("v", "q", "n", "") if pos < 1 else ("n", "s", "n", "")
        shown = []

        def show(text):
            i = len(shown)
            shown.append(text)
            cid = order[i]  # the i-th packet shown belongs to the i-th case of draw.json's order
            self.assertEqual(text, read(set_dir / "packets" / f"{cid}.md"))
            self.assertEqual(hashlib.sha256(text.encode("utf-8")).hexdigest(),
                             {c["case_id"]: c["sha256"] for c in draw["cases"]}[cid])

        def ask(prompt):
            lab, deliv, recall, note = plan[order[len(shown) - 1]]
            return {label.LABELS_PROMPT: lab, label.DELIVERY_PROMPT: deliv, label.RECALL_PROMPT: recall,
                    label.NOTE_PROMPT: note, label.KEEP_PROMPT: ""}[prompt]

        ticks = []
        for i in range(10):  # case i takes 30 + i seconds
            ticks += [100.0 * i, 100.0 * i + 30 + i]
        out = label.run_next(set_dir, False, ask, show, iter(ticks).__next__, confirm=True, say=lambda m: None)
        self.assertEqual(out, {"labelled": 10, "refused": [], "total": 10, "remaining": 0})
        self.assertEqual(len(shown), 10)
        # verify and summary read what the tool wrote
        self.assertEqual(label.verify(set_dir), {"ok": 10, "mismatch": []})
        self.assertEqual(label.summary(set_dir), {
            "cases": 10, "labelled": 10,
            "labels": {"verify": 5, "qualify": 0, "correct": 0, "none": 5, "unresolved": 0},
            "delivery": {"silent": 5, "quiet": 5, "interrupt": 0}, "unresolved": 0, "recall": 1,
            "median_seconds": 34.5})
        # estimate and export accept those records
        rc, so, se = cli("estimate", "--set", set_dir, "--frame", self.frame_file(), "--seed", 11,
                         "--out", self.root / "result.json")
        self.assertEqual((rc, se), (0, ""))
        res = json.loads(so)
        self.assertEqual(res["status"], "COMPLETE")
        self.assertEqual(res["labelled"], {"substantive": 6, "routine": 4})
        sub = res["results"]["all_cases"]["substantive"]
        self.assertEqual((sub["n"], sub["k"], sub["n_sessions"]), (6, 4, 3))
        self.assertEqual(res["results"]["all_cases"]["median_seconds"]["value"], 34.5)
        exp = self.root / "fakerepo" / "data"
        (self.root / "fakerepo").mkdir()
        with mock.patch.object(run.archive, "REPO_ROOT", self.root / "fakerepo"):
            rc, so, se = cli("export", "--set", set_dir, "--out-dir", exp)
        self.assertEqual((rc, so, se), (0, "exported 10 cases, 10 labels, 0 relabels\n", ""))
        rows = [json.loads(l) for l in read(exp / MAIN_LABELS).splitlines()]
        # in draw order: 4 routine/sub interleaved as drawn; hits are the first 4 substantive and the first routine
        self.assertEqual(rows, [dict(zip(LABEL_FIELDS, row)) for row in [
            ("3c9284abec", "622d06adb833f76ec9030a5b6dafb2aa5f12c75e63c1fc72269fd7e54353e83c", V, Q, "n", 30.0),
            ("812c9a7436", "d2d2ca3d9fbd046210803a3325dc6feb714d284ee0ed6d6e1dc9518ed8c6a919", N, S, "n", 31.0),
            ("d6ec25c96f", "5fbe09887577ec2923f988d7ddc4b0e8554ea2bdeaebe35a620d95ea25e23a91", N, S, "n", 32.0),
            ("77ba2592f7", "efc86666d0868416a0ce10ff27f2725ead80672cd8d31b560bdb4003b12187b2", V, Q, "y", 33.0),
            ("9cc8493506", "430e653648753f6597d3e50082e34c1c4055ac55abb291bfb61f8bb63f2a8575", V, Q, "n", 34.0),
            ("1a64a9afdb", "57e81a664497d1700642d9e73708b14710e452457e2e2314bfc78db5ce60703c", V, Q, "n", 35.0),
            ("d6a1394c31", "3123af9870be2a87a27acf6054d9361b5efe60ca1fbe8c9d3bca0d550ae11e8e", V, Q, "n", 36.0),
            ("0761ce7ec5", "b195a24d7805e7a761e47e5b7535ee7ff676a7249e5a61d2775ce323874cee94", N, S, "n", 37.0),
            ("b837168a2c", "fc52191b68cd63951e316ae6db11bd582935ee1648acc375e8ed65ad2d79911c", N, S, "n", 38.0),
            ("30fc2c09dd", "776273d57d4f3812958034dd2d51f570dcb934d20d2839e4302c152d80a6bdd2", N, S, "n", 39.0)]])


class PacketCache(Base):
    """build_packet(cache=...) parses each transcript once for a caller iterating units grouped by transcript,
    and builds byte-identical packets."""

    def units(self, corpus):
        return sorted(sampler.frame(corpus, excluded_sids=set()), key=lambda u: (u.transcript, u.first_entry_index))

    def spy_reads(self):
        calls = []
        real = packet.transcripts.read_jsonl

        def spy(path, *a, **k):
            calls.append(str(path))
            return real(path, *a, **k)

        return calls, mock.patch.object(packet.transcripts, "read_jsonl", spy)

    def test_packets_are_byte_identical_with_and_without_the_cache_for_every_unit(self):
        units = self.units(self.corpus)
        self.assertEqual(len(units), 33)
        self.assertIn("handback", {u.kind for u in units})  # control: the skip_sidechain=False state is exercised too
        cache = {}
        for u in units:
            plain = packet.build_packet(self.corpus, u, "c")
            cached = packet.build_packet(self.corpus, u, "c", cache=cache)
            self.assertEqual((cached.sha256, cached.text, cached.n_context), (plain.sha256, plain.text, plain.n_context),
                             u.message_id)

    def test_the_cache_parses_each_transcript_once_and_no_cache_parses_once_per_unit(self):
        units = self.units(self.corpus)
        calls, patch = self.spy_reads()
        with patch:
            cache = {}
            for u in units:
                packet.build_packet(self.corpus, u, "c", cache=cache)
        self.assertEqual(len(calls), 9)  # 6 sessions + 3 subagent files, 33 units
        self.assertEqual(len(set(calls)), 9)
        calls2, patch2 = self.spy_reads()
        with patch2:
            for u in units:
                packet.build_packet(self.corpus, u, "c")
        self.assertEqual(len(calls2), 33)  # positive control: without the cache, one parse per unit

    def test_the_cache_does_not_leak_between_transcripts_or_corpora(self):
        # the same relative transcript path in two corpora, different content, ONE shared cache
        a = fx.build_corpus(self.root / "corpus-a", [session(0)])
        b = fx.build_corpus(self.root / "corpus-b", [dict(session(1), sid=sid_for(0))])
        cache = {}
        seen = {}
        for name, corpus in (("a", a), ("b", b)):
            texts = []
            for u in self.units(corpus):
                cached = packet.build_packet(corpus, u, "c", cache=cache)
                self.assertEqual(cached.text, packet.build_packet(corpus, u, "c").text, (name, u.message_id))
                texts.append(cached.text)
            seen[name] = texts
        self.assertIn("please work on task 0", " ".join(seen["a"]))
        self.assertIn("please work on task 1", " ".join(seen["b"]))
        self.assertNotIn("please work on task 0", " ".join(seen["b"]))
        # two transcripts of ONE corpus keep their own state as well (interleaved calls)
        units = self.units(self.corpus)
        cache = {}
        for u in units[::-1] + units:
            self.assertEqual(packet.build_packet(self.corpus, u, "c", cache=cache).text,
                             packet.build_packet(self.corpus, u, "c").text)

    def test_preflight_counts_are_identical_with_and_without_the_cache_and_hold_one_transcript(self):
        corpus = fx.build_corpus(self.root / "token-corpus", make_sessions(token_session=2))
        real = packet.build_packet
        sizes = []

        def uncached(c, u, i, cache=None):
            return real(c, u, i)

        def measuring(c, u, i, cache=None):
            sizes.append(len(cache))
            return real(c, u, i, cache=cache)

        with mock.patch.object(run.packet, "build_packet", uncached):
            without = cli("preflight", "--corpus", corpus)
        with mock.patch.object(run.packet, "build_packet", measuring):
            with_cache = cli("preflight", "--corpus", corpus)
        self.assertEqual(with_cache, without)
        self.assertEqual(json.loads(with_cache[1])["refused"], 1)  # control: there is something to count
        self.assertEqual(len(sizes), 33)
        self.assertEqual(max(sizes), 1)  # never more than one transcript held (memory bound)
        calls, patch = self.spy_reads()
        pre = self.units(corpus)  # framed outside the spy: only preflight's own parses are counted
        with patch, mock.patch.object(run, "_frame_units", lambda c: pre):
            cli("preflight", "--corpus", corpus)
        self.assertEqual(len(calls), 9)


class FrameBinding(TokenBase):
    """draw records the sha256 of the frame file's BYTES; estimate/export hold the set to it."""

    def excluded_and_frames(self):
        corpus = self.token_corpus()
        excluded = self.root / "excluded.json"
        self.assertEqual(cli("preflight", "--corpus", corpus, "--excluded-out", excluded)[0], 0)
        with_excl, plain = self.root / "frame-excl.json", self.root / "frame-plain.json"
        self.assertEqual(cli("frame", "--corpus", corpus, "--out", with_excl, "--exclude-units", excluded)[0], 0)
        self.assertEqual(cli("frame", "--corpus", corpus, "--out", plain)[0], 0)
        return corpus, excluded, with_excl, plain

    def drawn_and_labelled(self, corpus, excluded, frame):
        set_dir = self.root / (SETP + "main")
        rc, out, err = cli("draw", "--corpus", corpus, "--set", set_dir, "--seed", 7, "--substantive", 4,
                           "--routine", 2, "--frame", frame, "--exclude-units", excluded)
        self.assertEqual((rc, err), (0, ""), out)
        self.assertEqual(cli("render", "--corpus", corpus, "--set", set_dir)[0], 0)
        write_labels(set_dir, sub_hits=2, rou_hits=1)
        return set_dir

    def estimate(self, set_dir, frame, name="r.json", seed=7):
        out = self.root / name
        rc, so, se = cli("estimate", "--set", set_dir, "--frame", frame, "--seed", seed, "--out", out)
        return rc, so, se, out

    def test_draw_without_frame_is_refused_by_argparse_and_creates_nothing(self):
        target = self.root / (SETP + "main")
        err = io.StringIO()
        with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()), \
                self.assertRaises(SystemExit) as cm:
            run.main(["draw", "--corpus", str(self.corpus), "--set", str(target), "--seed", "1",
                      "--substantive", "2", "--routine", "1"])
        self.assertEqual(cm.exception.code, 2)
        self.assertIn("--frame", err.getvalue())
        self.assertFalse(target.exists())

    def test_draw_refuses_a_missing_or_malformed_frame_file_by_field_and_creates_nothing(self):
        base = ("draw", "--corpus", self.corpus, "--seed", 1, "--substantive", 2, "--routine", 1)
        good = json.loads(read(self.frame_file()))
        cases = {
            "missing": (None, "error: --frame no-such-frame.json: cannot be read (No such file or directory)\n"),
            "not json": (b"not json", "error: --frame: the file is not a frame file (a JSON object)\n"),
            "not an object": (b"[1]", "error: --frame: the file is not a frame file (a JSON object)\n"),
            "no counts": (json.dumps({"corpus_id": "c"}).encode(), "error: --frame: field counts is invalid\n"),
            "no corpus id": (json.dumps(dict(good, corpus_id=5)).encode(), "error: --frame: field corpus_id is invalid\n"),
        }
        for why, (data, message) in cases.items():
            with self.subTest(why=why):
                path = self.root / ("no-such-frame.json" if data is None else f"bad-{why.replace(' ', '-')}.json")
                if data is not None:
                    path.write_bytes(data)
                target = self.root / (SETP + "t" + str(len(why)))
                rc, out, err = cli(*base, "--set", target, "--frame", path)
                self.assertEqual((rc, out, err), (1, "", message))
                self.assertFalse(target.exists())

    def test_draw_records_the_frame_bytes_hash_and_estimate_accepts_a_byte_identical_copy_elsewhere(self):
        corpus, excluded, frame, plain = self.excluded_and_frames()
        set_dir = self.drawn_and_labelled(corpus, excluded, frame)
        recorded = json.loads(read(set_dir / "draw.json"))["frame_sha256"]
        self.assertEqual(recorded, hashlib.sha256(frame.read_bytes()).hexdigest())  # (a literal would embed the code hashes)
        self.assertRegex(recorded, r"[0-9a-f]{64}")
        rc, so, se, out = self.estimate(set_dir, frame)
        self.assertEqual((rc, se), (0, ""))
        other = self.root / "elsewhere" / "renamed-frame.json"  # the binding is to bytes, not to the path
        other.parent.mkdir()
        other.write_bytes(frame.read_bytes())
        rc, so, se, out = self.estimate(set_dir, other, "r2.json")
        self.assertEqual((rc, se), (0, ""))

    def test_a_frame_refrozen_without_the_exclusion_is_refused_although_the_code_hashes_match(self):
        corpus, excluded, frame, plain = self.excluded_and_frames()
        set_dir = self.drawn_and_labelled(corpus, excluded, frame)
        a, b = json.loads(read(frame)), json.loads(read(plain))
        self.assertEqual(a["code_sha256"], b["code_sha256"])  # control: only the exclusion differs...
        self.assertNotEqual(a["counts"], b["counts"])  # ...and with it the counts (the weights)
        ha, hb = hashlib.sha256(frame.read_bytes()).hexdigest(), hashlib.sha256(plain.read_bytes()).hexdigest()
        rc, so, se, out = self.estimate(set_dir, plain)
        self.assertEqual((rc, so), (1, ""))
        self.assertEqual(se, f"error: --frame is not the frame this set was drawn against "
                             f"(sha256 {hb[:12]}, drawn against {ha[:12]})\n")
        self.assertFalse(out.exists())

    def test_export_refuses_a_tampered_missing_or_uppercase_frame_hash(self):
        corpus, excluded, frame, plain = self.excluded_and_frames()
        set_dir = self.drawn_and_labelled(corpus, excluded, frame)
        repo = self.root / "fakerepo"
        repo.mkdir()
        draw = json.loads(read(set_dir / "draw.json"))
        good = draw["frame_sha256"]
        bad_values = {"short": ("abc", f"{set_dir}: draw.json: field frame_sha256 is invalid; nothing exported\n"),
                      "uppercase": (good.upper(), f"{set_dir}: draw.json: field frame_sha256 is invalid; nothing exported\n"),
                      "not a string": (5, f"{set_dir}: draw.json: field frame_sha256 is invalid; nothing exported\n")}
        for why, (value, message) in bad_values.items():
            with self.subTest(why=why):
                (set_dir / "draw.json").write_text(json.dumps(dict(draw, frame_sha256=value)))
                with mock.patch.object(run.archive, "REPO_ROOT", repo):
                    rc, out, err = cli("export", "--set", set_dir, "--out-dir", repo / "data")
                self.assertEqual((rc, out, err), (1, "", f"error: {message}"))
                self.assertFalse((repo / "data").exists())
        predates = {k: v for k, v in draw.items() if k != "frame_sha256"}
        (set_dir / "draw.json").write_text(json.dumps(predates))
        with mock.patch.object(run.archive, "REPO_ROOT", repo):
            rc, out, err = cli("export", "--set", set_dir, "--out-dir", repo / "data")
        self.assertEqual((rc, out), (1, ""))
        self.assertIn("predates the frame binding; nothing exported", err)
        rc, so, se, _ = self.estimate(set_dir, frame, "predates.json")
        self.assertEqual((rc, so), (1, ""))
        self.assertIn("predates the frame binding", se)
        # positive control + the choice: the value is validated but NOT exported (the manifest keeps its whitelist)
        (set_dir / "draw.json").write_text(json.dumps(draw))
        with mock.patch.object(run.archive, "REPO_ROOT", repo):
            rc, out, err = cli("export", "--set", set_dir, "--out-dir", repo / "data")
        self.assertEqual((rc, err), (0, ""))
        manifest = json.loads(read(repo / "data" / f"{MAIN}-draw.json"))
        self.assertEqual(sorted(manifest), ["cases", "seed", "set_id"])

    def test_estimate_refuses_a_missing_or_malformed_frame_by_field_not_by_traceback(self):
        corpus, excluded, frame, plain = self.excluded_and_frames()
        set_dir = self.drawn_and_labelled(corpus, excluded, frame)
        rc, so, se, out = self.estimate(set_dir, self.root / "gone.json")
        self.assertEqual((rc, so, se), (1, "", "error: --frame gone.json: cannot be read (No such file or directory)\n"))
        good = json.loads(read(frame))
        draw = json.loads(read(set_dir / "draw.json"))
        cases = {"not json": b"{oops", "no counts": json.dumps(dict(good, counts=[1])).encode(),
                 "no corpus id": json.dumps({k: v for k, v in good.items() if k != "corpus_id"}).encode()}
        want = {"not json": "error: --frame: the file is not a frame file (a JSON object)\n",
                "no counts": "error: --frame: field counts is invalid\n",
                "no corpus id": "error: --frame: field corpus_id is invalid\n"}
        for why, data in cases.items():
            with self.subTest(why=why):
                path = self.root / f"m-{why.replace(' ', '-')}.json"
                path.write_bytes(data)  # bound to the set by hash, so only the content is at fault
                (set_dir / "draw.json").write_text(json.dumps(dict(draw, frame_sha256=hashlib.sha256(data).hexdigest())))
                rc, so, se, out = self.estimate(set_dir, path, f"m{len(why)}.json")
                self.assertEqual((rc, so, se), (1, "", want[why]))
                self.assertFalse(out.exists())

    def test_a_frame_hash_that_is_not_a_64_char_lowercase_hex_string_is_refused_by_render_estimate_and_export(self):
        corpus, excluded, frame, plain = self.excluded_and_frames()
        set_dir = self.drawn_and_labelled(corpus, excluded, frame)
        repo = self.root / "fakerepo"
        repo.mkdir()
        draw = json.loads(read(set_dir / "draw.json"))
        good = draw["frame_sha256"]
        bad_values = {
            "64 decimal digits as a JSON integer": int("1" * 64),  # str() of it would match the hex pattern
            "a list of 64 characters": ["a"] * 64,
            "true": True,
            "63 hex characters": good[:63],
            "65 hex characters": good + "a",
        }
        field = f"{set_dir}: draw.json: field frame_sha256 is invalid"
        for why, value in bad_values.items():
            with self.subTest(why=why):
                (set_dir / "draw.json").write_text(json.dumps(dict(draw, frame_sha256=value)))
                self.assertEqual(cli("render", "--corpus", corpus, "--set", set_dir), (1, "", f"error: {field}\n"))
                rc, so, se, out = self.estimate(set_dir, frame, "bad.json")
                self.assertEqual((rc, so, se), (1, "", f"error: {field}\n"))
                self.assertFalse(out.exists())
                with mock.patch.object(run.archive, "REPO_ROOT", repo):
                    rc, so, se = cli("export", "--set", set_dir, "--out-dir", repo / "data")
                self.assertEqual((rc, so, se), (1, "", f"error: {field}; nothing exported\n"))
                self.assertFalse((repo / "data").exists())
        (set_dir / "draw.json").write_text(json.dumps(draw))  # positive control: the real value estimates
        self.assertEqual(self.estimate(set_dir, frame, "ok.json")[0], 0)

    def test_a_failed_write_removes_the_private_temp_file(self):
        corpus = self.token_corpus()
        target = self.root / "excluded.json"
        with mock.patch.object(run.os, "fsync", side_effect=OSError(28, "No space left on device")):
            with self.assertRaises(OSError):
                cli("preflight", "--corpus", corpus, "--excluded-out", target)
        self.assertFalse(target.exists())
        self.assertEqual(sorted(p.name for p in self.root.iterdir() if p.name.startswith(".")), [])
        # positive control: the temp file WAS created by that run (it existed while fsync failed)
        seen = []

        def failing_fsync(fd):
            seen.append(sorted(p.name for p in self.root.iterdir() if p.name.startswith(".excluded")))
            raise OSError(5, "Input/output error")

        with mock.patch.object(run.os, "fsync", failing_fsync):
            with self.assertRaises(OSError):
                cli("preflight", "--corpus", corpus, "--excluded-out", target)
        self.assertEqual(len(seen[0]), 1)
        self.assertEqual(list(self.root.glob(".excluded*")), [])

    def test_draw_refuses_a_frame_that_does_not_describe_this_corpus_and_creates_nothing(self):
        small = fx.build_corpus(self.root / "small-corpus", [session(0)])
        small_frame = self.root / "small-frame.json"
        self.assertEqual(cli("frame", "--corpus", small, "--out", small_frame)[0], 0)
        target = self.root / (SETP + "main")
        rc, out, err = cli("draw", "--corpus", self.corpus, "--set", target, "--seed", 1, "--substantive", 2,
                           "--routine", 1, "--frame", small_frame)
        self.assertEqual((rc, out), (1, ""))
        self.assertEqual(err, "error: the frame file's counts (6 units) do not match this corpus (33 units after "
                              "the frame's exclusion); freeze the frame from this corpus\n")
        self.assertFalse(target.exists())
        # positive control: the frame of THIS corpus passes with the same arguments
        rc, out, err = cli("draw", "--corpus", self.corpus, "--set", target, "--seed", 1, "--substantive", 2,
                           "--routine", 1, "--frame", self.frame_file())
        self.assertEqual((rc, err), (0, ""))

    def test_draw_refuses_doctored_counts_although_the_registered_exclusion_hash_matches(self):
        corpus, excluded, frame, plain = self.excluded_and_frames()
        doctored = json.loads(read(frame))
        self.assertIn("excluded_units", doctored)  # the exclusion is registered and its hash is the true one
        doctored["counts"]["substantive"]["top"] += 1  # only the counts lie
        path = self.root / "doctored.json"
        path.write_text(json.dumps(doctored))
        target = self.root / (SETP + "main")
        rc, out, err = cli("draw", "--corpus", corpus, "--set", target, "--seed", 1, "--substantive", 2,
                           "--routine", 1, "--frame", path, "--exclude-units", excluded)
        self.assertEqual((rc, out), (1, ""))
        self.assertEqual(err, "error: the frame file's counts (33 units) do not match this corpus (32 units after "
                              "the frame's exclusion); freeze the frame from this corpus\n")
        self.assertFalse(target.exists())
        # the frame frozen WITHOUT the exclusion, with the exclusion applied by the draw: counts differ too
        # (33 vs 32) but the earlier 'records no exclusion' guard owns that input, so it is not this guard's case
        # positive control: the true frame with the same exclusion passes
        rc, out, err = cli("draw", "--corpus", corpus, "--set", target, "--seed", 1, "--substantive", 2,
                           "--routine", 1, "--frame", frame, "--exclude-units", excluded)
        self.assertEqual((rc, err), (0, ""))

    def test_auto_frames_are_not_shared_between_corpora_at_one_path(self):
        first = fx.build_corpus(self.root / "same-path", [session(0)])
        f1 = _auto_frame(str(first))
        shutil.rmtree(first)
        second = fx.build_corpus(self.root / "same-path", make_sessions())
        self.assertNotEqual(_auto_frame(str(second)), f1)


    def test_the_excluded_units_file_is_written_atomically_and_never_over_an_existing_one(self):
        corpus = self.token_corpus()
        target = self.root / "excluded.json"
        with mock.patch.object(run.os, "link", side_effect=RuntimeError("crash between temp write and rename")):
            with self.assertRaises(RuntimeError):
                cli("preflight", "--corpus", corpus, "--excluded-out", target)
        self.assertFalse(target.exists())  # no partial target...
        self.assertEqual(list(self.root.glob("*excluded*")), [])  # ...and no temp file left behind
        rc, out, err = cli("preflight", "--corpus", corpus, "--excluded-out", target)  # positive control
        self.assertEqual((rc, err), (0, ""))
        self.assertEqual(mode(target), 0o600)
        self.assertEqual(len(json.loads(read(target))), 1)
        # the link step itself refuses an existing target (a race the early exists() check cannot see)
        before = read(target)
        with self.assertRaises(FileExistsError):
            run._write_exclusive(target, "overwritten")
        self.assertEqual(read(target), before)
        self.assertEqual(sorted(p.name for p in self.root.glob("*excluded*")), ["excluded.json"])


if __name__ == "__main__":
    unittest.main()
