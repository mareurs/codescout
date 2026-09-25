"""The Stage 3 segmenter: the units a sentence x rule head scores (phase-1 pre-registration).

Stage 3: "sentences segmented by code, with fences and table rows treated as one unit each".
Committed before the synthetic pilot (synthetic amendment, correction 8), because a synthetic
pair's label is only sentence-level if its violating and fixed sentences are each exactly one
unit under THIS function.

- a fenced block (``` or ~~~, through its closing fence) is one unit;
- a table row (a line starting with `|`) is one unit;
- a heading line is one unit;
- a list item starts a new block; prose within a block is split with the miner's own
  SENT_SPLIT, so mined and synthetic sentences break at the same places;
- whitespace inside a unit is normalised to single spaces.
"""
import re

SENT_SPLIT = re.compile(r"(?<=[.!?])\s+(?=[*_`\"(A-Z0-9])")   # = mine_pairs.SENT_SPLIT
FENCE = re.compile(r"^\s*(```|~~~)")
BULLET = re.compile(r"^\s*(?:[-*+]|\d+[.)])\s+")


def norm(s: str) -> str:
    return re.sub(r"\s+", " ", s).strip()


def segment(text: str) -> list[str]:
    units: list[str] = []
    block: list[str] = []

    def flush():
        if block:
            units.extend(u for u in (norm(s) for s in SENT_SPLIT.split(norm(" ".join(block)))) if u)
            block.clear()

    lines = text.splitlines()
    i = 0
    while i < len(lines):
        ln = lines[i]
        m = FENCE.match(ln)
        if m:
            flush()
            j = i + 1
            while j < len(lines) and not lines[j].strip().startswith(m.group(1)):
                j += 1
            units.append(norm("\n".join(lines[i:j + 1])))
            i = j + 1
            continue
        s = ln.strip()
        if not s:
            flush()
        elif s.startswith("|") or s.startswith("#"):
            flush()
            units.append(norm(s))
        elif BULLET.match(ln):
            flush()
            block.append(BULLET.sub("", ln, count=1))
        else:
            block.append(s)
        i += 1
    flush()
    return units


def is_one_unit(sentence: str, paragraph: str) -> bool:
    """True iff `sentence` is exactly one unit of `paragraph` under segment()."""
    return norm(sentence) in segment(paragraph)
