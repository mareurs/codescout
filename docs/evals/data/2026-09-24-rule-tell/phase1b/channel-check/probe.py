"""Scratch probe, v3: the judge session's own init event (what it loaded), via stream-json.
Same flags as the labelling invocation except the output format, which only changes how events
are printed. Saves every event line to <out>."""
import json
import os
import subprocess
import sys

cfg, model, out_path = sys.argv[1], sys.argv[2], sys.argv[3]
env = {k: v for k, v in os.environ.items() if k not in ("ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL")}
env["CLAUDE_CONFIG_DIR"] = cfg
p = subprocess.run(["claude", "-p", "Say OK.", "--model", model, "--tools", "",
                    "--system-prompt", "You are a careful auditor. Output only what the instructions ask for.",
                    "--strict-mcp-config", "--no-session-persistence", "--output-format", "stream-json", "--verbose"],
                   capture_output=True, text=True, timeout=300, stdin=subprocess.DEVNULL, env=env, cwd="/tmp")
open(out_path, "w").write(p.stdout)
events = [json.loads(line) for line in p.stdout.splitlines() if line.strip().startswith("{")]
init = next((e for e in events if e.get("type") == "system" and e.get("subtype") == "init"), {})
res = next((e for e in reversed(events) if e.get("type") == "result"), {})
drop = {"session_id", "uuid", "cwd"}
print("init:", json.dumps({k: v for k, v in init.items() if k not in drop}, sort_keys=True))
u = res.get("usage", {})
print("input tokens:", sum(u.get(k) or 0 for k in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")))
