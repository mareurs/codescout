"""Task 7 tests: the lesson inventory frozen at a commit (spec A1.4).

Every fixture is synthetic -- never the real operator CLAUDE.md content, never this repo's real
trackers/memories. Run with:
    ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_lessons.py -v
"""
import dataclasses
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


def _op_rules_with_imperative(imperative_text=None, op_id="OP-1"):
    """A minimal synthetic docs/trackers/operator-rules.md fixture for R84 tests: an index-table
    row plus one `## OP-N` section carrying Imperative/Binding/Evidence fields. When
    `imperative_text` is None, the file exists (so `_op_imperative_bodies` does not raise) but
    declares no OP sections at all -- an empty reference set.
    """
    if imperative_text is None:
        return "## Index\n\n| ID | Status |\n|---|---|\n"
    return (
        "## Index\n\n"
        "| ID | Status |\n"
        "|---|---|\n"
        f"| {op_id} | active |\n\n"
        f"## {op_id} — Synthetic operator rule\n\n"
        f"**Imperative:** {imperative_text}\n\n"
        "**Binding:** always\n\n"
        "**Evidence:** measured: n/a\n"
    )


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

            claude_c1 = [l for l in at_c1 if l.id.startswith("CLAUDE.md#")]
            claude_c2 = [l for l in at_c2 if l.id.startswith("CLAUDE.md#")]

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

            claude = [l for l in lessons.lessons_at(root, c1) if l.id.startswith("CLAUDE.md#")]
            self.assertEqual(len(claude), 1)
            self.assertFalse(any("Delta" in l.text for l in claude))


class UndatedGlobalLessons(unittest.TestCase):
    def test_global_claude_md_lessons_are_undated(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md", _op_rules_with_imperative(None))
            c1 = _commit_all(root, "c1")
            path = root / "CLAUDE.md"
            path.write_text(
                "## Personal Section\n\n"
                "- **Echo lesson from a private global config file.** Body.\n"
            )
            found = lessons.undated_lessons(path, root, c1)
            self.assertGreaterEqual(len(found), 1)
            self.assertTrue(all(l.dated is False for l in found))


# --- R79: generated operator-rules block exclusion ----------------------------------------------

_SYNTHETIC_OP_BODY = "Always double check a number before writing it down anywhere."


class R79GeneratedBlockExclusion(unittest.TestCase):
    def test_marked_block_is_excluded_and_the_rest_is_included(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            # No OP sections here -- isolates this test from the equality-fallback detector so
            # only the marker detector can be responsible for the exclusion.
            _write(root, "docs/trackers/operator-rules.md", _op_rules_with_imperative(None))
            c1 = _commit_all(root, "c1")
            path = root / "CLAUDE.md"
            path.write_text(
                "## Personal Notes\n\n"
                "- **Foxtrot personal lesson kept outside any block.** Body one.\n\n"
                "<!-- BEGIN operator-rules (generated from docs/trackers/operator-rules.md "
                "— do not edit) -->\n"
                "<!-- rules: OP-1 -->\n\n"
                f"{_SYNTHETIC_OP_BODY}\n\n"
                "<!-- END operator-rules -->\n"
            )
            found = lessons.undated_lessons(path, root, c1)
            texts = [l.text for l in found]
            self.assertTrue(any("Foxtrot" in t for t in texts))
            self.assertFalse(any(_SYNTHETIC_OP_BODY in t for t in texts))
            self.assertTrue(all(l.dated is False for l in found))

    def test_unmarked_text_equal_to_an_op_rule_imperative_is_excluded(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md",
                   _op_rules_with_imperative(_SYNTHETIC_OP_BODY))
            c1 = _commit_all(root, "c1")
            path = root / "CLAUDE.md"
            path.write_text(
                "## Personal Notes\n\n"
                "- **Golf personal lesson kept outside any block.** Body one.\n\n"
                "## Duplicated Operator Text\n\n"
                f"{_SYNTHETIC_OP_BODY}\n"
            )
            found = lessons.undated_lessons(path, root, c1)
            texts = [l.text for l in found]
            self.assertTrue(any("Golf" in t for t in texts))
            self.assertFalse(any(_SYNTHETIC_OP_BODY in t for t in texts))

    def test_unmarked_text_that_merely_resembles_an_op_rule_imperative_is_included(self):
        resembling = _SYNTHETIC_OP_BODY.replace("Always", "Usually")
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md",
                   _op_rules_with_imperative(_SYNTHETIC_OP_BODY))
            c1 = _commit_all(root, "c1")
            path = root / "CLAUDE.md"
            path.write_text(
                "## Personal Notes\n\n"
                "- **Hotel personal lesson kept outside any block.** Body one.\n\n"
                "## Nearly Duplicated Operator Text\n\n"
                f"{resembling}\n"
            )
            found = lessons.undated_lessons(path, root, c1)
            texts = [l.text for l in found]
            self.assertTrue(any(resembling in t for t in texts))


# --- R84: the Imperative-line reference set, derived from the production path -------------------


class R84ImperativeLineReferenceSet(unittest.TestCase):
    def test_production_derivation_excludes_matching_paragraph(self):
        imperative = "Run the smoke test before merging any release branch."
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md",
                   _op_rules_with_imperative(imperative))
            c1 = _commit_all(root, "c1")

            bodies = lessons._op_imperative_bodies(root, c1)
            self.assertIn(imperative, bodies)

            path = root / "CLAUDE.md"
            path.write_text(
                "## Personal Notes\n\n"
                "- **India personal lesson kept outside any block.** Body one.\n\n"
                "## Duplicated Text\n\n"
                f"{imperative}\n"
            )
            found = lessons.undated_lessons(path, root, c1)
            texts = [l.text for l in found]
            self.assertTrue(any("India" in t for t in texts))
            self.assertFalse(any(imperative in t for t in texts))

    def test_reference_set_is_never_silently_empty_for_a_populated_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md",
                   _op_rules_with_imperative("Keep the reference set non-empty for this fixture."))
            c1 = _commit_all(root, "c1")
            bodies = lessons._op_imperative_bodies(root, c1)
            self.assertEqual(len(bodies), 1)

    def test_absent_operator_rules_file_at_sha_raises_from_op_imperative_bodies(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "README.md", "placeholder\n")
            c1 = _commit_all(root, "c1")
            with self.assertRaises(FileNotFoundError):
                lessons._op_imperative_bodies(root, c1)

    def test_undated_lessons_raises_when_operator_rules_file_absent(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "README.md", "placeholder\n")
            c1 = _commit_all(root, "c1")
            path = root / "CLAUDE.md"
            path.write_text("## Section\n\n- **Oscar lesson, never reached.** Body.\n")
            with self.assertRaises(FileNotFoundError):
                lessons.undated_lessons(path, root, c1)


# --- R85: fence-awareness ------------------------------------------------------------------------


class FenceAwareParsing(unittest.TestCase):
    def test_status_after_a_fenced_shell_comment_is_still_read(self):
        recon_text = (
            "## R-173 — case with a fenced comment before its status\n\n"
            "```bash\n"
            "# this looks like a heading-ish comment but is fenced content\n"
            "echo hello\n"
            "```\n\n"
            "**Status:** promoted — after the fence.\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/reconnaissance-patterns.md", recon_text)
            c1 = _commit_all(root, "c1")
            ids = {
                l.id for l in lessons.lessons_at(root, c1)
                if l.id.startswith("reconnaissance-patterns.md#")
            }
            self.assertIn("reconnaissance-patterns.md#R-173", ids)

    def test_a_fenced_heading_line_in_claude_md_does_not_split_the_section(self):
        claude_text = (
            "## Real Section\n\n"
            "- **Juliet lesson before the fence, kept intact here.** Body.\n\n"
            "```\n"
            "## Not a real heading, just fenced example text\n"
            "```\n\n"
            "More prose after the fence, same section.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        heading_ids = {l.id for l in found if "/" not in l.id}
        self.assertEqual(heading_ids, {"CLAUDE.md#real-section"})
        prose = next(l for l in found if l.id == "CLAUDE.md#real-section")
        self.assertIn("Not a real heading", prose.text)
        self.assertIn("More prose after the fence", prose.text)


# --- R86: bullet continuation / section-prose coverage invariant ---------------------------------


class SectionProseCoverageInvariant(unittest.TestCase):
    def test_every_non_blank_non_heading_line_belongs_to_exactly_one_lesson(self):
        claude_text = (
            "## Mixed Section\n\n"
            "Leading prose line one.\n"
            "Leading prose line two.\n\n"
            "- **Kilo bullet with a continuation line right after it.**\n"
            "  Indented continuation line for Kilo.\n\n"
            "Trailing prose after Kilo, not part of its continuation.\n\n"
            "- Plain bullet with no bold lead at all.\n"
            "  Indented text under the plain bullet.\n\n"
            "Final trailing prose line.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        lines = [ln for ln in claude_text.splitlines() if ln.strip() and not ln.startswith("#")]
        for line in lines:
            hits = [l for l in found if line in l.text]
            self.assertEqual(
                len(hits), 1,
                f"line {line!r} covered by {len(hits)} lessons, expected exactly 1",
            )

    def test_a_bullet_with_no_section_prose_left_over_emits_no_empty_lesson(self):
        claude_text = (
            "## Bullet Only Section\n\n"
            "- **Lima the only content in this section, no prose at all.** Body.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        self.assertEqual(len(found), 1)
        self.assertIn("/", found[0].id)
class PreambleCoverage(unittest.TestCase):
    """R86 fix round 1: content before the first `##`/`###` heading (a CLAUDE.md title +
    description) is its own pseudo-section, `CLAUDE.md#preamble`, so the coverage invariant
    holds document-wide, not only from the first heading onward. Real-data probe found this gap
    -- 2 non-blank preamble lines outside any lesson."""

    def test_preamble_before_the_first_heading_gets_its_own_lesson(self):
        claude_text = (
            "Preamble description line one.\n"
            "Preamble description line two.\n\n"
            "## Real Section\n\n"
            "- **Romeo bullet in the real section.** Body.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        preamble = next(l for l in found if l.id == "CLAUDE.md#preamble")
        self.assertIn("Preamble description line one", preamble.text)
        self.assertIn("Preamble description line two", preamble.text)

    def test_every_non_blank_non_heading_line_including_preamble_belongs_to_exactly_one_lesson(self):
        claude_text = (
            "Preamble description line one.\n"
            "Preamble description line two.\n\n"
            "## Real Section\n\n"
            "- **Romeo bullet in the real section.** Body.\n\n"
            "Trailing prose in the real section.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        lines = [ln for ln in claude_text.splitlines() if ln.strip() and not ln.startswith("#")]
        for line in lines:
            hits = [l for l in found if line in l.text]
            self.assertEqual(
                len(hits), 1,
                f"line {line!r} covered by {len(hits)} lessons, expected exactly 1",
            )

    def test_no_preamble_content_before_the_first_heading_emits_no_preamble_lesson(self):
        claude_text = "## Real Section\n\n- **Sierra bullet, no preamble at all.** Body.\n"
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        ids = {l.id for l in found}
        self.assertNotIn("CLAUDE.md#preamble", ids)

    def test_a_real_section_titled_preamble_disambiguates_against_the_synthetic_one(self):
        claude_text = (
            "Actual preamble text before any heading.\n\n"
            "## Preamble\n\n"
            "Prose in a real section that happens to be titled Preamble.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        ids = sorted(l.id for l in found)
        self.assertEqual(ids, ["CLAUDE.md#preamble", "CLAUDE.md#preamble-2"])
        real_preamble_lesson = next(l for l in found if l.id == "CLAUDE.md#preamble")
        self.assertIn("Actual preamble text", real_preamble_lesson.text)
        second = next(l for l in found if l.id == "CLAUDE.md#preamble-2")
        self.assertIn("real section that happens to be titled Preamble", second.text)


class BulletContinuationBoundaryPlacement(unittest.TestCase):
    def test_trailing_prose_after_a_blank_line_is_never_the_bullets_own_text(self):
        claude_text = (
            "## Mixed Section\n\n"
            "- **Kilo bullet with a continuation line right after it.**\n"
            "  Indented continuation line for Kilo.\n\n"
            "Trailing prose after Kilo, not part of its continuation.\n\n"
            "- Plain bullet with no bold lead at all.\n"
            "  Indented text under the plain bullet.\n\n"
            "Final trailing prose line.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        kilo = next(l for l in found if "Kilo bullet" in l.text)
        prose = next(l for l in found if l.id == "CLAUDE.md#mixed-section")
        self.assertIn("Indented continuation line for Kilo", kilo.text)
        self.assertNotIn("Trailing prose after Kilo", kilo.text)
        self.assertIn("Trailing prose after Kilo", prose.text)
        self.assertNotIn("Indented continuation line for Kilo", prose.text)


class HeadingSlugCollisionAcrossSections(unittest.TestCase):
    def test_two_sections_sharing_a_heading_get_dash_2(self):
        claude_text = (
            "## Notes\n\n"
            "Prose for the first Notes section.\n\n"
            "## Notes\n\n"
            "Prose for the second Notes section.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        ids = sorted(l.id for l in found)
        self.assertEqual(ids, ["CLAUDE.md#notes", "CLAUDE.md#notes-2"])


# --- Critical: a wrapped or unclosed bold lead is never dropped ----------------------------------


class WrappedBoldLeadIsNotDropped(unittest.TestCase):
    def test_a_bold_lead_wrapping_onto_a_second_line_is_still_matched(self):
        claude_text = (
            "## Section\n\n"
            "- **Papa lesson whose bold lead wraps onto\n"
            "  a second physical line before it closes.** Tail text right here.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        bullets = [l for l in found if "/" in l.id]
        self.assertEqual(len(bullets), 1)
        self.assertIn("papa", bullets[0].id)
        self.assertIn("Tail text right here", bullets[0].text)

    def test_a_bold_lead_that_never_closes_falls_back_to_the_bullets_own_text(self):
        claude_text = (
            "## Section\n\n"
            "- **Quebec lesson whose bold emphasis is malformed and never closes at all here.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        bullets = [l for l in found if "/" in l.id]
        self.assertEqual(len(bullets), 1)
        self.assertIn("quebec", bullets[0].id)


# --- R87: frozen dataclass, repo-qualified source, path-free undated ids -------------------------


class LessonIsFrozen(unittest.TestCase):
    def test_attribute_assignment_raises(self):
        lesson = lessons.Lesson(id="x", source="y", text="z", dated=True)
        with self.assertRaises(dataclasses.FrozenInstanceError):
            lesson.dated = False


class RepoQualifiedSourceAndPathFreeIds(unittest.TestCase):
    def test_lessons_at_source_is_repo_qualified(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "CLAUDE.md", (
                "## Section\n\n"
                "- **Lima lesson used only to check source qualification.** Body.\n"
            ))
            c1 = _commit_all(root, "c1")
            repo_name = root.resolve().name
            found = [l for l in lessons.lessons_at(root, c1) if l.id.startswith("CLAUDE.md#")]
            self.assertTrue(found)
            self.assertTrue(all(l.source == f"{repo_name}:CLAUDE.md" for l in found))

    def test_undated_lessons_ids_and_sources_are_path_free_and_identical_across_profiles(self):
        content = (
            "## Shared Section\n\n"
            "- **Mike lesson shared verbatim across two profile dirs.** Body.\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md", _op_rules_with_imperative(None))
            c1 = _commit_all(root, "c1")

            profile_a = root / "profile-a"
            profile_b = root / "profile-b-different-name"
            profile_a.mkdir()
            profile_b.mkdir()
            path_a = profile_a / "CLAUDE.md"
            path_b = profile_b / "CLAUDE.md"
            path_a.write_text(content)
            path_b.write_text(content)

            found_a = lessons.undated_lessons(path_a, root, c1)
            found_b = lessons.undated_lessons(path_b, root, c1)

            ids_a = sorted(l.id for l in found_a)
            ids_b = sorted(l.id for l in found_b)
            self.assertTrue(ids_a)
            self.assertEqual(ids_a, ids_b)
            self.assertTrue(all(l.source == "global-CLAUDE.md" for l in found_a + found_b))
            self.assertTrue(all(
                str(profile_a) not in l.id and str(profile_b) not in l.id
                for l in found_a + found_b
            ))


class LeadSlugCollisionCounterIsPerSection(unittest.TestCase):
    def test_same_lead_in_two_different_sections_both_get_base_id(self):
        claude_text = (
            "## Section Alpha\n\n"
            "- **November lesson lead words repeated across sections.** Body A.\n\n"
            "## Section Bravo\n\n"
            "- **November lesson lead words repeated across sections.** Body B.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        bullet_ids = sorted(l.id for l in found if "/" in l.id)
        self.assertEqual(len(bullet_ids), 2)
        self.assertTrue(all(not i.endswith("-2") for i in bullet_ids))


class LeadSlugWordCountIsEight(unittest.TestCase):
    def test_leads_differing_only_in_the_eighth_word_get_distinct_ids(self):
        claude_text = (
            "## Section\n\n"
            "- **One two three four five six seven Alpha lead ends differently.** Body one.\n"
            "- **One two three four five six seven Bravo lead ends differently.** Body two.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        bullet_ids = sorted(l.id for l in found if "/" in l.id)
        self.assertEqual(len(bullet_ids), 2)
        self.assertNotEqual(bullet_ids[0], bullet_ids[1])
        self.assertFalse(any(i.endswith("-2") for i in bullet_ids))


class LeadSlugWordCountBoundaryCollision(unittest.TestCase):
    def test_leads_sharing_the_first_eight_words_collide_even_if_word_nine_differs(self):
        claude_text = (
            "## Section\n\n"
            "- **One two three four five six seven eight Alpha continues here.** Body one.\n"
            "- **One two three four five six seven eight Bravo continues here.** Body two.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        bullet_ids = sorted(l.id for l in found if "/" in l.id)
        self.assertEqual(len(bullet_ids), 2)
        self.assertTrue(any(i.endswith("-2") for i in bullet_ids))


# --- R80: CLAUDE.md bullet id stability + collisions --------------------------------------------


class R80BulletIdentity(unittest.TestCase):
    @staticmethod
    def _find_by_text(found, needle):
        for l in found:
            if l.id.startswith("CLAUDE.md#") and needle in l.text:
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

            found = [l for l in lessons.lessons_at(root, c1) if l.id.startswith("CLAUDE.md#")]
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

            ids_c1 = {
                l.id for l in lessons.lessons_at(root, c1)
                if l.id.startswith("operator-rules.md#")
            }
            ids_c2 = {
                l.id for l in lessons.lessons_at(root, c2)
                if l.id.startswith("operator-rules.md#")
            }
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
            ids = {
                l.id for l in lessons.lessons_at(root, c1)
                if l.id.startswith("operator-rules.md#")
            }
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
            recon = [l for l in found if l.id.startswith("reconnaissance-patterns.md#")]
            tool = [l for l in found if l.id.startswith("tool-usage-patterns.md#")]

            self.assertEqual(
                {l.id for l in recon},
                {"reconnaissance-patterns.md#R-1", "reconnaissance-patterns.md#R-2"},
            )
            self.assertEqual({l.id for l in tool}, {"tool-usage-patterns.md#T-1"})


class R82NegativeSubstringFixture(unittest.TestCase):
    def test_status_line_containing_the_word_promoted_as_a_substring_is_excluded(self):
        recon_text = (
            "## R-201 — case status contains the word but is not promoted\n\n"
            "**Status:** open — not yet promoted, still under review.\n\n"
            "## R-202 — case genuinely promoted\n\n"
            "**Status:** promoted — landed for real.\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/reconnaissance-patterns.md", recon_text)
            c1 = _commit_all(root, "c1")
            ids = {
                l.id for l in lessons.lessons_at(root, c1)
                if l.id.startswith("reconnaissance-patterns.md#")
            }
            self.assertEqual(ids, {"reconnaissance-patterns.md#R-202"})


class FirstStatusLineWins(unittest.TestCase):
    def test_first_of_two_disagreeing_status_lines_wins_when_promoted(self):
        recon_text = (
            "## R-301 — case with two disagreeing status lines\n\n"
            "**Status:** promoted — first line says promoted.\n\n"
            "**Status:** open — second line disagrees.\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/reconnaissance-patterns.md", recon_text)
            c1 = _commit_all(root, "c1")
            ids = {
                l.id for l in lessons.lessons_at(root, c1)
                if l.id.startswith("reconnaissance-patterns.md#")
            }
            self.assertIn("reconnaissance-patterns.md#R-301", ids)

    def test_first_status_open_excludes_even_when_second_says_promoted(self):
        recon_text = (
            "## R-302 — case where first line is open and second promoted\n\n"
            "**Status:** open — first line is open.\n\n"
            "**Status:** promoted — second line disagrees.\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/reconnaissance-patterns.md", recon_text)
            c1 = _commit_all(root, "c1")
            ids = {
                l.id for l in lessons.lessons_at(root, c1)
                if l.id.startswith("reconnaissance-patterns.md#")
            }
            self.assertNotIn("reconnaissance-patterns.md#R-302", ids)


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

            found = [l for l in lessons.lessons_at(root, c1) if l.id.startswith("memory:")]
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
            found = [l for l in lessons.lessons_at(root, c1) if l.id == "memory:plain"]
            self.assertEqual(len(found), 1)

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


class MemorySectionWithSubHeadingDoesNotSplit(unittest.TestCase):
    def test_a_nested_triple_hash_heading_stays_inside_its_section(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, ".codescout/memories/nested.md", (
                "## Outer Section\n\n"
                "Outer prose.\n\n"
                "### Inner Sub-Note\n\n"
                "Inner prose that must stay part of Outer Section.\n"
            ))
            c1 = _commit_all(root, "c1")
            found = [l for l in lessons.lessons_at(root, c1) if l.id.startswith("memory:nested")]
            ids = {l.id for l in found}
            self.assertEqual(ids, {"memory:nested#outer-section"})
            self.assertIn("Inner Sub-Note", found[0].text)


# --- Commit purity per site -----------------------------------------------------------------------


class CommitPurityAcrossSources(unittest.TestCase):
    def test_a_promoted_entry_and_a_memory_added_only_in_c2_are_absent_at_c1(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/reconnaissance-patterns.md", (
                "## R-401 — case present from the start\n\n"
                "**Status:** promoted — landed at c1.\n"
            ))
            _write(root, ".codescout/memories/existing.md", "## Kept\n\nContent at c1.\n")
            c1 = _commit_all(root, "c1")

            _write(root, "docs/trackers/reconnaissance-patterns.md", (
                "## R-401 — case present from the start\n\n"
                "**Status:** promoted — landed at c1.\n\n"
                "## R-402 — case added only in c2\n\n"
                "**Status:** promoted — landed at c2 only.\n"
            ))
            _write(root, ".codescout/memories/added-in-c2.md", "## New\n\nContent at c2.\n")
            c2 = _commit_all(root, "c2")

            at_c1 = lessons.lessons_at(root, c1)
            at_c2 = lessons.lessons_at(root, c2)

            recon_ids_c1 = {l.id for l in at_c1 if l.id.startswith("reconnaissance-patterns.md#")}
            recon_ids_c2 = {l.id for l in at_c2 if l.id.startswith("reconnaissance-patterns.md#")}
            memory_ids_c1 = {l.id for l in at_c1 if l.id.startswith("memory:")}
            memory_ids_c2 = {l.id for l in at_c2 if l.id.startswith("memory:")}

            self.assertNotIn("reconnaissance-patterns.md#R-402", recon_ids_c1)
            self.assertIn("reconnaissance-patterns.md#R-402", recon_ids_c2)
            self.assertNotIn("memory:added-in-c2#new", memory_ids_c1)
            self.assertIn("memory:added-in-c2#new", memory_ids_c2)

    def test_a_memory_dirty_in_the_working_tree_only_is_not_visible(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, ".codescout/memories/stable.md", "## Committed\n\nOriginal content.\n")
            c1 = _commit_all(root, "c1")
            # Dirty edit in the working tree only, never re-committed.
            _write(root, ".codescout/memories/stable.md",
                   "## Committed\n\nDIRTY content never committed.\n")

            found = [l for l in lessons.lessons_at(root, c1) if l.id == "memory:stable#committed"]
            self.assertEqual(len(found), 1)
            self.assertIn("Original content", found[0].text)
            self.assertNotIn("DIRTY", found[0].text)

    def test_a_memory_deleted_after_the_queried_commit_still_appears_there(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, ".codescout/memories/doomed.md", "## Kept\n\nPresent at c1 only.\n")
            c1 = _commit_all(root, "c1")
            (root / ".codescout/memories/doomed.md").unlink()
            # HEAD (c2) no longer has this file at all -- the file-listing must come from the
            # QUERIED sha (c1), never from HEAD, or a since-deleted memory silently vanishes from
            # a lookup at the commit where it still existed.
            _commit_all(root, "c2 -- doomed.md deleted")

            at_c1 = lessons.lessons_at(root, c1)
            memory_ids_c1 = {l.id for l in at_c1 if l.id.startswith("memory:")}
            self.assertIn("memory:doomed#kept", memory_ids_c1)


if __name__ == "__main__":
    unittest.main()
