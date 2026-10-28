"""Arm-integrity checker: an arm whose setup silently failed is INVALID, never scored.

    verdict = check(arm, transcript_path, stdout_json)     # Verdict(ok, reasons, ...)

`transcript` is the run's `claude -p --output-format stream-json --verbose --include-hook-events`
stream, one JSON object per line; `stdout_json` is the final result object (the stream's
`type: "result"` record, or the `--output-format json` stdout).

Record shapes (read from real streams under setup-ab-data/controls/round4c/r10-indexed/, Claude
Code 2.1.309; fixtures in fixtures/ are redacted excerpts of them; not guessed):

  init        {"type": "system", "subtype": "init", "tools": [name, ...],
               "mcp_servers": [{"name", "status", "source"}], "skills": ["plugin:skill" | "skill"],
               "plugins": [{"name", "path", "source", "version"?}], ...}
              `tools` lists native tools as bare names ("Read", "Bash") and MCP tools as
              "mcp__<server>__<tool>". Plugin skills are "<plugin>:<skill>" (superpowers skills:
              "superpowers:brainstorming"); the init record may arrive after the hook records.
  hook        {"type": "system", "subtype": "hook_started", "hook_name", "hook_event", "hook_id"}
              {"type": "system", "subtype": "hook_response", "hook_name", "hook_event", "output",
               "stdout", "stderr", "exit_code", "outcome"}
              The broken cs arm of Task 4 produced hook_response with exit_code 0 and output "",
              so only a hook_response whose `output` carries the COMPANION marker counts. Other
              plugins' hooks share hook_name "SessionStart:startup" (in the real full stream the
              superpowers hook, `<EXTREMELY_IMPORTANT> You have superpowers`, precedes the
              companion's), so non-empty output alone is not evidence. The companion's real
              SessionStart output is hookSpecificOutput.additionalContext starting
              "PROJECT BOOTSTRAP: As your FIRST codescout action ..." (cs and full, round4c).
  tool call   {"type": "assistant", "parent_tool_use_id": null | "<toolu id>",
               "message": {"content": [{"type": "tool_use", "id", "name", "input"}, ...]}}
              (a content block, same Messages-API shape as in a session transcript).
              `name` is "mcp__codescout__symbols" or a native name ("Read").
              `parent_tool_use_id` is null on the main thread and set on a subagent (sidechain).
  result      {"type": "result", "subtype", "is_error", "num_turns", "duration_api_ms",
               "total_cost_usd", "usage": {input_tokens, output_tokens, ...}, "result", ...}
              The real auth failure ("Not logged in") is is_error true, duration_api_ms 0,
              total_cost_usd 0, zero usage tokens, terminal_reason "api_error".

CAVEAT: no authenticated stream-json run exists yet (every round4c run ended "Not logged in"), so
the tool_use blocks in the fixtures are synthetic. Their shape was read from a real interactive
session transcript, whose assistant `message.content` uses the same block format; the init and
hook records are real.

Rules (every failed rule adds a reason; `ok` is True iff `reasons` is empty):
  all arms   transcript readable, non-empty, every line parses, an init record present;
             stdout_json is a dict; the arm is one of bare / cs / full. An `is_error` result is a
             validity reason ONLY when the run did no model work (infra/auth failure: explicit
             duration_api_ms 0, zero tokens, zero cost, no tool call); an is_error run that did
             model work (max turns, budget) is a real failure and is scored, not dropped.
             codescout_calls / native_calls are whole-run totals including subagent calls;
             sidechain_calls counts tool_use blocks whose parent_tool_use_id is non-null.
  bare       no `mcp__*` tool in the init tool list.
  cs, full   init tool list contains BOTH `mcp__codescout__references` and
             `mcp__codescout__symbol_at` (daily-tool-set markers); at least one `mcp__codescout__*`
             tool_use; at least one hook_response whose output contains COMPANION_MARKER.
  full       a `superpowers:*` skill is visible in the init skill list.
"""
import json
from pathlib import Path
from typing import NamedTuple

ARMS = ("bare", "cs", "full")
CODESCOUT_PREFIX = "mcp__codescout__"
COMPANION_MARKER = "PROJECT BOOTSTRAP"
MARKER_TOOLS = ("mcp__codescout__references", "mcp__codescout__symbol_at")
NATIVE_TOOLS = frozenset({"Read", "Grep", "Glob", "Bash", "Edit", "Write", "MultiEdit",
                          "NotebookEdit"})


class Verdict(NamedTuple):
    ok: bool
    reasons: list
    codescout_calls: int
    native_calls: int
    hook_events: int
    sidechain_calls: int = 0

def _load(transcript):
    """-> (records, reasons). Never skips a bad line silently."""
    try:
        text = Path(transcript).read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as e:
        return [], [f"transcript unreadable: {e}"]
    records, bad = [], []
    for n, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        try:
            rec = json.loads(line)
        except ValueError:
            bad.append(n)
            continue
        if isinstance(rec, dict):
            records.append(rec)
        else:
            bad.append(n)
    reasons = []
    if bad:
        reasons.append(f"malformed transcript lines: {len(bad)} (first at line {bad[0]})")
    if not records and not bad:
        reasons.append("transcript is empty")
    return records, reasons


def _tool_uses(records):
    """Yield (name, is_sidechain) per tool_use block, subagent calls included."""
    for rec in records:
        if rec.get("type") != "assistant":
            continue
        msg = rec.get("message")
        content = msg.get("content") if isinstance(msg, dict) else None
        if not isinstance(content, list):
            continue
        sidechain = rec.get("parent_tool_use_id") is not None
        for block in content:
            if isinstance(block, dict) and block.get("type") == "tool_use":
                yield str(block.get("name", "")), sidechain


def _is_evidence(rec):
    """A hook_response carrying the COMPANION's marker; another plugin's output is not evidence."""
    return (rec.get("type") == "system" and rec.get("subtype") == "hook_response"
            and isinstance(rec.get("output"), str) and COMPANION_MARKER in rec["output"])


def _did_no_model_work(stdout_json, tool_calls):
    """True only when the result explicitly shows zero model work (the real auth failure shape)."""
    if tool_calls:
        return False
    usage = stdout_json.get("usage")
    tokens = ([usage.get(k) for k in ("input_tokens", "output_tokens")]
              if isinstance(usage, dict) else [])
    return (stdout_json.get("duration_api_ms") == 0
            and not stdout_json.get("total_cost_usd")
            and len(tokens) == 2 and all(t == 0 for t in tokens))


def check(arm, transcript, stdout_json):
    records, reasons = _load(transcript)
    calls = list(_tool_uses(records))
    names = [n for n, _ in calls]
    codescout_calls = sum(n.startswith(CODESCOUT_PREFIX) for n in names)
    native_calls = sum(n in NATIVE_TOOLS for n in names)
    sidechain_calls = sum(side for _, side in calls)
    hook_events = sum(_is_evidence(r) for r in records)

    if arm not in ARMS:
        reasons.append(f"unknown arm {arm!r} (expected one of {', '.join(ARMS)})")
    if not isinstance(stdout_json, dict):
        reasons.append("stdout_json is not a JSON object")
    elif stdout_json.get("is_error") and _did_no_model_work(stdout_json, calls):
        reasons.append("run failed before any model work (is_error, zero api time/tokens/calls)")

    inits = [r for r in records if r.get("type") == "system" and r.get("subtype") == "init"]
    if records and not inits:
        reasons.append("no system/init record in transcript")
    tools, skills = [], []
    for init in inits:
        tools += [t for t in init.get("tools") or [] if isinstance(t, str)]
        skills += [s for s in init.get("skills") or [] if isinstance(s, str)]

    if inits and arm == "bare":
        mcp = sorted(t for t in tools if t.startswith("mcp__"))
        if mcp:
            reasons.append(f"bare arm has MCP tools in init: {', '.join(mcp)}")
    if arm in ("cs", "full"):
        if inits:
            for marker in MARKER_TOOLS:
                if marker not in tools:
                    reasons.append(f"{arm} arm init tool list lacks {marker}")
        if records and codescout_calls == 0:
            reasons.append(f"{arm} arm made zero codescout calls")
        if records and hook_events == 0:
            reasons.append(f"{arm} arm has no companion hook output "
                           f"({COMPANION_MARKER!r}); the companion did not run")
    if arm == "full" and inits and not any(s.startswith("superpowers:") for s in skills):
        reasons.append("full arm has no superpowers skills visible in init")

    return Verdict(not reasons, reasons, codescout_calls, native_calls, hook_events,
                   sidechain_calls)
