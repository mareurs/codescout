"""Tests for stage3: the pilot/extension plans and the per-task freeze pieces. No real docker."""
import contextlib
import io
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

import analyze
import run
import stage3

FOREIGN = [f"org__f-{i}" for i in range(6)]
PRIVATE = [f"cs-p{i:02d}" for i in range(6)]
SELECTED = [t for pair in zip(PRIVATE, FOREIGN) for t in pair]  # selection.json interleaves
SHA = "06c8167b707913b08bc8403e04a4cbb9f80c83c0f63a9d76eeef3eda15f3c3c7"


def make_root(tmp, kinds=None):
    root = Path(tmp) / "tasks"
    for t in SELECTED:
        (root / t).mkdir(parents=True)
        kind = (kinds or {}).get(t, "private" if t.startswith("cs-") else "foreign")
        (root / t / "bundle.json").write_text(json.dumps({"kind": kind}))
    return root


class FakeDocker:
    def __init__(self, stdout):
        self.stdout, self.calls = stdout, []

    def __call__(self, *args, **kw):
        self.calls.append([str(a) for a in args])
        return SimpleNamespace(returncode=0, stdout=self.stdout)


class Stage3Test(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="sab-stage3-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.root = make_root(self.tmp)

    def pieces(self, **kw):
        seeds = dict(split_seed=1, pilot_seed=2, extension_seed=3)
        seeds.update(kw)
        return stage3.registration_pieces(SELECTED, self.root, **seeds)

    def test_the_mapping_comes_from_the_bundles(self):
        m = stage3.registered_mapping(SELECTED, self.root)
        self.assertEqual(list(m), SELECTED)
        self.assertEqual({t for t, k in m.items() if k == "private"}, set(PRIVATE))
        bad = make_root(self.tmp / "b", {FOREIGN[0]: "public"})
        with self.assertRaisesRegex(ValueError, "public"):
            stage3.registered_mapping(SELECTED, bad)
        with self.assertRaisesRegex(ValueError, "twice"):
            stage3.registered_mapping(SELECTED + SELECTED[:1], self.root)

    def test_the_split_is_three_per_kind_seeded_and_metadata_only(self):
        m = stage3.registered_mapping(SELECTED, self.root)
        pilot, ext = stage3.pilot_split(m, 11)
        self.assertEqual((len(pilot), len(ext)), (6, 6))
        self.assertEqual(set(pilot) | set(ext), set(SELECTED))
        self.assertFalse(set(pilot) & set(ext))
        self.assertEqual(sum(m[t] == "private" for t in pilot), 3)
        self.assertEqual(stage3.pilot_split(m, 11), (pilot, ext))           # deterministic
        shuffled = dict(reversed(list(m.items())))
        self.assertEqual(set(stage3.pilot_split(shuffled, 11)[0]), set(pilot))  # order-free set
        self.assertEqual(pilot, [t for t in SELECTED if t in pilot])        # selection order
        self.assertGreater(len({tuple(stage3.pilot_split(m, s)[0]) for s in range(30)}), 5)
        # The draw is exactly the documented one.
        import random
        rng = random.Random(11)
        want = set(rng.sample(sorted(FOREIGN), 3)) | set(rng.sample(sorted(PRIVATE), 3))
        self.assertEqual(set(pilot), want)

    def test_two_plans_of_48_runs_with_their_own_seed_hash_and_budget(self):
        p = self.pieces()
        self.assertEqual(p["registered_min"], 10)
        self.assertEqual(p["stratum_min"], {"foreign": 5, "private": 5})
        self.assertEqual(p["rules"]["tolerance"], "a headline needs both strata to decide, so "
                         "the real tolerance is 1 invalid task per stratum")
        self.assertEqual(set(p["rules"]), {"stratum_min", "tolerance", "arm_integrity",
                                           "admission"})
        self.assertEqual(p["registered_tasks"], stage3.registered_mapping(SELECTED, self.root))
        for name, seed in (("pilot", 2), ("extension", 3)):
            part = p["plans"][name]
            order = [run.RunSpec(*s) for s in part["order"]]
            self.assertEqual(len(order), 48)
            self.assertEqual(part["seed"], seed)
            self.assertEqual(part["plan_sha256"], run.plan_sha256(order))
            self.assertEqual(run.plan_reps(order), "stage3")
            self.assertEqual(order, run.make_order(part["tasks"], run.MODELS, seed))
            self.assertEqual(part["budget"], {"cap_usd": 200.0, "spent_usd": 0.0,
                                              "per_call_usd": 5.0, "reserve_usd": 5.0})
            self.assertEqual(part["tasks"], p["split"][name])
            # N-2: the runner accepts exactly the registered part's budget.
            run.check_registered_budget(p, order, stage3.stage3_budget())
            with self.assertRaises(run.Refused):
                run.check_registered_budget(p, order, stage3.stage3_budget()._replace(
                    per_call_usd=2.0))
        self.assertNotEqual(p["plans"]["pilot"]["plan_sha256"],
                            p["plans"]["extension"]["plan_sha256"])
        # A registration carrying these pieces registers both plans for the runner.
        self.assertEqual(run.registered_plan_hashes(p),
                         {p["plans"][n]["plan_sha256"] for n in ("pilot", "extension")})

    def test_refusals(self):
        with self.assertRaisesRegex(ValueError, "differ"):
            self.pieces(pilot_seed=1)
        with self.assertRaisesRegex(ValueError, "12"):
            stage3.registration_pieces(SELECTED[:11], self.root, split_seed=1, pilot_seed=2,
                                       extension_seed=3)
        lopsided = make_root(self.tmp / "l", {PRIVATE[0]: "foreign"})
        with self.assertRaisesRegex(ValueError, "6 per kind"):
            stage3.registration_pieces(SELECTED, lopsided, split_seed=1, pilot_seed=2,
                                       extension_seed=3)

    def test_pilot_only_data_is_inconclusive(self):
        p = self.pieces()
        rows = []
        for t in p["split"]["pilot"]:
            for model in run.MODELS:
                for arm, rep, cost in (("bare", 0, 1.0), ("bare", 1, 1.02), ("cs", 0, 0.5),
                                       ("full", 0, 0.6)):
                    rows.append(run.make_row((t, model, arm, rep), "s", outcome="scored",
                                             costUsd=cost, grade={"pass": True},
                                             effectiveEffort="medium", capHit=False,
                                             integrity={"codescout_calls": 0, "native_calls": 0},
                                             bashCommands=[], editedFiles=[]))
        for model in run.MODELS:
            for comp in (("cs", "bare"), ("full", "bare")):
                out = analyze.strata(rows, model, p["registered_tasks"],
                                     registered_min=p["registered_min"], seed=5,
                                     instrument_failures=(), comparison=comp)
                for kind in ("foreign", "private", "pooled"):
                    self.assertEqual(out[kind].outcome, "inconclusive", (model, comp, kind))

    def test_the_cli_writes_plans_the_dry_run_accepts(self):
        sel = self.tmp / "selection.json"
        sel.write_text(json.dumps({"selected": SELECTED}))
        out = self.tmp / "plans"
        with contextlib.redirect_stdout(io.StringIO()):
            stage3.main(["plans", "--selection", str(sel), "--tasks-root", str(self.root),
                         "--split-seed", "1", "--pilot-seed", "2", "--extension-seed", "3",
                         "--out", str(out)])
            run.main(["dry-run", "--plan", str(out / "pilot-plan.json"), "--stage3",
                      "--data", str(self.tmp / "dry")])
        dry = json.loads((self.tmp / "dry" / "dry-run.json").read_text())
        pieces = json.loads((out / "stage3-pieces.json").read_text())
        self.assertEqual(dry["plan_sha256"], pieces["plans"]["pilot"]["plan_sha256"])
        self.assertEqual((dry["count"], dry["reps"]), (48, "stage3"))
        self.assertEqual(dry["worst_case_usd"], 240.0)


class FreezePiecesTest(unittest.TestCase):
    def test_arm_binary_sha256_reads_the_first_line_read_only(self):
        fake = FakeDocker(f"{SHA}  /out/bin/codescout\nabc  /out/bin/rust-analyzer\n")
        with contextlib.redirect_stdout(io.StringIO()) as printed:
            self.assertEqual(stage3.arm_binary_sha256("sab-arm-x:t", fake), SHA)
        self.assertIn(SHA, printed.getvalue())
        argv = fake.calls[0]
        self.assertEqual(argv[:2], ["run", "--rm"])
        self.assertTrue(argv[argv.index("--name") + 1].startswith("sab-binhash-"))
        self.assertEqual(argv[argv.index("--network") + 1], "none")
        self.assertEqual(argv[-2:], ["sab-arm-x:t", "/opt/sab/sha256.txt"])
        for bad in ("", "nothex  /out/bin/codescout\n", f"{SHA}  /out/bin/rust-analyzer\n",
                    f"{SHA}  /out/bin/rust-analyzer\n{SHA}  /out/bin/codescout\n"):
            with self.subTest(bad), self.assertRaises(RuntimeError):
                stage3.arm_binary_sha256("sab-arm-x:t", FakeDocker(bad))

    def test_template_volume_hash_mounts_read_only_without_network(self):
        listing = "aa  ./collection/a\nbb  ./collection/b\n"
        fake = FakeDocker(listing)
        got = stage3.template_volume_sha256("sab-tpl-x", fake)
        self.assertEqual(got["files"], 2)
        self.assertEqual(len(got["sha256"]), 64)
        argv = fake.calls[0]
        self.assertIn("sab-tpl-x:/tpl:ro", argv)
        self.assertEqual(argv[argv.index("--network") + 1], "none")
        self.assertTrue(argv[argv.index("--name") + 1].startswith("sab-tplhash-"))
        self.assertNotEqual(stage3.template_volume_sha256("sab-tpl-x", FakeDocker(
            listing.replace("bb", "cc")))["sha256"], got["sha256"])
        with self.assertRaises(ValueError):
            stage3.template_volume_sha256("codescout-qdrant", fake)
        with self.assertRaises(RuntimeError):
            stage3.template_volume_sha256("sab-tpl-x", FakeDocker(""))

    def test_template_dump_hash_is_the_scanned_dump(self):
        tmp = Path(tempfile.mkdtemp(prefix="sab-dump-"))
        self.addCleanup(shutil.rmtree, tmp, ignore_errors=True)
        (tmp / "t" / "controls").mkdir(parents=True)
        (tmp / "t" / "controls" / "qdrant-dump.jsonl").write_bytes(b"{}\n")
        import hashlib
        self.assertEqual(stage3.template_dump_sha256("t", tmp),
                         hashlib.sha256(b"{}\n").hexdigest())


if __name__ == "__main__":
    unittest.main()
