# Promoting the background-job wake hook, and how to measure it later (2026-11-10)

Goal: sessions that start a long `run_command` job should end their turn and wait for the companion hook's wake ("Stop hook feedback", text `codescout background job @bg_N ended: ...`). They should not poll, sleep or schedule a wake. Related: [[cs-hint-transcript-mining-20261107]], [[hint-visibility-channels-20261109]].

## What was changed (2026-11-10)
- codescout `22cc87a0f`: `src/tools/run_command/inner.rs` start hint now reads "Started; outcome not yet observed. If the codescout-companion hook is active, end your turn: a wake message arrives when the job ends. Otherwise read the job with run_command("tail -50 @bg_N") — that response carries this job's state." The old hint said only "Read output with run_command(tail -50 ...)". Same commit: one Phase 0 bullet in `src/prompts/guides/project-activation-bootstrap.md`. A longer first bullet put the cold emission 232 B over its ceiling (`a_cold_p50_session_stays_under_the_committed_cold_emission_byte_ceiling`), so the bullet is about 130 B.
- claude-plugins `c607dcd` (main): one line in the CODESCOUT RULES block of `codescout-companion/hooks/subagent-guidance.mjs`: no wake reaches a subagent, so run a long task in the foreground with `timeout_secs`. Hooks load from the repo working tree (claude-plugins CLAUDE.md), so this is live for new Claude Code subagents without a version bump.
- Four profile files `~/.claude*/CLAUDE.md`: the Iron Rule "Long Tasks — Activate a Monitor" became "Long Tasks — Use the Wake Hook, Do Not Poll". These files are not in any repo.
- Live check after the rebuild (`/mcp`, same day): the new hint text and the bootstrap bullet both arrived, and a trivial background job woke the session after 4 s.
- Not pushed at the time of writing. codescout `experiments` and claude-plugins `main` were ahead of their remotes.

## Baseline (before the change)
Scope: transcripts of the codescout project in `~/.claude`, `~/.claude-sdd`, `~/.claude-kat`, `~/.claude-stef`, files modified in the 7 days before 2026-11-10, main sessions and `subagents/` files. 1,069 codescout background starts by raw count, 835 wakes delivered. Pairing a start with its wake by `@bg_` handle worked for 624 starts (the rest had no `output_id` the regex could read).

| Measure | Main sessions | Subagents |
|---|---|---|
| Paired starts | 523 | 101 |
| Woken by the hook | 440 (84%) | 0 (by design) |
| Clean flow: wake, no read or sleep first | 375 (72%) | n/a |
| Read the job before the wake | 78 (script 2) / 87 (script 3) | 58 |
| Sleep, `tail --pid` or `ScheduleWakeup` while the job was open | 90 | 88 |
| Started with no wake in a session that got other wakes | 83 (cause not found) | n/a |

- Early reads (script 3): 87; calls between start and first read: 1:18, 2:14, 3:15, 4:5, 5:3, 6+:32; seconds to the first read: median 88, p90 439.
- `Monitor` tool calls: 0. `ScheduleWakeup`: 95. `CronCreate`: 3.
- Long foreground `run_command` calls (gate, cargo, pytest, with-slot, rb, mutation-probe): 709 background and 401 foreground below 300 s and 256 foreground at 300 s or more for main sessions; 89, 68 and 264 for subagents. Whether the foreground ones were short in practice was not checked.
- The two counts of reads before the wake (78 and 87) come from two scripts that pair slightly differently. Compare like with like: use the same script before and after.

## How to repeat it (suggested date: 2026-11-17 or later)
1. Pick the window start: sessions that began after the rebuild on 2026-11-10 (about 11:30 local; the exact time was not recorded). In both scripts replace the line `cut = time.time() - 7 * 86400` with a fixed epoch, for example `cut = 1794306600.0` (compute it with `date -d '2026-11-10 11:30' +%s`).
2. Run both scripts with `python3 -I <script>.py` from a directory that holds no other Python files. They are read-only. They read `~/.claude*/projects/-home-marius-work-claude-codescout/**/*.jsonl`.
3. Compare with the baseline table. A fair reading needs the same script on both sides.
4. Say what the window includes: peer sessions that started before the rebuild keep the OLD server binary until `/mcp` or a restart (`scripts/peer-sessions.sh` showed many sessions with `cs REPLACED`), so they still see the old hint. Sessions on the old binary dilute the effect. Check the binary state of the sessions in the window, or restrict to sessions that started after the rebuild.

Decision rule I would use (my choice, not tested): the hint worked if clean flow rises well above 72% and early reads within 3 calls fall well below 47, on a window long enough to hold at least 200 paired main-session starts. A flat result means the hint is not the lever; look at the wake text itself and at the spawn-time context idea below.

## Limits
- No control group exists. A before/after comparison cannot separate the hint from a change in the kind of work in the window.
- An early read can be a legitimate progress check (a "tail -n 6 @bg" after 285 s), not a poll. The scripts do not separate them.
- The 83 main-session starts with no wake in a session that got other wakes were not explained. Candidates not checked: a read that disarmed the hook, a missing record, a restarted server.
- Subagents get no wake by design, so the subagent numbers measure only whether the new subagent line is obeyed.
- The Codex side was not touched: Codex has no wake from this hook and its users approve plugin hooks per content hash.

## Not done, possible next step
- A spawn-time line from the companion hook itself. It knows it is armed, so the text would be true without the "if the hook is active" condition. Not checked: whether `wake-on-job-end.mjs` (PostToolUse, asyncRewake) can emit context at spawn.

## Script 1: pairing, main vs subagent, reads before the wake (`mine_wake2.py`)
Output for the baseline: the `main` and `subagent` lines. Its start-id regex only matches starts whose result carries `output_id`, which is why only 624 of 1,069 starts pair.

```python
#!/usr/bin/env python3
"""Pair each codescout bg start with its wake; detect polling; list tool names like monitor."""
import json, os, re, sys, time, glob, collections

HOME = os.path.expanduser("~")
cut = time.time() - 7 * 86400
PROFILES = [".claude", ".claude-sdd", ".claude-kat", ".claude-stef"]
OUTID = re.compile(r'"output_id"\s*:\s*"(@bg_[0-9a-f]+)"|output_id\\?"\s*:\s*\\?"(@bg_[0-9a-f]+)')
BGID = re.compile(r"@bg_[0-9a-f]+")
WAKE = re.compile(r"codescout background job (@bg_[0-9a-f]+) (ended|is still running)")
SLEEP = re.compile(r"(^|[;&|\s])sleep\s+\d|tail\s+--pid")

def text_of(content):
    if isinstance(content, str):
        return content
    out = []
    for b in content or []:
        if isinstance(b, dict):
            if b.get("type") == "text":
                out.append(b.get("text", ""))
            elif b.get("type") == "tool_result":
                c = b.get("content")
                out.append(c if isinstance(c, str) else text_of(c))
    return "\n".join(out)

names = collections.Counter()
res = collections.defaultdict(collections.Counter)

for prof in PROFILES:
    d = f"{HOME}/{prof}/projects/-home-marius-work-claude-codescout"
    for f in glob.glob(d + "/**/*.jsonl", recursive=True):
        if os.path.getmtime(f) < cut:
            continue
        sub = "/subagents/" in f
        who = "subagent" if sub else "main"
        events = []   # (kind, id, ts, extra)
        starts = {}   # tool_use_id -> ts
        for line in open(f, errors="replace"):
            try:
                r = json.loads(line)
            except Exception:
                continue
            ts = r.get("timestamp")
            m = r.get("message") or {}
            if r.get("type") == "assistant":
                for b in m.get("content") or []:
                    if not (isinstance(b, dict) and b.get("type") == "tool_use"):
                        continue
                    n = b.get("name", "")
                    names[n] += 1
                    inp = b.get("input") or {}
                    cmd = inp.get("command", "") if isinstance(inp, dict) else ""
                    if n == "mcp__codescout__run_command" and inp.get("run_in_background") in (True, "true"):
                        starts[b["id"]] = ts
                    else:
                        ids = BGID.findall(cmd) + BGID.findall(str(inp.get("path", "")))
                        if ids:
                            events.append(("read", ids[0], ts, ""))
                        if SLEEP.search(cmd):
                            events.append(("sleep", None, ts, ""))
                        if n in ("Monitor", "ScheduleWakeup"):
                            events.append((n, None, ts, ""))
            elif r.get("type") == "user":
                # tool_result for a start?
                for b in (m.get("content") if isinstance(m.get("content"), list) else []):
                    if isinstance(b, dict) and b.get("type") == "tool_result" and b.get("tool_use_id") in starts:
                        s = text_of([b])
                        mm = OUTID.search(s)
                        if mm:
                            events.append(("start", mm.group(1) or mm.group(2), starts[b["tool_use_id"]], ""))
                s = text_of(m.get("content"))
                if "Stop hook feedback" in s:
                    for w in WAKE.finditer(s):
                        events.append(("wake", w.group(1), ts, w.group(2)))
        # pair
        open_jobs = {}
        for kind, hid, ts, extra in events:
            if kind == "start":
                open_jobs[hid] = {"ts": ts, "woke": False, "polled": 0, "sleeps": 0}
            elif kind == "wake" and hid in open_jobs:
                j = open_jobs[hid]
                if not j["woke"]:
                    j["woke"] = True; j["wake_ts"] = ts
            elif kind == "read" and hid in open_jobs:
                j = open_jobs[hid]
                if not j["woke"]:
                    j["polled"] += 1
            elif kind in ("sleep", "Monitor", "ScheduleWakeup"):
                for j in open_jobs.values():
                    if not j["woke"]:
                        j["sleeps"] += 1
        for hid, j in open_jobs.items():
            c = res[who]
            c["bg_starts"] += 1
            c["woke"] += j["woke"]
            if not j["woke"]:
                c["no_wake"] += 1
            if j["polled"]:
                c["read_before_wake"] += 1
            if j["sleeps"]:
                c["sleep_monitor_wakeup_while_open"] += 1
            if j["woke"] and not j["polled"] and not j["sleeps"]:
                c["clean_wake_flow"] += 1

for who, c in res.items():
    print(who, dict(c))
print("\ntool names matching monitor/wakeup/cron/loop:")
for n, v in names.most_common():
    if re.search(r"monitor|wakeup|cron|loop", n, re.I):
        print(f"  {v:5d} {n}")
print("\ntop 12 tool names:")
for n, v in names.most_common(12):
    print(f"  {v:5d} {n}")
```

## Script 2: timing of the first read before the wake, and starts with no wake (`mine_wake3.py`)
Output for the baseline: "early reads (before wake): 87", the calls-between histogram and the seconds line. Main sessions only.

```python
#!/usr/bin/env python3
"""Detail: early reads before the wake, and main-session starts with no wake."""
import json, os, re, time, glob, collections

HOME = os.path.expanduser("~")
cut = time.time() - 7 * 86400
PROFILES = [".claude", ".claude-sdd", ".claude-kat", ".claude-stef"]
OUTID = re.compile(r'(@bg_[0-9a-f]{8})')
BGID = re.compile(r"@bg_[0-9a-f]+")
WAKE = re.compile(r"codescout background job (@bg_[0-9a-f]+) (ended|is still running)")

def text_of(content):
    if isinstance(content, str):
        return content
    out = []
    for b in content or []:
        if isinstance(b, dict):
            if b.get("type") == "text":
                out.append(b.get("text", ""))
            elif b.get("type") == "tool_result":
                c = b.get("content")
                out.append(c if isinstance(c, str) else text_of(c))
    return "\n".join(out)

def secs(a, b):
    from datetime import datetime
    f = lambda s: datetime.fromisoformat(s.replace("Z", "+00:00")).timestamp()
    return f(b) - f(a)

gap_calls = collections.Counter()
gap_secs = []
reasons = collections.Counter()
samples = []
nowake_samples = []
for prof in PROFILES:
    d = f"{HOME}/{prof}/projects/-home-marius-work-claude-codescout"
    for f in glob.glob(d + "/**/*.jsonl", recursive=True):
        if os.path.getmtime(f) < cut or "/subagents/" in f:
            continue
        seq = []  # ordered events
        starts = {}
        any_wake_in_file = False
        for line in open(f, errors="replace"):
            try:
                r = json.loads(line)
            except Exception:
                continue
            ts = r.get("timestamp"); m = r.get("message") or {}
            if r.get("type") == "assistant":
                for b in m.get("content") or []:
                    if isinstance(b, dict) and b.get("type") == "tool_use":
                        inp = b.get("input") or {}
                        cmd = inp.get("command", "") if isinstance(inp, dict) else ""
                        if b["name"] == "mcp__codescout__run_command" and inp.get("run_in_background") in (True, "true"):
                            starts[b["id"]] = (ts, cmd)
                        else:
                            seq.append(("call", b["name"], cmd, ts))
            elif r.get("type") == "user":
                for b in (m.get("content") if isinstance(m.get("content"), list) else []):
                    if isinstance(b, dict) and b.get("type") == "tool_result" and b.get("tool_use_id") in starts:
                        mm = OUTID.search(text_of([b]))
                        if mm:
                            seq.append(("start", mm.group(1), starts[b["tool_use_id"]][1], starts[b["tool_use_id"]][0]))
                s = text_of(m.get("content"))
                if "Stop hook feedback" in s:
                    for w in WAKE.finditer(s):
                        seq.append(("wake", w.group(1), "", ts)); any_wake_in_file = True
        live = {}
        for i, e in enumerate(seq):
            if e[0] == "start":
                live[e[1]] = {"i": i, "ts": e[3], "cmd": e[2], "woke": False, "first_read": None, "n": 0}
            elif e[0] == "wake" and e[1] in live:
                live[e[1]]["woke"] = True
            elif e[0] == "call":
                ids = BGID.findall(e[2])
                for h in ids:
                    j = live.get(h)
                    if j and not j["woke"] and j["first_read"] is None:
                        j["first_read"] = (i - j["i"], secs(j["ts"], e[3]), e[2])
        for h, j in live.items():
            if j["first_read"]:
                gap_calls[min(j["first_read"][0], 6)] += 1
                gap_secs.append(j["first_read"][1])
                if len(samples) < 8:
                    samples.append((j["cmd"][:70], j["first_read"][1], j["first_read"][2][:60]))
            if not j["woke"]:
                reasons["file_has_other_wakes" if any_wake_in_file else "file_never_woken"] += 1
                if len(nowake_samples) < 6:
                    nowake_samples.append((prof, j["cmd"][:80], bool(j["first_read"])))

gap_secs.sort()
n = len(gap_secs)
print("early reads (before wake):", n)
print("calls between start and first read (6 = 6+):", sorted(gap_calls.items()))
if n:
    print("seconds to first read: min %.0f  median %.0f  p90 %.0f  max %.0f" % (gap_secs[0], gap_secs[n//2], gap_secs[int(n*.9)], gap_secs[-1]))
print("\nno-wake main starts:", dict(reasons))
print("\nearly-read samples (start cmd | secs | read cmd):")
for s in samples: print("  ", s)
print("\nno-wake samples (profile | cmd | read anyway):")
for s in nowake_samples: print("  ", s)
```

Script 2 (`mine_wake3.py`) is byte-identical to the file that produced the baseline. Script 1 (`mine_wake2.py`) differs from it by one deleted line, `lat = []`, which the original defined and never used, so the output does not change. Both were checked by diff and both compile. They were run from the session scratchpad with `python3 -I`. The only edit needed for a repeat is the `cut` line.
