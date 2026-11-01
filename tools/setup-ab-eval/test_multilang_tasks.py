import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import multilang_tasks as mt
import tasks

BASE = "a92166897f4b3d5f3910101d9380796fc2ce15bf"
PARSER = "def parser(log):\n    return dict(l.split('=') for l in log.split())\n"
RUN1 = "fn first_fn(a_arg: u32, b_arg: u32) -> u32 { compute_alpha(a_arg, b_arg) + offset_value }"
RUN2 = "fn second_fn(c_arg: u64, d_arg: u64) -> u64 { compute_omega(c_arg, d_arg) * scale_factor }"
PATCH = (f"diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,3 +1,5 @@\n"
         f"+{RUN1}\n context\n+{RUN2}\n")
TEST_PATCH = ("diff --git a/tests/t.rs b/tests/t.rs\n--- a/tests/t.rs\n+++ b/tests/t.rs\n@@ -1 +1,2 @@\n"
              "+fn test_new_behaviour() { assert_eq!(widget_total(), expected_widget_total_value); }\n")


def record(**kw):
    r = {"instance_id": "o__r-1", "docker_image": "img:1", "base_commit": BASE,
         "patch": PATCH, "test_patch": TEST_PATCH, "problem_statement": "Fix it.\r\nPlease.\r\n",
         "FAIL_TO_PASS": ["t::f1"], "PASS_TO_PASS": ["t::p1", "t::p2", "t::p3"],
         "test_cmds": ["run"], "print_cmds": ["cat log"], "log_parser": PARSER}
    r.update(kw)
    return r


def write_control(d, base, gold, *, workdir="/testbed", mismatch=None, error=False, rid="o__r-1"):
    """control.json + base.log + gold.log as multilang_control.run_one writes them."""
    d = Path(d)
    d.mkdir(parents=True, exist_ok=True)
    (d / "base.log").write_text("".join(f"{k}={v}\n" for k, v in base.items()))
    (d / "gold.log").write_text("".join(f"{k}={v}\n" for k, v in gold.items()))
    ev = {"id": rid, "workdir": workdir, "verdict": {"ok": False, "error": error},
          "gold_repeat_mismatch": mismatch or {"count": 0, "names": []}}
    (d / "control.json").write_text(json.dumps(ev))
    return d / "control.json"


GOOD_BASE = {"t::f1": "fail", "t::p1": "pass", "t::p2": "fail", "t::p3": "pass"}
GOOD_GOLD = {"t::f1": "pass", "t::p1": "pass", "t::p2": "fail", "t::p3": "pass"}


class LiveDockerfileTests(unittest.TestCase):
    def test_live_dockerfile_resets_to_base_and_keeps_target(self):
        text = mt.live_dockerfile("src@sha256:" + "a" * 64, record(), "/testbed")
        self.assertTrue(text.startswith("FROM src@sha256:" + "a" * 64 + "\nUSER root\n"))
        self.assertIn(f"git reset --hard -q {BASE}", text)
        self.assertIn("git clean -ffdx -e target -e Cargo.lock -e .venv -e generated-do-not-edit", text)
        for line in text.splitlines():
            if "rm -rf" in line:
                self.assertNotIn("rm -rf target ", line + " ", "target/ is kept, never removed whole")
                self.assertNotIn("/testbed/target", line)
        self.assertIn("printf '/target\\n' >> .git/info/exclude", text)

    def test_live_dockerfile_removes_reports_in_both_locations(self):
        text = mt.live_dockerfile("src", record(), "/testbed/repo")
        self.assertIn("rm -rf /testbed/repo/reports /testbed/reports", text)
        self.assertIn("rm -rf /testbed/repo/.git /testbed/.git", text)
        same = mt.live_dockerfile("src", record(), "/testbed")
        self.assertIn("rm -rf /testbed/reports \\", same)  # one path, not listed twice

    def test_live_dockerfile_creates_agent_user_and_clean_one_commit_repo(self):
        text = mt.live_dockerfile("src", record(), "/testbed")
        self.assertIn("useradd --uid 1000 --gid 1000 --create-home", text)
        self.assertIn("groupadd --gid 1000 agent", text)
        self.assertIn("chown -R 1000:1000 /testbed /home/agent", text)
        self.assertIn("git commit -qm 'Benchmark base source'", text)
        self.assertIn("ENV HOME=/home/agent CARGO_NET_OFFLINE=true\nUSER 1000:1000\nWORKDIR /testbed\n", text)
        last = text.rstrip("\n").splitlines()[-1]
        self.assertTrue(last.startswith("RUN test -z \"$(git status --porcelain)\""))
        self.assertIn('git rev-list --count HEAD)" = 1', last)
        self.assertIn("Benchmark base source", last)
        self.assertLess(text.index("USER root"), text.index("USER 1000:1000"))

    def test_live_dockerfile_refuses_a_shell_unsafe_commit_or_workdir(self):
        with self.assertRaises(mt.LiveRefused):
            mt.live_dockerfile("src", record(base_commit="x; rm -rf /"), "/testbed")
        with self.assertRaises(mt.LiveRefused):
            mt.live_dockerfile("src", record(), "/testbed; echo")
        with self.assertRaises(mt.LiveRefused):
            mt.live_dockerfile("src", record(), "/tmp/x")


class AssembleLiveTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.tmp, ignore_errors=True))
        self.builds = []
        self.base_files = {"src/a.rs": f"// base\n{RUN1}\n", "README.md": "readme\n"}

    def run_assemble(self, rec=None):
        def export(image, paths, dest, **kw):
            for rel, text in self.base_files.items():
                p = Path(dest) / "testbed" / rel
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text(text)
            return ["testbed"]

        def build(ctx, text, tag, log, **kw):
            self.builds.append((text, tag, kw))
            Path(log).write_text("")
            return "sha256:1"

        with mock.patch.object(tasks, "_source_ref", return_value="src@sha256:" + "a" * 64), \
                mock.patch.object(tasks, "_docker_build", side_effect=build), \
                mock.patch.object(tasks, "export_tree", side_effect=export):
            return mt.assemble_live(rec or record(), self.tmp / "b", "sab-task-o__r-1:t1", "/testbed")

    def test_assemble_live_writes_prompt_with_prefix_and_lf(self):
        needles, prompt, rec = self.run_assemble()
        raw = (self.tmp / "b" / "prompt.txt").read_bytes()
        self.assertEqual(raw, (tasks.PROMPT_PREFIX + "Fix it.\nPlease.\n").encode())
        self.assertNotIn(b"\r", raw)
        self.assertEqual(prompt, self.tmp / "b" / "prompt.txt")
        self.assertEqual((self.tmp / "b" / "gold.patch").read_text(), PATCH)
        self.assertEqual(json.loads((self.tmp / "b" / "record.json").read_text())["instance_id"], "o__r-1")
        assy = json.loads((self.tmp / "b" / "assembly.json").read_text())
        self.assertEqual((assy["kind"], assy["workdir"], assy["base_commit"]), ("live", "/testbed", BASE))
        text, tag, kw = self.builds[0]
        self.assertEqual(tag, "sab-task-o__r-1:t1")
        self.assertIn(f"git reset --hard -q {BASE}", text)
        self.assertIn("FROM src@sha256:", text)

    def test_assemble_live_needles_exclude_strings_already_in_the_base_tree(self):
        needles, _, rec = self.run_assemble()
        self.assertGreater(needles.dropped["shingle"], 0)
        shingles = [n for n in needles.needles if " " in n]
        self.assertTrue(any("second_fn" in s for s in shingles))
        self.assertFalse(any("first_fn" in s or "compute_alpha" in s for s in shingles),
                         "run 1 is already in the base tree, so it is no leak evidence")
        self.assertIn("test_new_behaviour", needles.needles)  # added by test_patch, not in base
        self.assertEqual(rec["needles"]["dropped"], needles.dropped)

    def test_assemble_live_reads_a_nextest_test_id_with_a_dollar_separator(self):
        rec = record(FAIL_TO_PASS=["crate-x::bin_name$test_new_behaviour"])
        needles, _, _ = self.run_assemble(rec)
        self.assertIn("test_new_behaviour", needles.needles)
        self.assertFalse(any("$" in n for n in needles.needles))


    def test_needle_name_reads_the_live_test_id_shapes(self):
        cases = {
            "obscura-cdp::bin$test_one": "test_one",
            "Doc-tests rmcp::crates/rmcp/src/t.rs - transport::Cfg::custom_headers (line 1725) - compile":
                "custom_headers",
            "Doc-tests rmcp::crates/rmcp/src/m.rs - model::ElicitRequestParams (line 3535)":
                "ElicitRequestParams",
            "Doc-tests rmcp::crates/rmcp/src/t.rs - transport::Tr<C>::with_client (line 1596) - compile":
                "with_client",
            "unittests src/lib.rs::transport::tests::clear_pending": "clear_pending",
            "tests/a/main.rs (target/debug/deps/a-1d)::virtual_branches::init::boots_ok": "boots_ok",
            "target/debug/deps/hir_ty-49::mir::lower::tests::no_panic": "no_panic",
            "polars_core::frame::tests::test_x": "test_x",
        }
        for raw, want in cases.items():
            with self.subTest(raw):
                self.assertEqual(mt.needle_name(raw), want)
        needles, _, _ = self.run_assemble(record(FAIL_TO_PASS=[
            "Doc-tests rmcp::crates/rmcp/src/t.rs - transport::Cfg::custom_headers (line 1725) - compile"]))
        self.assertIn("custom_headers", needles.needles)


    def test_a_name_needle_is_kept_unless_the_base_tree_really_holds_the_name(self):
        """rust-analyzer's id shape gave name needle 0 because `test-output.log`, a gold-run log the
        image kept as ignored debris, held the name: the baseline took it for base content. The name
        is derived correctly; with the log scrubbed from the tree the needle stays."""
        tid = "target/debug/deps/hir_ty-491963d1a8621083::mir::lower::tests::borrowck_opaque_downcast_ok"
        rec = record(FAIL_TO_PASS=[tid])
        self.assertIn("borrowck_opaque_downcast_ok", mt.leak_scan.needles_for(mt.leak_record(rec)))
        needles, _, _ = self.run_assemble(rec)
        self.assertIn("borrowck_opaque_downcast_ok", needles.needles)
        self.base_files["test-output.log"] = "test mir::lower::tests::borrowck_opaque_downcast_ok ... ok\n"
        (self.tmp / "b").rename(self.tmp / "b1")
        needles, _, _ = self.run_assemble(rec)
        self.assertNotIn("borrowck_opaque_downcast_ok", needles.needles,
                         "an excusing file in the baseline tree drops the name")


    def test_assemble_live_refuses_to_merge_into_an_existing_dir(self):
        (self.tmp / "b").mkdir()
        with self.assertRaises(FileExistsError):
            self.run_assemble()
        self.assertEqual(self.builds, [])


class BuildLiveTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.tmp, ignore_errors=True))
        self.agent_calls = []

    def build(self, control_json, rec=None, out=None):
        def assemble(record_, out_, task_image, workdir):
            Path(out_).mkdir(parents=True)
            (Path(out_) / "record.json").write_text(json.dumps(record_))
            self.assembled = (task_image, workdir)
            return SimpleNamespace(kept={}), Path(out_) / "prompt.txt", {
                "id": record_["instance_id"], "needles": {"kept": {}, "dropped": {}}}

        self.tooled = []

        def tooled(image, needles):
            self.tooled.append(image)
            return (SimpleNamespace(kept={"shingle": 7}),
                    {"manifest_sha256": "m" * 64, "files": {"./bin/node": "n" * 64},
                     "dropped": {"shingle": 2}})

        with mock.patch.object(tasks, "short_tag", return_value="t1"), \
                mock.patch.object(mt, "assemble_live", side_effect=assemble), \
                mock.patch.object(mt, "apply_tool_layer", side_effect=tooled), \
                mock.patch.object(tasks, "build_agent_image",
                                  side_effect=lambda *a: self.agent_calls.append(a)):
            return mt.build_live(rec or record(), out or self.tmp / "task" / "build", control_json)

    def test_build_live_bundle_records_workdir_expected_and_control_sha256(self):
        cj = write_control(self.tmp / "c", GOOD_BASE, GOOD_GOLD)
        b = self.build(cj)
        on_disk = json.loads((self.tmp / "task" / "bundle.json").read_text())
        self.assertEqual(on_disk, b.record)
        out = str(self.tmp / "task" / "build")
        self.assertEqual(on_disk, {
            "kind": "live", "out": out, "task_image": "sab-task-o__r-1:t1",
            "image": "sab-agent-cs-o__r-1:t1", "record_path": out + "/record.json",
            "workdir": "/testbed",
            "expected": {"fail_to_pass": 1, "pass_to_pass_kept": 2, "pass_to_pass_dropped": 1,
                         "dropped": ["t::p2"]},
            "control_sha256": hashlib.sha256(cj.read_bytes()).hexdigest()})
        self.assertNotIn("template", on_disk)
        self.assertEqual(b.template, "")
        self.assertEqual(self.assembled, ("sab-task-o__r-1:t1", "/testbed"))
        self.assertEqual(self.agent_calls[0][:2], ("sab-task-o__r-1:t1", "sab-agent-cs-o__r-1:t1"))

    def test_build_live_baselines_against_the_agent_image_and_records_the_tool_layer(self):
        cj = write_control(self.tmp / "c", GOOD_BASE, GOOD_GOLD)
        b = self.build(cj)
        self.assertEqual(self.tooled, ["sab-agent-cs-o__r-1:t1"])
        self.assertEqual(b.needles.kept, {"shingle": 7})
        assy = json.loads((self.tmp / "task" / "build" / "assembly.json").read_text())
        self.assertEqual(assy["needles"]["kept"], {"shingle": 7})
        self.assertEqual(assy["needles"]["tool_layer_dropped"], {"shingle": 2})
        self.assertEqual(assy["tool_layer"]["manifest_sha256"], "m" * 64)
        self.assertEqual(assy["tool_layer"]["files"], {"./bin/node": "n" * 64})


    def test_build_live_refuses_a_control_that_is_not_relaxed_ok(self):
        cases = {
            "f2p passes at base": (dict(GOOD_BASE, **{"t::f1": "pass"}), GOOD_GOLD, {}),
            "f2p fails at gold": (GOOD_BASE, dict(GOOD_GOLD, **{"t::f1": "fail"}), {}),
            "f2p missing at gold": (GOOD_BASE, {k: v for k, v in GOOD_GOLD.items() if k != "t::f1"}, {}),
            "kept test differs between gold runs": (GOOD_BASE, GOOD_GOLD,
                                                    {"mismatch": {"count": 1, "names": ["t::p1"]}}),
            "mismatch list truncated": (GOOD_BASE, GOOD_GOLD,
                                        {"mismatch": {"count": 11, "names": ["t::p2"] * 10}}),
            "control error": (GOOD_BASE, GOOD_GOLD, {"error": True}),
            "other task's control": (GOOD_BASE, GOOD_GOLD, {"rid": "o__other-2"}),
            "workdir the runner cannot use": (GOOD_BASE, GOOD_GOLD, {"workdir": "/testbed/repo"}),
        }
        for name, (base, gold, kw) in cases.items():
            with self.subTest(name):
                cj = write_control(self.tmp / name.replace(" ", "_"), base, gold, **kw)
                with self.assertRaises(mt.LiveRefused):
                    self.build(cj)
                self.assertEqual(self.agent_calls, [])
                self.assertFalse((self.tmp / "task").exists(), "nothing is written for a refusal")

    def test_a_dropped_test_that_differs_between_gold_runs_is_not_a_refusal(self):
        cj = write_control(self.tmp / "c", GOOD_BASE, GOOD_GOLD,
                           mismatch={"count": 1, "names": ["t::p2"]})
        self.assertEqual(self.build(cj).record["expected"]["dropped"], ["t::p2"])

    def test_the_relaxed_set_keeps_only_tests_that_pass_at_gold(self):
        gold = dict(GOOD_GOLD, **{"t::p3": "skipped"})  # absent or not "pass" is dropped
        gold.pop("t::p1")
        cj = write_control(self.tmp / "c", GOOD_BASE, gold)
        e = mt.relaxed_expected(record(), cj)
        self.assertEqual(e["dropped"], ["t::p1", "t::p2", "t::p3"])
        self.assertEqual((e["pass_to_pass_kept"], e["fail_to_pass"]), (0, 1))


class ArmLiveTests(unittest.TestCase):
    def test_arm_live_tags_run_image_builds_arm_and_drops_template(self):
        tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(tmp, ignore_errors=True))
        bj = tmp / "bundle.json"
        bj.write_text(json.dumps({"kind": "live", "out": str(tmp / "build"), "template": "",
                                  "task_image": "sab-task-o__r-1:t1", "image": "sab-agent-cs-o__r-1:t1"}))
        calls = []

        def docker(*args, **kw):
            calls.append(args)
            if args[:2] == ("image", "inspect") and "--format" in args and "{{.Id}}" in args:
                return SimpleNamespace(stdout="sha256:run\n", returncode=0)
            return SimpleNamespace(stdout="", returncode=1)

        built = []
        with mock.patch.object(mt.pilot, "docker", docker), \
                mock.patch.object(mt.arms, "exec_only_image", side_effect=lambda *a: built.append(a)):
            rec = mt.arm_live(bj)
        self.assertIn(("tag", "sab-agent-cs-o__r-1:t1", "sab-run-o__r-1:t1"), calls)
        self.assertEqual(built[0][:2], ("sab-run-o__r-1:t1", "sab-arm-o__r-1:t1"))
        self.assertEqual((rec["run_image"], rec["arm_image"], rec["image"]),
                         ("sab-run-o__r-1:t1", "sab-arm-o__r-1:t1", "sab-arm-o__r-1:t1"))
        self.assertNotIn("template", json.loads(bj.read_text()))
class WorkspaceArtifactTests(unittest.TestCase):
    """The raw image's target/ was built with the gold fix applied; its workspace artifacts are
    cleaned and rebuilt from the base tree."""

    def text(self, **kw):
        return mt.live_dockerfile("src", record(rebuild_cmds=["cargo build --workspace"], **kw), "/testbed")

    def test_workspace_artifacts_are_cleaned_then_rebuilt_after_the_reset_and_before_the_chown(self):
        t = self.text()
        order = [t.index(x) for x in ("git reset --hard", "cargo clean --offline $ARGS",
                                      "cargo clean --offline --release $ARGS", "rm -rf target/*/incremental",
                                      ".fingerprint", "( cargo build --workspace )", "git init",
                                      "chown -R")]
        self.assertEqual(order, sorted(order))
        self.assertIn("cargo metadata --no-deps --offline", t)
        self.assertIn("export CARGO_NET_OFFLINE=true", t)

    def test_the_final_tree_must_equal_the_base_commit_tree(self):
        t = self.text()
        self.assertIn("TREE=$(git rev-parse HEAD^{tree})", t)
        self.assertIn('test "$(git rev-parse HEAD^{tree})" = "$TREE"', t)
        self.assertLess(t.index("TREE=$("), t.index("git init"))
        self.assertGreater(t.index('= "$TREE"'), t.index("git commit"))

    def test_a_record_without_rebuild_commands_still_cleans(self):
        t = mt.live_dockerfile("src", record(), "/testbed")
        self.assertIn("cargo clean --offline $ARGS", t)
        self.assertNotIn("( cargo", t)

    def test_a_multiline_rebuild_command_stays_on_the_run_line(self):
        t = mt.live_dockerfile("src", record(rebuild_cmds=["cd x\ncargo build"]), "/testbed")
        self.assertIn("( cd x ; cargo build )", t)

    def test_orphan_sweep_removes_workspace_artifacts_of_a_vanished_target_and_keeps_dependencies(self):
        import shutil
        import subprocess
        tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, tmp, True)
        wd = tmp / "testbed"
        deps = wd / "target" / "debug" / "deps"
        fp = wd / "target" / "debug" / ".fingerprint"
        deps.mkdir(parents=True)

        def make(stem, source):
            # as cargo writes it: the output path is absolute for every artifact
            (deps / f"{stem}.d").write_text(f"{deps}/{stem}.d: {source}\n{source}:\n")
            (deps / stem).write_text("GOLD-ONLY" if "crates/" in source else "dep")
            (deps / f"{stem}.o").write_text("obj")
            (fp / f"pkg-{stem.rsplit('-', 1)[1]}").mkdir(parents=True)

        make("gold_test-aaaa1111aaaa1111", "crates/c/tests/gold_test.rs crates/c/src/lib.rs")
        make("abs_ws-cccc3333cccc3333", f"{wd}/crates/c/src/lib.rs")
        make("libserde-bbbb2222bbbb2222", "/usr/local/cargo/registry/src/serde/lib.rs")
        subprocess.run(["bash", "-c", mt.orphan_sweep_sh(str(wd))], cwd=wd, check=True)
        left = sorted(p.name for p in deps.iterdir())
        self.assertEqual(left, ["libserde-bbbb2222bbbb2222", "libserde-bbbb2222bbbb2222.d",
                                "libserde-bbbb2222bbbb2222.o"])
        self.assertEqual([p.name for p in fp.iterdir()], ["pkg-bbbb2222bbbb2222"])


class GoldLiteralsTests(unittest.TestCase):
    def test_only_long_added_lines_missing_from_the_base_tree_are_literals(self):
        tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(tmp, ignore_errors=True))
        (tmp / "a.rs").write_text(f"x\n{RUN1}\n")
        got = mt.gold_literals(record(), tmp)
        self.assertIn(RUN2, got)
        self.assertNotIn(RUN1, got, "already in the base tree")
        self.assertTrue(all(len(x) >= mt.SWEEP_MIN_LITERAL for x in got))
        self.assertTrue(any("test_new_behaviour" in x for x in got), "test_patch lines count too")


class SweepTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.tmp, ignore_errors=True))

    def put(self, rel, data):
        p = self.tmp / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(data if isinstance(data, bytes) else data.encode())
        return p

    def sweep(self, needles=(), literals=(), excludes=(), **kw):
        import live_sweep
        return live_sweep.sweep(self.tmp, needles, literals, excludes, **kw)

    def test_the_sweep_reads_target_and_finds_a_literal_in_a_large_binary(self):
        lit = "// a gold-only comment line that is longer than thirty chars"
        self.put("testbed/target/debug/build/x/build-script-build", b"\0" * 5000 + lit.encode() + b"\0" * 99)
        r = self.sweep(literals=[lit], token_max=100)
        self.assertEqual([(h["path"], h["needle"]) for h in r["hits"]],
                         [("/testbed/target/debug/build/x/build-script-build", lit)])
        self.assertEqual((r["literal_only"], r["token_scanned"]), (1, 0))

    def test_a_literal_across_a_window_edge_is_found(self):
        lit = "0123456789abcdefghij" * 3
        self.put("testbed/target/x.o", b"z" * 37 + lit.encode() + b"z" * 50)
        for window in (16, 40, 64, 97):
            with self.subTest(window=window):
                self.assertEqual(len(self.sweep(literals=[lit], token_max=1, window=window)["hits"]), 1)

    def test_the_sweep_finds_a_token_needle_in_a_small_target_file(self):
        self.put("testbed/target/debug/.fingerprint/p/dep-info", "alpha beta gamma delta epsilon zeta eta theta iota")
        r = self.sweep(needles=["alpha beta gamma delta epsilon zeta eta theta"])
        self.assertEqual(r["hits"][0]["path"], "/testbed/target/debug/.fingerprint/p/dep-info")
        self.assertEqual(r["token_scanned"], 1)

    def test_a_path_needle_matches_the_name_of_a_large_file(self):
        self.put("testbed/target/debug/deps/gold_test_name-abc", b"x" * 500)
        r = self.sweep(needles=["gold_test_name"], token_max=10)
        self.assertEqual([h["needle"] for h in r["hits"]], ["gold_test_name"])

    def test_the_excludes_are_skipped_and_recorded_with_whether_they_exist(self):
        lit = "a literal that is only inside the excluded toolchain dir"
        self.put("usr/local/cargo/registry/x.rs", lit)
        self.put("testbed/ok.txt", "clean")
        r = self.sweep(literals=[lit], excludes=["/usr/local/cargo", "/usr/local/rustup"])
        self.assertEqual(r["hits"], [])
        self.assertEqual(r["excluded"], {"/usr/local/cargo": True, "/usr/local/rustup": False})
        self.assertEqual(r["files"], 1)

    def test_patch_diff_and_eval_files_are_listed_and_system_ones_do_not_fail(self):
        for rel in ("testbed/fix.patch", "tmp/eval_script.sh", "home/agent/gold.txt", "usr/share/doc/x/a.diff",
                    "testbed/src/main.rs"):
            self.put(rel, "x")
        r = self.sweep()
        self.assertEqual(sorted(r["suspicious"]), ["/home/agent/gold.txt", "/testbed/fix.patch",
                                                  "/tmp/eval_script.sh", "/usr/share/doc/x/a.diff"])
        reasons = mt.sweep_reasons(r)
        self.assertEqual(len(reasons), 1)
        self.assertNotIn("/usr/share", reasons[0])
        self.assertEqual(mt.sweep_reasons({"hits": [], "files": 3, "suspicious": ["/usr/share/doc/x/a.diff"]}), [])

    @unittest.skipIf(os.geteuid() == 0, "root reads everything")
    def test_unreadable_files_and_dirs_are_counted_not_fatal(self):
        self.put("testbed/secret/a.txt", "x")
        self.put("testbed/b.txt", "y")
        os.chmod(self.tmp / "testbed" / "secret", 0)
        self.addCleanup(os.chmod, self.tmp / "testbed" / "secret", 0o755)
        r = self.sweep()
        self.assertGreaterEqual(r["unreadable_count"], 1)
        self.assertEqual(r["files"], 1)

    def test_the_gate_never_excludes_the_workdir_or_target(self):
        for e in mt.SWEEP_EXCLUDES:
            self.assertFalse(e == "/" or "/testbed".startswith(e + "/") or e.startswith("/testbed")
                             or e.startswith("/home") or e.startswith("/tmp"), e)


class PrescanLiveTests(unittest.TestCase):
    """prescan_live must run the sweep over the real filesystem, `target/` included, and fail on a hit."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.tmp, ignore_errors=True))
        out = self.tmp / "t" / "build"
        (out / "baseline-tree" / "testbed").mkdir(parents=True)
        (out / "baseline-tree" / "testbed" / "a.rs").write_text(f"{RUN1}\n")
        (out / "record.json").write_text(json.dumps(record()))
        self.bj = self.tmp / "t" / "bundle.json"
        self.bj.write_text(json.dumps({"kind": "live", "out": str(out), "record_path": str(out / "record.json"),
                                       "task_image": "sab-task-o__r-1:t1", "image": "sab-agent-cs-o__r-1:t1",
                                       "workdir": "/testbed",
                                       "expected": {"fail_to_pass": 1, "pass_to_pass_kept": 3,
                                                    "pass_to_pass_dropped": 0, "dropped": []}}))
        self.data = self.tmp / "t" / "prescan"
        self.data.mkdir()
        self.docker = []
        self.tool = self.tmp / "tool" / "opt" / "sab"
        (self.tool / "bin").mkdir(parents=True)
        (self.tool / "bin" / "node").write_text("unrelated words only")
        p = mock.patch.object(mt, "tool_layer_tree", return_value=(self.tool, {}, "d"))
        p.start()
        self.addCleanup(p.stop)

    def prescan(self, report):
        def fake_docker(*args, **kw):
            self.docker.append(args)
            return SimpleNamespace(stdout=json.dumps(report), returncode=0)

        clean = {"id": "o__r-1", "control": "pass", "hits": [], "reasons": []}
        with mock.patch.object(mt, "prescan_agent_image", return_value=clean), \
                mock.patch.object(mt.pilot, "docker", fake_docker):
            return mt.prescan_live(self.bj, {}, self.data)

    def good(self, **kw):
        r = {"files": 10, "hits": [], "suspicious": [], "unreadable_count": 0}
        r.update(kw)
        return r

    def test_live_prescan_runs_the_sweep_as_the_image_user_over_root_with_target_not_excluded(self):
        r = self.prescan(self.good())
        self.assertTrue(r["ok"])
        (args,) = self.docker
        self.assertEqual(args[:3], ("run", "--rm", "--network"))
        self.assertIn("none", args)
        self.assertNotIn("-u", args)  # the image user (1000), not root
        self.assertEqual(args[-3:], ("/sweep/live_sweep.py", "/sweep/input.json", "/"))
        spec = json.loads((self.data / "sweep-input.json").read_text())
        self.assertFalse([e for e in spec["excludes"] if "target" in e or e.startswith("/testbed")])
        self.assertIn(RUN2, spec["literals"])
        self.assertNotIn(RUN1, spec["literals"])  # already in the base tree
        self.assertTrue((self.data / "sweep.json").exists())
        self.assertEqual(json.loads((self.data / "prescan.json").read_text())["ok"], True)

    def test_live_prescan_fails_on_a_sweep_hit_in_target(self):
        hit = {"path": "/testbed/target/debug/build/x/build-script-build", "needle": RUN2, "offset": 5}
        r = self.prescan(self.good(hits=[hit]))
        self.assertFalse(r["ok"])
        self.assertIn("target/debug/build", " ".join(r["reasons"]))
        self.assertFalse(json.loads((self.data / "prescan.json").read_text())["ok"])

    def test_live_prescan_fails_on_a_stray_patch_file_and_on_an_empty_sweep(self):
        self.assertFalse(self.prescan(self.good(suspicious=["/tmp/fix.patch"]))["ok"])
        self.assertFalse(self.prescan(self.good(files=0))["ok"])

    def test_live_prescan_fails_when_the_base_prescan_failed(self):
        clean = {"id": "o__r-1", "control": "FAILED", "error": "boom"}
        with mock.patch.object(mt, "prescan_agent_image", return_value=clean), \
                mock.patch.object(mt.pilot, "docker", lambda *a, **k: SimpleNamespace(
                    stdout=json.dumps(self.good()), returncode=0)):
            self.assertFalse(mt.prescan_live(self.bj, {}, self.data)["ok"])
class RepoRecreateTests(unittest.TestCase):
    """The shell steps that recreate the one-commit repo, run by bash on a real git repository."""

    # what the recipe did before fix round 2
    OLD = ["rm -rf .git", "git init -q", "git config user.email a@b.invalid", "git config user.name x",
           "printf '/target\\n' >> .git/info/exclude", "git add .", "git commit -qm 'Benchmark base source'",
           'test "$(git rev-parse HEAD^{tree})" = "$TREE"']

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.tmp, ignore_errors=True))
        self.repo = self.tmp / "repo"
        self.repo.mkdir()
        self.env = {**os.environ, "HOME": str(self.tmp), "GIT_CONFIG_GLOBAL": os.devnull,
                    "GIT_CONFIG_SYSTEM": os.devnull}
        old = mock.patch.object(mt, "TRACKED_LIST", str(self.tmp / "tracked.lst"))
        old.start()
        self.addCleanup(old.stop)

    def git(self, *args, cwd=None, check=True, binary=False):
        import subprocess
        return subprocess.run(["git", *args], cwd=cwd or self.repo, env=self.env, check=check,
                              capture_output=True, text=not binary)

    def make_repo(self, *, crlf=True, ignored=True):
        """A base commit that tracks an ignored file and a CRLF blob under a `text` attribute, then
        a second commit (the 'fix'), with the tree left at the second commit plus stray state."""
        self.git("init", "-q")
        self.git("config", "user.email", "a@b.invalid")
        self.git("config", "user.name", "x")
        (self.repo / ".gitignore").write_text("*.env\ntarget/\n")
        (self.repo / "src").mkdir()
        (self.repo / "src" / "lib.rs").write_text("fn a() {}\n")
        if ignored:
            (self.repo / "app.env").write_text("TOKEN=public\n")
        if crlf:
            (self.repo / "doc.txt").write_bytes(b"one\r\ntwo\r\n")
        self.git("add", "-f", ".")
        self.git("commit", "-qm", "base")
        if crlf:  # the attribute arrives after the CRLF blob was stored
            (self.repo / ".gitattributes").write_text("*.txt text\n")
            self.git("add", ".gitattributes")
            self.git("commit", "-qm", "attrs")
        self.base = self.git("rev-parse", "HEAD").stdout.strip()
        (self.repo / "src" / "lib.rs").write_text("fn a() { /* fix */ }\n")
        self.git("commit", "-qam", "gold fix")
        (self.repo / "stray.txt").write_text("untracked\n")
        (self.repo / "target").mkdir()
        (self.repo / "target" / "keep.o").write_text("object")
        return self.git("rev-parse", f"{self.base}^{{tree}}").stdout.strip()

    def run_steps(self, steps, cwd=None):
        import subprocess
        return subprocess.run(["bash", "-c", "set -eu; " + " && ".join(steps)], cwd=cwd or self.repo,
                              env=self.env, capture_output=True, text=True)

    def test_tracked_ignored_and_crlf_files_end_with_the_base_tree_hash(self):
        tree = self.make_repo()
        r = self.run_steps([*mt.reset_sh(self.base), *mt.recreate_repo_sh(".git")])
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.git("rev-parse", "HEAD^{tree}").stdout.strip(), tree)
        self.assertEqual(self.git("rev-list", "--count", "HEAD").stdout.strip(), "1")
        self.assertEqual(self.git("log", "-1", "--format=%s").stdout.strip(), "Benchmark base source")
        self.assertEqual(self.git("status", "--porcelain").stdout, "")
        tracked = self.git("ls-files").stdout.split()
        self.assertIn("app.env", tracked)
        self.assertFalse([p for p in tracked if p.startswith("target/") or p == "stray.txt"])
        self.assertEqual(self.git("cat-file", "blob", "HEAD:doc.txt", binary=True).stdout, b"one\r\ntwo\r\n")
        self.assertTrue((self.repo / "target" / "keep.o").exists())
        self.assertFalse((self.tmp / "tracked.lst").exists(), "the list is removed")

    def test_the_old_steps_lose_the_ignored_file_and_normalise_the_crlf_blob(self):
        for name, kw in (("ignored", dict(crlf=False)), ("crlf", dict(ignored=False))):
            with self.subTest(name):
                self.repo = self.tmp / name
                self.repo.mkdir()
                tree = self.make_repo(**kw)
                r = self.run_steps([*mt.reset_sh(self.base), *self.OLD])
                self.assertNotEqual(r.returncode, 0, "the equality check must fail on the old steps")
                self.assertNotEqual(self.git("rev-parse", "HEAD^{tree}").stdout.strip(), tree)

    def test_a_repo_with_nothing_special_still_ends_equal(self):
        tree = self.make_repo(crlf=False, ignored=False)
        r = self.run_steps([*mt.reset_sh(self.base), *mt.recreate_repo_sh(".git")])
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.git("rev-parse", "HEAD^{tree}").stdout.strip(), tree)

    def test_a_tree_that_differs_from_the_base_fails_the_equality_check(self):
        self.make_repo()
        steps = [*mt.reset_sh(self.base), "echo changed > src/new.rs", *mt.recreate_repo_sh(".git")]
        self.assertNotEqual(self.run_steps(steps).returncode, 0)


class ScrubAndDebrisTests(unittest.TestCase):
    """The shell that removes the third party's run debris, run on a real repository."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.tmp, ignore_errors=True))
        self.repo = self.tmp / "repo"
        self.repo.mkdir()
        self.env = {**os.environ, "HOME": str(self.tmp), "GIT_CONFIG_GLOBAL": os.devnull,
                    "GIT_CONFIG_SYSTEM": os.devnull}
        p = mock.patch.object(mt, "TRACKED_LIST", str(self.tmp / "tracked.lst"))
        p.start()
        self.addCleanup(p.stop)

    def git(self, *args):
        import subprocess
        return subprocess.run(["git", *args], cwd=self.repo, env=self.env, check=True,
                              capture_output=True, text=True)

    def write(self, rel, text="x\n"):
        p = self.repo / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)

    def test_ignored_run_output_is_removed_and_the_build_inputs_are_kept(self):
        import subprocess
        self.git("init", "-q")
        self.git("config", "user.email", "a@b.invalid")
        self.git("config", "user.name", "x")
        self.write(".gitignore", "*.log\ntarget/\nCargo.lock\n.venv/\ngenerated-do-not-edit/\n")
        self.write("src/lib.rs")
        self.git("add", ".")
        self.git("commit", "-qm", "base")
        base = self.git("rev-parse", "HEAD").stdout.strip()
        self.write("test-output.log", "test mir::lower::tests::the_gold_test ... ok\n")  # ignored debris
        self.write("crates/a/run.log")  # ignored debris, nested
        self.write("Cargo.lock")
        self.write(".venv/bin/python")
        self.write("crates/b/tests/fixtures/generated-do-not-edit/x/data.txt")
        self.write("target/debug/x.o")
        self.write("stray.txt")  # untracked, not ignored
        (self.repo / "nested").mkdir()
        subprocess.run(["git", "init", "-q"], cwd=self.repo / "nested", env=self.env, check=True)
        self.write("nested/ignored.log")
        r = subprocess.run(["bash", "-c", "set -eu; " + " && ".join(mt.reset_sh(base))], cwd=self.repo,
                           env=self.env, capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        left = {p.relative_to(self.repo).as_posix() for p in self.repo.rglob("*")
                if p.is_file() and ".git/" not in p.as_posix() + "/" and not p.as_posix().endswith("/.git")}
        gone = {"test-output.log", "crates/a/run.log", "stray.txt", "nested/ignored.log"}
        self.assertFalse(left & gone, left & gone)
        for kept in ("Cargo.lock", ".venv/bin/python", "target/debug/x.o", "src/lib.rs",
                     "crates/b/tests/fixtures/generated-do-not-edit/x/data.txt"):
            self.assertIn(kept, left)
        self.assertIn("Removing test-output.log", r.stdout, "the removals are printed for the build log")

    def test_the_removals_in_a_build_log_are_parsed_with_and_without_buildkit_prefixes(self):
        log = self.tmp / "build.log"
        log.write_text("#7 12.5 Removing test-output.log\n#7 12.6 Removing crates/a/run.log\n"
                       "Removing plain.log\nRemoving-not a line\n#7 DONE\n")
        self.assertEqual(mt.scrubbed_paths(log), ["test-output.log", "crates/a/run.log", "plain.log"])
        self.assertEqual(mt.scrubbed_paths(self.tmp / "missing.log"), [])

    def test_debris_is_removed_even_when_a_root_only_lock_file_sits_inside(self):
        import subprocess
        tmp = self.tmp / "tmp"
        for rel, mode in (("testdir/123_4/target/incremental/x.lock", 0o600),   # rust-analyzer
                          (".tmpAbC/.git/virtual-branches.toml", 0o600),         # gitbutler
                          ("tmp.Xyz/readable.txt", 0o644), ("fine/a.txt", 0o644), ("fine/sub/b.txt", 0o644)):
            p = tmp / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text("x")
            os.chmod(p, mode)
        (tmp / "rootonly").mkdir()
        (tmp / "rootonly" / "inner").write_text("x")
        os.chmod(tmp / "rootonly", 0o700)
        (tmp / "plain-secret").write_text("x")
        os.chmod(tmp / "plain-secret", 0o600)
        os.symlink("fine/a.txt", tmp / "a-link")
        os.symlink("/nonexistent", tmp / "dangling")
        r = subprocess.run(["bash", "-c", "set -eu; " + mt.debris_sh([str(tmp / "testdir")], str(tmp))],
                           capture_output=True, text=True, check=True)
        left = sorted(p.name for p in tmp.iterdir())
        self.assertEqual(left, ["a-link", "dangling", "fine", "tmp.Xyz"])
        removed = sorted(line.split()[-1].rsplit("/", 1)[-1]
                         for line in r.stdout.splitlines() if line.startswith("Removing-debris"))
        self.assertEqual(removed, [".tmpAbC", "plain-secret", "rootonly", "testdir"])
        self.assertTrue((tmp / "fine" / "sub" / "b.txt").exists(), "readable debris stays for the gate")
        log = self.tmp / "b.log"
        log.write_text("".join(f"#9 1.0 {line}\n" for line in r.stdout.splitlines()))
        self.assertEqual(len(mt.scrubbed_paths(log, mt._DEBRIS)), 4)
        self.assertEqual(mt.scrubbed_paths(log), [], "git clean's lines are a different class")

    def test_the_dockerfile_removes_the_debris_before_the_user_and_arm_layers(self):
        t = mt.live_dockerfile("src", record(), "/testbed")
        self.assertIn("rm -rf \"$p\"", t)
        self.assertIn("/tmp/testdir", t)
        self.assertLess(t.index("/tmp/testdir"), t.index("chown -R"))
        self.assertLess(t.index("git clean -ffdx"), t.index("/tmp/testdir"))
        self.assertEqual(mt.IMAGE_DEBRIS, ("/tmp/testdir",))


class SpecialFileTests(unittest.TestCase):
    """A FIFO in a tree must not block the baseline, the export scan or the sweep (gitbutler)."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.tmp, ignore_errors=True))
        self.root = self.tmp / "testbed"
        (self.root / "crates" / "fix").mkdir(parents=True)
        (self.root / "src.rs").write_text("fn plain() {}\n")
        (self.root / "crates" / "fix" / "link").symlink_to("../../src.rs")
        os.mkfifo(self.root / "crates" / "fix" / "named-pipe")

    def run_py(self, code, timeout=30):
        """Run python code with the tool dir on the path; a hang fails the test instead of the suite."""
        import subprocess
        import sys
        here = str(Path(mt.__file__).resolve().parent)
        try:
            r = subprocess.run([sys.executable, "-B", "-c", f"import sys; sys.path.insert(0, {here!r})\n{code}"],
                               capture_output=True, text=True, timeout=timeout)
        except subprocess.TimeoutExpired:
            self.fail("blocked on a FIFO (the reader opened it)")
        self.assertEqual(r.returncode, 0, r.stderr)
        return json.loads(r.stdout)

    def test_regular_files_returns_without_reading_the_fifo_and_records_it(self):
        out = self.run_py(f"""
import json, multilang_tasks as mt
skipped = []
files = {{rel: data.decode() for rel, data in mt.regular_files({str(self.root)!r}, skipped)}}
print(json.dumps({{"files": files, "skipped": skipped}}))""")
        self.assertEqual(out["files"], {"src.rs": "fn plain() {}\n", "crates/fix/link": "../../src.rs"})
        self.assertEqual(out["skipped"], [{"path": "crates/fix/named-pipe", "kind": "fifo"}])

    def test_the_needle_baseline_over_a_tree_with_a_fifo_finishes(self):
        out = self.run_py(f"""
import json, leak_scan, multilang_tasks as mt
skipped = []
ns = leak_scan.baseline_needles(["plain_name_only", "not_there_x"], lambda: mt.regular_files({str(self.root)!r}, skipped))
print(json.dumps({{"kept": ns.kept, "skipped": len(skipped)}}))""")
        self.assertEqual(out["skipped"], 1)

    def test_prescan_agent_image_clears_fifos_from_the_export_and_lists_them(self):
        cfg = self.tmp / "cfg.json"
        cfg.write_text("{}")
        plug = self.tmp / "plugin"
        plug.mkdir()
        (plug / "p.txt").write_text("plugin text")
        manifest = {"full_config_files": [str(cfg)], "plugins_cs": {"install_path": str(plug)},
                    "plugins_full": []}
        needles = mt.leak_scan.baseline_needles(
            ["zz_gold_only_name", "alpha_x beta_y gamma_z delta_w epsilon_v zeta_u eta_t theta_s"],
            lambda: iter([("a.rs", b"nothing")]))

        def export(image, paths, dest, **kw):
            (Path(dest) / "testbed").mkdir(parents=True)
            (Path(dest) / "testbed" / "a.rs").write_text("fn clean() {}\n")
            os.mkfifo(Path(dest) / "testbed" / "p")
            return ["testbed"]

        bundle = SimpleNamespace(id="o__r-1", needles=needles)
        with mock.patch.object(tasks, "export_tree", side_effect=export), \
                mock.patch.object(mt.subprocess, "run",
                                  return_value=SimpleNamespace(stdout=b"git objects")):
            out = mt.prescan_agent_image(bundle, manifest, self.tmp / "data", "sab-agent-cs-o__r-1:t1")
        self.assertEqual(out["control"], "pass")
        self.assertEqual(out["hits"], [])
        self.assertEqual(out["specials_removed"], [{"path": "testbed/p", "kind": "fifo"}])
        saved = json.loads((self.tmp / "data" / "prescan.json").read_text())
        self.assertEqual(saved["specials_removed"], out["specials_removed"])
        self.assertFalse((self.tmp / "data" / "export" / "testbed" / "p").exists())


    def test_old_tree_files_reader_is_what_blocked(self):
        # documents the cause without hanging the suite: tasks.tree_files opens every non-link file
        import inspect
        self.assertIn("p.read_bytes()", inspect.getsource(tasks.tree_files))
        self.assertNotIn("S_ISREG", inspect.getsource(tasks.tree_files))

    def test_remove_specials_lists_and_deletes_them_so_leak_scan_can_scan(self):
        out = self.run_py(f"""
import json, leak_scan, multilang_tasks as mt
gone = mt.remove_specials({str(self.root)!r})
rep = leak_scan.scan([__import__("pathlib").Path({str(self.root)!r})], None,
                     leak_scan.baseline_needles(["zz_needle_name"], lambda: iter([("a.rs", b"x")])))
print(json.dumps({{"gone": gone, "skipped": len(rep.skipped), "files": rep.files_scanned}}))""")
        self.assertEqual(out["gone"], [{"path": "crates/fix/named-pipe", "kind": "fifo"}])
        self.assertEqual(out["skipped"], 0)
        self.assertFalse((self.root / "crates" / "fix" / "named-pipe").exists())
        self.assertTrue((self.root / "src.rs").exists())

    def test_the_sweep_lists_the_fifo_and_counts_links_without_opening_either(self):
        out = self.run_py(f"""
import json, live_sweep
r = live_sweep.sweep({str(self.tmp)!r}, [], ["a literal that is not in the tree anywhere"], [])
print(json.dumps({{k: r[k] for k in ("files", "specials", "specials_count", "symlinks", "hits")}}))""")
        self.assertEqual(out["specials"], [{"path": "/testbed/crates/fix/named-pipe", "kind": "fifo"}])
        self.assertEqual((out["specials_count"], out["symlinks"], out["files"], out["hits"]), (1, 1, 1, []))

    def test_a_planted_needle_next_to_a_fifo_is_still_found_by_sweep_and_baseline_reader(self):
        (self.root / "planted.rs").write_text("// planted_secret_literal_line_for_the_gate\n")
        out = self.run_py(f"""
import json, live_sweep, multilang_tasks as mt
r = live_sweep.sweep({str(self.tmp)!r}, [], ["planted_secret_literal_line_for_the_gate"], [])
files = [rel for rel, data in mt.regular_files({str(self.root)!r}) if b"planted_secret" in data]
print(json.dumps({{"hits": [h["path"] for h in r["hits"]], "files": files}}))""")
        self.assertEqual(out["hits"], ["/testbed/planted.rs"])
        self.assertEqual(out["files"], ["planted.rs"])



class ToolLayerTests(unittest.TestCase):
    """/opt/sab is the same for every task; a shingle that already occurs in it is no leak evidence."""
    PHRASE = "if value null typeof value object return value if seen has value return"
    GOLD = "alpha_one beta_two gamma_three delta_four epsilon_five zeta_six eta_seven theta_eight"

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.tmp, ignore_errors=True))
        self.tool = self.tmp / "opt" / "sab"
        (self.tool / "bin").mkdir(parents=True)
        (self.tool / "bin" / "node").write_bytes(
            b"\0\0if (value === null || typeof value !== 'object') return value; if (seen.has(value)) return;\0")

    def needleset(self, needles):
        counts = dict.fromkeys(mt.leak_scan.KINDS, 0)
        for n in needles:
            counts[mt.leak_scan.kind_of(n)] += 1
        return mt.leak_scan.NeedleSet(list(needles), counts, {"name": 1, "path": 0, "shingle": 3}, 5, 99)

    def test_a_shingle_found_in_the_tool_layer_is_dropped_and_counted(self):
        generic = "if value null typeof value object return value"
        ns, dropped = mt.baseline_tool_layer(self.needleset([generic, self.GOLD, "gold_name_x"]), self.tool)
        self.assertEqual(ns.needles, [self.GOLD, "gold_name_x"])
        self.assertEqual(dropped, 1)
        self.assertEqual(ns.kept, {"name": 1, "path": 0, "shingle": 1})
        self.assertEqual(ns.dropped, {"name": 1, "path": 0, "shingle": 3}, "the base-tree counts stay")
        self.assertEqual((ns.baseline_files, ns.baseline_bytes), (5, 99))

    def test_a_name_or_path_needle_found_in_the_tool_layer_is_not_dropped(self):
        (self.tool / "bin" / "other").write_text("unlucky_test_name appears here crates/x/y.rs")
        names = ["unlucky_test_name", "crates/x/y.rs"]
        ns, dropped = mt.baseline_tool_layer(self.needleset([*names, self.GOLD]), self.tool)
        self.assertEqual(ns.needles, [*names, self.GOLD])
        self.assertEqual(dropped, 0)

    def test_a_gold_shingle_not_in_the_tool_layer_still_hits_in_the_repo_tree(self):
        generic = "if value null typeof value object return value"
        ns, _ = mt.baseline_tool_layer(self.needleset([generic, self.GOLD]), self.tool)
        repo = self.tmp / "testbed"
        repo.mkdir()
        (repo / "clean.rs").write_text("fn ok() { nothing_to_see(); }\n")
        clean = mt.leak_scan.scan([repo], None, ns)
        self.assertEqual(clean.hits, [])
        (repo / "planted.rs").write_text("// " + self.GOLD.replace(" ", "  ") + "\n")
        hit = mt.leak_scan.scan([repo], None, ns)
        self.assertEqual([(h.path.name, h.needle) for h in hit.hits], [("planted.rs", self.GOLD)])
        # and the tool layer itself, as scanned by the prescan, no longer fails the gate
        self.assertEqual(mt.leak_scan.scan([self.tool], None, ns).hits, [])
        raw = mt.leak_scan.scan([self.tool], None, self.needleset([generic, self.GOLD]))
        self.assertEqual([h.needle for h in raw.hits], [generic], "without the baseline it would hit")

    def test_dropping_every_shingle_is_refused(self):
        with self.assertRaises(ValueError):
            mt.baseline_tool_layer(self.needleset(["if value null typeof value object return value"]), self.tool)

    def test_no_shingles_means_nothing_to_baseline(self):
        ns = self.needleset(["only_a_name"])
        self.assertEqual(mt.baseline_tool_layer(ns, self.tool), (ns, 0))

    def test_tool_layer_tree_exports_once_per_manifest_and_verifies_the_copy(self):
        cache = self.tmp / "cache"
        body = b"binary bytes"
        listing = f"{hashlib.sha256(body).hexdigest()}  ./bin/node\n"
        exports = []

        def export(image, paths, dest, **kw):
            exports.append((image, paths))
            (Path(dest) / "opt" / "sab" / "bin").mkdir(parents=True)
            (Path(dest) / "opt" / "sab" / "bin" / "node").write_bytes(body)

        with mock.patch.object(mt.pilot, "docker", lambda *a, **k: SimpleNamespace(stdout=listing)), \
                mock.patch.object(tasks, "export_tree", side_effect=export):
            root, files, digest = mt.tool_layer_tree("img-a", cache)
            root2, _, digest2 = mt.tool_layer_tree("img-b", cache)  # another task, same layer
        self.assertEqual(len(exports), 1)
        self.assertEqual((root, digest), (root2, digest2))
        self.assertEqual(files, {"./bin/node": hashlib.sha256(body).hexdigest()})
        self.assertEqual(digest, hashlib.sha256(listing.encode()).hexdigest())

    def test_tool_layer_tree_refuses_an_export_that_does_not_match_the_manifest(self):
        def export(image, paths, dest, **kw):
            (Path(dest) / "opt" / "sab" / "bin").mkdir(parents=True)
            (Path(dest) / "opt" / "sab" / "bin" / "node").write_bytes(b"other bytes")

        listing = f"{'0' * 64}  ./bin/node\n"
        with mock.patch.object(mt.pilot, "docker", lambda *a, **k: SimpleNamespace(stdout=listing)), \
                mock.patch.object(tasks, "export_tree", side_effect=export), \
                self.assertRaises(RuntimeError):
            mt.tool_layer_tree("img", self.tmp / "cache")

    def test_record_tool_layer_writes_the_count_and_manifest_into_an_old_assembly_json(self):
        out = self.tmp / "t" / "build"
        (out / "baseline-tree" / "testbed").mkdir(parents=True)
        (out / "baseline-tree" / "testbed" / "a.rs").write_text("nothing here\n")
        generic = "if value null typeof value object return value if seen has"
        (out / "record.json").write_text(json.dumps(record(patch=PATCH + f"+{generic}\n")))
        (out / "assembly.json").write_text(json.dumps({"needles": {"kept": {}, "dropped": {}}}))
        bj = self.tmp / "t" / "bundle.json"
        bj.write_text(json.dumps({"kind": "live", "out": str(out), "record_path": str(out / "record.json"),
                                  "task_image": "sab-task-o__r-1:t1", "image": "sab-agent-cs-o__r-1:t1"}))
        with mock.patch.object(mt, "tool_layer_tree", return_value=(self.tool, {"./bin/node": "h"}, "d" * 64)):
            tool = mt.record_tool_layer(bj)
        assy = json.loads((out / "assembly.json").read_text())
        self.assertGreater(tool["dropped"]["shingle"], 0)
        self.assertEqual(assy["needles"]["tool_layer_dropped"], tool["dropped"])
        self.assertEqual(assy["tool_layer"]["manifest_sha256"], "d" * 64)


    def test_load_live_applies_the_tool_layer_to_the_recomputed_needles(self):
        out = self.tmp / "t" / "build"
        (out / "baseline-tree" / "testbed").mkdir(parents=True)
        (out / "baseline-tree" / "testbed" / "a.rs").write_text("nothing here\n")
        generic = "if value null typeof value object return value if seen has"
        rec = record(patch=PATCH + f"+{generic}\n", problem_statement="x")
        (out / "record.json").write_text(json.dumps(rec))
        bj = self.tmp / "t" / "bundle.json"
        bj.write_text(json.dumps({"kind": "live", "out": str(out), "record_path": str(out / "record.json"),
                                  "task_image": "sab-task-o__r-1:t1", "image": "sab-agent-cs-o__r-1:t1",
                                  "workdir": "/testbed", "expected": {"fail_to_pass": 1, "pass_to_pass_kept": 3,
                                                                   "pass_to_pass_dropped": 0, "dropped": []}}))
        with mock.patch.object(mt, "tool_layer_tree", return_value=(self.tool, {}, "d")):
            b = mt.load_live(bj)
        self.assertNotIn("if value null typeof value object return value", b.needles.needles)
        self.assertNotIn("value null typeof value object return value if", b.needles.needles)
        self.assertTrue([n for n in b.needles.needles if "second_fn" in n])



class FakeDocker:
    """Just enough docker for `live_grader`: the tests' logs depend on whether the agent's patch
    was applied (gold state) or not (base state)."""
    BASE_LOG = "".join(f"{k}={v}\n" for k, v in GOOD_BASE.items())
    GOLD_LOG = "".join(f"{k}={v}\n" for k, v in GOOD_GOLD.items())

    def __init__(self, fail=None):
        self.calls, self.patched, self.fail, self.applied = [], False, fail, []

    def __call__(self, *args, **kw):
        import base64
        args = [str(a) for a in args]
        self.calls.append(args)
        out = lambda text="", rc=0: SimpleNamespace(returncode=rc, stdout=text)
        if self.fail and self.fail in args[:1] + args[1:2]:
            raise RuntimeError(f"docker {self.fail} failed")
        if args[0] != "exec":
            return out("false\n" if args[0] == "inspect" else "")
        i = next(n for n, a in enumerate(args) if a.startswith(mt.GRADE_PREFIX))
        cmd = args[i + 1:]
        if cmd[:2] == ["git", "apply"]:
            self.applied.append(kw.get("data"))
            if cmd[2:3] == ["--binary"]:
                self.patched = True
            return out()
        if cmd[0] == "bash":
            script = cmd[-1]
            if "head -c" in script:  # a print command
                return out(base64.b64encode((self.GOLD_LOG if self.patched else self.BASE_LOG).encode()).decode())
            return out(base64.b64encode(b"").decode())
        return out()

    def containers(self):
        return [a[a.index("--name") + 1] for a in self.calls if a[0] == "run"]

    def removed(self):
        return [a[-1] for a in self.calls if a[0] == "rm"]


def live_bundle(tmp, dropped=("t::p2",), p2p=3):
    return {"kind": "live", "workdir": "/testbed", "record_path": str(Path(tmp) / "record.json"),
            "expected": {"fail_to_pass": 1, "pass_to_pass_kept": p2p - len(dropped),
                         "pass_to_pass_dropped": len(dropped), "dropped": list(dropped)}}


class LiveGraderTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.tmp, ignore_errors=True))
        self.fake = FakeDocker()

    def grade(self, patch, rec=None, bundle=None, fake=None, name="r0"):
        fake = fake or self.fake
        grader = mt.live_grader(rec or record(), bundle or live_bundle(self.tmp), "sab-arm-o__r-1:t",
                                self.tmp / "grades")
        with mock.patch("pilot.docker", fake):
            return grader(patch, name)

    def test_live_grader_empty_patch_fails_and_gold_passes_with_fake_docker(self):
        empty = self.grade("")
        self.assertFalse(empty["pass"])
        self.assertEqual(empty["not_passing"], 1)  # t::f1 fails at base
        self.assertFalse(self.fake.patched)
        self.assertEqual(self.fake.applied, [TEST_PATCH])  # no agent patch, only the test patch
        gold = self.grade(PATCH, fake=FakeDocker())
        self.assertTrue(gold["pass"], gold)
        self.assertNotIn("error", gold)
        self.assertEqual((gold["expected"], gold["missing"]), (3, 0))
        for r in (empty, gold):
            self.assertEqual(r["name"], "r0")
            self.assertFalse(r["oom_killed"])
            self.assertTrue(r["container_removed"])
            self.assertIn("wall_seconds", r)
            self.assertTrue(Path(r["log"]).is_file())
            self.assertTrue(r["container"].startswith("sab-grade-o__r-1-r0-"))

    def test_live_grader_container_is_isolated_resourced_and_always_removed(self):
        self.grade(PATCH)
        run = next(a for a in self.fake.calls if a[0] == "run")
        self.assertEqual(run[run.index("--network") + 1], "none")
        self.assertEqual(run[run.index("--cpus") + 1], "4")
        self.assertEqual(run[run.index("--memory") + 1], "8g")
        self.assertIn("sab-arm-o__r-1:t", run)
        self.assertEqual(self.fake.removed(), self.fake.containers())

    def test_live_grader_error_is_a_failed_grade_not_an_exception(self):
        import subprocess
        bad_parser = "def parser(log):\n    raise ValueError('boom')\n"
        cases = {
            "parser error": dict(rec=record(log_parser=bad_parser)),
            "docker start error": dict(fake=FakeDocker(fail="run")),
            "patch does not apply": dict(fake=FakeDocker(fail="exec")),
        }
        for label, kw in cases.items():
            with self.subTest(label):
                r = self.grade(PATCH, **kw)
                self.assertIs(r["pass"], False)
                self.assertTrue(r.get("error"), r)
                self.assertTrue(r["container_removed"])
        fake = FakeDocker()
        real_call = fake.__call__

        def slow(*args, **kw):
            if str(args[0]) == "exec" and "bash" in map(str, args):
                raise subprocess.TimeoutExpired("docker", 1)
            return real_call(*args, **kw)
        r = self.grade(PATCH, fake=slow)
        self.assertIs(r["pass"], False)
        self.assertIn("timed out", r["error"])

    def test_an_empty_patch_that_would_pass_is_an_error_not_a_pass(self):
        class AlwaysGold(FakeDocker):
            def __init__(self):
                super().__init__()
                self.patched = True
        r = self.grade("", fake=AlwaysGold())
        self.assertIs(r["pass"], False)
        self.assertIn("empty patch", r["error"])

    def test_live_grader_uses_the_bundles_expected_set_not_the_records_p2p(self):
        # t::p2 fails at gold: with it dropped the gold patch passes, without it it cannot
        self.assertTrue(self.grade(PATCH)["pass"])
        r = self.grade(PATCH, bundle=live_bundle(self.tmp, dropped=()), fake=FakeDocker())
        self.assertFalse(r["pass"])
        self.assertEqual(r["expected"], 4)
        self.assertEqual(r["examples"], ["t::p2"])
        # a name the bundle does not drop is demanded even when the log lacks it
        rec = record(PASS_TO_PASS=["t::p1", "t::p2", "t::p3", "t::absent"])
        r = self.grade(PATCH, rec=rec, bundle=live_bundle(self.tmp, p2p=4), fake=FakeDocker())
        self.assertFalse(r["pass"])
        self.assertEqual(r["missing"], 1)
        self.assertEqual(mt.expected_sets(rec, live_bundle(self.tmp, dropped=("t::p2", "t::absent"), p2p=4)),
                         (["t::f1"], ["t::p1", "t::p3"]))

    def test_live_grader_refuses_another_workdir(self):
        bundle = dict(live_bundle(self.tmp), workdir="/testbed/repo")
        with self.assertRaises(mt.LiveRefused):
            mt.live_grader(record(), bundle, "img", self.tmp)
    def execs(self, fake):
        """The recorded exec commands, as short labels in order."""
        labels = []
        for a in fake.calls:
            if a[0] != "exec":
                continue
            i = next(n for n, x in enumerate(a) if x.startswith(mt.GRADE_PREFIX))
            cmd = a[i + 1:]
            if cmd[:3] == ["git", "apply", "--binary"]:
                labels.append("agent-patch")
            elif cmd[:2] == ["git", "apply"]:
                labels.append("test-patch")
            elif cmd[0] == "rm" and cmd[-1].endswith("reports"):
                labels.append("reports-rm")
            elif cmd[0] == "mkdir" and cmd[-1].endswith("reports"):
                labels.append("reports-mkdir")
            elif cmd[0] == "bash" and "head -c" in cmd[-1]:
                labels.append("print")
            elif cmd[0] == "bash":
                labels.append("rebuild-or-test")
            else:
                labels.append(cmd[0])
        return labels

    def test_reports_dir_is_wiped_and_recreated_after_both_patches_and_before_the_rebuild(self):
        self.grade(PATCH)
        order = self.execs(self.fake)
        i = order.index("reports-rm")
        self.assertEqual(order[i + 1], "reports-mkdir")
        self.assertLess(order.index("agent-patch"), i)
        self.assertLess(order.index("test-patch"), i)
        first_bash = order.index("rebuild-or-test")
        self.assertLess(i + 1, first_bash)

    def test_a_test_command_that_tees_into_reports_without_mkdir_still_grades(self):
        class NeedsReports(FakeDocker):
            def __init__(self):
                super().__init__()
                self.reports = False
                self.broken = False

            def __call__(self, *args, **kw):
                a = [str(x) for x in args]
                if a[0] == "exec" and "mkdir" in a and a[-1].endswith("reports"):
                    self.reports = True
                if a[0] == "exec" and "bash" in a and "head -c" not in a[-1] and "reports/" in a[-1] \
                        and not self.reports:
                    self.broken = True  # `tee reports/x` with no reports/ directory
                    return SimpleNamespace(returncode=1, stdout="")
                res = super().__call__(*args, **kw)
                if self.broken and a[0] == "exec" and "head -c" in a[-1]:
                    return SimpleNamespace(returncode=0, stdout="")
                return res
        rec = record(test_cmds=["cargo test 2>&1 | tee reports/out.txt"])
        fake = NeedsReports()
        r = self.grade(PATCH, rec=rec, fake=fake)
        self.assertFalse(fake.broken)
        self.assertTrue(r["pass"], r)

    def test_live_grader_refuses_a_bundle_that_disagrees_with_the_record(self):
        for label, bundle in {
                "f2p count": dict(live_bundle(self.tmp), expected=dict(live_bundle(self.tmp)["expected"], fail_to_pass=2)),
                "kept count": live_bundle(self.tmp, p2p=4),
                "dropped not in record": live_bundle(self.tmp, dropped=("t::p2", "t::nope"), p2p=4)}.items():
            with self.subTest(label), self.assertRaises(mt.LiveRefused):
                mt.live_grader(record(), bundle, "img", self.tmp)

    def test_live_grader_from_record_reads_the_dataset_record_from_the_bundle(self):
        (self.tmp / "record.json").write_text(json.dumps(record()))
        g = mt.live_grader_from_record(live_bundle(self.tmp), "img", self.tmp / "g")
        with mock.patch("pilot.docker", self.fake):
            self.assertTrue(g(PATCH, "x")["pass"])




if __name__ == "__main__":
    unittest.main()
