"""Read one relocation-probe run: what the model got as the tool result, and what arrived as hook context.
Usage: analyze.py <run-dir>   (finds the transcript of the run's session id under the profile's projects dir)
"""
import glob, json, os, sys
sys.path.insert(0, "/home/marius/work/claude/codescout/tools/hint-channel-eval")
from score import parse

run = sys.argv[1]
sid = open(os.path.join(run, "session-id.txt")).read().strip()
cfg = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
paths = glob.glob(os.path.join(cfg, "projects", "*", sid + ".jsonl"))
out = {"run": os.path.basename(run), "transcript_found": bool(paths)}
if paths:
    transcript = [json.loads(l) for l in open(paths[0]) if l.strip()]
    calls, results, contexts = parse(transcript)
    probe = [c for c in calls if c["name"] == "mcp__stub__probe"]
    out["probe_calls"] = len(probe)
    if probe:
        tid = probe[0]["id"]
        res = " ".join(results.get(tid, []))
        ctx = " ".join(contexts.get(tid, []))
        out["result_text"] = res
        out["result_has_hint"] = "HINT_TEXT_3Z" in res
        out["result_has_value"] = "R_5X" in res
        out["context_text"] = ctx
        out["context_has_hint"] = "HINT_TEXT_3Z" in ctx
        out["context_tagged"] = ctx.lstrip().startswith("[cs-hint]")
log = os.path.join(run, "hook-log.jsonl")
if os.path.exists(log):
    rows = [json.loads(l) for l in open(log) if l.strip()]
    out["hook_calls"] = len(rows)
    out["tool_response_type"] = rows[0]["tool_response_type"] if rows else None
    out["event_keys"] = rows[0]["event_keys"] if rows else None
else:
    out["hook_calls"] = 0
out["exit"] = open(os.path.join(run, "exit.txt")).read().strip()
out["binary"] = open(os.path.join(run, "binary.txt")).read().strip()
print(json.dumps(out, indent=1))
