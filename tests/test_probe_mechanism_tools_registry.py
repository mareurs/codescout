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


if __name__ == "__main__":
    unittest.main()
