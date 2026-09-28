"""Task 7 tests: the lesson inventory frozen at a commit (spec A1.4).

Every fixture is synthetic -- never the real operator CLAUDE.md content, never this repo's real
trackers/memories. Run with:
    ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_lessons.py -v
"""
import importlib.util
import pathlib
import subprocess
import sys
import tempfile
import unittest

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
MEASURE = REPO_ROOT / "scripts" / "measure"
sys.path.insert(0, str(MEASURE))


def _load(name):
    spec = importlib.util.spec_from_file_location(f"measure_{name}", MEASURE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


lessons = _load("lessons")


# --- git fixture helpers -----------------------------------------------------------------------


def _git(root, *args):
    return subprocess.run(
        ["git", *args], cwd=str(root), check=True, capture_output=True, text=True,
    ).stdout


def _init_repo(root):
    root.mkdir(parents=True, exist_ok=True)
    _git(root, "init", "-q")
    _git(root, "config", "user.name", "t")
    _git(root, "config", "user.email", "t@t")


def _write(root, rel, content):
    p = root / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(content)


def _commit_all(root, message):
    _git(root, "add", "-A")
    _git(root, "commit", "-q", "-m", message)
    return _git(root, "rev-parse", "HEAD").strip()


# --- the brief's two required tests, plus the "working tree vs git show" mutation target --------


class BriefRequiredTests(unittest.TestCase):
    def test_a_lesson_added_after_the_commit_is_absent(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "CLAUDE.md", (
                "## Section One\n\n"
                "- **Alpha lesson bullet with plenty of lead words.** Body one.\n"
            ))
            c1 = _commit_all(root, "c1")
            _write(root, "CLAUDE.md", (
                "## Section One\n\n"
                "- **Alpha lesson bullet with plenty of lead words.** Body one.\n"
                "- **Bravo lesson bullet added only in commit two.** Body two.\n"
            ))
            c2 = _commit_all(root, "c2")

            at_c1 = lessons.lessons_at(root, c1)
            at_c2 = lessons.lessons_at(root, c2)

            claude_c1 = [l for l in at_c1 if l.source == "CLAUDE.md"]
            claude_c2 = [l for l in at_c2 if l.source == "CLAUDE.md"]

            self.assertEqual(len(claude_c1), 1)
            self.assertEqual(len(claude_c2), 2)
            self.assertFalse(any("Bravo" in l.text for l in claude_c1))
            self.assertTrue(any("Bravo" in l.text for l in claude_c2))

    def test_working_tree_edits_after_commit_are_invisible_to_lessons_at(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "CLAUDE.md", (
                "## Section One\n\n"
                "- **Charlie lesson committed and never touched again.** Body.\n"
            ))
            c1 = _commit_all(root, "c1")
            # Working-tree-only edit AFTER the commit, never added or committed.
            _write(root, "CLAUDE.md", (
                "## Section One\n\n"
                "- **Charlie lesson committed and never touched again.** Body.\n"
                "- **Delta lesson written only to disk, never committed.** Body.\n"
            ))

            claude = [l for l in lessons.lessons_at(root, c1) if l.source == "CLAUDE.md"]
            self.assertEqual(len(claude), 1)
            self.assertFalse(any("Delta" in l.text for l in claude))


class UndatedGlobalLessons(unittest.TestCase):
    def test_global_claude_md_lessons_are_undated(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "CLAUDE.md"
            path.write_text(
                "## Personal Section\n\n"
                "- **Echo lesson from a private global config file.** Body.\n"
            )
            found = lessons.undated_lessons(path, op_rule_bodies=[])
            self.assertGreaterEqual(len(found), 1)
            self.assertTrue(all(l.dated is False for l in found))


# --- R79: generated operator-rules block exclusion ----------------------------------------------

_SYNTHETIC_OP_BODY = "Always double check a number before writing it down anywhere."


class R79GeneratedBlockExclusion(unittest.TestCase):
    def test_marked_block_is_excluded_and_the_rest_is_included(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "CLAUDE.md"
            path.write_text(
                "## Personal Notes\n\n"
                "- **Foxtrot personal lesson kept outside any block.** Body one.\n\n"
                "<!-- BEGIN operator-rules (generated from docs/trackers/operator-rules.md "
                "— do not edit) -->\n"
                "<!-- rules: OP-1 -->\n\n"
                f"{_SYNTHETIC_OP_BODY}\n\n"
                "<!-- END operator-rules -->\n"
            )
            # op_rule_bodies=[] so the ONLY mechanism that could exclude the block is the
            # marker detector -- isolates this test from the equality-fallback detector.
            found = lessons.undated_lessons(path, op_rule_bodies=[])
            texts = [l.text for l in found]
            self.assertTrue(any("Foxtrot" in t for t in texts))
            self.assertFalse(any(_SYNTHETIC_OP_BODY in t for t in texts))
            self.assertTrue(all(l.dated is False for l in found))

    def test_unmarked_text_equal_to_an_op_rule_body_is_excluded(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "CLAUDE.md"
            path.write_text(
                "## Personal Notes\n\n"
                "- **Golf personal lesson kept outside any block.** Body one.\n\n"
                "## Duplicated Operator Text\n\n"
                f"{_SYNTHETIC_OP_BODY}\n"
            )
            found = lessons.undated_lessons(path, op_rule_bodies=[_SYNTHETIC_OP_BODY])
            texts = [l.text for l in found]
            self.assertTrue(any("Golf" in t for t in texts))
            self.assertFalse(any(_SYNTHETIC_OP_BODY in t for t in texts))

    def test_unmarked_text_that_merely_resembles_an_op_rule_body_is_included(self):
        resembling = _SYNTHETIC_OP_BODY.replace("Always", "Usually")
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "CLAUDE.md"
            path.write_text(
                "## Personal Notes\n\n"
                "- **Hotel personal lesson kept outside any block.** Body one.\n\n"
                "## Nearly Duplicated Operator Text\n\n"
                f"{resembling}\n"
            )
            found = lessons.undated_lessons(path, op_rule_bodies=[_SYNTHETIC_OP_BODY])
            texts = [l.text for l in found]
            self.assertTrue(any(resembling in t for t in texts))


# --- R80: CLAUDE.md bullet id stability + collisions --------------------------------------------


class R80BulletIdentity(unittest.TestCase):
    @staticmethod
    def _find_by_text(found, needle):
        for l in found:
            if l.source == "CLAUDE.md" and needle in l.text:
                return l.id
        return None

    def test_inserting_a_bullet_before_an_existing_one_leaves_its_id_unchanged(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "CLAUDE.md", (
                "## Stability Section\n\n"
                "- **India lesson that must keep its identity forever.** Body.\n"
            ))
            c1 = _commit_all(root, "c1")
            _write(root, "CLAUDE.md", (
                "## Stability Section\n\n"
                "- **Juliet lesson inserted before the stable one now.** New body.\n"
                "- **India lesson that must keep its identity forever.** Body.\n"
            ))
            c2 = _commit_all(root, "c2")

            id_c1 = self._find_by_text(lessons.lessons_at(root, c1), "India")
            id_c2 = self._find_by_text(lessons.lessons_at(root, c2), "India")
            self.assertIsNotNone(id_c1)
            self.assertEqual(id_c1, id_c2)

    def test_a_colliding_lead_gets_dash_2(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "CLAUDE.md", (
                "## Collision Section\n\n"
                "- **Kilo lesson lead words that collide right here.** First tail.\n"
                "- **Kilo lesson lead words that collide right here too.** Second tail.\n"
            ))
            c1 = _commit_all(root, "c1")

            found = [l for l in lessons.lessons_at(root, c1) if l.source == "CLAUDE.md"]
            ids = sorted(l.id for l in found)
            self.assertEqual(len(ids), 2)
            twos = [i for i in ids if i.endswith("-2")]
            bases = [i for i in ids if not i.endswith("-2")]
            self.assertEqual(len(twos), 1)
            self.assertEqual(len(bases), 1)
            self.assertEqual(twos[0], bases[0] + "-2")


# --- R81: operator-rules.md index-table status gates the section ------------------------------


class R81OperatorRuleActivation(unittest.TestCase):
    @staticmethod
    def _op_rules_text(status_line):
        return (
            "## Index\n\n"
            "| ID | Binding | Covers | Evidence | Status |\n"
            "|---|---|---|---|---|\n"
            f"| OP-1 | always | synthetic-check | measured: n/a | {status_line} |\n\n"
            "## OP-1 — Synthetic operator rule\n\n"
            "Body text for the synthetic OP-1 rule, used only in this test.\n"
        )

    def test_active_at_c1_and_retired_at_c2_toggles_presence(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md", self._op_rules_text("active"))
            c1 = _commit_all(root, "c1")
            _write(root, "docs/trackers/operator-rules.md", self._op_rules_text("**retired**"))
            c2 = _commit_all(root, "c2")

            ids_c1 = {l.id for l in lessons.lessons_at(root, c1) if l.source == "operator-rules.md"}
            ids_c2 = {l.id for l in lessons.lessons_at(root, c2) if l.source == "operator-rules.md"}
            self.assertIn("operator-rules.md#OP-1", ids_c1)
            self.assertNotIn("operator-rules.md#OP-1", ids_c2)

    def test_a_section_with_no_index_row_is_absent(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md", (
                "## Index\n\n"
                "| ID | Binding | Covers | Evidence | Status |\n"
                "|---|---|---|---|---|\n"
                "| OP-1 | always | synthetic-check | measured: n/a | active |\n\n"
                "## OP-1 — Synthetic operator rule\n\n"
                "Body text.\n\n"
                "## OP-2 — A rule with no index row at all\n\n"
                "This section exists but the table above has no OP-2 row.\n"
            ))
            c1 = _commit_all(root, "c1")
            ids = {l.id for l in lessons.lessons_at(root, c1) if l.source == "operator-rules.md"}
            self.assertIn("operator-rules.md#OP-1", ids)
            self.assertNotIn("operator-rules.md#OP-2", ids)


# --- R82: R-N / T-N promoted-status exact set ---------------------------------------------------


class R82PromotedStatusExactSet(unittest.TestCase):
    def test_only_promoted_and_promoted_to_permanent_docs_count(self):
        recon_text = (
            "## R-1 — case promoted\n\n"
            "**Status:** promoted — landed in SKILL.md.\n\n"
            "## R-2 — case promoted to permanent docs\n\n"
            "**Status:** promoted-to-permanent-docs — landed in CLAUDE.md.\n\n"
            "## R-3 — case promote-when\n\n"
            "**Status:** promote-when a second instance is seen.\n\n"
            "## R-4 — case validated\n\n"
            "**Status:** validated — one datapoint so far.\n\n"
            "## R-5 — case open\n\n"
            "**Status:** open — nothing decided yet.\n\n"
            "## History\n\n"
            "Some closing prose that must not be mistaken for an entry.\n"
        )
        tool_text = (
            "## Some Observations\n\n"
            "### T-1 — case promoted\n\n"
            "**Status:** promoted — folded into a skill.\n\n"
            "### T-2 — case open\n\n"
            "**Status:** open — still pending.\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/reconnaissance-patterns.md", recon_text)
            _write(root, "docs/trackers/tool-usage-patterns.md", tool_text)
            c1 = _commit_all(root, "c1")

            found = lessons.lessons_at(root, c1)
            recon = [l for l in found if l.source == "reconnaissance-patterns.md"]
            tool = [l for l in found if l.source == "tool-usage-patterns.md"]

            self.assertEqual(
                {l.id for l in recon},
                {"reconnaissance-patterns.md#R-1", "reconnaissance-patterns.md#R-2"},
            )
            self.assertEqual({l.id for l in tool}, {"tool-usage-patterns.md#T-1"})


# --- R83: tracked .codescout/memories/**/*.md ---------------------------------------------------


class R83MemoryFiles(unittest.TestCase):
    def test_tracked_memory_with_two_sections_gives_two_lessons_and_subdir_included(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, ".codescout/memories/two-section.md", (
                "## First Section\n\nContent one.\n\n"
                "## Second Section\n\nContent two.\n"
            ))
            _write(root, ".codescout/memories/sub/dir-note.md", (
                "## Only Section\n\nSubdir content.\n"
            ))
            c1 = _commit_all(root, "c1")

            found = [
                l for l in lessons.lessons_at(root, c1)
                if l.source.startswith(".codescout/memories/")
            ]
            ids = {l.id for l in found}
            self.assertIn("memory:two-section#first-section", ids)
            self.assertIn("memory:two-section#second-section", ids)
            self.assertIn("memory:sub/dir-note#only-section", ids)

    def test_a_memory_file_with_no_heading_is_one_lesson(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, ".codescout/memories/plain.md", "Languages: markdown\nRoot: .\n")
            c1 = _commit_all(root, "c1")
            found = [
                l for l in lessons.lessons_at(root, c1)
                if l.source == ".codescout/memories/plain.md"
            ]
            self.assertEqual(len(found), 1)
            self.assertEqual(found[0].id, "memory:plain")

    def test_an_untracked_memory_file_is_absent(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, ".codescout/memories/tracked.md", "## Kept\n\nTracked content.\n")
            c1 = _commit_all(root, "c1")
            # Written to disk AFTER the commit, never git-added or committed.
            _write(root, ".codescout/memories/untracked.md", "## Ghost\n\nShould never appear.\n")

            found = lessons.lessons_at(root, c1)
            ids = {l.id for l in found}
            self.assertIn("memory:tracked#kept", ids)
            self.assertNotIn("memory:untracked#ghost", ids)

    def test_an_anchors_toml_file_is_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, ".codescout/memories/tracked.md", "## Kept\n\nTracked content.\n")
            _write(root, ".codescout/memories/tracked.anchors.toml", "[anchors]\nfoo = 1\n")
            c1 = _commit_all(root, "c1")

            found = lessons.lessons_at(root, c1)
            self.assertFalse(any(l.source.endswith(".anchors.toml") for l in found))
            self.assertFalse(any("anchors" in l.id for l in found))


if __name__ == "__main__":
    unittest.main()
