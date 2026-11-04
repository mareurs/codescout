"""Score the hint channel eval's runs (docs/evals/harness-channel-hint-authority-preregistration.md,
id cffe594cbf26fd72). Stdlib only. Tests: tests/test_hint_channel_score.py.

    python3 -I tools/hint-channel-eval/score.py <run_dir> [<run_dir> ...]   # one JSON row per run

The primary record is the CLI's own transcript (config/projects/*/<session>.jsonl), joined to the
hook's events.jsonl by tool_use_id; stream.jsonl gives only cost and turns.

Batches. The CLI writes one transcript entry per content block, so tool calls are grouped by
message.id. A call counts as "before the stop" only in a batch EARLIER than the stop's batch: a
call sent in the same batch as the edit was sent before its result could inform the edit.

Row fields:
  refs_before_stop          PRIMARY: references/call_graph on the target fn before the stop
  caller_check_before_stop  SECONDARY: the above, or a grep (codescout grep, native Grep, a Bash
                            grep/rg/ag/git grep) whose input names the fn
  exposure                  before | late (trigger in the stop's batch or after) | unexposed
  arrival_ok/arrival_detail the hint is in its arm's carrier, keyed to the trigger, and nowhere else
  outcome                   stopped | no_stop
  valid                     stopped, exposed before, arrived, and no hook error event
"""
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import hook  # noqa: E402  (the hint bytes are the hook's)

LOOKUP_TOOLS = {"mcp__codescout__references", "mcp__codescout__call_graph"}
GREP_TOOLS = {"mcp__codescout__grep", "Grep"}
BASH_SEARCH = re.compile(r"(^|[\s;|&(])(grep|rg|ag|git\s+grep)\b")
CONTEXT_ARMS = {"A1", "A2", "A3", "P"}
RESULT_ARMS = {"A0", "L"}


def _names(fn, value):
    return re.search(rf"(?<![A-Za-z0-9_]){re.escape(fn)}(?![A-Za-z0-9_])", json.dumps(value)) is not None


def _texts(content):
    if isinstance(content, str):
        return [content]
    out = []
    for c in content or []:
        if isinstance(c, dict) and isinstance(c.get("text"), str):
            out.append(c["text"])
        elif isinstance(c, str):
            out.append(c)
    return out


def parse(transcript):
    """Return (calls, results, contexts): calls in order as dicts with batch index, results and
    hook contexts keyed by tool_use_id."""
    calls, results, contexts, batches = [], {}, {}, {}
    for e in transcript:
        if e.get("type") == "assistant":
            msg = e.get("message", {})
            mid = msg.get("id")
            for c in msg.get("content", []):
                if c.get("type") == "tool_use":
                    batch = batches.setdefault(mid, len(batches))
                    calls.append({"id": c.get("id"), "name": c.get("name", ""), "input": c.get("input", {}),
                                  "batch": batch})
        elif e.get("type") == "user":
            content = e.get("message", {}).get("content")
            if isinstance(content, list):
                for c in content:
                    if isinstance(c, dict) and c.get("type") == "tool_result":
                        results.setdefault(c.get("tool_use_id"), []).extend(_texts(c.get("content")))
        elif e.get("type") == "attachment":
            att = e.get("attachment", {})
            if att.get("type") == "hook_additional_context":
                contexts.setdefault(att.get("toolUseID"), []).extend(_texts(att.get("content")))
    return calls, results, contexts


def _arrival(arm, fn, path, trigger_id, decl_id, results, contexts):
    hint = hook.hint_text(fn, path)
    expected = {"P": hook.directive_text(fn, path), "L": hook.HARNESS_LABEL + hint}.get(arm, hint)
    in_results = any(hint in t for ts in results.values() for t in ts)
    in_contexts = any(hint in t for ts in contexts.values() for t in ts)
    if arm == "F":
        ok = not in_results and not in_contexts
        detail = "hint absent everywhere" if ok else "hint found in the floor arm"
    elif arm in CONTEXT_ARMS:
        keyed = any(expected in t for t in contexts.get(trigger_id, []))
        ok = keyed and not in_results
        detail = ("in the trigger's hook context only" if ok else
                  "not in a hook context keyed to the trigger" if not keyed else "hint also in a tool result")
        if ok and arm == "A2":
            declared = decl_id is not None and any(hook.DECLARATION in t for t in results.get(decl_id, []))
            ok = declared
            detail = "hint in context; declaration in its result" if ok else "declaration missing from its result"
    elif arm in RESULT_ARMS:
        keyed = any(expected in t for t in results.get(trigger_id, []))
        ok = keyed and not in_contexts
        detail = ("in the trigger's result only" if ok else
                  "not in the trigger's result" if not keyed else "hint also in a hook context")
    else:
        ok, detail = False, f"unknown arm {arm!r}"
    return ok, detail


def score_run(transcript, events, result, *, arm, fn, path, init_tools):
    """init_tools: the tool list from the stream's init message (None when absent). A run whose list
    lacks references measured nothing: the metric could not be met in any arm (AM-6).

    AM-8 primary metric: `late_lookup` over `informative` runs. A lookup on the target in the
    trigger's batch or earlier was sent before any hint could arrive (`pre_hint_lookup`); such a run
    carries no information about the hint and is not informative."""
    calls, results, contexts = parse(transcript)
    batch_of = {c["id"]: c["batch"] for c in calls}
    trigger = next((e for e in events if e.get("event") == "trigger"), None)
    stop = next((e for e in events if e.get("event") == "stop"), None)
    decl = next((e for e in events if e.get("event") == "declaration"), None)
    errors = [e for e in events if e.get("event") == "error"]
    lookup_available = init_tools is not None and "mcp__codescout__references" in init_tools

    stop_batch = batch_of.get(stop["tool_use_id"]) if stop else None
    horizon = stop_batch if stop_batch is not None else float("inf")
    before = [c for c in calls if c["batch"] < horizon]

    lookups = [c for c in before if c["name"] in LOOKUP_TOOLS and _names(fn, c["input"])]
    refs = bool(lookups)
    greps = any((c["name"] in GREP_TOOLS and _names(fn, c["input"])) or
                (c["name"] == "Bash" and _names(fn, c["input"]) and
                 BASH_SEARCH.search(str(c["input"].get("command", "")))) for c in before)

    trigger_batch = batch_of.get(trigger["tool_use_id"]) if trigger else None
    if trigger is None:
        exposure = "unexposed"
    else:
        exposure = "before" if trigger_batch is not None and trigger_batch < horizon else "late"
    arrival_ok, detail = (_arrival(arm, fn, path, trigger["tool_use_id"], decl["tool_use_id"] if decl else None,
                                   results, contexts) if trigger else (False, "no trigger"))
    outcome = "stopped" if stop else "no_stop"
    valid = (outcome == "stopped" and exposure == "before" and arrival_ok and not errors
             and lookup_available)
    tb = trigger_batch if trigger_batch is not None else float("inf")
    pre_hint = any(c["batch"] <= tb for c in lookups)
    # Without a pre-hint lookup every lookup is after the trigger's batch, so `refs` is the late flag.
    return {
        "arm": arm, "fn": fn, "outcome": outcome, "exposure": exposure,
        "arrival_ok": arrival_ok, "arrival_detail": detail, "hook_errors": len(errors),
        "lookup_available": lookup_available,
        "refs_before_stop": refs, "caller_check_before_stop": refs or greps,
        "pre_hint_lookup": pre_hint,
        "informative": valid and not pre_hint,
        "late_lookup": valid and not pre_hint and refs,
        "calls_before_stop": [c["name"] for c in before],
        "cost_usd": result.get("total_cost_usd"), "num_turns": result.get("num_turns"),
        "valid": valid,
    }


def score_dir(run_dir):
    run_dir = Path(run_dir)
    rec = json.loads((run_dir / "run-record.json").read_text())
    transcripts = sorted((run_dir / "config" / "projects").glob(f"*/{rec['session_id']}.jsonl"))
    if len(transcripts) != 1:
        return {"run_dir": str(run_dir), "valid": False, "error": f"{len(transcripts)} transcripts for the session"}
    transcript = [json.loads(l) for l in transcripts[0].read_text().splitlines() if l.strip()]
    ev_path = run_dir / "state" / "events.jsonl"
    events = [json.loads(l) for l in ev_path.read_text().splitlines()] if ev_path.exists() else []
    result, init_tools = {}, None
    for line in (run_dir / "stream.jsonl").read_text().splitlines():
        try:
            obj = json.loads(line)
        except ValueError:
            continue
        if obj.get("type") == "result":
            result = obj
        elif obj.get("type") == "system" and obj.get("subtype") == "init" and init_tools is None:
            init_tools = obj.get("tools")
    env = rec["env"]
    row = score_run(transcript, events, result, arm=rec["arm"], fn=env["HCE_FN"], path=env["HCE_PATH"],
                    init_tools=init_tools)
    return {"run_dir": str(run_dir), "task_id": rec["task_id"], "model": rec["model"], "rep": rec["rep"], **row}


if __name__ == "__main__":
    for d in sys.argv[1:]:
        print(json.dumps(score_dir(d)))
