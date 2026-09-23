#!/usr/bin/env python3
"""Emit BLIND scoring tasks for the rule-tell prompts.

WHAT THIS EMITS
    One JSONL record per (prompt, passage) pair, carrying exactly three fields:

        {"task_id": "...", "prompt_id": "RTD-3", "prompt": "<verbatim prompt>",
         "passage_id": "CTL3-1", "text": "<verbatim passage>"}

    By default it emits the FULL CROSS PRODUCT -- every prompt against every
    passage, not just the passages collected for that prompt. The diagonal
    measures precision within the shape a prompt was designed for; the
    off-diagonal measures CROSS-TALK, which is the deployment question. Five
    prompts that each look precise in isolation and all fire on everything
    produce a channel nobody reads. Pass --diagonal to restrict.

WHAT THIS STRIPS, AND WHY EACH ONE LEAKS
    The corpus (docs/evals/rule-tell-controls.md) carries per passage:

      why_it_resembles  -- opens with the literal words "full-shape" or
                           "near-miss", i.e. the expected answer.
      never_corrected   -- reads "withheld" on the five blind POSITIVES and
                           names a commit on every control. It is the answer key.
      source            -- a file:line into this repository. A judge that
                           recognises the provenance can infer the label, and
                           for the positives the source is the very document
                           the violation was corrected in.

    A scorer must see the prompt and the text. Nothing else.

WHAT THIS CANNOT BLIND
    The passages are verbatim prose from this repository. A judge given the
    whole repository, or one that has read either eval document, is already
    contaminated and this script does not help. Blinding is a property of the
    JUDGE's context, not of this file. Run the scorer with no repository access
    and no prior turn about this experiment.

SELF-CHECK
    Before writing anything, every emitted record is scanned for the strings
    this script claims to have removed. A leak ABORTS with exit 2 and writes
    nothing -- a blinding step you never verified is one you should not trust.

USAGE
    python3 scripts/blind-rule-tell-tasks.py [--diagonal] [-o FILE]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CONTROLS = REPO / "docs/evals/rule-tell-controls.md"
PROMPTS = REPO / "docs/evals/rule-tell-detection.md"

# A correctly-extracted `text` is the INSIDE of a fence; every metadata field lives
# outside one. So a field label appearing within an emitted text means the fence
# boundaries were misread — that is diagnostic, and it cannot collide with prose.
#
# An earlier version of this guard scanned for the answer-key VOCABULARY instead
# (`withheld`, `near-miss`, `full-shape`). It fired immediately, on a passage that
# quotes CLAUDE.md prose using the word "withheld" in its ordinary sense. That is
# the same defect this eval's own RTD-10 prompt has: a detector keyed on a surface
# string that is not diagnostic of the thing it names. Worse, it would have been
# invisible had no passage happened to quote that word — a guard that passes and
# discriminates nothing. Keyed on structure instead.
FIELD_LABEL_RE = re.compile(
    r"^- \*\*(?:source|why_it_resembles|never_corrected|for_prompt|text)\b", re.M
)
ALLOWED_KEYS = {"task_id", "prompt_id", "prompt", "passage_id", "text", "parts"}

PASSAGE_RE = re.compile(r"^### Passage (\S+) —", re.M)
FOR_PROMPT_RE = re.compile(r"^- \*\*for_prompt:\*\*\s*(\S+)", re.M)


def fenced_blocks(chunk: str) -> list[str]:
    """Every ```-fenced block in `chunk`, in order.

    Deliberately simple: the corpus uses plain triple-backtick fences with no
    info string and no nesting. A passage yielding anything other than exactly
    one block is a parse failure, not something to paper over -- see
    `parse_controls`. The CommonMark subtleties (four backticks wrapping three,
    closers with trailing text) are real and already cost this project a phantom
    127-entry gap; they are handled here by REFUSING the ambiguous case rather
    than by guessing at it.
    """
    out, buf, inside = [], [], False
    for line in chunk.splitlines():
        if line.strip() == "```":
            if inside:
                out.append("\n".join(buf))
                buf, inside = [], False
            else:
                inside = True
            continue
        if inside:
            buf.append(line)
    if inside:
        raise ValueError("unterminated fence")
    return out


def passage_texts(chunk: str) -> list[str]:
    """Every `- **text…:**` block in a passage, in order.

    Most passages carry one. The `contradiction` controls carry TWO — `text (A)`
    and `text (B)` — necessarily, since that prompt asks whether two passages of
    one document conflict. Anchoring on the label rather than on "the only fence
    in the chunk" is what lets both shapes parse without guessing: a fenced block
    elsewhere in the record (inside `why_it_resembles`, say) is never mistaken
    for the passage.
    """
    out: list[str] = []
    lines = chunk.splitlines()
    i = 0
    while i < len(lines):
        if lines[i].lstrip().startswith("- **text"):
            j = i + 1
            while j < len(lines) and lines[j].strip() != "```":
                j += 1
            if j >= len(lines):
                raise SystemExit("text label with no following fence")
            buf, j = [], j + 1
            while j < len(lines) and lines[j].strip() != "```":
                buf.append(lines[j])
                j += 1
            if j >= len(lines):
                raise SystemExit("unterminated fence after a text label")
            out.append("\n".join(buf))
            i = j
        i += 1
    return out


def parse_controls(md: str) -> list[dict[str, str]]:
    starts = [(m.group(1), m.start()) for m in PASSAGE_RE.finditer(md)]
    # The file documents its own record shape with a literal `### Passage <id> — …`
    # exemplar. It has no for_prompt and no text, and counting it would put the
    # task total one above the corpus. Skip any placeholder id.
    starts = [(pid, s) for pid, s in starts if "<" not in pid]
    if not starts:
        raise SystemExit("no passages found — has the corpus heading shape changed?")
    bounds = [s for _, s in starts] + [len(md)]
    rows = []
    for i, (pid, _) in enumerate(starts):
        chunk = md[bounds[i] : bounds[i + 1]]
        fp = FOR_PROMPT_RE.search(chunk)
        if not fp:
            raise SystemExit(f"{pid}: no for_prompt field")
        texts = passage_texts(chunk)
        if not texts:
            raise SystemExit(f"{pid}: no text block")
        # Multi-part passages are joined into one document for the judge. THE
        # DISTANCE IS NOT SIMULATED, and that is a real limitation of this
        # harness rather than an implementation detail: the finding that
        # motivated the contradiction prompt sat 78 lines and six `##` headings
        # apart, with the correction between them. Presenting A and B adjacent
        # makes the task strictly easier than the case it was written for, so a
        # pass here does NOT establish whole-document scope. Recorded in the
        # emitted record as `parts`, so a scorer can report multi-part results
        # separately.
        text = "\n\n[…]\n\n".join(texts)
        rows.append(
            {"passage_id": pid, "for_prompt": fp.group(1), "text": text, "parts": len(texts)}
        )
    return rows


def parse_prompts(md: str) -> dict[str, str]:
    """The prompts live under `## Tells — the bool prompts`, one fenced block each,
    under a subsection whose heading names the case it serves.

    Keys must match the corpus's `for_prompt` values: four are `RTD-N`, and the
    fifth serves no `CLAUDE.md` rule at all, so its heading carries no id and the
    corpus calls it `contradiction`. Keying that one `UNWRITTEN` would silently
    drop every one of its passages from the cross product — a join that yields
    fewer rows and no error.
    """
    try:
        section = md.split("## Tells — the bool prompts", 1)[1]
    except IndexError:
        raise SystemExit("prompts section not found in " + str(PROMPTS))
    section = section.split("\n## ", 1)[0]
    out: dict[str, str] = {}
    heads = [(m.start(), m.group(0)) for m in re.finditer(r"^### .*$", section, re.M)]
    bounds = [s for s, _ in heads] + [len(section)]
    for i, (_, head) in enumerate(heads):
        chunk = section[bounds[i] : bounds[i + 1]]
        blocks = fenced_blocks(chunk)
        if not blocks:
            continue
        key = re.search(r"RTD-\d+", head)
        if key:
            out[key.group(0)] = blocks[0]
        elif "contradiction" in head.lower():
            out["contradiction"] = blocks[0]
        else:
            raise SystemExit(f"prompt heading names no case this corpus uses: {head!r}")
    if not out:
        raise SystemExit("no prompts parsed — has the tells section changed shape?")
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--diagonal", action="store_true", help="only score each prompt against its own passages")
    ap.add_argument("-o", "--out", type=Path, help="write here instead of stdout")
    args = ap.parse_args()

    passages = parse_controls(CONTROLS.read_text())
    prompts = parse_prompts(PROMPTS.read_text())

    records = []
    for prompt_id, prompt_text in sorted(prompts.items()):
        for p in passages:
            if args.diagonal and p["for_prompt"] != prompt_id:
                continue
            records.append(
                {
                    "task_id": f"{prompt_id}::{p['passage_id']}",
                    "prompt_id": prompt_id,
                    "prompt": prompt_text,
                    "passage_id": p["passage_id"],
                    "text": p["text"],
                    "parts": p["parts"],
                }
            )

    blob = "\n".join(json.dumps(r, ensure_ascii=False) for r in records)

    # Two structural checks. Neither can be satisfied by a passage that merely
    # discusses this experiment's vocabulary.
    stray_keys = sorted({k for r in records for k in r} - ALLOWED_KEYS)
    if stray_keys:
        print(f"BLINDING FAILED — records carry fields beyond the allowed set: {stray_keys}", file=sys.stderr)
        print("Nothing written.", file=sys.stderr)
        return 2
    mislabelled = [r["task_id"] for r in records if FIELD_LABEL_RE.search(r["text"])]
    if mislabelled:
        print(
            f"BLINDING FAILED — {len(mislabelled)} text field(s) contain a metadata field label, "
            f"so a fence boundary was misread: {mislabelled[:5]}",
            file=sys.stderr,
        )
        print("Nothing written.", file=sys.stderr)
        return 2

    header = (
        f"# {len(records)} tasks · {len(prompts)} prompts × {len(passages)} passages"
        f"{' (diagonal only)' if args.diagonal else ' (full cross product)'}\n"
    )
    sys.stderr.write(header)
    sys.stderr.write("# blinding self-check: passed — allowed fields only, no misread fence\n")
    if args.out:
        args.out.write_text(blob + "\n")
        sys.stderr.write(f"# wrote {args.out}\n")
    else:
        print(blob)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
