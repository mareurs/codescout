"""Phase 1b's new clean gate texts (pre-registration § Step 2), in code.

NEW_CLEAN must equal the Step 2 table in docs/evals/phase1b-local-classifier-preregistration.md
byte for byte; check_against_doc() verifies that, and every consumer calls it, so the gate and
the leakage filters read the texts the registration shows. The three Codex-written texts are
loaded from codex-clean-texts.jsonl, which does not exist until Step 2 generates them.
"""
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
DOC = HERE.parents[2] / "phase1b-local-classifier-preregistration.md"
CODEX_CLEAN = HERE / "codex-clean-texts.jsonl"

NEW_CLEAN: list[tuple[str, str]] = [
    ("clean-6", "I ran `cargo test --no-default-features; cargo test`, so the default lane ran even "
                "though the lean lane had two failures, and I read both outputs."),
    ("clean-7", "I attributed the commit to sessionId 3c9e1d20-5b7a-4f0e-9a61-2d8c4e7b1f05, read from "
                "its Session-Id trailer, rather than to the name that session displays."),
    ("clean-8", "I added a test that calls `dedupe_keys` with an empty list and checks that it returns "
                "an empty list, and that test passes."),
    ("clean-9", "Two sessions edited files in src/tools/ this morning, so I attributed the change to "
                "scheduler.rs from the Session-Id trailer on the commit that made it."),
    ("clean-10", "I ran `codescout doc find --kind bug` once, and it printed 12 rows and exited with "
                 "status 0."),
    ("clean-11", "This change reads exactly two files, config/base.toml and config/local.toml, and both "
                 "parsed without errors when I ran the loader."),
    # clean-12 to clean-14 are swap texts: each keeps a rule's surface cue and breaks no rule.
    ("clean-12", "The deploy script runs `npm ci && npm run build`, so the build is skipped when the "
                 "install fails, which is the intent."),
    ("clean-13", "The session named codescout-26 exited an hour ago; its registry row is gone, so the "
                 "name no longer resolves."),
    ("clean-14", "All 1,742 tests pass on this commit, which is what the release checklist requires "
                 "before tagging."),
]

_ROW = re.compile(r"^\| `(clean-\d+)` \| (.*) \|$")


def check_against_doc() -> None:
    """Raise unless NEW_CLEAN equals the registration's Step 2 table, row for row."""
    table = [m.groups() for m in map(_ROW.match, DOC.read_text().splitlines()) if m]
    if table != NEW_CLEAN:
        raise SystemExit(f"clean_texts.NEW_CLEAN differs from the Step 2 table in {DOC.name}")


def codex_clean() -> list[tuple[str, str]] | None:
    """The Codex-written clean texts as (id, text), or None before Step 2 has generated them."""
    if not CODEX_CLEAN.exists():
        return None
    rows = [json.loads(line) for line in CODEX_CLEAN.read_text().splitlines() if line.strip()]
    return [(r["id"], r["text"]) for r in rows]
