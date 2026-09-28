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


def _qualified_id(root, bare_id):
    """The `lessons_at` id/source string R96(a) mints for `bare_id`: `<repo-name>:<bare_id>`,
    repo name = the checkout directory's own basename. Deliberately NOT `lessons._repo_name`,
    which does not exist before fix round 2 -- constructing the expected value this way keeps
    every test that calls this helper a genuine behavioral assertion (not an AttributeError) when
    run against a822828f for task7-fix2-red.txt. Valid for a plain, non-worktree fixture; it
    agrees with `_repo_name` there, since a worktree is the only case where the two diverge
    (R96(b), see WorktreeSafeRepoNaming below, which compares two `lessons_at` outputs directly
    instead of using this helper).
    """
    return f"{pathlib.Path(root).resolve().name}:{bare_id}"


def _op_rules_with_imperative(imperative_text=None, op_id="OP-1"):
    """A minimal synthetic docs/trackers/operator-rules.md fixture for R84 tests: an index-table
    row plus one `## OP-N` section carrying Imperative/Binding/Evidence fields. When
    `imperative_text` is None, the file exists but declares no `## OP-N` sections at all -- an
    empty reference set, which `_op_imperative_bodies` now REJECTS with `ValueError` (fix round 2,
    R84/R97): there is no silent empty-reference-set fallback. Callers that need a populated,
    non-colliding fixture (so `_op_imperative_bodies` does not raise) must pass `imperative_text`.
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

            claude_c1 = [l for l in at_c1 if "CLAUDE.md#" in l.id]
            claude_c2 = [l for l in at_c2 if "CLAUDE.md#" in l.id]

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

            claude = [l for l in lessons.lessons_at(root, c1) if "CLAUDE.md#" in l.id]
            self.assertEqual(len(claude), 1)
            self.assertFalse(any("Delta" in l.text for l in claude))


class UndatedGlobalLessons(unittest.TestCase):
    def test_global_claude_md_lessons_are_undated(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md", _op_rules_with_imperative(
                "Synthetic imperative text used only to keep this fixture's reference set "
                "non-empty."))
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
            # A populated OP-1 section whose Imperative matches nothing in the fixture below --
            # isolates this test from the equality-fallback detector so only the marker detector
            # can be responsible for the exclusion (fix round 2: an empty reference set now
            # raises, so this fixture must stay populated and non-colliding).
            _write(root, "docs/trackers/operator-rules.md", _op_rules_with_imperative(
                "This imperative deliberately matches no paragraph in the fixture below."))
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

    def test_zero_active_op_sections_raises_value_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md", _op_rules_with_imperative(None))
            c1 = _commit_all(root, "c1")
            with self.assertRaises(ValueError):
                lessons._op_imperative_bodies(root, c1)

    def test_a_file_with_only_a_retired_op_section_raises_value_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md", (
                "## Index\n\n"
                "| ID | Status |\n"
                "|---|---|\n"
                "| OP-1 | retired |\n\n"
                "## OP-1 — Retired synthetic rule\n\n"
                "**Imperative:** Do the retired thing, never counted.\n"
            ))
            c1 = _commit_all(root, "c1")
            with self.assertRaises(ValueError):
                lessons._op_imperative_bodies(root, c1)


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
                if "reconnaissance-patterns.md#" in l.id
            }
            self.assertIn(_qualified_id(root, "reconnaissance-patterns.md#R-173"), ids)

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
    description) is its own pseudo-section, so the coverage invariant holds document-wide, not
    only from the first heading onward. Real-data probe found this gap -- 2 non-blank preamble
    lines outside any lesson.

    Fix round 2 (item 8): the synthetic preamble's id slug is the UNCONDITIONAL literal
    `-preamble`, never plain `preamble` -- `_slugify` strips a leading `-` from any real heading's
    slug, so no real `## Preamble` heading can ever produce `-preamble`. The two ids therefore
    never collide, and the synthetic id is stable across commits even when a real section titled
    "Preamble" is added later (New Breakage #2 in the fix-round-1 re-review).
    """

    def test_preamble_before_the_first_heading_gets_its_own_lesson(self):
        claude_text = (
            "Preamble description line one.\n"
            "Preamble description line two.\n\n"
            "## Real Section\n\n"
            "- **Romeo bullet in the real section.** Body.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        preamble = next(l for l in found if l.id == "CLAUDE.md#-preamble")
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
        self.assertNotIn("CLAUDE.md#-preamble", ids)

    def test_a_real_section_titled_preamble_does_not_collide_with_the_synthetic_preamble_id(self):
        claude_text = (
            "Actual preamble text before any heading.\n\n"
            "## Preamble\n\n"
            "Prose in a real section that happens to be titled Preamble.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        ids = sorted(l.id for l in found)
        # Two distinct, non-colliding ids -- never "-preamble" plus "-preamble-2" or similar,
        # because the synthetic slug ("-preamble") is not producible by any real heading's slug.
        self.assertEqual(ids, ["CLAUDE.md#-preamble", "CLAUDE.md#preamble"])
        synthetic_preamble_lesson = next(l for l in found if l.id == "CLAUDE.md#-preamble")
        self.assertIn("Actual preamble text", synthetic_preamble_lesson.text)
        real_preamble_lesson = next(l for l in found if l.id == "CLAUDE.md#preamble")
        self.assertIn(
            "real section that happens to be titled Preamble", real_preamble_lesson.text,
        )

    def test_a_real_preamble_section_keeps_its_id_whether_or_not_preamble_text_precedes_it(self):
        # P7: a mutant that pre-reserves the "preamble" slug whenever there is NO preamble text
        # (an `else: used_ids.add("preamble")` branch) forces a real "## Preamble" section to be
        # disambiguated to "preamble-2" at c1 (no preamble text above it) while it stays
        # "preamble" at c2 (preamble text added above it) -- the real section's id must be
        # IDENTICAL at both commits regardless of whether preamble text exists.
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "CLAUDE.md", "## Preamble\n\nBody of the real preamble section.\n")
            c1 = _commit_all(root, "c1")
            at_c1 = [l for l in lessons.lessons_at(root, c1) if "CLAUDE.md#" in l.id]
            self.assertEqual(
                sorted(l.id for l in at_c1), [_qualified_id(root, "CLAUDE.md#preamble")],
            )

            _write(root, "CLAUDE.md", (
                "Some preamble text added above.\n\n"
                "## Preamble\n\nBody of the real preamble section.\n"
            ))
            c2 = _commit_all(root, "c2")
            at_c2 = [l for l in lessons.lessons_at(root, c2) if "CLAUDE.md#" in l.id]
            self.assertEqual(
                sorted(l.id for l in at_c2),
                sorted([
                    _qualified_id(root, "CLAUDE.md#-preamble"),
                    _qualified_id(root, "CLAUDE.md#preamble"),
                ]),
            )

            # The invariant P7 breaks: the REAL section's id must not move between commits.
            real_c1 = next(l for l in at_c1 if l.id.endswith("#preamble"))
            real_c2 = next(l for l in at_c2 if l.id.endswith("#preamble"))
            self.assertEqual(real_c1.id, real_c2.id)

    def test_preamble_id_is_stable_across_commits_even_when_a_real_preamble_section_is_added_later(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "CLAUDE.md", (
                "Uniform preamble text present at both commits.\n\n"
                "## Real Section\n\n"
                "- **Tango bullet in the real section.** Body.\n"
            ))
            c1 = _commit_all(root, "c1")
            _write(root, "CLAUDE.md", (
                "Uniform preamble text present at both commits.\n\n"
                "## Preamble\n\n"
                "A real section titled Preamble, added only at c2.\n\n"
                "## Real Section\n\n"
                "- **Tango bullet in the real section.** Body.\n"
            ))
            c2 = _commit_all(root, "c2")

            at_c1 = [l for l in lessons.lessons_at(root, c1) if "CLAUDE.md#" in l.id]
            at_c2 = [l for l in lessons.lessons_at(root, c2) if "CLAUDE.md#" in l.id]
            preamble_c1 = next(l for l in at_c1 if l.id.endswith("#-preamble"))
            preamble_c2 = next(l for l in at_c2 if l.id.endswith("#-preamble"))

            # The synthetic preamble id names the SAME content at both commits...
            self.assertEqual(preamble_c1.id, preamble_c2.id)
            self.assertIn("Uniform preamble text", preamble_c1.text)
            self.assertIn("Uniform preamble text", preamble_c2.text)
            # ...and the real "## Preamble" section added only at c2 takes the bare slug, never
            # displacing the synthetic one.
            real_preamble_c2 = next(l for l in at_c2 if l.id.endswith("#preamble"))
            self.assertIn("A real section titled Preamble", real_preamble_c2.text)



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

    def test_a_blank_line_followed_by_indented_text_stays_continuation(self):
        span_lines = [
            "- Bullet text.",
            "",
            "  indented continuation line.",
            "",
            "non-indented trailing prose.",
        ]
        continuation, prose = lessons._bullet_continuation_split(span_lines)
        self.assertEqual(
            continuation,
            ["- Bullet text.", "", "  indented continuation line.", ""],
        )
        self.assertEqual(prose, ["non-indented trailing prose."])

    def test_a_directly_following_non_indented_line_stays_continuation(self):
        span_lines = [
            "- Bullet text.",
            "directly following non-indented line, still continuation.",
            "",
            "trailing prose after a blank line.",
        ]
        continuation, prose = lessons._bullet_continuation_split(span_lines)
        self.assertEqual(
            continuation,
            [
                "- Bullet text.",
                "directly following non-indented line, still continuation.",
                "",
            ],
        )
        self.assertEqual(prose, ["trailing prose after a blank line."])


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



class ShortBoldLeadGetsADirectIdAssertion(unittest.TestCase):
    def test_a_short_bold_lead_gets_the_expected_id_and_full_bullet_text(self):
        # P6: a direct id-level assertion for a plain short bold lead, so the only test able to
        # kill a mutant here is not the (unrelated) runaway-fence fixture.
        text = "## Section\n\n- **Short lead.** Rest of the bullet text goes here.\n"
        found = lessons._claude_md_lessons(text, "CLAUDE.md", "x")
        bullets = [(l.id, l.text) for l in found if "/" in l.id]
        self.assertEqual(
            bullets,
            [("CLAUDE.md#section/short-lead", "- **Short lead.** Rest of the bullet text goes here.")],
        )


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
            found = [l for l in lessons.lessons_at(root, c1) if "CLAUDE.md#" in l.id]
            self.assertTrue(found)
            self.assertTrue(all(l.source == f"{repo_name}:CLAUDE.md" for l in found))
            self.assertTrue(all(l.id.startswith(f"{repo_name}:CLAUDE.md#") for l in found))

    def test_undated_lessons_ids_and_sources_are_path_free_and_identical_across_profiles(self):
        content = (
            "## Shared Section\n\n"
            "- **Mike lesson shared verbatim across two profile dirs.** Body.\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md", _op_rules_with_imperative(
                "Synthetic imperative text used only to keep this fixture's reference set "
                "non-empty."))
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

    def test_repeated_lead_within_one_section_gets_three_distinct_ids(self):
        # 7b: the second "Foo bar." collides with the first and is disambiguated to "-2" --
        # but the THIRD bullet's own natural slug ("foo-bar-2") then collides with that very
        # disambiguated id. `_dedupe_slug` must be checked against the full per-section used-id
        # set (not a per-natural-slug counter) so all three still come out distinct.
        claude_text = (
            "## Section\n\n"
            "- **Foo bar.** First occurrence.\n"
            "- **Foo bar.** Second occurrence, same natural slug as the first.\n"
            "- **Foo bar 2.** Third occurrence.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "codescout:CLAUDE.md")
        bullet_ids = [l.id for l in found if "/" in l.id]
        self.assertEqual(len(bullet_ids), len(set(bullet_ids)))
        self.assertEqual(
            bullet_ids,
            [
                "CLAUDE.md#section/foo-bar",
                "CLAUDE.md#section/foo-bar-2",
                "CLAUDE.md#section/foo-bar-2-2",
            ],
        )


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
            if "CLAUDE.md#" in l.id and needle in l.text:
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

            found = [l for l in lessons.lessons_at(root, c1) if "CLAUDE.md#" in l.id]
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
                if "operator-rules.md#" in l.id
            }
            ids_c2 = {
                l.id for l in lessons.lessons_at(root, c2)
                if "operator-rules.md#" in l.id
            }
            self.assertIn(_qualified_id(root, "operator-rules.md#OP-1"), ids_c1)
            self.assertNotIn(_qualified_id(root, "operator-rules.md#OP-1"), ids_c2)

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
                if "operator-rules.md#" in l.id
            }
            self.assertIn(_qualified_id(root, "operator-rules.md#OP-1"), ids)
            self.assertNotIn(_qualified_id(root, "operator-rules.md#OP-2"), ids)


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
            recon = [l for l in found if "reconnaissance-patterns.md#" in l.id]
            tool = [l for l in found if "tool-usage-patterns.md#" in l.id]

            self.assertEqual(
                {l.id for l in recon},
                {
                    _qualified_id(root, "reconnaissance-patterns.md#R-1"),
                    _qualified_id(root, "reconnaissance-patterns.md#R-2"),
                },
            )
            self.assertEqual(
                {l.id for l in tool}, {_qualified_id(root, "tool-usage-patterns.md#T-1")},
            )


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
                if "reconnaissance-patterns.md#" in l.id
            }
            self.assertEqual(ids, {_qualified_id(root, "reconnaissance-patterns.md#R-202")})


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
                if "reconnaissance-patterns.md#" in l.id
            }
            self.assertIn(_qualified_id(root, "reconnaissance-patterns.md#R-301"), ids)

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
                if "reconnaissance-patterns.md#" in l.id
            }
            self.assertNotIn(_qualified_id(root, "reconnaissance-patterns.md#R-302"), ids)


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

            found = [l for l in lessons.lessons_at(root, c1) if "memory:" in l.id]
            ids = {l.id for l in found}
            self.assertIn(_qualified_id(root, "memory:two-section#first-section"), ids)
            self.assertIn(_qualified_id(root, "memory:two-section#second-section"), ids)
            self.assertIn(_qualified_id(root, "memory:sub/dir-note#only-section"), ids)

    def test_a_memory_file_with_no_heading_is_one_lesson(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, ".codescout/memories/plain.md", "Languages: markdown\nRoot: .\n")
            c1 = _commit_all(root, "c1")
            found = [
                l for l in lessons.lessons_at(root, c1)
                if l.id == _qualified_id(root, "memory:plain")
            ]
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
            self.assertIn(_qualified_id(root, "memory:tracked#kept"), ids)
            self.assertNotIn(_qualified_id(root, "memory:untracked#ghost"), ids)

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
            found = [l for l in lessons.lessons_at(root, c1) if "memory:nested" in l.id]
            ids = {l.id for l in found}
            self.assertEqual(ids, {_qualified_id(root, "memory:nested#outer-section")})
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

            recon_ids_c1 = {l.id for l in at_c1 if "reconnaissance-patterns.md#" in l.id}
            recon_ids_c2 = {l.id for l in at_c2 if "reconnaissance-patterns.md#" in l.id}
            memory_ids_c1 = {l.id for l in at_c1 if "memory:" in l.id}
            memory_ids_c2 = {l.id for l in at_c2 if "memory:" in l.id}

            self.assertNotIn(_qualified_id(root, "reconnaissance-patterns.md#R-402"), recon_ids_c1)
            self.assertIn(_qualified_id(root, "reconnaissance-patterns.md#R-402"), recon_ids_c2)
            self.assertNotIn(_qualified_id(root, "memory:added-in-c2#new"), memory_ids_c1)
            self.assertIn(_qualified_id(root, "memory:added-in-c2#new"), memory_ids_c2)

    def test_a_memory_dirty_in_the_working_tree_only_is_not_visible(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, ".codescout/memories/stable.md", "## Committed\n\nOriginal content.\n")
            c1 = _commit_all(root, "c1")
            # Dirty edit in the working tree only, never re-committed.
            _write(root, ".codescout/memories/stable.md",
                   "## Committed\n\nDIRTY content never committed.\n")

            found = [
                l for l in lessons.lessons_at(root, c1)
                if l.id == _qualified_id(root, "memory:stable#committed")
            ]
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
            memory_ids_c1 = {l.id for l in at_c1 if "memory:" in l.id}
            self.assertIn(_qualified_id(root, "memory:doomed#kept"), memory_ids_c1)


class R97RetiredRuleImperativeIsNotInTheReferenceSet(unittest.TestCase):
    def test_a_retired_rules_imperative_is_excluded_while_an_active_ones_is_included(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md", (
                "## Index\n\n"
                "| ID | Status |\n"
                "|---|---|\n"
                "| OP-1 | active |\n"
                "| OP-2 | retired |\n\n"
                "## OP-1 — Active synthetic rule\n\n"
                "**Imperative:** Always run the active-rule check before merging.\n\n"
                "## OP-2 — Retired synthetic rule\n\n"
                "**Imperative:** Do the retired thing, never counted.\n"
            ))
            c1 = _commit_all(root, "c1")
            bodies = lessons._op_imperative_bodies(root, c1)
            self.assertIn("Always run the active-rule check before merging.", bodies)
            self.assertNotIn("Do the retired thing, never counted.", bodies)


    def test_a_proposed_rules_imperative_is_excluded_same_as_retired(self):
        # P10: the reference-set site must be "ACTIVE per R81", not "not retired" -- a status of
        # "proposed" (neither active nor retired) must be excluded exactly like a retired one.
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md", (
                "## Index\n\n| ID | Status |\n|---|---|\n| OP-1 | active |\n| OP-2 | proposed |\n\n"
                "## OP-1 — Active rule\n\n**Imperative:** Do the active thing.\n\n"
                "## OP-2 — Proposed rule\n\n**Imperative:** Do the proposed thing.\n"
            ))
            c1 = _commit_all(root, "c1")
            bodies = lessons._op_imperative_bodies(root, c1)
            self.assertEqual(bodies, ["Do the active thing."])


class OpTableStatusParsingIsFenceAware(unittest.TestCase):
    def test_a_fenced_example_index_row_reading_active_does_not_resurrect_a_retired_rule(self):
        text = (
            "## Index\n\n"
            "| ID | Status |\n"
            "|---|---|\n"
            "| OP-5 | retired |\n\n"
            "## OP-5 — Retired rule\n\n"
            "**Imperative:** Do the retired thing.\n\n"
            "To restore it, the row would read:\n\n"
            "```markdown\n"
            "| OP-5 | active |\n"
            "```\n"
        )
        found = lessons._operator_rules_lessons(text, "x:operator-rules.md")
        ids = {l.id for l in found}
        self.assertNotIn("operator-rules.md#OP-5", ids)


class RunawayFenceOpenerIsRefused(unittest.TestCase):
    def test_a_backtick_opener_whose_info_string_contains_a_backtick_does_not_swallow_the_rest_of_the_file(self):
        claude_text = (
            "## Alpha Heading\n\n"
            "- **Alpha bullet lead words here and more.** Body.\n\n"
            "```x``` is inline code at line start.\n\n"
            "## Bravo Heading\n\n"
            "- **Bravo bullet lead words here and more.** Body.\n"
        )
        found = lessons._claude_md_lessons(claude_text, "CLAUDE.md", "x")
        ids = {l.id for l in found}
        # A discriminator that survives only if BOTH headings were parsed as their OWN
        # sections -- if the runaway fence swallowed "## Bravo Heading" as opaque fence
        # content, no id naming that heading's own slug would exist at all, even though
        # the raw words "Bravo bullet" would still appear (as a substring) inside Alpha's
        # merged prose lesson. A substring check on `.text` cannot tell those apart; an
        # exact heading-qualified id can.
        self.assertIn(
            "CLAUDE.md#alpha-heading/alpha-bullet-lead-words-here-and-more", ids,
        )
        self.assertIn(
            "CLAUDE.md#bravo-heading/bravo-bullet-lead-words-here-and-more", ids,
        )


    def test_a_tilde_fence_whose_info_string_contains_a_backtick_still_opens_normally(self):
        # P4: the backtick-in-info refusal is backtick-fence-specific -- a ~~~ fence with a
        # backtick in its info string must still open and close like any other tilde fence.
        text = (
            "## Section\n\n"
            "~~~lang`with-backtick\n"
            "fenced content\n"
            "~~~\n"
            "- **Real bullet.** After the tilde fence, should be parsed normally.\n"
        )
        found = lessons._claude_md_lessons(text, "CLAUDE.md", "x")
        ids = [l.id for l in found]
        self.assertIn("CLAUDE.md#section/real-bullet", ids)
        self.assertIn("CLAUDE.md#section", ids)

    def test_an_indented_backtick_opener_with_a_backtick_in_its_info_string_is_still_refused(self):
        # P5: the refusal must still apply when the backtick opener is indented -- an indented
        # opener never opens a fence at all, so content after it (including a bogus closer) is
        # ordinary prose.
        text = (
            "## Section\n\n"
            "   ```lang`with-backtick\n"
            "- **Real bullet.** Should still be parsed; the indented opener never opened a fence.\n"
        )
        found = lessons._claude_md_lessons(text, "CLAUDE.md", "x")
        ids = [l.id for l in found]
        self.assertIn("CLAUDE.md#section/real-bullet", ids)
        self.assertIn("CLAUDE.md#section", ids)


class FieldLineParsingIsFenceAware(unittest.TestCase):
    def test_a_fenced_status_line_is_not_read_as_the_first_status_line(self):
        body = (
            "Some prose before.\n\n"
            "```\n"
            "**Status:** promoted\n"
            "```\n\n"
            "**Status:** open — the real status.\n"
        )
        self.assertEqual(lessons._first_field_line(body, lessons._STATUS_LINE_RE), "open")


class BulletStartParsingIsFenceAware(unittest.TestCase):
    def test_a_fenced_bullet_start_line_is_not_treated_as_a_bullet(self):
        body = (
            "Prose before the fence.\n\n"
            "```\n"
            "- **Fenced example bullet, never a real lesson.** Body.\n"
            "```\n\n"
            "More trailing prose.\n"
        )
        bold_bullets, prose_lines = lessons._partition_bullets_and_prose(body)
        self.assertEqual(bold_bullets, [])
        self.assertIn(
            "- **Fenced example bullet, never a real lesson.** Body.", prose_lines,
        )


class PreambleParsingIsFenceAware(unittest.TestCase):
    def test_a_fenced_heading_line_before_the_first_real_heading_is_not_the_first_heading(self):
        text = (
            "Preamble prose line one.\n\n"
            "```\n"
            "## Fenced example heading, not real\n"
            "```\n\n"
            "Preamble prose line two.\n\n"
            "## Real First Heading\n\n"
            "Body.\n"
        )
        preamble = lessons._leading_preamble(text, 2, 2)
        self.assertIn("Preamble prose line two.", preamble)
        self.assertNotIn("Real First Heading", preamble)


class FenceClosingRequiresSameCharAndAtLeastSameLength(unittest.TestCase):
    def test_fence_does_not_close_on_a_different_fence_character(self):
        lines = ["```", "~~~", "still inside the original fence", "```"]
        self.assertEqual(lessons._fence_flags(lines), [True, True, True, True])

    def test_fence_does_not_close_on_a_shorter_run_length(self):
        lines = ["````", "```", "still inside the original fence", "````"]
        self.assertEqual(lessons._fence_flags(lines), [True, True, True, True])


class FenceOpenerRecognizedAtAnyIndentation(unittest.TestCase):
    def test_an_indented_fence_opener_is_still_recognized(self):
        lines = ["  ```", "  fenced content here", "  ```"]
        self.assertEqual(lessons._fence_flags(lines), [True, True, True])


class R84ReferenceSetIsReadAtTheShaNeverTheWorkingTreeOrHead(unittest.TestCase):
    def test_reference_set_reflects_the_queried_sha_not_a_later_working_tree_edit_or_head(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md",
                   _op_rules_with_imperative("Imperative A, true at c1."))
            c1 = _commit_all(root, "c1")

            # A genuine uncommitted working-tree edit -- the file on disk now names Imperative C,
            # but c1 is still the sha being queried and must not see it.
            _write(root, "docs/trackers/operator-rules.md",
                   _op_rules_with_imperative("Imperative C, uncommitted working-tree edit."))
            bodies_with_dirty_tree = lessons._op_imperative_bodies(root, c1)
            self.assertIn("Imperative A, true at c1.", bodies_with_dirty_tree)
            self.assertNotIn(
                "Imperative C, uncommitted working-tree edit.", bodies_with_dirty_tree,
            )

            # Commit that working-tree edit under a different imperative (B), moving HEAD to c2.
            _write(root, "docs/trackers/operator-rules.md",
                   _op_rules_with_imperative("Imperative B, only true after c1."))
            _commit_all(root, "c2")

            bodies = lessons._op_imperative_bodies(root, c1)
            self.assertIn("Imperative A, true at c1.", bodies)
            self.assertNotIn("Imperative B, only true after c1.", bodies)
            self.assertNotIn("Imperative C, uncommitted working-tree edit.", bodies)


class HeadingSlugDisambiguationAvoidsATripleCollision(unittest.TestCase):
    def test_claude_md_three_way_heading_collision_gets_three_unique_ids(self):
        text = (
            "## Notes\n\n"
            "First notes section prose.\n\n"
            "## Notes\n\n"
            "Second notes section prose.\n\n"
            "## Notes 2\n\n"
            "Third notes section prose.\n"
        )
        found = lessons._claude_md_lessons(text, "CLAUDE.md", "x")
        ids = [l.id for l in found]
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual(
            set(ids), {"CLAUDE.md#notes", "CLAUDE.md#notes-2", "CLAUDE.md#notes-2-2"},
        )

    def test_three_identical_headings_get_three_distinct_ids(self):
        # Unlike the fixture above (whose third heading's OWN natural slug happens to
        # equal the second heading's disambiguated id), this fixture forces a SECOND
        # heading to take "-2" and a THIRD to need one increment past it -- but that is
        # still only ONE increment total (check "-2": taken; increment to "-3": free), which
        # a single `if` that increments then uses the result WITHOUT rechecking would also
        # get right. It does NOT distinguish `_dedupe_slug`'s `while` loop from a single `if`
        # (P9 survives this fixture). The direct test for that is
        # `test_two_increments_are_needed_when_natural_slugs_occupy_dash_2_and_dash_3` below,
        # whose fourth heading needs the loop to check "-2" AND "-3" before landing on "-4".
        text = (
            "## Notes\n\nFirst notes section prose.\n\n"
            "## Notes\n\nSecond notes section prose.\n\n"
            "## Notes\n\nThird notes section prose.\n"
        )
        found = lessons._claude_md_lessons(text, "CLAUDE.md", "x")
        ids = [l.id for l in found]
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual(
            set(ids), {"CLAUDE.md#notes", "CLAUDE.md#notes-2", "CLAUDE.md#notes-3"},
        )

    def test_two_increments_are_needed_when_natural_slugs_occupy_dash_2_and_dash_3(self):
        # 7a / P9: headings "Notes", "Notes 2", "Notes 3" each take their OWN natural slug
        # first, so when a fourth "Notes" heading repeats the very first one, disambiguation
        # must check "-2" (taken by the second heading's natural slug), THEN "-3" (taken by the
        # third heading's natural slug) before landing on the first actually-free candidate,
        # "-4" -- two increments past the initial guess, not one. A single `if` (P9) stops after
        # incrementing once and returns "notes-3" without rechecking it, which COLLIDES with the
        # third heading's own id.
        text = (
            "## Notes\n\nFirst notes section prose.\n\n"
            "## Notes 2\n\nSecond notes section prose.\n\n"
            "## Notes 3\n\nThird notes section prose.\n\n"
            "## Notes\n\nFourth notes section prose (repeats the very first heading).\n"
        )
        found = lessons._claude_md_lessons(text, "CLAUDE.md", "x")
        ids = [l.id for l in found]
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual(
            set(ids),
            {"CLAUDE.md#notes", "CLAUDE.md#notes-2", "CLAUDE.md#notes-3", "CLAUDE.md#notes-4"},
        )

    def test_memory_file_three_way_heading_collision_gets_three_unique_ids(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, ".codescout/memories/topic.md", (
                "## Notes\n\nFirst notes section prose.\n\n"
                "## Notes\n\nSecond notes section prose.\n\n"
                "## Notes 2\n\nThird notes section prose.\n"
            ))
            c1 = _commit_all(root, "c1")
            found = lessons._memory_lessons(root, c1, "reponame")
            ids = [l.id for l in found]
            self.assertEqual(len(ids), len(set(ids)))
            self.assertEqual(
                set(ids),
                {
                    "memory:topic#notes",
                    "memory:topic#notes-2",
                    "memory:topic#notes-2-2",
                },
            )

    def test_lessons_at_head_has_zero_duplicate_ids(self):
        root = REPO_ROOT
        head = _git(root, "rev-parse", "HEAD").strip()
        found = lessons.lessons_at(root, head)
        ids = [l.id for l in found]
        dupes = {i for i in ids if ids.count(i) > 1}
        self.assertEqual(dupes, set())



class R99DuplicateOutputIdsRaiseAtReturnTime(unittest.TestCase):
    def test_lessons_at_raises_value_error_naming_the_duplicated_id(self):
        # A memory heading "## Bar" in foo.md and a headingless file foo#bar.md both produce the
        # id "memory:foo#bar" -- a real (if contrived) way two disambiguating sites can still
        # collide, which is exactly what the return-time R99 guard exists to catch.
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, ".codescout/memories/foo.md", "## Bar\n\nx\n")
            _write(root, ".codescout/memories/foo#bar.md", "no heading\n")
            c1 = _commit_all(root, "c1")
            with self.assertRaises(ValueError) as ctx:
                lessons.lessons_at(root, c1)
            self.assertIn("duplicate", str(ctx.exception).lower())
            self.assertIn("memory:foo#bar", str(ctx.exception))



class RepeatedTrackerEntryHeadingIsDisambiguated(unittest.TestCase):
    def test_a_repeated_tracker_entry_heading_gets_a_dash_2_id_and_keeps_both_bodies(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/reconnaissance-patterns.md", (
                "## R-5 — first\n\n**Status:** promoted — a.\n\n"
                "## R-5 — duplicated heading\n\n**Status:** promoted — b.\n"
            ))
            c1 = _commit_all(root, "c1")
            found = lessons._tracker_lessons(
                lessons._git_show(root, c1, "docs/trackers/reconnaissance-patterns.md"),
                lessons._R_ID_RE, 2, "reconnaissance-patterns.md", "x",
            )
            ids = [l.id for l in found]
            self.assertEqual(ids, ["reconnaissance-patterns.md#R-5", "reconnaissance-patterns.md#R-5-2"])
            self.assertTrue(any("a." in l.text for l in found))
            self.assertTrue(any("b." in l.text for l in found))



class R98UnresolvableShaRaisesValueError(unittest.TestCase):
    def test_lessons_at_raises_on_an_unknown_sha(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp) / "repo"
            _init_repo(root)
            _write(root, "CLAUDE.md", "hello\n")
            _commit_all(root, "c1")
            with self.assertRaises(ValueError) as ctx:
                lessons.lessons_at(root, "not-a-real-sha")
            self.assertIn("not-a-real-sha", str(ctx.exception))

    def test_lessons_at_raises_on_a_non_git_directory(self):
        # The non-git directory is a SIBLING of the repo, never nested inside it -- nesting it
        # would let git walk up and find the enclosing .git, silently defeating the test.
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp) / "repo"
            _init_repo(root)
            _write(root, "CLAUDE.md", "hello\n")
            _commit_all(root, "c1")
            non_git = pathlib.Path(tmp) / "not-a-repo"
            non_git.mkdir()
            with self.assertRaises(ValueError):
                lessons.lessons_at(non_git, "HEAD")

    def test_undated_lessons_raises_on_an_unknown_sha_too(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "docs/trackers/operator-rules.md", (
                "## Index\n\n| ID | Status |\n|---|---|\n| OP-1 | active |\n\n"
                "## OP-1 — Synthetic rule\n\n**Imperative:** Do the thing.\n"
            ))
            _commit_all(root, "c1")
            global_path = pathlib.Path(tmp) / "global-CLAUDE.md"
            global_path.write_text("## Section\n\nSome undated prose.\n")
            with self.assertRaises(Exception):
                lessons.undated_lessons(global_path, root, "not-a-real-sha")


class LeadSlugDerivesFromTheBoldSpanNotFullText(unittest.TestCase):
    def test_a_short_bold_lead_does_not_pull_in_words_from_after_the_close(self):
        bullet_text = "- **Short lead.** Body text with several more words after the close."
        source = lessons._bullet_lead_source(bullet_text)
        self.assertEqual(source, "Short lead.")
        self.assertEqual(lessons._lead_slug(source), "short-lead")

    def test_a_wrapped_short_bold_lead_does_not_pull_in_words_from_after_the_close(self):
        bullet_text = "- **Wrapped\nshort lead.** Body text with several more words after the close."
        source = lessons._bullet_lead_source(bullet_text)
        self.assertEqual(source, "Wrapped\nshort lead.")
        self.assertEqual(lessons._lead_slug(source), "wrapped-short-lead")


class SourceIsRepoQualifiedAtEverySite(unittest.TestCase):
    def test_source_is_repo_qualified_at_every_lessons_at_site(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            _init_repo(root)
            _write(root, "CLAUDE.md", (
                "## Alpha Section\n\n"
                "- **Alpha bullet lead words here and more text.** Body one.\n"
            ))
            _write(root, "docs/trackers/reconnaissance-patterns.md", (
                "## R-1 — case promoted\n\n"
                "**Status:** promoted — landed in SKILL.md.\n"
            ))
            _write(root, "docs/trackers/tool-usage-patterns.md", (
                "## Some Observations\n\n"
                "### T-1 — case promoted\n\n"
                "**Status:** promoted — folded into a skill.\n"
            ))
            _write(root, "docs/trackers/operator-rules.md",
                   _op_rules_with_imperative("Always run the source-qualification check."))
            _write(root, ".codescout/memories/topic.md",
                   "## A Memory Section\n\nSome memory body text.\n")
            c1 = _commit_all(root, "c1")

            found = lessons.lessons_at(root, c1)
            repo_name = pathlib.Path(root).resolve().name
            by_site = {
                "CLAUDE.md": [l for l in found if "CLAUDE.md#" in l.id],
                "recon": [l for l in found if "reconnaissance-patterns.md#" in l.id],
                "tool-usage": [l for l in found if "tool-usage-patterns.md#" in l.id],
                "operator-rules": [l for l in found if "operator-rules.md#" in l.id],
                "memory": [l for l in found if ":memory:" in l.id],
            }
            for site, site_lessons in by_site.items():
                self.assertTrue(site_lessons, f"no lessons found for site {site}")
                for l in site_lessons:
                    self.assertTrue(
                        l.source.startswith(f"{repo_name}:"),
                        f"{site} lesson source not repo-qualified: {l.source!r}",
                    )


class WorktreeSafeRepoNaming(unittest.TestCase):
    def test_lessons_at_from_a_worktree_matches_the_main_checkout(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp) / "main-repo-name"
            _init_repo(root)
            _write(root, "CLAUDE.md", (
                "## Alpha Section\n\n"
                "- **Alpha bullet lead words here and more text.** Body one.\n"
            ))
            c1 = _commit_all(root, "c1")

            worktree = pathlib.Path(tmp) / "a-differently-named-worktree-dir"
            _git(root, "worktree", "add", "-q", str(worktree), c1)

            main_found = lessons.lessons_at(root, c1)
            wt_found = lessons.lessons_at(worktree, c1)

            self.assertEqual(
                {(l.id, l.source) for l in main_found},
                {(l.id, l.source) for l in wt_found},
            )


if __name__ == "__main__":
    unittest.main()
