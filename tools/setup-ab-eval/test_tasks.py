"""Task assembly (tasks.py) and metadata-only selection (select_tasks.py).

The S5-backed tests build a tiny private task from a throwaway git repo through the real S5
build-task.mjs (invoked with node, never modified); they skip when node or the S5 checkout is
missing. The grader control needs docker and the s5/codescout image.
"""
import contextlib
import hashlib
import io
import json
import os
import shutil
import subprocess
import tempfile
import unittest
import unittest.mock
from pathlib import Path

import arms
import leak_scan
import select_tasks
import tasks

HERE = Path(__file__).resolve().parent
RULE = json.loads((HERE / "selection-rule.json").read_text())
DEV_IDS = ["burntsushi__ripgrep-2576", "tokio-rs__axum-734", "tokio-rs__axum-1119",
           "burntsushi__ripgrep-2209", "tokio-rs__tokio-6551", "astral-sh__ruff-15443",
           "astral-sh__ruff-15309"]
HAVE_S5 = (tasks.S5_DIR / "build-task.mjs").is_file() and shutil.which("node") is not None
HAVE_GIT = shutil.which("git") is not None


def have_docker_image(image):
    if shutil.which("docker") is None:
        return False
    try:
        return subprocess.run(["docker", "image", "inspect", image], capture_output=True,
                              timeout=30).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


def cand(id_, repo, files=2, added=40, **extra):
    return dict({"id": id_, "repo": repo, "gold_files": [f"f{i}.rs" for i in range(files)],
                 "gold_added_lines": added}, **extra)


def population():
    out = [cand(f"cs-p{i:02d}", "codescout", files=1 + i % 5, added=20 + 7 * i) for i in range(1, 21)]
    for repo in ("astral-sh/ruff", "nushell/nushell", "sharkdp/bat", "tokio-rs/axum",
                 "tokio-rs/tokio", "uutils/coreutils", "burntsushi/ripgrep"):
        owner, name = repo.split("/")
        for n in range(4):
            out.append(cand(f"{owner}__{name}-{1000 + n}", repo, files=1 + n, added=10 + 9 * n))
    return out


# --- selection ---------------------------------------------------------------------------------

class Selection(unittest.TestCase):

    def test_selection_ignores_every_outcome_field(self):
        base = population()
        plain = select_tasks.select(base, RULE)
        self.assertEqual(len(plain), 12)
        # Outcomes chosen so that any sort on them would reorder the draw.
        for seed in range(3):
            noisy = []
            for i, c in enumerate(base):
                h = int(hashlib.sha256(f"{seed}:{c['id']}".encode()).hexdigest(), 16)
                noisy.append(dict(c, **{"pass": bool(h % 2), "cost": (h % 1000) / 7.0,
                                        "costUsd": h % 13, "duration": h % 997, "numTurns": i,
                                        "grade": {"pass": not (h % 3)}, "outcome": "scored"}))
            self.assertEqual(select_tasks.select(noisy, RULE), plain)
        # Removing the outcome fields entirely changes nothing either.
        self.assertEqual(select_tasks.select([dict(c) for c in base], RULE), plain)

    def test_codex_development_tasks_are_never_selected(self):
        self.assertEqual(sorted(RULE["exclude_ids"]), sorted(DEV_IDS))
        pool = population()
        # The development tasks get metadata that would otherwise make them eligible.
        for dev in DEV_IDS:
            owner_repo = {"burntsushi": "burntsushi/ripgrep", "tokio-rs": None,
                          "astral-sh": "astral-sh/ruff"}[dev.split("__")[0]]
            repo = owner_repo or ("tokio-rs/axum" if "axum" in dev else "tokio-rs/tokio")
            pool.append(cand(dev, repo, files=2, added=40))
        chosen = select_tasks.select(pool, RULE)
        self.assertFalse(set(chosen) & set(DEV_IDS))
        # Even a pool made only of them selects nothing, and asking for every eligible task
        # (the full draw order) never lists them.
        self.assertEqual(select_tasks.select([cand(d, "tokio-rs/tokio") for d in DEV_IDS], RULE), [])
        for order in select_tasks.ranked(pool, RULE).values():
            self.assertFalse(set(order) & set(DEV_IDS))

    def test_draw_is_the_documented_salted_sha256_with_a_repo_cap(self):
        pool = population()
        chosen = select_tasks.select(pool, RULE)
        foreign = [i for i in chosen if "__" in i]
        repos = [next(c["repo"] for c in pool if c["id"] == i) for i in foreign]
        self.assertEqual(len(repos), len(set(repos)), "max_per_repo 1 for foreign tasks")
        private = [i for i in chosen if i.startswith("cs-")]
        eligible = [c for c in pool if c["repo"] == "codescout"
                    and 1 <= len(c["gold_files"]) <= 8 and 5 <= c["gold_added_lines"] <= 400]
        salt = RULE["draw"]["salt"]
        expect = sorted(eligible, key=lambda c: (hashlib.sha256(f"{salt}:{c['id']}".encode())
                                                 .hexdigest(), c["id"]))[:6]
        self.assertEqual(private, [c["id"] for c in expect])
        # Interleaved: private 1, foreign 1, private 2, ...
        self.assertEqual([i.startswith("cs-") for i in chosen], [True, False] * 6)

    def test_eligibility_bounds_and_unknown_repos(self):
        pool = [cand("cs-small", "codescout", added=4), cand("cs-big", "codescout", added=401),
                cand("cs-wide", "codescout", files=9), cand("cs-ok", "codescout", files=8, added=400),
                cand("axios__axios-1", "axios/axios")]
        self.assertEqual(select_tasks.select(pool, RULE), ["cs-ok"])

    def test_a_failed_task_is_replaced_by_the_next_in_its_source_order(self):
        pool = population()
        chosen = select_tasks.select(pool, RULE)
        victim = chosen[1]  # foreign rank 1
        again = select_tasks.select(pool, RULE, failed=[victim])
        self.assertNotIn(victim, again)
        self.assertEqual(len(again), 12)
        self.assertEqual([x for x in again if x != chosen[1]][:1], [chosen[0]])
        repo = next(c["repo"] for c in pool if c["id"] == victim)
        new = (set(again) - set(chosen)).pop()
        self.assertEqual(next(c["repo"] for c in pool if c["id"] == new), repo)
    def test_an_unknown_failed_id_is_refused(self):
        # Task 7 review minor 2: a typo must not silently keep the task it meant to replace.
        with self.assertRaises(ValueError):
            select_tasks.select(population(), RULE, failed=["cs-p99"])

    def test_malformed_candidates_are_refused(self):
        with self.assertRaises(ValueError):
            select_tasks.select([{"id": "cs-x", "repo": "codescout", "gold_added_lines": 9}], RULE)
        with self.assertRaises(ValueError):
            select_tasks.select([cand("cs-x", "codescout"), cand("cs-x", "codescout")], RULE)
        with self.assertRaises(ValueError):
            select_tasks.select([cand("cs-x", "codescout", added="9")], RULE)

    def test_foreign_candidate_metadata_comes_from_the_patch_only(self):
        record = {"instance_id": "tokio-rs__tokio-1", "repo": "tokio-rs/tokio",
                  "patch": ("diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1,2 @@\n-x\n"
                            "+y\n+z\ndiff --git a/b.rs b/b.rs\n--- a/b.rs\n+++ b/b.rs\n@@ -1 +1 @@\n"
                            "-p\n+q\n"),
                  "test_patch": "diff --git a/t.rs b/t.rs\n+++ b/t.rs\n+t\n",
                  "FAIL_TO_PASS": "['x']"}
        self.assertEqual(select_tasks.foreign_candidate(record),
                         {"id": "tokio-rs__tokio-1", "repo": "tokio-rs/tokio",
                          "gold_files": ["a.rs", "b.rs"], "gold_added_lines": 3})


# --- private assembly through S5 build-task.mjs --------------------------------------------------

F2P = "add_returns_the_sum_of_its_operands"
LIB_P = ("/// Adds two operands for the report writer.\n"
         "pub fn add(left_operand: i32, right_operand: i32) -> i32 {\n"
         "    left_operand.wrapping_sub(right_operand)\n}\n\n"
         "pub const STABLE: u32 = 7;\n")
LIB_C = ("/// Adds two operands for the report writer.\n"
         "pub fn add(left_operand: i32, right_operand: i32) -> i32 {\n"
         "    let total_of_both_operands = left_operand.wrapping_add(right_operand).max(i32::MIN);\n"
         "    total_of_both_operands\n}\n\n"
         "pub const STABLE: u32 = 7;\n")
HIDDEN = (f"#[test]\nfn {F2P}() {{\n    assert_eq!(fx::add(2, 3), 5);\n}}\n")
EXISTING = "#[test]\nfn existing_constant_is_stable() {\n    assert_eq!(fx::STABLE, 7);\n}\n"
LOCK = ('# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\n'
        'version = 4\n\n[[package]]\nname = "fx"\nversion = "0.1.0"\n')
ISSUE = "docs/issues/2026-10-01-add-subtracts.md"
ISSUE_NEEDLE = "docs/issues/2026-09-02-test-names.md"
ISSUE_OTHER = "docs/issues/2026-09-01-unrelated.md"


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), "-c", "user.name=t",
                           "-c", "user.email=t@example.invalid", *args],
                          check=True, capture_output=True, text=True).stdout.strip()


def make_fixture(root):
    """A repo with parent P and fix C, and an S5 task dir for it. Returns (task_dir, repo)."""
    repo = root / "repo"
    files_p = {"Cargo.toml": '[package]\nname = "fx"\nversion = "0.1.0"\nedition = "2021"\n',
               "Cargo.lock": LOCK, ".gitignore": "/target\n", "src/lib.rs": LIB_P,
               "tests/existing.rs": EXISTING,
               ISSUE: "# add subtracts\n\nstatus: open\nThe adder subtracts.\n",
               ISSUE_NEEDLE: f"# triage\n\nwrite a test named {F2P} later\n",
               ISSUE_OTHER: "# unrelated\n\nnothing to do with addition\n",
               "CLAUDE.md": "repo instructions\n"}
    for rel, body in files_p.items():
        (repo / rel).parent.mkdir(parents=True, exist_ok=True)
        (repo / rel).write_text(body)
    subprocess.run(["git", "init", "-q", "-b", "main", str(repo)], check=True)
    git(repo, "add", "-A")
    git(repo, "commit", "-qm", "parent")
    parent = git(repo, "rev-parse", "HEAD")
    (repo / "src/lib.rs").write_text(LIB_C)
    (repo / "tests/add_sum.rs").write_text(HIDDEN)
    (repo / ISSUE).write_text("# add subtracts\n\nstatus: fixed\nThe adder subtracts.\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-qm", "fix")
    fix = git(repo, "rev-parse", "HEAD")
    task_dir = root / "s5tasks" / "fx-p01"
    task_dir.mkdir(parents=True)
    task = {"id": "fx-p01", "repo": "codescout", "repoPath": str(repo), "fix": fix,
            "parent": parent, "subsystem": "adder", "testRunner": "cargo",
            "hidden": {"files": ["tests/add_sum.rs"]},
            "buildCommand": ["cargo", "test", "--no-run", "--offline"],
            "f2p": {"command": ["cargo", "test", "--offline", "--no-fail-fast", "--test", "add_sum"],
                    "names": [F2P]},
            "p2p": {"command": ["cargo", "test", "--offline", "--no-fail-fast", "--test", "existing"],
                    "names": ["existing_constant_is_stable"]},
            "testPatterns": ["cargo test"], "scrub": {"removePaths": [], "terms": ["total_of_both"]},
            "timeoutsSec": {"build": 600, "test": 300}}
    (task_dir / "task.json").write_text(json.dumps(task, indent=2) + "\n")
    (task_dir / "TASK.md").write_text("# Task\n\nThe adder is wrong.\n")
    (task_dir / "prompt.txt").write_text("add(2, 3) returns -1. Make it return the sum.\n")
    return task_dir, repo


@unittest.skipUnless(HAVE_S5 and HAVE_GIT, "needs node and the S5 checkout (build-task.mjs)")
class PrivateAssembly(unittest.TestCase):

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="sab-tasks-test-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.task_dir, self.repo = make_fixture(self.tmp)

    def test_issue_file_is_absent_from_the_built_tree(self):
        source_bytes = (self.task_dir / "task.json").read_bytes()
        a = tasks.assemble_private(self.task_dir, self.tmp / "out")
        tree = a.tree
        self.assertTrue((tree / "src/lib.rs").is_file())
        # R5 rule 1: present at P and modified by the fix.
        self.assertFalse((tree / ISSUE).exists())
        # R5 rule 2: present at P and holds a needle (an f2p name).
        self.assertFalse((tree / ISSUE_NEEDLE).exists())
        # Any other issue file stays: removal is by rule, not by directory.
        self.assertTrue((tree / ISSUE_OTHER).is_file())
        # S5's default removals still apply: the repo's CLAUDE.md became the task CLAUDE.md.
        self.assertNotIn("repo instructions", (tree / "CLAUDE.md").read_text())
        # The S5 source is untouched; the COPIED definition carries the removal.
        self.assertEqual((self.task_dir / "task.json").read_bytes(), source_bytes)
        copied = json.loads((a.def_dir / "task.json").read_text())
        self.assertEqual(sorted(copied["scrub"]["removePaths"]), sorted([ISSUE, ISSUE_NEEDLE]))
        self.assertEqual({r["path"]: r["rule"] for r in a.issue_files},
                         {ISSUE: "modified_by_fix", ISSUE_NEEDLE: "holds_a_needle"})
        # Nor is any of it in the task commit's objects.
        objects = subprocess.run(["git", "-C", str(tree), "cat-file", "--batch-all-objects",
                                  "--batch"], capture_output=True, check=True).stdout
        self.assertNotIn(b"status: open", objects)
        self.assertNotIn(F2P.encode(), objects)

    def test_needles_are_baselined_against_the_built_parent(self):
        a = tasks.assemble_private(self.task_dir, self.tmp / "out")
        self.assertIsInstance(a.needles, leak_scan.NeedleSet)
        self.assertIn(F2P, a.needles.needles)
        self.assertTrue(any("total_of_both_operands" in n for n in a.needles.needles))
        # The built tree itself reads clean against them.
        report = leak_scan.scan([a.tree], None, a.needles)
        self.assertEqual(report.hits, [])

    def test_grader_gold_patch_holds_the_fix_and_no_hidden_test_or_removed_path(self):
        a = tasks.assemble_private(self.task_dir, self.tmp / "out")
        gold = a.gold_patch.read_text()
        self.assertIn("wrapping_add", gold)
        self.assertNotIn("tests/add_sum.rs", gold)
        self.assertNotIn("docs/issues", gold)
        subprocess.run(["git", "-C", str(a.tree), "apply", "--check", str(a.gold_patch)],
                       check=True)

    def test_an_existing_output_is_never_merged_into(self):
        tasks.assemble_private(self.task_dir, self.tmp / "out")
        with self.assertRaises(FileExistsError):
            tasks.assemble_private(self.task_dir, self.tmp / "out")

    @unittest.skipUnless(have_docker_image(tasks.PRIVATE_BASE_IMAGE),
                         f"needs docker and the {tasks.PRIVATE_BASE_IMAGE} image")
    def test_grader_control_base_fails_and_gold_passes(self):
        a = tasks.assemble_private(self.task_dir, self.tmp / "out")
        tag = "sab-test-fx-p01:" + hashlib.sha256(str(self.tmp).encode()).hexdigest()[:8]
        image = tasks.build_private_task_image(a, tag, cargo_home=None)
        self.addCleanup(subprocess.run, ["docker", "image", "rm", "-f", tag], capture_output=True)
        grade = tasks.private_grader(a, image, self.tmp / "grades")
        base = grade("", "base")
        gold = grade(a.gold_patch.read_text(), "gold")
        self.assertFalse(base["pass"], base)
        self.assertEqual(base["f2p"]["failed"], [F2P])
        self.assertTrue(gold["pass"], gold)
        self.assertEqual(gold["p2p"]["passed"], ["existing_constant_is_stable"])
        names = subprocess.run(["docker", "ps", "-a", "--format", "{{.Names}}"],
                               capture_output=True, text=True).stdout.split()
        self.assertFalse([n for n in names if n.startswith("sab-grade-fx-p01")])


# --- foreign helpers, history, control order ----------------------------------------------------

EVAL = """#!/bin/bash
set -uxo pipefail
cd /testbed
git apply -v - <<'EOF_1'
diff --git a/t.rs b/t.rs
EOF_1
: '>>>>> Start Test Output'
(cargo test --test sync_broadcast --no-fail-fast) | cat
: '>>>>> End Test Output'
git checkout abc t.rs
"""


class ForeignHelpers(unittest.TestCase):

    def test_test_command_is_read_from_the_eval_script(self):
        self.assertEqual(tasks.eval_test_command(EVAL),
                         "(cargo test --test sync_broadcast --no-fail-fast) | cat")
        with self.assertRaises(ValueError):
            tasks.eval_test_command(EVAL.replace(": '>>>>> End Test Output'\n", ""))

    def test_test_files_are_the_test_patch_paths(self):
        tp = ("diff --git a/tokio/tests/a.rs b/tokio/tests/a.rs\n--- a/x\n+++ b/x\n"
              "diff --git a/tokio/tests/b.rs b/tokio/tests/b.rs\n")
        self.assertEqual(tasks.patch_files(tp), ["tokio/tests/a.rs", "tokio/tests/b.rs"])

    def test_foreign_dockerfile_follows_the_codex_recipe(self):
        text = tasks.foreign_dockerfile("swebench/x@sha256:" + "a" * 64,
                                        "(cargo test --test t --no-fail-fast) | cat")
        self.assertIn("FROM swebench/x@sha256:" + "a" * 64, text)
        self.assertIn("rm -rf /testbed/.git /testbed/target", text)
        self.assertIn("git commit -qm 'Benchmark base source'", text)
        self.assertIn("CARGO_NET_OFFLINE=true", text)
        self.assertIn('test -z "$(git status --porcelain)"', text)


@unittest.skipUnless(HAVE_GIT, "needs git")
class History(unittest.TestCase):

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="sab-hist-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.repo = self.tmp / "r"
        self.repo.mkdir()
        subprocess.run(["git", "init", "-q", "-b", "main", str(self.repo)], check=True)
        (self.repo / "a.txt").write_text("a\n")
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-qm", "task fx-p01")
        (self.repo / "b.txt").write_text("b\n")
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-qm", "sab: codescout project config")

    def run_in_repo(self, *argv):
        return subprocess.run(list(argv), cwd=self.repo, capture_output=True, check=True).stdout

    def test_a_single_build_chain_passes(self):
        r = tasks.history_report(self.run_in_repo, "task fx-p01")
        self.assertTrue(r["ok"], r)
        self.assertEqual(r["subjects"], ["task fx-p01", "sab: codescout project config"])
        self.assertEqual(r["unreachable_objects"], [])
        self.assertTrue(r["plain_repo"])

    def test_a_foreign_object_fails_the_history_check(self):
        blob = subprocess.run(["git", "-C", str(self.repo), "hash-object", "-w", "--stdin"],
                              input=b"the fix from the future\n", capture_output=True,
                              check=True).stdout.decode().strip()
        r = tasks.history_report(self.run_in_repo, "task fx-p01")
        self.assertFalse(r["ok"])
        self.assertEqual(r["unreachable_objects"], [blob])
    def test_a_foreign_object_behind_an_extra_ref_fails_the_history_check(self):
        # Task 7 review I1: a lightweight tag on a blob made the object "reachable" from --all.
        blob = subprocess.run(["git", "-C", str(self.repo), "hash-object", "-w", "--stdin"],
                              input=b"the fix from the future\n", capture_output=True,
                              check=True).stdout.decode().strip()
        git(self.repo, "tag", "leak", blob)
        r = tasks.history_report(self.run_in_repo, "task fx-p01")
        self.assertFalse(r["ok"])
        self.assertEqual(r["unreachable_objects"], [blob])
        self.assertFalse(r["one_branch_ref"])

    def test_a_second_ref_alone_fails_the_history_check(self):
        git(self.repo, "branch", "other")
        r = tasks.history_report(self.run_in_repo, "task fx-p01")
        self.assertEqual(r["unreachable_objects"], [])
        self.assertFalse(r["one_branch_ref"])
        self.assertFalse(r["ok"])

    def test_a_second_root_or_an_unexpected_subject_fails(self):
        git(self.repo, "commit", "--allow-empty", "-qm", "a later commit")
        self.assertFalse(tasks.history_report(self.run_in_repo, "task fx-p01")["ok"])
        self.assertFalse(tasks.history_report(self.run_in_repo, "Benchmark base source")["ok"])

    def test_a_linked_worktree_is_not_a_plain_repo(self):
        wt = self.tmp / "wt"
        git(self.repo, "worktree", "add", "-q", str(wt))
        r = tasks.history_report(lambda *a: subprocess.run(list(a), cwd=wt, capture_output=True,
                                                           check=True).stdout, "task fx-p01")
        self.assertFalse(r["plain_repo"])
        self.assertFalse(r["ok"])


class BrokenLinks(unittest.TestCase):
    """Live finding (ruff fuzz/corpus): a dangling symlink made the scan ScanIncomplete."""

    def test_dangling_links_become_a_scanned_listing(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td) / "export"
            (root / "d").mkdir(parents=True)
            (root / "d" / "real.txt").write_text("fine\n")
            os.symlink("real.txt", root / "d" / "ok_link")
            os.symlink("../missing/zz_secret_target", root / "d" / "dangling")
            with self.assertRaises(leak_scan.ScanIncomplete):
                leak_scan.scan([root], None, ["zz_secret_target"], allow_unbaselined=True)
            listing = Path(td) / "links.txt"
            found = tasks.unlink_broken_links(root, listing)
            self.assertEqual(found, [("d/dangling", "../missing/zz_secret_target")])
            self.assertTrue((root / "d" / "ok_link").is_symlink())
            report = leak_scan.scan([root, listing], None, ["zz_secret_target"], allow_unbaselined=True)
            self.assertEqual([(h.path, h.needle) for h in report.hits], [(listing, "zz_secret_target")])

def complete_controls(kind="private", task_id="a", **over):
    """A synthetic controls.json that every Step-5 check accepts."""
    base = ({"pass": False, "graft": "ok", "build": "ok", "infra": None,
             "f2p": {"passed": [], "failed": ["t"], "notRun": []}} if kind == "private"
            else {"pass": False, "failure_kind": "test_failure", "failed": ["t"]})
    root = tasks.expected_root_subject(kind, task_id)
    r = {"id": task_id, "kind": kind, "image": "sab-arm-a:1", "image_id": "sha256:arm", "pass": True,
         "leak": {"hits": [], "control": "pass", "planted": 2, "qdrant_points": {"code_chunks": 9},
                  "unreadable_to_agent": [{"path": "/opt/sab/bin/codescout",
                                           "reason": "Cannot open: Permission denied"}],
                  "history": {"ok": True, "plain_repo": True, "single_chain": True,
                              "subjects": [root, tasks.CONFIG_SUBJECT],
                              "refs": ["refs/heads/main"], "unreachable_objects": []}},
         "grader": {"base": base, "gold": {"pass": True}}}
    for path, value in over.items():
        node = r
        *head, last = path.split(".")
        for k in head:
            node = node[k]
        node[last] = value
    return r


class Table(unittest.TestCase):

    def test_a_row_prefers_the_full_control_and_never_reads_pass_from_a_prescan(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            (root / "cs-a" / "prescan").mkdir(parents=True)
            (root / "cs-a" / "bundle.json").write_text(json.dumps({"kind": "private"}))
            (root / "cs-a" / "prescan" / "prescan.json").write_text(json.dumps(
                {"control": "pass", "hits": [{"path": "/x/opt/sab/bin/codescout", "needle": "n", "offset": 1}]}))
            row = tasks._row(root, "cs-a")
            self.assertEqual((row["status"], row["pass"], row["hits"]), ("pre-index scan only", None, 1))
            self.assertTrue(row["binary_postdates_fix"])
            (root / "cs-a" / "controls").mkdir()
            # A stored `pass: true` is not trusted: it is recomputed from the evidence.
            (root / "cs-a" / "controls" / "controls.json").write_text(json.dumps(
                {"pass": True, "kind": "private", "leak": {"hits": [], "control": "pass", "history": {"ok": True}},
                 "grader": {"base_fails": True, "gold_passes": True}}))
            row = tasks._row(root, "cs-a")
            self.assertEqual((row["status"], row["pass"]), ("complete", False))
            (root / "cs-a" / "controls" / "controls.json").write_text(
                json.dumps(complete_controls(task_id="cs-a")))
            row = tasks._row(root, "cs-a")
            self.assertEqual((row["status"], row["pass"], row["hits"], row["reasons"]), ("complete", True, 0, []))
            self.assertIsNone(tasks._row(root, "b")["binary_postdates_fix"])  # kind unknown

    def test_binary_postdates_fix_has_one_definition_by_kind(self):
        self.assertTrue(tasks.binary_postdates_fix("private"))
        self.assertFalse(tasks.binary_postdates_fix("foreign"))
        self.assertIsNone(tasks.binary_postdates_fix(None))


class FakeImages:
    """inspect(image) -> (id, exec-only label) over a dict; a missing image is (None, None)."""

    def __init__(self, images):
        self.images = images

    def __call__(self, image):
        return self.images.get(image, (None, None))


GOOD_IMAGES = {"sab-arm-a:1": ("sha256:arm", "sha256:run"), "sab-run-a:1": ("sha256:run", None)}
GOOD_BUNDLE = {"kind": "private", "arm_image": "sab-arm-a:1", "run_image": "sab-run-a:1"}


class Verify(unittest.TestCase):
    """The gate Task 10 runs (`tasks.py verify`): review I3 and round-2 gaps."""

    def tree(self, td, controls=None, bundle=GOOD_BUNDLE, task_id="a"):
        root = Path(td)
        (root / task_id / "controls").mkdir(parents=True, exist_ok=True)
        if controls is not None:
            (root / task_id / "controls" / "controls.json").write_text(json.dumps(controls))
        if bundle is not None:
            (root / task_id / "bundle.json").write_text(json.dumps(bundle))
        return root

    def test_complete_controls_pass_for_both_kinds(self):
        self.assertEqual(tasks.verify_controls(complete_controls("private"), "a"), [])
        self.assertEqual(tasks.verify_controls(complete_controls("foreign"), "a"), [])

    def test_each_missing_control_is_named(self):
        cases = {
            "leak.hits": [{"path": "x", "needle": "n", "offset": 0}],
            "leak.control": "FAILED",
            "leak.planted": 0,
            "leak.unreadable_to_agent": [{"path": "/testbed/src/fix.rs", "reason": "Cannot open: Permission denied"}],
            "leak.history.refs": ["refs/heads/main", "refs/tags/t"],
            "leak.history.plain_repo": False,
            "leak.history.single_chain": False,
            "leak.history.subjects": ["task a", "a later commit"],
            "leak.history.unreachable_objects": ["abc"],
            "leak.qdrant_points": {},
            "grader.gold": {"pass": False},
            "grader.base": {"pass": False, "graft": "failed", "f2p": {"failed": []}},
        }
        for path, value in cases.items():
            why = tasks.verify_controls(complete_controls(**{path: value}), "a")
            self.assertEqual(len(why), 1, (path, why))
        # The stored `history.ok` is not trusted.
        self.assertEqual(len(tasks.verify_controls(
            complete_controls(**{"leak.history.subjects": ["Benchmark base source"]}), "a")), 1)
        # Evidence for another task (its subjects name that task too) is refused.
        why = tasks.verify_controls(complete_controls(task_id="cs-other"), "a")
        self.assertEqual(why, ["controls.json is for 'cs-other', not 'a'"])

    def test_only_the_exec_only_binary_may_be_unreadable(self):
        allowed = complete_controls()
        self.assertEqual(tasks.verify_controls(allowed, "a"), [])
        # The round-1 string form is read too.
        old = complete_controls(**{"leak.unreadable_to_agent": [
            "opt/sab/bin/codescout: Warning: Cannot open: Permission denied",
            "home/agent/secret: Cannot stat: Permission denied"]})
        why = tasks.verify_controls(old, "a")
        self.assertEqual(len(why), 1)
        self.assertIn("/home/agent/secret", why[0])

    def test_tar_stderr_records_stat_failures_and_rejects_unknown_lines(self):
        err = ("tar: opt/sab/bin/codescout: Warning: Cannot open: Permission denied\n"
               "tar: testbed/locked/fix.rs: Cannot stat: Permission denied\n"
               "tar: Exiting with failure status due to previous errors\n")
        found, unknown = tasks.parse_tar_stderr(err)
        self.assertEqual([f["path"] for f in found], ["/opt/sab/bin/codescout", "/testbed/locked/fix.rs"])
        self.assertEqual(unknown, ["tar: Exiting with failure status due to previous errors"])
        self.assertEqual(tasks.unreadable_not_allowed(found), ["/testbed/locked/fix.rs"])
        self.assertEqual(tasks.parse_tar_stderr("tar: opt/sab/bin/codescout: Warning: Cannot open: "
                                                "Permission denied\n"), (found[:1], []))

    def test_an_allowlisted_path_with_another_reason_is_not_accepted(self):
        for err in ("tar: opt/sab/bin/codescout: Warning: Cannot open: No such file or directory\n",
                    "tar: opt/sab/bin/codescout: Cannot stat: No such file or directory\n",
                    "tar: opt/sab/bin/codescout: file changed as we read it\n"):
            with self.subTest(err=err):
                found, unknown = tasks.parse_tar_stderr(err)
                self.assertEqual((found, unknown), ([], [err.strip()]))
        found, unknown = tasks.parse_tar_stderr(
            "tar: opt/sab/bin/codescout: Warning: Cannot open: Permission denied\n")
        self.assertEqual(([f["path"] for f in found], unknown), (["/opt/sab/bin/codescout"], []))

    def test_controls_must_have_run_on_the_arm_image_not_the_bare_run_image(self):
        self.assertEqual(tasks.verify_controls(complete_controls(), "a", arm_image="sab-arm-a:1"), [])
        why = tasks.verify_controls(complete_controls(image="sab-run-a:1"), "a",
                                    arm_image="sab-arm-a:1")
        self.assertEqual(len(why), 1)
        self.assertIn("sab-arm-a:1", why[0])
        with tempfile.TemporaryDirectory() as td:
            ok = FakeImages(GOOD_IMAGES)
            root = self.tree(td, complete_controls(image="sab-run-a:1", image_id="sha256:run"))
            why = tasks.verify(["a"], root, inspect=ok)["a"]
            self.assertTrue(any("sab-arm-a:1" in w for w in why), why)
        with tempfile.TemporaryDirectory() as td:
            root = self.tree(td, complete_controls())
            self.assertEqual(tasks.verify(["a"], root, inspect=FakeImages(GOOD_IMAGES)), {"a": []})

    def test_verify_checks_ids_images_and_the_selection(self):
        with tempfile.TemporaryDirectory() as td:
            root = self.tree(td, complete_controls())
            ok = FakeImages(GOOD_IMAGES)
            self.assertEqual(tasks.verify(["a"], root, selected=["a"], inspect=ok), {"a": []})
            # Stale evidence: the arm image was rebuilt after the control ran.
            stale = FakeImages(dict(GOOD_IMAGES, **{"sab-arm-a:1": ("sha256:new", "sha256:run")}))
            self.assertIn("stale evidence", tasks.verify(["a"], root, inspect=stale)["a"][0])
            # The arm layer no longer derives from the current run image.
            moved = FakeImages(dict(GOOD_IMAGES, **{"sab-run-a:1": ("sha256:run2", None)}))
            self.assertTrue(any("exec-only layer" in w for w in tasks.verify(["a"], root, inspect=moved)["a"]))
            # A selected id that is not listed fails the gate.
            out = tasks.verify(["a"], root, selected=["a", "b"], inspect=ok)
            self.assertEqual(out["a"], [])
            self.assertIn("selected but not verified", out["b"][0])
            # No controls.json, no arm image.
            self.tree(td, None, task_id="c")
            self.assertIn("no controls.json", tasks.verify(["c"], root, inspect=ok)["c"][0])
            self.tree(td, complete_controls(task_id="d"), bundle={"run_image": "sab-run-a:1"}, task_id="d")
            self.assertTrue(any("arm image" in w for w in tasks.verify(["d"], root, inspect=ok)["d"]))

    def test_the_cli_exits_non_zero_unless_every_selected_task_is_verified(self):
        with tempfile.TemporaryDirectory() as td:
            data = Path(td)
            root = self.tree(data / "tasks", complete_controls())
            (data / "selection").mkdir()
            (data / "selection" / "selection.json").write_text(json.dumps({"selected": ["a", "b"]}))
            manifest = data / "m.json"
            manifest.write_text("{}")
            ok = FakeImages(GOOD_IMAGES)
            real = tasks.verify
            with unittest.mock.patch.object(tasks, "DATA", data), \
                    unittest.mock.patch.object(tasks, "verify", lambda ids, r, **kw: real(ids, r, inspect=ok, **kw)), \
                    contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaises(SystemExit) as cm:
                    tasks.main(["verify", str(manifest), "a"])  # one task cannot pass the gate
                self.assertEqual(cm.exception.code, 1)
                (root / "b" / "controls").mkdir(parents=True)
                (root / "b" / "controls" / "controls.json").write_text(json.dumps(complete_controls(task_id="b")))
                (root / "b" / "bundle.json").write_text(json.dumps(GOOD_BUNDLE))
                tasks.main(["verify", str(manifest)])  # every selected id complete: exit 0

    def test_base_fails_needs_an_observed_failure(self):
        f = tasks.base_fails
        ok = {"pass": False, "graft": "ok", "infra": None}
        self.assertTrue(f("private", dict(ok, f2p={"failed": ["t"]})))
        self.assertTrue(f("private", dict(ok, build="failed", build_compile_error=True, f2p={"failed": []})))
        self.assertFalse(f("private", dict(ok, build="failed", build_compile_error=False, f2p={"failed": []})))
        self.assertFalse(f("private", dict(ok, graft="failed", f2p={"failed": ["t"]})))
        self.assertFalse(f("private", dict(ok, infra="incomplete", f2p={"failed": ["t"]})))
        self.assertFalse(f("private", dict(ok, **{"pass": True}, f2p={"failed": []})))
        self.assertTrue(f("foreign", {"pass": False, "failure_kind": "build_failure"}))
        self.assertFalse(f("foreign", {"pass": False, "error": "unexplained grader exit"}))
        self.assertFalse(f("foreign", {"pass": False, "failure_kind": None}))


class ArmImage(unittest.TestCase):
    """Round-2 carry for Tasks 8/10: a runner can obtain only the exec-only arm image."""

    def test_arm_image_name_needs_a_run_image(self):
        self.assertEqual(tasks.arm_image_name("sab-run-x:1"), "sab-arm-x:1")
        for bad in ("sab-agent-cs-x:1", "x-sab-run-y:1", "sab-arm-x:1"):
            with self.assertRaises(ValueError):
                tasks.arm_image_name(bad)

    def test_bundle_run_image_returns_only_a_verified_arm_image(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "bundle.json"
            path.write_text(json.dumps(GOOD_BUNDLE))
            self.assertEqual(tasks.bundle_run_image(path, FakeImages(GOOD_IMAGES)), "sab-arm-a:1")
            with self.assertRaises(RuntimeError):  # the arm layer is not over the current run image
                tasks.bundle_run_image(path, FakeImages(dict(GOOD_IMAGES, **{"sab-run-a:1": ("sha256:x", None)})))
            with self.assertRaises(RuntimeError):  # the arm image is gone
                tasks.bundle_run_image(path, FakeImages({"sab-run-a:1": ("sha256:run", None)}))
            path.write_text(json.dumps({"run_image": "sab-run-a:1"}))
            with self.assertRaises(RuntimeError):  # never falls back to the run image
                tasks.bundle_run_image(path, FakeImages(GOOD_IMAGES))



@unittest.skipUnless(have_docker_image(tasks.PRIVATE_BASE_IMAGE),
                     f"needs docker and the {tasks.PRIVATE_BASE_IMAGE} image")
class ExportFailsClosed(unittest.TestCase):
    """Review N1, live: a mode-000 file the agent owns is unreadable to tar but one chmod away for
    the agent, so the export must fail; the exec-only binary alone is allowed."""

    def test_only_the_exec_only_binary_may_be_unreadable_in_a_real_export(self):
        tag = f"sab-test-export-{os.getpid()}"
        base, arm = f"{tag}-base:1", f"{tag}-arm:1"
        self.addCleanup(subprocess.run, ["docker", "image", "rm", "-f", base, arm, f"{tag}-bad:1"],
                        capture_output=True)
        with tempfile.TemporaryDirectory() as td:
            ctx = Path(td) / "base"
            ctx.mkdir()
            (ctx / "Dockerfile").write_text(
                f"FROM {tasks.PRIVATE_BASE_IMAGE}\nUSER root\nRUN mkdir -p /opt/sab/bin /testbed "
                "&& cp /bin/true /opt/sab/bin/codescout && echo ok > /testbed/a.txt "
                "&& chown -R 1000:1000 /testbed\nUSER 1000:1000\n")
            subprocess.run(["docker", "build", "--network", "none", "-t", base, str(ctx)],
                           check=True, capture_output=True, timeout=600)
            arms.exec_only_image(base, arm, Path(td) / "layer")
            got = []
            present = tasks.export_tree(arm, ["testbed", "opt/sab"], Path(td) / "ok", unreadable=got)
            self.assertEqual(present, ["testbed", "opt/sab"])
            self.assertEqual([g["path"] for g in got], ["/opt/sab/bin/codescout"])
            bad = Path(td) / "bad"
            bad.mkdir()
            (bad / "Dockerfile").write_text(f"FROM {arm}\nRUN echo fix > /testbed/fix.rs && chmod 000 /testbed/fix.rs\n")
            subprocess.run(["docker", "build", "--network", "none", "-t", f"{tag}-bad:1", str(bad)],
                           check=True, capture_output=True, timeout=600)
            with self.assertRaisesRegex(RuntimeError, "/testbed/fix.rs"):
                tasks.export_tree(f"{tag}-bad:1", ["testbed", "opt/sab"], Path(td) / "bad-out")

class ControlOrder(unittest.TestCase):
    """Task 5 carried minor: the planted shingle must not contain another kept name needle."""

    def test_planted_shingle_and_path_hold_no_kept_name(self):
        name = "zz_hidden_name"
        bad = f"alpha beta {name} gamma delta epsilon zeta eta"
        good = "alpha beta gamma delta epsilon zeta eta theta"
        ns = leak_scan.NeedleSet([name, f"tests/{name}.rs", "tests/other_file.rs", bad, good],
                                 {}, {}, 1, 1)
        ordered = tasks.control_order(ns)
        self.assertEqual(sorted(ordered.needles), sorted(ns.needles))
        first = {k: next(n for n in ordered.needles if leak_scan.kind_of(n) == k)
                 for k in leak_scan.KINDS}
        self.assertEqual(first["shingle"], good)
        self.assertEqual(first["path"], "tests/other_file.rs")
        with tempfile.TemporaryDirectory() as td:
            (Path(td) / "clean.txt").write_text("nothing here\n")
            report = leak_scan.positive_control([Path(td)], ordered, None)
            self.assertEqual(report.hits, [])
            # The unordered set trips the false ControlFailed the review described.
            with self.assertRaises(leak_scan.ControlFailed):
                leak_scan.positive_control([Path(td)], ns, None)


if __name__ == "__main__":
    unittest.main()
