"""Regression coverage for scripts/probe_guide_section_use.py's recognition of an injection.

docs/issues/archive/2026-10-06-the-guide-use-probe-opening-marker-cannot-see-section-grain-injections.md:
`opening_marker` searched transcripts for the WHOLE-topic opening line only
(`auto-injected get_guide('T') — first call`). Once a topic declared sections, `guide_emit.rs`
began delivering it as one block per section, each opened by
`auto-injected get_guide('T') § <heading> — first call`, and the probe could not see any of them:
a well-formed, wrong count (cluster/selector-narrower-than-its-population, IC-18).

The fixtures here are rendered from the REAL format strings in src/tools/core/guide_emit.rs, which
this file reads at run time. A hand-written fixture would pass while the emitter drifted away from
it, which is the failure this bug is an instance of. If the emitter's wording changes, the
`rust_literal` lookup below fails first and names the string it could not find.

Pre-fix, the section-form tests fail (`scan_transcript` returns None for them); the whole-topic and
closing-form tests pass both before and after, as controls.
"""

import importlib.util
import json
from pathlib import Path
import re
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/probe_guide_section_use.py"
SPEC = importlib.util.spec_from_file_location("probe_guide_section_use", SCRIPT)
probe = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = probe
SPEC.loader.exec_module(probe)

GUIDE_EMIT = (ROOT / "src/tools/core/guide_emit.rs").read_text(encoding="utf-8")


def rust_literal(needle: str) -> str:
    """The raw text of the one Rust string literal that begins `"<!-- ` and contains `needle`.

    None of the emitter's opening/closing literals contains an unescaped double quote, so the
    literal runs to the next `"`. Fails loudly, naming the needle, if the emitter's wording moved.
    """
    for m in re.finditer(r'"(<!-- [^"]*)"', GUIDE_EMIT):
        if needle in m.group(1):
            return m.group(1)
    raise AssertionError(f"no format literal in guide_emit.rs contains {needle!r}: the emitter changed")


def unescape(literal: str) -> str:
    """Rust string-literal rules for the two escapes these literals use.

    A backslash at the end of a line drops the newline and the next line's indentation; `\\n` is a
    newline.
    """
    return re.sub(r"\\\n[ \t]*", "", literal).replace("\\n", "\n")


WHOLE = rust_literal("get_guide('{topic}') — first call this session")  # guide_block
SECTION = rust_literal("§ {} — first call this")  # guide_blocks_for, one declared section
PREAMBLE = rust_literal("preamble — no section")  # guide_blocks_for, no section declares the shape
POINTER = rust_literal("NOT auto-injected")  # whole_topic_blocks, body over the auto-inject bound


def render_whole(topic: str, body: str = "BODY") -> str:
    return unescape(WHOLE).replace("{topic}", topic).replace("{body}", body)


def render_section(topic: str, heading: str, body: str = "BODY") -> str:
    text = unescape(SECTION).replace("{topic}", topic)
    for value in (heading, body, heading):  # the three `{}` slots, in source order
        text = text.replace("{}", value, 1)
    return text


def render_preamble(topic: str, preamble: str = "PREAMBLE") -> str:
    return unescape(PREAMBLE).replace("{topic}", topic).replace("{}", preamble, 1)


def render_pointer(topic: str) -> str:
    return unescape(POINTER).replace("{topic}", topic).replace("{size}", "60415").replace("{max_bytes}", "16384")


def user_line(content) -> dict:
    return {
        "type": "user",
        "timestamp": "2026-10-07T00:00:00Z",
        "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t1", "content": content}]},
    }


def assistant_text_line(text: str) -> dict:
    return {
        "type": "assistant",
        "timestamp": "2026-10-07T00:00:01Z",
        "message": {"role": "assistant", "content": [{"type": "text", "text": text}]},
    }


def scan(lines: list[dict], topic: str):
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "00000000-0000-0000-0000-000000000000.jsonl"
        # ensure_ascii=False: Claude Code writes the em dash and the section sign as raw UTF-8,
        # and the probe's fast-reject looks for exactly those bytes.
        path.write_text("\n".join(json.dumps(d, ensure_ascii=False) for d in lines) + "\n", encoding="utf-8")
        return probe.scan_transcript(path, topic)


class EmitterFixtures(unittest.TestCase):
    """The fixtures must be what the emitter writes, or every test below proves nothing."""

    def test_the_rendered_forms_have_the_shape_the_bug_describes(self):
        self.assertIn("auto-injected get_guide('librarian') — first call", render_whole("librarian"))
        self.assertIn(
            "auto-injected get_guide('librarian') § Artifact Model — first call",
            render_section("librarian", "Artifact Model"),
        )
        # Rendering left no placeholder behind.
        for text in (render_whole("t"), render_section("t", "H"), render_preamble("t"), render_pointer("t")):
            self.assertNotIn("{", text.replace("{ ", ""), text)


class OpeningFormsAreInjections(unittest.TestCase):
    def test_a_whole_topic_block_is_one_injection(self):
        rec = scan([user_line(render_whole("symbol-navigation"))], "symbol-navigation")
        self.assertIsNotNone(rec)
        self.assertEqual(1, rec["injections"])

    def test_a_section_block_is_one_injection(self):
        # LOAD-BEARING: this is the bug. Pre-fix `scan_transcript` returned None here, because the
        # probe's marker had no `§ <heading>` slot.
        rec = scan([user_line(render_section("librarian", "Artifact Model"))], "librarian")
        self.assertIsNotNone(rec, "a section-grain injection must be seen")
        self.assertEqual(1, rec["injections"])

    def test_a_heading_that_contains_an_em_dash_is_still_one_injection(self):
        # Real heading from the librarian guide. The lazy heading part must anchor on the
        # ` — first call` that closes the opening line, not on the dash inside the heading.
        heading = "docs/trackers/ — Backing Store, Not a Docs Folder"
        rec = scan([user_line(render_section("librarian", heading))], "librarian")
        self.assertIsNotNone(rec)
        self.assertEqual(1, rec["injections"])

    def test_list_shaped_tool_result_content_is_read_too(self):
        rec = scan([user_line([{"type": "text", "text": render_section("librarian", "Artifact Model")}])], "librarian")
        self.assertIsNotNone(rec)
        self.assertEqual(1, rec["injections"])

    def test_each_delivery_counts_so_a_whole_block_and_section_blocks_of_one_topic_all_count(self):
        # Decision recorded in the module docstring: one injection per opening, whichever form. Two
        # whole-topic deliveries in one session always counted twice; a section is not special.
        text = "\n".join(
            [render_whole("librarian"), render_section("librarian", "A"), render_section("librarian", "B")]
        )
        rec = scan([user_line(text)], "librarian")
        self.assertIsNotNone(rec)
        self.assertEqual(3, rec["injections"])


class NotInjections(unittest.TestCase):
    def test_closing_markers_alone_are_not_injections(self):
        # The twin of the two tests above: strip each block down to its closing line.
        closing_whole = "<!-- end auto-injected get_guide('librarian') -->"
        closing_section = "<!-- end auto-injected get_guide('librarian') § Artifact Model -->"
        self.assertIn(closing_whole, render_whole("librarian"))
        self.assertIn(closing_section, render_section("librarian", "Artifact Model"))
        # LOAD-BEARING: another topic's OPENING line rides along. Without it the transcript holds no
        # ` — first call` at all, the probe's cheap fast-reject returns None before the pattern is
        # ever consulted, and this test would pass even if the pattern matched closing markers.
        decoy = render_whole("symbol-navigation")
        self.assertIsNone(scan([user_line(closing_whole + "\n" + closing_section + "\n" + decoy)], "librarian"))

    def test_an_opening_and_its_closing_are_counted_once_not_twice(self):
        for text in (render_whole("librarian"), render_section("librarian", "Artifact Model")):
            rec = scan([user_line(text)], "librarian")
            self.assertEqual(1, rec["injections"], text)

    def test_a_closing_marker_does_not_borrow_the_next_blocks_opening(self):
        # `scan_transcript` joins a list-shaped tool_result with a single space, so one block's
        # closing marker and the next block's opening share a line. Without the `-->` bound in the
        # pattern, `librarian`'s closing `§ H -->` would run on into the NEXT topic's opening and be
        # counted as a second librarian injection.
        content = [
            {"type": "text", "text": render_section("librarian", "Artifact Model")},
            {"type": "text", "text": render_whole("symbol-navigation")},
        ]
        rec = scan([user_line(content)], "librarian")
        self.assertEqual(1, rec["injections"])
        other = scan([user_line(content)], "symbol-navigation")
        self.assertEqual(1, other["injections"])

    def test_another_topics_section_block_is_not_this_topics_injection(self):
        self.assertIsNone(scan([user_line(render_section("workspace-state", "Pinning"))], "librarian"))

    def test_a_section_opening_quoted_in_assistant_text_is_a_discussion_not_a_delivery(self):
        # Trap 3, for the new form: only a marker inside a tool_result is a delivery.
        quoted = render_section("librarian", "Artifact Model")
        self.assertIsNone(scan([assistant_text_line(quoted)], "librarian"))

    def test_the_preamble_and_pointer_lines_are_not_injections(self):
        # Deliberate, and stated in the module docstring: a preamble delivers no section, and the
        # pointer line says the guide was NOT delivered. If either should count, that is a design
        # decision for the probe's owner, not a side effect of this fix.
        self.assertIsNone(scan([user_line(render_preamble("librarian"))], "librarian"))
        self.assertIsNone(scan([user_line(render_pointer("tracker-conventions"))], "tracker-conventions"))


if __name__ == "__main__":
    unittest.main()
