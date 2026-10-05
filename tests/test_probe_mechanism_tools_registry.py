"""Regression coverage for scripts/probe_guide_section_use.py's registry-vs-MECHANISM_TOOLS check.

docs/issues/archive/2026-09-24-residual-mechanism-tools-from-served-registry.md /
docs/issues/archive/2026-09-03-probe-mechanism-filter-omits-the-renamed-doc-tool.md: MECHANISM_TOOLS
was a hand list with no connection to the served MCP registry, so the 2026-09-02 `artifact` -> `doc`
rename went unnoticed for days -- a rename elsewhere silently desynchronised a selector here
(cluster/selector-narrower-than-its-population, IC-18). `mechanism_tools_registry_problems()` takes
a live registry snapshot as plain data (never spawns a process itself) precisely so this file can
hand it a FABRICATED registry and observe the exact historical failure mode reproducibly, without
racing the shared `target/` this checkout's other sessions build in.

Before these two functions existed, `mechanism_tools_registry_problems` was not an attribute of the
module at all -- calling it against `git show HEAD~:scripts/probe_guide_section_use.py`'s content
(the version before this file's fix) raises `AttributeError`, which is the observed pre-fix red this
suite's green is measured against. There was no prior boolean this replaced; the check is new.
"""

import importlib.util
from pathlib import Path
import sys
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts/probe_guide_section_use.py"
SPEC = importlib.util.spec_from_file_location("probe_guide_section_use", SCRIPT)
probe = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = probe
SPEC.loader.exec_module(probe)


class MechanismToolsRegistryProblems(unittest.TestCase):
    def test_a_renamed_tool_is_reported_as_an_existence_problem(self):
        # LOAD-BEARING: no name in this set contains "doc" or "librarian" as a substring --
        # this isolates the EXISTENCE check from the BOUNDEDNESS check below. Adding a
        # colliding name here would make this test pass even if existence-checking were
        # deleted, because the boundedness problem alone would make `problems` non-empty.
        # This replays the actual 2026-09-02 collapse: `artifact` (and every mcp__-prefixed
        # form of it) was renamed to `doc` and nothing here still answers to `doc`.
        live = {"symbols", "grep", "edit_code", "memory", "librarian"}

        problems = probe.mechanism_tools_registry_problems(live)

        self.assertEqual(1, len(problems), problems)
        self.assertIn("doc", problems[0])
        self.assertIn("no tool by that name", problems[0])

    def test_an_unintended_substring_collision_is_reported_as_a_boundedness_problem(self):
        # LOAD-BEARING: "doc" and "librarian" are BOTH present here (existence passes clean)
        # so this isolates the BOUNDEDNESS check. "docs_export" is not in MECHANISM_TOOLS but
        # contains "doc" as a substring, so `is_mechanism_tool("docs_export")` is True -- the
        # exact "doc is slightly greedy" risk the comment above MECHANISM_TOOLS names as
        # possible-but-unenforced. If is_mechanism_tool is ever tightened to match the full
        # `mcp__codescout__<name>` form instead of a bare substring, this collision stops
        # firing and this fixture name would need to change to keep discriminating.
        live = {"doc", "librarian", "docs_export", "symbols"}

        problems = probe.mechanism_tools_registry_problems(live)

        self.assertEqual(1, len(problems), problems)
        self.assertIn("docs_export", problems[0])

    def test_todays_real_registry_produces_no_problems(self):
        # LOAD-BEARING: this is a FROZEN snapshot of the 21 tool names
        # `probe_tool_surface.fetch_tools()` returned against a live `target/debug/codescout`
        # on 2026-09-27 (captured via scratch script, not re-fetched here to keep this test
        # independent of a local build being present) -- a positive control proving the two
        # checks above do not false-positive on the real, current registry. If a future tool
        # is added whose name contains "doc" or "librarian" as a substring (e.g. a
        # `docs_export` tool actually shipped), this fixture must gain that name or this test
        # will start failing for a real reason: the boundedness check would be correctly
        # reporting a genuine new collision, not a stale fixture.
        live = {
            "approve_write", "call_graph", "create_file", "doc", "edit_code", "edit_file",
            "get_guide", "grep", "index", "librarian", "library", "memory", "onboarding",
            "read_file", "references", "run_command", "semantic_search", "symbol_at",
            "symbols", "tree", "workspace",
        }

        problems = probe.mechanism_tools_registry_problems(live)

        self.assertEqual([], problems)
class MissingGuideIsNotNoRules(unittest.TestCase):
    """docs/issues/2026-09-24-residual-section-use-signatures-for-nine-topics.md (missing-guide half).

    `topics_with_rules()` used to swallow a missing guide file with `continue`, so a deleted
    guide fell out of `measurable` and `main` refused it as "no SECTION_SIGNATURES rule matches"
    -- a rules gap, when the real defect is a registered topic with no file behind it. These
    cases point `GUIDE_DIR` at a FABRICATED directory so both causes are reproducible without
    touching the real guides.
    """

    RULED_HEADING = next(iter(probe.SECTION_SIGNATURES))  # a heading some rule is keyed on

    def setUp(self):
        import tempfile

        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self._saved = probe.GUIDE_DIR
        probe.GUIDE_DIR = Path(self._tmp.name)
        self.addCleanup(setattr, probe, "GUIDE_DIR", self._saved)

    def _write(self, topic, heading):
        (probe.GUIDE_DIR / f"{topic}.md").write_text(f"# t\n\n## {heading}\n\nbody\n", encoding="utf-8")

    def test_a_missing_guide_gets_its_own_refusal_not_the_no_rules_one(self):
        # `librarian` is a registered topic with NO file in the fabricated dir.
        refusal = probe.refusal_for_topic("librarian")

        self.assertIsNotNone(refusal)
        self.assertIn("guide file", refusal)
        self.assertIn("missing", refusal)
        self.assertNotIn("no SECTION_SIGNATURES rule matches", refusal)

    def test_a_present_guide_with_no_matching_rule_keeps_the_no_rules_refusal(self):
        # TWIN of the case above: the file exists, its headings just match no rule. Without
        # this, an implementation that said "missing" for every refusal would pass the test above.
        self._write("librarian", "A heading no rule is keyed on")

        refusal = probe.refusal_for_topic("librarian")

        self.assertIsNotNone(refusal)
        self.assertIn("no SECTION_SIGNATURES rule matches", refusal)
        self.assertNotIn("missing", refusal)

    def test_a_present_guide_with_a_matching_rule_is_not_refused(self):
        # Positive control: an implementation that refused everything would fail here.
        self._write("tracker-conventions", self.RULED_HEADING)

        self.assertIsNone(probe.refusal_for_topic("tracker-conventions"))

    def test_missing_guides_are_listed_apart_from_ruleless_ones(self):
        self._write("tracker-conventions", self.RULED_HEADING)
        self._write("librarian", "A heading no rule is keyed on")

        missing = probe.topics_with_missing_guide()

        self.assertNotIn("tracker-conventions", missing)
        self.assertNotIn("librarian", missing)  # present, merely ruleless
        self.assertIn("error-handling", missing)  # registered, no file
        self.assertEqual(["tracker-conventions"], probe.topics_with_rules())


if __name__ == "__main__":
    unittest.main()
