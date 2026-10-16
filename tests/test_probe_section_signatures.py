"""Coverage for the `(topic, heading)` keying of scripts/probe_guide_section_use.py's signatures.

docs/issues/2026-09-24-residual-section-use-signatures-for-nine-topics.md. Before this,
`SECTION_SIGNATURES` was keyed by section HEADING alone, so a rule authored for one topic's
section would be credited to any other topic's section sharing the heading (`Related` is in four
guides, `(preamble)` in all ten), and the only measurable topic was whichever one the keys
happened to spell. These cases fabricate guides and transcripts so each failure is reproducible
without the real corpus.

NOT run by `scripts/gate.sh` or CI (nothing runs `tests/test_*.py`; see
`tests/python_test_entry_guard.rs`). Run by hand: `python3 -I tests/test_probe_section_signatures.py`.
"""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
import sys


SCRIPT = Path(__file__).resolve().parents[1] / "scripts/probe_guide_section_use.py"
SPEC = importlib.util.spec_from_file_location("probe_guide_section_use_sigs", SCRIPT)
probe = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = probe
SPEC.loader.exec_module(probe)


class SignaturesAreKeyedByTopic(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self._saved = probe.GUIDE_DIR
        probe.GUIDE_DIR = Path(self._tmp.name)
        self.addCleanup(setattr, probe, "GUIDE_DIR", self._saved)

    def _write(self, topic, heading):
        (probe.GUIDE_DIR / f"{topic}.md").write_text(
            f"# t\n\n## {heading}\n\nbody\n", encoding="utf-8"
        )

    def test_another_topics_heading_does_not_make_a_topic_measurable(self):
        # `Querying with the librarian` is a tracker-conventions heading with rules. A guide of a
        # DIFFERENT topic that happens to carry the same heading must not inherit them: under the
        # old heading-only key it did, and the run would have scored it with another topic's rules.
        self._write("error-handling", "Querying with the librarian")
        self._write("tracker-conventions", "Querying with the librarian")

        self.assertEqual(["tracker-conventions"], probe.topics_with_rules())
        self.assertIsNotNone(probe.refusal_for_topic("error-handling"))
        self.assertIsNone(probe.refusal_for_topic("tracker-conventions"))

    def test_a_heading_shared_by_two_topics_is_ruled_only_where_a_rule_is_keyed(self):
        probe.SECTION_SIGNATURES["error-handling"] = {}
        self.addCleanup(probe.SECTION_SIGNATURES.pop, "error-handling", None)
        probe.SECTION_SIGNATURES["librarian"]["Related"] = [(None, ['"action": "get"'])]
        self.addCleanup(probe.SECTION_SIGNATURES["librarian"].pop, "Related", None)
        self._write("librarian", "Related")
        self._write("workspace-state", "Related")

        self.assertEqual(["librarian"], probe.topics_with_rules())

    def test_every_rule_key_names_a_real_heading_of_its_own_guide(self):
        # The converse guard: a guide heading renamed or deleted strands its rules, which then
        # score a section that no longer exists -- silently, since nothing else reads the key.
        probe.GUIDE_DIR = self._saved
        for topic, rules in probe.SECTION_SIGNATURES.items():
            headings = set(probe.current_section_bytes(topic))
            stale = sorted(set(rules) - headings)
            self.assertEqual([], stale, f"{topic}: rule keys with no heading in the guide")


class EveryRegisteredTopicIsAccountedFor(unittest.TestCase):
    def test_every_topic_is_ruled_or_has_a_recorded_reason(self):
        self.assertEqual([], probe.topics_unaccounted_for())

    def test_a_newly_registered_topic_is_reported_as_unaccounted_for(self):
        # Without this the guard above could be an always-empty list.
        probe.TOPICS.append("a-brand-new-guide")
        self.addCleanup(probe.TOPICS.remove, "a-brand-new-guide")

        self.assertEqual(["a-brand-new-guide"], probe.topics_unaccounted_for())

    def test_no_topic_is_both_ruled_and_declared_unmeasurable(self):
        both = [t for t in probe.UNMEASURABLE_TOPICS if probe.signatures_for(t)]
        self.assertEqual([], both)

    def test_unmeasurable_reasons_name_their_kind_and_say_something(self):
        for topic, why in probe.UNMEASURABLE_TOPICS.items():
            self.assertIn(topic, probe.TOPICS)
            self.assertTrue(
                why.startswith(("NO POPULATION", "OUT OF SCOPE")),
                f"{topic}: a reason must open with its kind, got {why[:40]!r}",
            )
            self.assertGreater(len(why), 80, topic)

    def test_the_refusal_carries_the_reason_for_an_unmeasurable_topic_only(self):
        with_reason = probe.refusal_for_topic("iron-laws-detail")
        self.assertIsNotNone(with_reason)
        self.assertIn(probe.UNMEASURABLE_TOPICS["iron-laws-detail"], with_reason)
        # Twin: a ruled topic is not refused at all, so the reason cannot leak onto it.
        self.assertIsNone(probe.refusal_for_topic("librarian"))
        self.assertIsNone(probe.refusal_for_topic("tracker-conventions"))


class ScanUsesThePerTopicRules(unittest.TestCase):
    """Drive `scan_transcript` with a fabricated transcript, per the parsing contract."""

    OPENING = (
        "<!-- auto-injected get_guide('librarian') § Filter Syntax — first call this "
        "session that serves this section. Do NOT re-call get_guide for it. -->"
    )

    def _transcript(self, *tool_uses):
        lines = [
            {
                "type": "user",
                "timestamp": "2026-10-16T00:00:00Z",
                "message": {
                    "content": [
                        {"type": "tool_result", "content": [{"type": "text", "text": self.OPENING}]}
                    ]
                },
            }
        ]
        for name, inp in tool_uses:
            lines.append(
                {
                    "type": "assistant",
                    "message": {"content": [{"type": "tool_use", "name": name, "input": inp}]},
                }
            )
        d = tempfile.TemporaryDirectory()
        self.addCleanup(d.cleanup)
        path = Path(d.name) / "s.jsonl"
        path.write_text(
            "\n".join(json.dumps(x, ensure_ascii=False) for x in lines), encoding="utf-8"
        )
        return path

    def test_a_filtered_find_after_the_injection_engages_filter_syntax(self):
        path = self._transcript(
            ("mcp__codescout__doc", {"action": "find", "filter": {"kind": {"eq": "bug"}}})
        )

        rec = probe.scan_transcript(path, "librarian")

        self.assertIsNotNone(rec)
        self.assertEqual(1, rec["engaged"]["Filter Syntax"])
        self.assertEqual(0, rec["engaged"]["doc — Event Log"])

    def test_the_same_words_in_a_non_mechanism_write_engage_nothing(self):
        # The self-trigger control this probe's zeros depend on: an agent WRITING about the
        # filter syntax (create_file) is not operating the librarian.
        path = self._transcript(
            ("mcp__codescout__create_file", {"path": "x.md", "content": '"action": "find" "filter"'})
        )

        rec = probe.scan_transcript(path, "librarian")

        self.assertIsNotNone(rec)
        self.assertEqual(0, sum(rec["engaged"].values()))

    def test_a_call_before_the_injection_does_not_count(self):
        path = self._transcript()  # the injection is line 0; add a call that precedes nothing
        lines = path.read_text(encoding="utf-8").split("\n")
        pre = json.dumps(
            {
                "type": "assistant",
                "message": {
                    "content": [
                        {
                            "type": "tool_use",
                            "name": "mcp__codescout__doc",
                            "input": {"action": "event_list"},
                        }
                    ]
                },
            },
            ensure_ascii=False,
        )
        # Put the call BEFORE the injection line.
        path.write_text("\n".join([pre] + lines), encoding="utf-8")

        rec = probe.scan_transcript(path, "librarian")

        self.assertIsNotNone(rec)
        self.assertEqual(0, rec["engaged"]["doc — Event Log"])

    def test_a_topic_with_rules_is_scored_only_with_its_own_rules(self):
        # The same transcript scanned as `tracker-conventions` is None: no tracker-conventions
        # injection in it, so a `librarian` rule cannot leak across the topic boundary.
        path = self._transcript(("mcp__codescout__doc", {"action": "find", "filter": {}}))

        self.assertIsNone(probe.scan_transcript(path, "tracker-conventions"))


class ReportScoresOnlyRuledSections(unittest.TestCase):
    def test_unruled_sections_are_named_not_counted_as_never_engaged(self):
        import contextlib
        import io

        sizes = {"(preamble)": 100, "Ruled": 1000, "Unruled": 9000}
        rec = {"engaged": {"Ruled": 0}}
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            probe.report([rec], sizes, "t", {"Ruled"})
        out = buf.getvalue()

        # 100 (preamble) + 1000 (ruled), NOT the 9,000 B section nothing could score.
        self.assertIn("delivered 1,100 B", out)
        self.assertIn("NOT SCORED", out)
        self.assertIn("Unruled (9,000 B)", out)


if __name__ == "__main__":
    unittest.main()
