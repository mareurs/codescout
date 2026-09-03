#!/usr/bin/env python3
"""Which SECTIONS of an auto-injected guide topic does a session go on to engage?

Motivating question (docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md):
`tracker-conventions` is ~39 KB delivered whole. Splitting it is gated on knowing which of
its sections real sessions actually engage. This probe measures that.

WHAT EACH PREDICATE LITERALLY COUNTS
------------------------------------
injection
    One occurrence of the OPENING auto-injection marker
        <!-- auto-injected get_guide('TOPIC') — first call this session ... -->
    inside a `tool_result` block on a `type:"user"` line. Per the 2026-08-27 parsing
    contract (docs/evals/data/2026-08-27-guide-injection/rubric-BRIEF.md § 2), which
    documents three traps this honours:
      * every injection carries BOTH an opening and a closing marker -> count openings only;
      * `{topic}` / `{}` are the literal Rust format string from src/tools/core/types.rs
        and appear in transcripts that merely READ that source -> only real topic names;
      * a marker in assistant `text` or a user's typed message is a DISCUSSION mention,
        not a delivery -> only markers inside a tool_result count.

section-attributable activity
    A `tool_use` block on an `assistant` line, at a LATER line index in the same
    transcript than the injection, whose tool name is a MECHANISM TOOL (artifact* /
    librarian*) and whose serialized input matches one of the SECTION_SIGNATURES below.

    The mechanism-tool restriction is load-bearing, not tidiness. Matching signature
    strings against ANY tool input makes the probe fire on sessions that merely WRITE
    ABOUT these mechanisms -- docs, guides, this script. Measured 2026-08-31 on the
    positive control: the single `create_file` call that wrote this file has an input
    containing `link_scan`, `entry_prefix`, `expects_augmentation`, `render_template`,
    `params_schema`, `**Valid:**`, `patch-id` and `docs/issues/archive/`, and scored the
    session as engaging ALL SIX sections at 100%. In a repo whose sessions routinely
    write about the librarian, that channel is the common case, not an edge case.

    *** THIS MEASURES RELEVANCE, NOT CAUSATION. ***
    It cannot distinguish "did this because the guide said so" from "would have done it
    anyway". The asymmetry is what makes it usable, and it only points one way:

      - a section with ZERO post-injection activity is STRONG evidence of waste. The
        session never engaged that subject matter at all, so those bytes cannot have
        been used, whatever the agent was thinking.
      - a section WITH activity is WEAK evidence of use. It is an upper bound; the
        activity may be entirely guide-independent.

    So: trust the zeros, discount the non-zeros. Any conclusion this probe supports must
    run in the zero direction.

WHAT IT CANNOT SEE
------------------
* Prose citation (the 08-27 study's `U3_CITED`) and echo (`U1_ECHO`). Those need a reader.
  This is deliberately the behavioural arm only -- which is the arm that study called
  decision-relevant, because it is the one unaffected by redacted thinking text.
* Reasoning. Thinking text became visible in transcripts on 2026-08-27
  (llm-proxy:6f3cb62); this probe does not read it. A session that read a section and
  correctly decided to do nothing is scored identically to one that never read it.
* Anything in a session whose transcript has been deleted. See --frame-attrition:
  76 of the 106 sessions in the frozen 2026-08-27 frame were already gone by 2026-08-31,
  and the attrition is severely structured (5% survival in .claude vs 71% in .claude-sdd).
  Do NOT treat a disk scan as a re-read of that frame.

USAGE
-----
    python3 scripts/probe_guide_section_use.py                    # default topic, all profiles
    python3 scripts/probe_guide_section_use.py --topic librarian
    python3 scripts/probe_guide_section_use.py --since 2026-08-27
    python3 scripts/probe_guide_section_use.py --frame-attrition  # why the old frame is unusable
    python3 scripts/probe_guide_section_use.py --session <uuid>   # positive control on one session
    python3 scripts/probe_guide_section_use.py --json
"""

from __future__ import annotations

import argparse
import collections
import json
import os
import re
import sys
from pathlib import Path

# --- topics ------------------------------------------------------------------------
# The ten real topic names. Trap 2 above: accept only these, never a bare `{topic}`.
TOPICS = [
    "error-handling",
    "iron-laws-detail",
    "librarian",
    "librarian-runtime",
    "progressive-disclosure",
    "project-activation-bootstrap",
    "symbol-navigation",
    "tracker-conventions",
    "untrusted-content",
    "workspace-state",
]

PROFILES = [".claude", ".claude-sdd", ".claude-kat"]

# Only calls that OPERATE the librarian mechanism count as activity. A call that merely
# mentions it in a payload (writing a guide, a doc, this script) does not -- see the
# module docstring's measured self-trigger. Substring match, so it covers the MCP-prefixed
# forms (`mcp__codescout__doc`, `mcp__codescout__artifact`, `artifact_augment`, ...).
#
# BOTH names are listed and the union is REQUIRED, not tidiness. The 2026-09-02 tool
# collapse renamed `artifact` / `artifact_event` / `artifact_augment` / `artifact_refresh`
# to `doc` (`6e8b4170`, `dac1068a`), but the corpus this probe reads is HISTORICAL: a
# transcript written before the collapse carries `artifact`, one written after carries
# `doc`, and a session keeps its old binary until `cargo rb` + `/mcp`. Drop either name
# and the probe goes blind in one direction while its output stays well-formed and
# differentiated -- the worst shape, because nothing signals the change. Measured
# 2026-09-03: 9 of 1,382 relevant transcripts already carried the new name, and every
# future session moves that ratio the same way, so a run today is nearly correct and the
# same command in a month is materially wrong.
# docs/issues/2026-09-03-probe-mechanism-filter-omits-the-renamed-doc-tool.md
#
# `"doc"` is slightly greedy under a substring test: it would also match a hypothetical
# `mcp__codescout__docs_*`. No such tool exists in the registry today. If one is added,
# tighten `is_mechanism_tool` to match the full `mcp__codescout__<name>` form rather than
# widening this tuple further -- widening is what made this defect possible.
MECHANISM_TOOLS = ("artifact", "doc", "librarian")

GUIDE_DIR = Path(__file__).resolve().parent.parent / "src" / "prompts" / "guides"
FROZEN_FRAME = (
    Path(__file__).resolve().parent.parent
    / "docs/evals/data/2026-08-27-guide-injection/corpus-frame.json"
)


"""Input keys whose values are free prose an agent WROTE, not parameters it SET.

Stripping these is what separates operating the mechanism from writing about it. A bug
file whose body discusses `expects_augmentation` is not a session declaring one -- but
its `artifact(action="create")` call carries that word in `body`, and scored 26 hits on
`Declaring an augmentation` in the 2026-08-31 positive control before this existed.

This matters more than the symmetric error: a false positive turns a true zero into a
non-zero, and the zeros are the only direction this probe is licensed to conclude in.
"""
PROSE_KEYS = {
    "body",
    "content",
    "text",
    "title",
    "summary",
    "prompt",
    "message",
    "description",
    "new_string",
    "old_string",
    "semantic",
    "query",
    "notes",
}

# A string value longer than this is prose an agent composed, not a parameter it set.
MAX_PARAM_CHARS = 200

# Values here are topical labels about the subject, never parameters operating on it.
LABEL_KEYS = {"tags", "owners"}

# Author-supplied metadata: the KEYS are structural signal (`entry_high_water_GF`),
# the VALUES are prose. Keep the former, drop the latter.
KEYS_ONLY_CONTAINERS = {"extra"}


def strip_prose(obj):
    """Blank out prose VALUES while keeping every key NAME and every short parameter.

    Three rules, each closing a leak found by the 2026-08-31 positive control:

    * a value under a PROSE_KEYS key is blanked -- but the key survives, because
      `patch={"extra": {"unverified": "..."}}` carries its signal in the key and IS
      mechanism operation;
    * `tags` and `owners` values are blanked. Tags are topical labels DESCRIBING the
      subject -- a bug filed about `append_entry` carries it as a tag, which is the
      purest form of describe-not-operate. `extra` keeps its keys and blanks its
      values, so `extra: {"entry_high_water_GF": 8}` still counts (a real allocator
      write) while `extra: {"unverified": "...append_entry..."}` no longer does;
    * any string value longer than MAX_PARAM_CHARS is blanked, whatever its key. Short
      strings are parameters (`"create"`, `"bug"`, `"docs/issues/x.md"`); long ones are
      prose an agent composed. This is the general form of the rule, and it catches the
      leak a fixed key list cannot -- a signature word buried in an unanticipated
      long-text field.
    """
    if isinstance(obj, dict):
        out = {}
        for k, v in obj.items():
            if k in PROSE_KEYS:
                out[k] = ""
            elif k in LABEL_KEYS:
                # Keep the key and the shape; discard what the labels SAY.
                out[k] = [""] * len(v) if isinstance(v, list) else ""
            elif k in KEYS_ONLY_CONTAINERS and isinstance(v, dict):
                out[k] = {ik: "" for ik in v}
            else:
                out[k] = strip_prose(v)
        return out
    if isinstance(obj, list):
        return [strip_prose(v) for v in obj]
    if isinstance(obj, str) and len(obj) > MAX_PARAM_CHARS:
        return ""
    return obj



def is_mechanism_tool(name: str) -> bool:
    """True for calls that OPERATE the librarian, false for calls that merely mention it."""
    return any(t in name for t in MECHANISM_TOOLS)



def opening_marker(topic: str) -> str:
    """The opening form only. The closing form is preceded by 'end ' (trap 1)."""
    return f"auto-injected get_guide('{topic}') — first call"


# --- section signatures ------------------------------------------------------------
# Each rule: (section-name, tool-name or None for any, [substrings that must ALL appear
# in the serialized tool input]). Deliberately NARROW -- a rule that fires on ordinary
# activity would manufacture use where there is none, and this probe's whole value is in
# its zeros. Each entry names what it misses.
SECTION_SIGNATURES: dict[str, list[tuple[str | None, list[str]]]] = {
    # Archive flow, bug-file frontmatter vocabulary, patch-id citation.
    # MISSES: reading an existing bug file (that is `Querying`), and writing a bug file
    # via native Write/Edit rather than the artifact tool.
    "Bug files (docs/issues/)": [
        (None, ["docs/issues/archive/"]),
        (None, ["docs/issues/", '"kind": "bug"']),
        (None, ["patch-id"]),
        (None, ["unverified"]),
        (None, ['"status": "fixed"']),
        (None, ['"status": "mitigated"']),
    ],
    # Tracker creation and in-place archival.
    # MISSES: appending to a tracker (that is `Entry-level standard`).
    "Tracker artifacts (docs/trackers/)": [
        (None, ['"kind": "tracker"']),
        (None, ['"status": "archived"']),
        (None, ["docs/trackers/archive/"]),
    ],
    # Augmentation shape: the sidecar, the export fix, artifact_augment itself.
    "Declaring an augmentation": [
        ("artifact_augment", []),
        (None, ["expects_augmentation"]),
        (None, ["export_augmentations"]),
        (None, ["render_template"]),
        (None, ["params_schema"]),
    ],
    # The entry-grain rules: allocation, headings, required fields, the high-water mark.
    # MISSES, deliberately: `**Valid:**` / `**Rests on:**` written into a body. Those are
    # real compliance, but they live in prose, and any rule reading prose reopens the
    # describe-vs-operate leak that costs this probe its zeros. The structural signals
    # here already cover the section; the marginal extra signal is not worth the trade.
    "Entry-level standard — the shape INSIDE a tracker": [
        (None, ["append_entry"]),
        (None, ["update_entry"]),
        (None, ["entry_prefix"]),
        (None, ["id_prefix"]),
        (None, ["anchor_heading"]),
        (None, ["entry_high_water"]),
    ],
    # Catalog queries: the triage filter, entry-grain filtering, archived visibility.
    "Querying with the librarian": [
        (None, ["entry_filter"]),
        (None, ["include_archived"]),
        (None, ['"action": "find"', "status"]),
        (None, ['"action": "get"', "heading"]),
    ],
    # The citation graph.
    "Cross-linking (edges are derived — cite in prose)": [
        (None, ["link_scan"]),
        (None, ['"action": "link"']),
        (None, ["include_links"]),
        (None, ['"action": "graph"']),
    ],
}


def current_section_bytes(topic: str) -> dict[str, int]:
    """Section byte sizes of the guide AS IT IS ON DISK NOW.

    Deliberately recomputed rather than read from the 2026-08-27 `guide_sections.json`:
    that snapshot has `tracker-conventions` at 34,333 B and the live topic measured
    39,106 B on 2026-08-31, so the frozen sizes would misweight a decomposition argument
    by ~14%. A probe that dates itself is worth more than one that inherits a number.

    FENCE-AWARE, and that is not a nicety. `tracker-conventions` § *Entry headings*
    contains a fenced worked example whose first line is literally

        ## <ID> — <title>          token, whitespace, dash (— – -), whitespace, text

    A naive line-start split treats it as a real heading: it invents a 12,124 B phantom
    section, STEALS those bytes from `Entry-level standard` (17,240 -> 5,116), and --
    having no signature rules -- reports the phantom as never engaged, i.e. a fabricated
    "31% of the topic is dead" pointing exactly the way an author hoping to split the
    file would want. Caught 2026-08-31 by the positive control, and by disagreeing with
    the frozen guide_sections.json on the SECTION COUNT (7 vs 8).

    The guide being measured documents this very trap for its own field detection:
    "a line inside a fenced code block is skipped -- a worked example teaching the
    syntax is never mistaken for a declaration, including the examples on this page."
    """
    path = GUIDE_DIR / f"{topic}.md"
    if not path.exists():
        return {}
    out: dict[str, int] = {}
    current = "(preamble)"
    buf: list[str] = []
    in_fence = False
    for line in path.read_text(encoding="utf-8").splitlines(keepends=True):
        stripped = line.lstrip()
        if stripped.startswith("```") or stripped.startswith("~~~"):
            in_fence = not in_fence
            buf.append(line)
            continue
        if line.startswith("## ") and not in_fence:
            out[current] = out.get(current, 0) + len("".join(buf).encode())
            current = line[3:].strip()
            buf = []
        else:
            buf.append(line)
    out[current] = out.get(current, 0) + len("".join(buf).encode())
    return out
def topics_with_rules() -> list[str]:
    """Topics whose section headings intersect `SECTION_SIGNATURES`' keys.

    DERIVED, never hand-listed. `SECTION_SIGNATURES` is keyed by section HEADING, so the
    set of topics it can measure is a function of the guides on disk right now. A
    hand-maintained list here would be the same defect this function exists to guard --
    `cluster/selector-narrower-than-its-population`, whose sibling instance in this very
    file was `MECHANISM_TOOLS` going stale across a tool rename.

    A topic whose guide file is missing is reported as unmeasurable rather than crashing:
    the caller's job is to refuse the run, and a missing guide is one reason to.
    """
    out = []
    for t in TOPICS:
        try:
            sections = current_section_bytes(t)
        except (FileNotFoundError, OSError):
            continue
        if set(sections) & set(SECTION_SIGNATURES):
            out.append(t)
    return out


def iter_transcripts(profiles: list[str], roots: list[str] | None) -> list[tuple[str, Path]]:
    """Yield (profile-label, transcript-path).

    Default: this machine's three Claude Code profile dirs. With --roots, any directory
    tree instead -- e.g. a copy of another machine's transcripts, which is how the
    second-observer check is run without needing that machine to hold this script.
    The label is the root's own directory name, so `.../transcripts/{main,sdd,kat}`
    self-labels correctly.
    """
    out: list[tuple[str, Path]] = []
    if roots:
        for r in roots:
            root = Path(r).expanduser()
            if root.is_dir():
                label = root.name
                out.extend((label, p) for p in root.rglob("*.jsonl"))
        return out
    home = Path.home()
    for prof in profiles:
        root = home / prof / "projects"
        if root.is_dir():
            out.extend((prof, p) for p in root.rglob("*.jsonl"))
    return out


def blocks(msg) -> list:
    """message.content is a string OR a list of blocks (parsing contract)."""
    if isinstance(msg, dict):
        c = msg.get("content")
        if isinstance(c, list):
            return c
        if isinstance(c, str):
            return [{"type": "text", "text": c}]
    return []


def scan_transcript(path: Path, topic: str, profile: str = "?") -> dict | None:
    """Return per-session record, or None if the topic was never injected here."""
    marker = opening_marker(topic)
    try:
        raw = path.read_bytes()
    except OSError:
        return None
    # Fast reject: only fully parse transcripts that mention the opening form at all.
    if marker.encode() not in raw:
        return None

    injections: list[dict] = []
    discussion_mentions = 0
    # Session start from the transcript's own first `timestamp`, NOT from mtime --
    # mtime is a property of the file, and a copied corpus may not preserve it.
    session_date: str | None = None
    # (line_index, tool_name, serialized_input) for every assistant tool_use
    tool_uses: list[tuple[int, str, str]] = []

    for idx, line in enumerate(raw.splitlines()):
        if not line.strip():
            continue
        try:
            d = json.loads(line)
        except (json.JSONDecodeError, UnicodeDecodeError):
            continue
        if session_date is None and d.get("timestamp"):
            session_date = str(d["timestamp"])[:10]
        typ = d.get("type")
        msg = d.get("message")
        if typ == "assistant":
            for b in blocks(msg):
                if isinstance(b, dict) and b.get("type") == "tool_use":
                    # Structural skeleton only -- see strip_prose.
                    tool_uses.append(
                        (
                            idx,
                            b.get("name", ""),
                            json.dumps(strip_prose(b.get("input", {})), ensure_ascii=False),
                        )
                    )
                elif isinstance(b, dict) and b.get("type") == "text":
                    if marker in (b.get("text") or ""):
                        discussion_mentions += 1  # trap 3
        elif typ == "user":
            for b in blocks(msg):
                if not isinstance(b, dict):
                    continue
                if b.get("type") == "tool_result":
                    content = b.get("content")
                    text = (
                        content
                        if isinstance(content, str)
                        else " ".join(
                            x.get("text", "")
                            for x in content
                            if isinstance(x, dict)
                        )
                        if isinstance(content, list)
                        else ""
                    )
                    n = text.count(marker)
                    for _ in range(n):
                        injections.append({"line": idx})
                elif b.get("type") == "text" and marker in (b.get("text") or ""):
                    discussion_mentions += 1  # trap 3

    if not injections:
        return None

    first_inj = min(i["line"] for i in injections)
    after = [
        (i, n, s) for (i, n, s) in tool_uses if i > first_inj and is_mechanism_tool(n)
    ]

    engaged: dict[str, int] = {}
    evidence: dict[str, list[str]] = {}
    for section, rules in SECTION_SIGNATURES.items():
        hits = 0
        ev: list[str] = []
        for _, name, struct in after:
            for want_tool, subs in rules:
                if want_tool is not None and want_tool not in name:
                    continue
                if all(s in struct for s in subs) if subs else True:
                    hits += 1
                    if len(ev) < 3:
                        ev.append(f"{name} ~ {want_tool or '*'}:{subs} -- {struct[:150]}")
                    break
        engaged[section] = hits
        evidence[section] = ev

    return {
        "path": str(path),
        "sid": path.stem,
        "profile": profile,
        "project": str(path.parent.name),
        # Claude Code writes subagent transcripts to <project>/<session-uuid>/subagents/.
        # They are a DIFFERENT POPULATION and must never be blended -- see report().
        "kind": "subagent" if "/subagents/" in str(path) else "main",
        "mtime": int(path.stat().st_mtime),
        "session_date": session_date or "?",
        "injections": len(injections),
        "discussion_mentions": discussion_mentions,
        "mechanism_calls_after_first_injection": len(after),
        "engaged": engaged,
        "evidence": evidence,
    }


def frame_attrition() -> None:
    """Why a disk scan is NOT a re-read of the 2026-08-27 frame."""
    if not FROZEN_FRAME.exists():
        print(f"frozen frame not found at {FROZEN_FRAME}", file=sys.stderr)
        return
    frame = json.loads(FROZEN_FRAME.read_text())
    tc = [s for s in frame if "tracker-conventions" in (s.get("topics") or {})]
    alive = [s for s in tc if os.path.exists(s["path"])]
    print("FROZEN FRAME (2026-08-27) vs DISK TODAY -- tracker-conventions sessions")
    print(f"  in frame: {len(tc)}   still on disk: {len(alive)}   gone: {len(tc) - len(alive)}")
    print()
    print("  survival by profile:")
    for p in sorted({s["profile"] for s in tc}):
        a = sum(1 for s in alive if s["profile"] == p)
        t = sum(1 for s in tc if s["profile"] == p)
        print(f"    {p:<14s} {a:>3d}/{t:<3d}  {a / t * 100:5.1f}%")
    av = [s["topics"]["tracker-conventions"] for s in alive]
    gv = [s["topics"]["tracker-conventions"] for s in tc if not os.path.exists(s["path"])]
    if av and gv:
        print()
        print("  injections per session (the guide-heavy tail is what died):")
        print(f"    alive: n={len(av)} mean={sum(av) / len(av):.2f} max={max(av)}")
        print(f"    gone : n={len(gv)} mean={sum(gv) / len(gv):.2f} max={max(gv)}")
    print()
    print("  => attrition is STRUCTURED. Drawing from survivors is a convenience sample,")
    print("     not the frame. Use a freshly built frame and report it as its own period.")


def report(records: list[dict], sizes: dict[str, int], label: str) -> None:
    """One population's table. NEVER call this on main+subagent blended.

    Measured 2026-08-31: blending them yields 71.7% never-engaged, which coincidentally
    matches the 2026-08-27 agent-scored study's 71% for this topic and reads as a
    striking cross-instrument convergence. Split, main sessions are 54.7% and subagents
    88.7% -- so the "agreement" was an artifact of mixing two populations that behave
    nothing alike. That near-miss is why the split is in the instrument rather than in
    the reader's discipline.
    """
    n = len(records)
    total_bytes = sum(sizes.values())
    real = [s for s in sizes if s != "(preamble)"]
    print(f"\n=== {label}  (n={n} sessions) ===")
    if n == 0:
        print("  none -- a statement about the scan, not the world. Check --profiles/--since.")
        return
    engaged_bytes = sum(
        sum(sizes[s] for s in real if r["engaged"].get(s, 0) > 0) for r in records
    )
    delivered = n * total_bytes
    print(f"{'section':<50s} {'bytes':>7s} {'%B':>5s} {'sess':>5s} {'%sess':>6s}")
    print("-" * 78)
    for section in sorted(real, key=lambda s: -sizes[s]):
        eng = sum(1 for r in records if r["engaged"].get(section, 0) > 0)
        print(
            f"{section[:49]:<50s} {sizes[section]:>7,d} "
            f"{sizes[section] / total_bytes * 100:>4.1f}% {eng:>5d} {eng / n * 100:>5.0f}%"
        )
    per = [sum(1 for s in real if r["engaged"].get(s, 0) > 0) for r in records]
    print(
        f"  delivered {delivered:,} B; never engaged "
        f"{delivered - engaged_bytes:,} B ({(delivered - engaged_bytes) / delivered * 100:.1f}%)"
    )
    print(
        f"  sections engaged per session: median {sorted(per)[n // 2]} of {len(real)}; "
        f"{per.count(0)}/{n} engaged NOTHING"
    )
    dead = [s for s in real if not any(r["engaged"].get(s, 0) for r in records)]
    if dead:
        print(f"  engaged by ZERO sessions: {', '.join(s[:38] for s in dead)}")



def main() -> int:
    ap = argparse.ArgumentParser(
        description="Which sections of an injected guide topic do sessions go on to engage?",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    ap.add_argument("--topic", default="tracker-conventions", choices=TOPICS)
    ap.add_argument("--profiles", nargs="*", default=PROFILES)
    ap.add_argument(
        "--roots",
        nargs="*",
        help="scan these directory trees instead of this machine's profile dirs "
        "(e.g. a copy of another machine's transcripts). Label = each root's dir name.",
    )
    ap.add_argument(
        "--machine-label",
        help="what to call the machine these transcripts came FROM. Set it whenever "
        "--roots points at another machine's copy, or the report will claim this host.",
    )
    ap.add_argument("--since", help="only sessions with mtime on/after YYYY-MM-DD")
    ap.add_argument(
        "--split-at",
        metavar="YYYY-MM-DD",
        help="report each population split at this SESSION-START date (from the "
        "transcript's own first timestamp, not mtime). Read the stratification "
        "warning it prints before calling a difference a regime effect.",
    )
    ap.add_argument("--session", help="restrict to one session id (positive control)")
    ap.add_argument(
        "--kind",
        default="all",
        choices=["main", "subagent", "all"],
        help="which population to report (default all, reported SEPARATELY, never blended)",
    )
    ap.add_argument("--frame-attrition", action="store_true", help="show why the frozen frame is unusable")
    ap.add_argument(
        "--explain",
        action="store_true",
        help="print the matching call for every non-zero, so each can be audited by hand",
    )
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    if args.frame_attrition:
        frame_attrition()
        return 0

    # REFUSE rather than report. `--topic` accepts all ten registered topics, but
    # `SECTION_SIGNATURES` is keyed by section HEADING and today only
    # `tracker-conventions`' headings appear there. For any other topic the two key sets
    # are disjoint, so `r["engaged"].get(section, 0)` returns 0 for every section of
    # every session and the report reads `100.0% never engaged` -- a confident figure
    # produced by CONSTRUCTION rather than by measurement, exit 0, no warning.
    #
    # The guard is the fix, and authoring signatures for another topic is a separate
    # change. Shipping signatures without this guard would leave the next eight topics
    # silently broken, which is the trap that made this defect worth filing.
    # docs/issues/2026-09-03-section-use-probe-zeroes-every-untargeted-topic.md
    measurable = topics_with_rules()
    if args.topic not in measurable:
        print(
            f"REFUSING to report on `{args.topic}`: no SECTION_SIGNATURES rule matches "
            f"any section of that topic, so every section would score zero and the run "
            f"would report ~100% never-engaged by construction rather than by "
            f"measurement.\n"
            f"  topics with rules: {', '.join(measurable) if measurable else '(none)'}\n"
            f"  Authoring rules for `{args.topic}` is a separate change -- the guard is "
            f"the fix.\n"
            f"  docs/issues/2026-09-03-section-use-probe-zeroes-every-untargeted-topic.md",
            file=sys.stderr,
        )
        return 2

    since_ts = None
    if args.since:
        import datetime as dt

        since_ts = dt.datetime.strptime(args.since, "%Y-%m-%d").timestamp()

    records = []
    for profile, path in iter_transcripts(args.profiles, args.roots):
        if args.session and args.session not in path.stem:
            continue
        rec = scan_transcript(path, args.topic, profile)
        if rec is None:
            continue
        if since_ts and rec["mtime"] < since_ts:
            continue
        records.append(rec)

    sizes = current_section_bytes(args.topic)
    total_bytes = sum(sizes.values())

    if args.json:
        print(
            json.dumps(
                {
                    "topic": args.topic,
                    "machine": args.machine_label or os.uname().nodename,
                    "profiles": args.roots or args.profiles,
                    "guide_from_checkout": os.uname().nodename,
                    "guide_bytes": total_bytes,
                    "sections": sizes,
                    "sessions": records,
                },
                indent=1,
            )
        )
        return 0

    n = len(records)
    print(f"TOPIC: {args.topic}   sessions with >=1 injection: {n}")
    print(f"guide on disk NOW: {total_bytes:,} B across {len(sizes)} sections")
    # Name the rule-set's scope on EVERY run, not only when it is empty. Per
    # docs/adrs/2026-08-27-negative-results-name-their-scope.md: a zero that is
    # suspicious must name the scope examined. An unruled section reports zero
    # engagement whatever the sessions did, so the reader needs the denominator of
    # RULES beside the denominator of bytes.
    ruled = sorted(set(sizes) & set(SECTION_SIGNATURES))
    print(
        f"rule-set scope: {len(ruled)} of {len(sizes)} sections of this topic have "
        f"SECTION_SIGNATURES rules -- a zero on an UNRULED section is a statement "
        f"about the rules, not about the sessions"
    )
    machine = args.machine_label or os.uname().nodename
    print(f"transcripts from: {machine}   scanned: {' '.join(args.roots or args.profiles)}")
    if args.roots:
        # The sessions saw THAT machine's guide; the byte sizes above are THIS
        # checkout's. Comparable only if both are near the same commit -- say so.
        print(f"  NOTE: section bytes computed from the checkout on {os.uname().nodename}, "
              f"not from whatever guide those sessions actually received.")
    if since_ts:
        print(f"filter: mtime >= {args.since}")
    if n == 0:
        print("\nno sessions matched -- this is a statement about the scan, not the world.")
        print("check --profiles and --since before reading it as absence.")
        return 0

    mains = [r for r in records if r["kind"] == "main"]
    subs = [r for r in records if r["kind"] == "subagent"]
    cut = args.split_at
    slices = (
        (lambda rs, base: [(rs, base)])
        if not cut
        else (
            lambda rs, base: [
                ([r for r in rs if r["session_date"] < cut], f"{base}  < {cut}"),
                ([r for r in rs if r["session_date"] >= cut], f"{base}  >= {cut}"),
            ]
        )
    )
    if args.kind in ("main", "all"):
        for rs, lab in slices(mains, "MAIN sessions"):
            report(rs, sizes, lab)
    if args.kind in ("subagent", "all"):
        for rs, lab in slices(subs, "SUBAGENT sessions"):
            report(rs, sizes, lab)
    if args.kind == "all":
        print("\n(DIFFERENT POPULATIONS -- do not average them; see report()'s docstring)")
    if cut:
        print(
            "\nBEFORE reading a period difference as a REGIME effect, stratify by project.\n"
            "  Measured 2026-08-31 across the 2026-08-27 section-grain ship, main sessions:\n"
            "  the aggregate moved 49.0% -> 34.4%, but codescout-project sessions moved\n"
            "  28.0% -> 27.5% while the codescout SHARE moved 38% -> 57%. The aggregate\n"
            "  shift was MIX, not regime -- project drives more variance here than time.\n"
            "  (Mechanically expected: tracker-conventions declares no `serves:`, so it is\n"
            "  delivered whole in every regime. Prediction and measurement agree.)"
        )

    if args.explain:
        print()
        print("EVIDENCE for every non-zero (audit these -- non-zeros are an upper bound):")
        for r in records:
            shown = {s: e for s, e in r["evidence"].items() if e}
            if not shown:
                continue
            print(f"\n  {r['sid'][:8]} [{r['profile']}/{r['kind']}] {r['project'][:40]}")
            for section, ev in shown.items():
                print(f"    {section[:60]}  (x{r['engaged'][section]})")
                for line in ev:
                    print(f"      - {line}")

    print()
    print("profiles:", dict(collections.Counter(r["profile"] for r in records)))
    print("REMINDER: zeros are strong evidence, non-zeros are an upper bound. "
          "See the module docstring.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
