"""Regression coverage for scripts/probe_augmentation_restore.py's declared-sidecar count.

docs/issues/2026-09-10-probe-augmentation-restore-counts-prose-mentions-as-declarations.md:
`find_declarers` used to anchor on "line starts with `expects_augmentation:`" — which a
documentation example inside a fenced YAML code block also satisfies, since fence content
starts at column 0 exactly like a real frontmatter line. That counted a *mention* of the
convention as a *declaration*, inflating the probe's denominator until "documenting the
mechanism" made the probe fail on a corpus that restored everything correctly. The fix
anchors on the YAML frontmatter block itself, not on line-start alone.
"""

import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts/probe_augmentation_restore.py"
SPEC = importlib.util.spec_from_file_location("probe_augmentation_restore", SCRIPT)
probe = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = probe
SPEC.loader.exec_module(probe)


class FindDeclarersControls(unittest.TestCase):
    def test_a_mention_in_a_fenced_example_is_not_counted_as_a_declaration(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            (repo / "real-tracker.md").write_text(
                "---\n"
                "kind: tracker\n"
                "title: Test tracker\n"
                "expects_augmentation: sidecar.yaml\n"
                "---\n"
                "\n"
                "Body text.\n"
            )
            (repo / "sidecar.yaml").write_text('prompt: "test prompt"\n')
            # LOAD-BEARING: this line starts at column 0, matches the DECL prefix
            # exactly, and names a real-looking `.yaml` path -- the same shape a
            # genuine frontmatter declaration has. It sits inside a fenced code
            # example in the BODY, not in frontmatter, which is what the fix must
            # tell apart. Delete this file (or move the line into frontmatter) and
            # this test stops discriminating: it would pass whether or not the
            # frontmatter-anchoring fix is present.
            (repo / "mentions-only.md").write_text(
                "# How to declare an augmentation\n"
                "\n"
                "Say so in frontmatter, naming the sidecar:\n"
                "\n"
                "```yaml\n"
                "expects_augmentation: sidecar.yaml\n"
                "```\n"
                "\n"
                "That is documentation, not a declaration.\n"
            )

            pairs = probe.find_declarers(repo)

        self.assertEqual(1, len(pairs), pairs)
        self.assertEqual(Path("real-tracker.md"), pairs[0][0])


if __name__ == "__main__":
    unittest.main()
