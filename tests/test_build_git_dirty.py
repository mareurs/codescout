"""Execute the real build.rs against isolated Git fixtures; no Cargo dependency build."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

BUILD_RS = Path(os.environ.get("BUILD_RS", Path(__file__).resolve().parents[1] / "build.rs"))
SOURCE_PATHS = [
    "build.rs", "Cargo.toml", "Cargo.lock", "src/lib.rs",
    "crates/demo/src/lib.rs", "scripts/attribute-red.py",
    "docs/PROGRESSIVE_DISCOVERABILITY.md", "docs/trackers/operator-rules.md",
]


class BuildGitDirtyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.compiled = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.compiled.cleanup)
        cls.binary = Path(cls.compiled.name) / "build-probe"
        result = subprocess.run(
            ["rustc", "--edition=2021", "--crate-name", "build_probe",
             str(BUILD_RS), "-o", str(cls.binary)],
            capture_output=True, text=True, timeout=60)
        if result.returncode:
            raise AssertionError("Actual build.rs failed to compile: " + result.stderr)

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name) / "repo with spaces"
        self.root.mkdir()
        self.env = os.environ.copy()
        for key in ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"):
            self.env.pop(key, None)
        self.env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)
        files = {path: "original\n" for path in SOURCE_PATHS}
        files["build.rs"] = BUILD_RS.read_text()
        files["src/prompts/source.md"] = (
            "<!-- @surface server_instructions -->\nSERVER\n<!-- @end -->\n"
            "<!-- @surface onboarding_prompt -->\nONBOARD\n<!-- @end -->\n")
        files["docs/notes.md"] = "notes\n"
        files["docs/trackers/session-log.md"] = "session\n"
        files[".codescout/audit/host-month.jsonl"] = "{}\n"
        for path, content in files.items():
            self.write(path, content)
        self.git("init", "-q")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "user.name", "Fixture")
        self.git("add", "-A")
        self.git("commit", "-qm", "clean fixture")

    def write(self, rel, text):
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def git(self, *args):
        return subprocess.run(
            ["git", "-c", "core.hooksPath=" + os.devnull,
             "-c", "commit.gpgsign=false", *args],
            cwd=self.root, env=self.env, capture_output=True, text=True,
            check=True, timeout=15).stdout.strip()

    def metadata(self, cwd=None, **env_extra):
        out = Path(self.tmp.name) / "generated"
        out.mkdir(exist_ok=True)
        env = {**self.env, "CARGO_MANIFEST_DIR": str(self.root), "OUT_DIR": str(out), **env_extra}
        result = subprocess.run([str(self.binary)], cwd=cwd or self.root, env=env,
                                capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((out / "server_instructions.md").read_text(), "SERVER\n")
        self.assertEqual((out / "onboarding_prompt.md").read_text(), "ONBOARD\n")
        return dict(line.removeprefix("cargo:rustc-env=").split("=", 1)
                    for line in result.stdout.splitlines()
                    if line.startswith("cargo:rustc-env="))

    def watched(self, root=None):
        """Absolute paths build.rs asks Cargo to watch for the git state (resolved)."""
        root = root or self.root
        out = Path(self.tmp.name) / "generated-watch"
        out.mkdir(exist_ok=True)
        env = {**self.env, "CARGO_MANIFEST_DIR": str(root), "OUT_DIR": str(out)}
        result = subprocess.run([str(self.binary)], cwd=root, env=env,
                                capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        paths = (Path(line.removeprefix("cargo:rerun-if-changed="))
                 for line in result.stdout.splitlines()
                 if line.startswith("cargo:rerun-if-changed="))
        return {p.resolve() for p in paths if p.is_absolute()}

    def git_path(self, name, root=None):
        """Where git keeps `name` for the checkout at `root`, resolved."""
        root = root or self.root
        out = subprocess.run(
            ["git", "-C", str(root), "rev-parse", "--path-format=absolute", "--git-path", name],
            env=self.env, capture_output=True, text=True, check=True, timeout=15).stdout.strip()
        return Path(out).resolve()

    def linked(self, *worktree_args):
        other = Path(self.tmp.name) / "linked"
        self.git("worktree", "add", "-q", *worktree_args[:-1], str(other), worktree_args[-1])
        return other

    def test_linked_worktree_watches_only_its_own_branch_ref(self):
        other = self.linked("-b", "feature-a", "HEAD")
        self.assertEqual(self.watched(other), {
            self.git_path("HEAD", other),
            self.git_path("index", other),
            self.git_path("refs/heads/feature-a", other),
        })

    def test_detached_head_watches_only_head_and_index(self):
        other = self.linked("--detach", "HEAD")
        self.assertEqual(self.watched(other),
                         {self.git_path("HEAD", other), self.git_path("index", other)})

    def test_packed_branch_watches_packed_refs_not_the_whole_refs_tree(self):
        self.git("pack-refs", "--all", "--prune")
        branch = self.git("symbolic-ref", "HEAD")
        self.assertFalse(self.git_path(branch).exists(), "fixture must have a packed branch")
        # `refs/heads` is where the next commit creates the loose ref; nothing wider.
        self.assertEqual(self.watched(), {
            self.git_path("HEAD"),
            self.git_path("index"),
            self.git_path("packed-refs"),
            self.git_path("refs/heads"),
        })

    def test_unborn_branch_watches_where_its_first_commit_lands(self):
        fresh = Path(self.tmp.name) / "fresh"
        fresh.mkdir()
        (fresh / "src/prompts").mkdir(parents=True)
        (fresh / "src/prompts/source.md").write_text(
            "<!-- @surface server_instructions -->\nSERVER\n<!-- @end -->\n"
            "<!-- @surface onboarding_prompt -->\nONBOARD\n<!-- @end -->\n")
        subprocess.run(["git", "init", "-q", str(fresh)], env=self.env, check=True,
                       capture_output=True, timeout=15)
        branch = subprocess.run(["git", "-C", str(fresh), "symbolic-ref", "HEAD"], env=self.env,
                                capture_output=True, text=True, check=True).stdout.strip()
        loose = self.git_path(branch, fresh)
        self.assertFalse(loose.exists())
        # The first commit creates `loose` under `refs/heads`; no index or packed-refs yet.
        self.assertEqual(self.watched(fresh), {
            self.git_path("HEAD", fresh),
            self.git_path("refs/heads", fresh),
        })

    def test_commit_on_checked_out_branch_changes_baked_sha_and_the_watched_ref(self):
        before = self.metadata()["CODESCOUT_GIT_SHA_FULL"]
        self.write("src/lib.rs", "second\n")
        self.git("commit", "-qam", "second")
        after = self.metadata()["CODESCOUT_GIT_SHA_FULL"]
        self.assertNotEqual(before, after)
        self.assertEqual(after, self.git("rev-parse", "HEAD"))
        branch = self.git("symbolic-ref", "HEAD")
        ref_file = self.git_path(branch)
        self.assertIn(ref_file, self.watched())
        self.assertEqual(ref_file.read_text().strip(), after)

    def test_clean_fixture_preserves_git_identity(self):
        data = self.metadata()
        self.assertEqual(data["CODESCOUT_GIT_DIRTY"], "0")
        self.assertEqual(data["CODESCOUT_GIT_SHA"], self.git("rev-parse", "--short", "HEAD"))
        self.assertEqual(data["CODESCOUT_GIT_SHA_FULL"], self.git("rev-parse", "HEAD"))

    def test_untracked_non_source_scratch_does_not_mark_dirty(self):
        self.write("scratch/untracked.txt", "scratch")
        self.write("docs/new-notes.md", "notes")
        self.assertEqual(self.metadata()["CODESCOUT_GIT_DIRTY"], "0")

    def test_tracked_notes_and_runtime_audit_do_not_mark_dirty(self):
        for path in ("docs/notes.md", "docs/trackers/session-log.md",
                     ".codescout/audit/host-month.jsonl"):
            self.write(path, "changed\n")
        self.assertEqual(self.metadata()["CODESCOUT_GIT_DIRTY"], "0")

    def test_staged_non_source_notes_do_not_mark_dirty(self):
        self.write("docs/notes.md", "changed\n")
        self.git("add", "docs/notes.md")
        self.assertEqual(self.metadata()["CODESCOUT_GIT_DIRTY"], "0")

    def test_each_source_class_marks_dirty(self):
        for path in SOURCE_PATHS:
            with self.subTest(path=path):
                previous = (self.root / path).read_text()
                self.write(path, previous + "\n// changed\n")
                self.assertEqual(self.metadata()["CODESCOUT_GIT_DIRTY"], "1")
                self.write(path, previous)

    def test_staged_source_marks_dirty(self):
        self.write("src/lib.rs", "changed\n")
        self.git("add", "src/lib.rs")
        self.assertEqual(self.metadata()["CODESCOUT_GIT_DIRTY"], "1")

    def test_index_change_is_detected_when_worktree_content_matches_head(self):
        self.write("src/lib.rs", "staged\n")
        self.git("add", "src/lib.rs")
        self.write("src/lib.rs", "original\n")
        self.assertEqual(self.metadata()["CODESCOUT_GIT_DIRTY"], "1")

    def test_new_untracked_source_marks_dirty(self):
        self.write("src/new_module.rs", "// new\n")
        self.assertEqual(self.metadata()["CODESCOUT_GIT_DIRTY"], "1")

    def test_deleted_source_input_marks_dirty(self):
        (self.root / "Cargo.lock").unlink()
        self.assertEqual(self.metadata()["CODESCOUT_GIT_DIRTY"], "1")

    def test_source_renamed_outside_scope_marks_dirty(self):
        self.git("mv", "src/lib.rs", "docs/old-lib.rs")
        self.assertEqual(self.metadata()["CODESCOUT_GIT_DIRTY"], "1")

    def test_manifest_root_wins_over_unrelated_process_cwd(self):
        other = Path(self.tmp.name) / "outside"
        other.mkdir()
        data = self.metadata(cwd=other)
        self.assertEqual(data["CODESCOUT_GIT_DIRTY"], "0")
        self.assertEqual(data["CODESCOUT_GIT_SHA"], self.git("rev-parse", "--short", "HEAD"))
        self.assertEqual(data["CODESCOUT_GIT_SHA_FULL"], self.git("rev-parse", "HEAD"))

    def test_manifest_source_changes_detected_from_other_cwd(self):
        self.write("src/lib.rs", "changed\n")
        other = Path(self.tmp.name) / "outside"
        other.mkdir()
        self.assertEqual(self.metadata(cwd=other)["CODESCOUT_GIT_DIRTY"], "1")

    def test_git_unavailable_is_unknown_not_clean(self):
        empty = Path(self.tmp.name) / "empty-path"
        empty.mkdir()
        data = self.metadata(PATH=str(empty))
        self.assertEqual(data["CODESCOUT_GIT_DIRTY"], "unknown")
        self.assertEqual(data["CODESCOUT_GIT_SHA"], "unknown")
        self.assertEqual(data["CODESCOUT_GIT_SHA_FULL"], "unknown")

    def test_non_repository_manifest_is_unknown(self):
        shutil.rmtree(self.root / ".git")
        data = self.metadata()
        self.assertEqual(data["CODESCOUT_GIT_DIRTY"], "unknown")
        self.assertEqual(data["CODESCOUT_GIT_SHA"], "unknown")
        self.assertEqual(data["CODESCOUT_GIT_SHA_FULL"], "unknown")
    def test_linked_worktree_uses_its_manifest_status(self):
        other = Path(self.tmp.name) / "linked"
        self.git("worktree", "add", "--detach", str(other), "HEAD")
        self.root = other
        self.write("src/lib.rs", "// changed in linked tree\n")
        self.assertEqual(self.metadata()["CODESCOUT_GIT_DIRTY"], "1")

    def test_rename_from_notes_into_source_is_dirty(self):
        self.git("mv", "docs/notes.md", "src/from-notes.rs")
        self.assertEqual(self.metadata()["CODESCOUT_GIT_DIRTY"], "1")

    def test_failed_git_with_plausible_output_is_unknown(self):
        fake = Path(self.tmp.name) / "fake-bin"
        fake.mkdir()
        git = fake / "git"
        git.write_text("#!/bin/sh\nprintf ' M src/lib.rs\\n'\nexit 42\n")
        git.chmod(0o755)
        data = self.metadata(PATH=str(fake))
        self.assertEqual(data["CODESCOUT_GIT_DIRTY"], "unknown")
        self.assertEqual(data["CODESCOUT_GIT_SHA"], "unknown")
        self.assertEqual(data["CODESCOUT_GIT_SHA_FULL"], "unknown")

    def test_non_utf8_source_filename_marks_dirty(self):
        if os.name != "posix":
            self.skipTest("Unix byte filenames")
        self.git("config", "core.quotePath", "false")
        path = os.fsencode(self.root) + b"/src/raw\xff.rs"
        with open(path, "wb") as stream:
            stream.write(b"original\n")
        self.assertEqual(self.metadata()["CODESCOUT_GIT_DIRTY"], "1")
        self.git("add", "-A")
        self.git("commit", "-qm", "track raw filename")
        with open(path, "ab") as stream:
            stream.write(b"changed\n")
        self.assertEqual(self.metadata()["CODESCOUT_GIT_DIRTY"], "1")

    def foreign_clone(self):
        other = Path(self.tmp.name) / "foreign"
        self.git("clone", "-q", str(self.root), str(other))
        return other

    def assert_manifest_identity(self, data):
        self.assertEqual(data["CODESCOUT_GIT_SHA"], self.git("rev-parse", "--short", "HEAD"))
        self.assertEqual(data["CODESCOUT_GIT_SHA_FULL"], self.git("rev-parse", "HEAD"))

    def test_git_dir_cannot_redirect_manifest(self):
        other = self.foreign_clone()
        self.git("--git-dir=" + str(other / ".git"), "config", "core.worktree", str(other))
        self.write("src/lib.rs", "changed\n")
        data = self.metadata(GIT_DIR=str(other / ".git"))
        self.assertEqual(data["CODESCOUT_GIT_DIRTY"], "1")
        self.assert_manifest_identity(data)

    def test_git_work_tree_cannot_redirect_manifest(self):
        other = self.foreign_clone()
        self.write("src/lib.rs", "changed\n")
        data = self.metadata(GIT_WORK_TREE=str(other))
        self.assertEqual(data["CODESCOUT_GIT_DIRTY"], "1")
        self.assert_manifest_identity(data)

    def test_git_index_file_cannot_hide_staged_source(self):
        alternate = Path(self.tmp.name) / "clean-index"
        shutil.copyfile(self.root / ".git/index", alternate)
        self.write("src/lib.rs", "staged\n")
        self.git("add", "src/lib.rs")
        self.write("src/lib.rs", "original\n")
        data = self.metadata(GIT_INDEX_FILE=str(alternate))
        self.assertEqual(data["CODESCOUT_GIT_DIRTY"], "1")
        self.assert_manifest_identity(data)

    def test_git_common_dir_cannot_redirect_identity(self):
        other = self.foreign_clone()
        self.git("--git-dir=" + str(other / ".git"), "--work-tree=" + str(other),
                 "-c", "user.name=Foreign", "-c", "user.email=foreign@example.invalid",
                 "commit", "--allow-empty", "-qm", "foreign revision")
        data = self.metadata(GIT_COMMON_DIR=str(other / ".git"))
        self.assert_manifest_identity(data)
        self.assertEqual(data["CODESCOUT_GIT_DIRTY"], "0")

    def cargo_run(self):
        target = Path(self.tmp.name) / "cargo-target"
        env = {**self.env, "CARGO_TARGET_DIR": str(target)}
        result = subprocess.run(
            ["cargo", "run", "--offline", "--quiet"], cwd=self.root,
            env=env, capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.cargo_build_output_mtime = next(target.glob("debug/build/*/output")).stat().st_mtime_ns
        return result.stdout.strip()

    def check_incremental_metadata(self, linked=False):
        self.write("Cargo.toml", '[package]\nname = "dirty-fixture"\nversion = "0.1.0"\nedition = "2021"\n')
        self.write("src/lib.rs", "// fixture library\n")
        main = ('fn main() { println!("v1|{}|{}|{}", env!("CODESCOUT_GIT_DIRTY"), '
                'env!("CODESCOUT_GIT_SHA"), env!("CODESCOUT_GIT_SHA_FULL")); }\n')
        self.write("src/main.rs", main)
        self.git("rm", "-q", "Cargo.lock")
        subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.root,
                       env=self.env, check=True, capture_output=True, timeout=60)
        self.git("add", "-A")
        self.git("commit", "-qm", "Cargo fixture")
        if linked:
            destination = Path(self.tmp.name) / "linked"
            self.git("worktree", "add", "-qb", "linked", str(destination))
            self.root = destination
        expected = "v1|0|" + self.git("rev-parse", "--short", "HEAD") + "|" + self.git("rev-parse", "HEAD")
        self.assertEqual(self.cargo_run(), expected)
        self.assertEqual(self.cargo_run(), expected)
        warm_output_mtime = self.cargo_build_output_mtime
        self.assertEqual(self.cargo_run(), expected)
        self.assertEqual(self.cargo_build_output_mtime, warm_output_mtime, "warm build reran metadata")
        self.write("src/main.rs", main.replace("v1|", "v2|"))
        self.assertEqual(self.cargo_run(), expected.replace("v1|0|", "v2|1|"))
        self.git("add", "src/main.rs")
        self.git("commit", "-qm", "new source revision")
        committed = "v2|0|" + self.git("rev-parse", "--short", "HEAD") + "|" + self.git("rev-parse", "HEAD")
        self.assertEqual(self.cargo_run(), committed)
        self.assertEqual(self.cargo_run(), committed)
        self.write("scripts/new-script.py", "# new source input\n")
        self.assertEqual(self.cargo_run(), committed.replace("v2|0|", "v2|1|"))
        (self.root / "scripts/new-script.py").unlink()
        self.assertEqual(self.cargo_run(), committed)
        self.write("docs/PROGRESSIVE_DISCOVERABILITY.md", "changed embedded input\n")
        self.assertEqual(self.cargo_run(), committed.replace("v2|0|", "v2|1|"))

    def test_cargo_incremental_metadata(self):
        self.check_incremental_metadata()
        self.check_git_admin_and_scope_watches()

    def test_cargo_incremental_metadata_in_linked_worktree(self):
        self.check_incremental_metadata(linked=True)
        self.check_git_admin_and_scope_watches()

    def check_git_admin_and_scope_watches(self):
        self.write("docs/PROGRESSIVE_DISCOVERABILITY.md", "original\n")
        def expected(dirty="0"):
            return "v2|" + dirty + "|" + self.git("rev-parse", "--short", "HEAD") + "|" + self.git("rev-parse", "HEAD")
        self.assertEqual(self.cargo_run(), expected())
        for path in ("src/new.rs", "crates/new.rs", "scripts/new.py",
                     "docs/PROGRESSIVE_DISCOVERABILITY.md", "docs/trackers/operator-rules.md"):
            with self.subTest(incremental_path=path):
                full = self.root / path
                before = full.read_bytes() if full.exists() else None
                full.write_text("changed input\n")
                self.assertEqual(self.cargo_run(), expected("1"))
                if before is None:
                    full.unlink()
                else:
                    full.write_bytes(before)
                self.assertEqual(self.cargo_run(), expected())
        # Modify only the index: neither source bytes nor directory mtimes change.
        blob_file = Path(self.tmp.name) / "staged-blob"
        blob_file.write_text("index-only change\n")
        blob = self.git("hash-object", "-w", str(blob_file))
        self.git("update-index", "--cacheinfo", "100644," + blob + ",src/lib.rs")
        self.assertEqual(self.cargo_run(), expected("1"))
        self.git("reset", "-q", "HEAD")
        self.assertEqual(self.cargo_run(), expected())
        # This update creates a loose branch ref and must invalidate the refs watch.
        self.git("pack-refs", "--all", "--prune")
        self.assertEqual(self.cargo_run(), expected())
        revision = self.git("commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", "ref-only revision")
        self.git("update-ref", "HEAD", revision)
        self.assertEqual(self.cargo_run(), expected())
        # Change only an existing packed ref; update-ref would instead create a loose ref.
        self.git("pack-refs", "--all", "--prune")
        self.assertEqual(self.cargo_run(), expected())
        self.assertEqual(self.cargo_run(), expected())
        old = self.git("rev-parse", "HEAD")
        revision = self.git("commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", "packed revision")
        packed = self.root / self.git("rev-parse", "--git-path", "packed-refs")
        before = packed.read_text()
        self.assertIn(old, before)
        packed.write_text(before.replace(old, revision))
        self.assertEqual(self.cargo_run(), expected())
        # Detach HEAD directly, leaving source, index and branch refs untouched.
        self.assertEqual(self.cargo_run(), expected())
        revision = self.git("commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", "detached revision")
        self.git("update-ref", "--no-deref", "HEAD", revision)
        self.assertEqual(self.cargo_run(), expected())
        self.assertEqual(self.cargo_run(), expected())
        last_output_mtime = self.cargo_build_output_mtime
        self.assertEqual(self.cargo_run(), expected())
        self.assertEqual(self.cargo_build_output_mtime, last_output_mtime)


if __name__ == "__main__":
    unittest.main()
