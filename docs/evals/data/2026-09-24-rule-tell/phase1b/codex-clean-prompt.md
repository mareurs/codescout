# Codex clean-text prompt — phase 1b, Step 2 (fixed; registered in the phase-1b pre-registration)

Write three short passages of ordinary engineering prose, each one to three sentences, of the
kind a developer writes in a work log, a commit message or a code review.

Each passage must break NONE of the 14 rules in `menu.json`. Judge by each rule's law text; the
spec under it describes the usual violation shape.

Each passage should touch the subject of at least one rule (tests, test lanes, sessions,
attribution, commands, queries or selectors, guards, documentation) without breaking it. Use a
different rule's subject in each passage.

Do not mention the rules, the menu or this task in the passages.

Output three JSON objects, one per line, nothing else:

    {"id": "codex-clean-1", "text": "<passage>"}
    {"id": "codex-clean-2", "text": "<passage>"}
    {"id": "codex-clean-3", "text": "<passage>"}
