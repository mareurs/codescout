---
status: open
opened: 2026-08-20
closed:
severity: high
owner: marius
related: []
tags:
  - tooling
  - measurement
  - cross-repo
kind: bug
---

# BUG: cc.py sums message.usage per JSONL line, so every cost and token figure it reports is inflated 2.1–2.6×

## Summary

`cc.py` (the `claude-traces` skill's transcript analyser, reached from this repo through a
symlink) aggregates `message.usage` over **every** `type:"assistant"` JSONL entry. One model
completion is frequently written as 2–3 separate entries — thinking, text, tool_use — each
carrying an **identical** `usage` dict. With no dedup by `message.id`, every duplicate is
counted again. Measured overcount: **2.606×** on one session, **2.18×** on another. Every
cost, token and cache figure the skill has ever reported is wrong by a session-specific
factor, and the skill is the documented entry point for "session cost" questions.

## Symptom (Effect)

Measured 2026-08-19 on session `23b22760` (main profile, codescout project):

```
assistant entries carrying usage : 222
distinct message.id              : 87
duplication factor               : 2.552x
naive cost (cc.py method)        : $14.0384
deduped by message.id            : $5.3861
overcount ratio                  : 2.606x
```

`$14.0384` is byte-for-byte what `cc.py stats` / `cc.py sessions` prints for that session.
An independent pass measured session `55515bc5` at `$236.66` naive versus `$108.44`
deduped — **2.18×**, with `cache_read` specifically inflated 2.109×.

The factor is **not constant**: it depends on how often completions were split, which
varies with thinking and tool use. There is no single correction multiplier to apply to
past reports.

## Reproduction

```
# from /home/marius/work/claude/codescout
python3 - <<'EOF'
import json
from pathlib import Path
f = Path.home()/".claude/projects/-home-marius-work-claude-codescout/23b22760-0e21-4f2e-8ce8-77df354f0372.jsonl"
def cost(u):
    return (u.get("input_tokens",0)*3 + u.get("output_tokens",0)*15
            + u.get("cache_creation_input_tokens",0)*3.75
            + u.get("cache_read_input_tokens",0)*0.30)/1e6
raw, uniq = 0, {}
for line in open(f):
    e = json.loads(line)
    if e.get("type") != "assistant": continue
    m = e.get("message") or {}
    u = m.get("usage")
    if not u: continue
    raw += 1
    uniq.setdefault(m.get("id"), u)
print("entries:", raw, "distinct message.id:", len(uniq))
print("naive:", round(sum(cost(json.loads(l)["message"]["usage"])
      for l in open(f)
      if json.loads(l).get("type")=="assistant"
      and (json.loads(l).get("message") or {}).get("usage")), 4))
print("deduped:", round(sum(cost(u) for u in uniq.values()), 4))
EOF
```

Compare against `python3 .claude/skills/claude-traces/scripts/cc.py stats 23b22760`.

## Environment

Linux; Claude Code 2.1.233–2.1.235; transcripts under `~/.claude*/projects/`. **The fix is
cross-repo:** this repo's `.claude/skills/claude-traces` is a symlink to
`/home/marius/agents/llm-proxy/.claude/skills/claude-traces`, so the defect lives in the
llm-proxy repo while being consumed here (and by `analyze-usage`-adjacent workflows).

## Root cause

`cc.py:145-166`, `aggregate_usage`: iterates entries, and for each `type:"assistant"` entry
with a `message.usage`, adds every token field into a running total. No `message.id` or
`requestId` dedup. The transcript writes one completion as multiple entries when the
response contains multiple content-block types, and each entry carries the same `usage`
payload for the whole completion — so the payload is a per-*completion* fact being summed
per-*line*.

measured 2026-08-19: the reproduction above (222 entries → 87 distinct `message.id`), plus
`cc.py`'s source read directly at `:145-166`.

This is the failure mode the project's own measurement rule names — *calibrate against a
known-good instrument on the overlap*. `cc.py` was treated as the known-good instrument for
cost and never calibrated against anything.

## Evidence

### Two sessions, two factors

| Session | naive | deduped | ratio |
|---|---|---|---|
| `23b22760` | $14.0384 | $5.3861 | 2.606× |
| `55515bc5` | $236.66 | $108.44 | 2.18× |

### Blast radius

Any figure produced by `cc.py sessions`, `cc.py stats`, or the `claude-traces` skill's
cost table. In this repo that includes cost figures quoted into session analyses on
2026-08-19/20. The **call/turn/tool** counts from `cc.py` are unaffected — only
`aggregate_usage`/`estimate_cost` are wrong.

## Hypotheses tried

1. **Hypothesis:** the duplicate entries carry *partial* usage that legitimately sums to
   the completion total. **Test:** compared the `usage` dicts of entries sharing one
   `message.id`. **Verdict:** rejected — they are identical, not complementary, so summing
   them multiplies rather than accumulates. **Evidence:** *Symptom* (222 → 87 with a 2.55×
   line factor and a 2.61× cost factor; the near-match of the two ratios is what identical
   payloads predict).

## Fix

Not yet implemented. In `cc.py:145-166`, dedup by `message.id` before summing — keep the
first `usage` seen per id and ignore later repeats. `requestId` is a candidate fallback for
entries lacking `message.id`; check whether any exist before relying on it.

Two adjacent fixes the same function wants, found in the same analysis and worth landing
together since they all concern "what does this session's activity actually total":

- **Walk subagent files.** `<session-dir>/subagents/agent-*.jsonl` are separate files whose
  activity `cc.py` never sees. Subagent cost share was measured at 0.5%–63.4% depending on
  workflow, so a main-file-only total can understate a session by more than half.
- **Fix `PROJECTS_DIR`.** It is hardcoded to `~/.claude` and ignores `CLAUDE_CONFIG_DIR`,
  so on this host it silently cannot see the `~/.claude-sdd` and `~/.claude-kat` profiles.
  The skill's own doc already warns that `--all`'s path decode mangles directory names
  containing `-`; this is a second, unwarned instance of the same class.

Since the fix lands in llm-proxy, cite it cross-repo as `llm-proxy:<sha>` per this
project's SHA-prefix discipline, and record the patch-id
(`git show <sha> | git patch-id --stable`).

## Tests added

None. A fixture with one completion split across three entries sharing a `message.id`,
asserting the total equals one completion's cost rather than three, would pin this
permanently.

## Workarounds

Do not use `cc.py` for cost or token figures. Use the reproduction snippet above, or dedup
by `message.id` inline. Counts of calls, turns and tools from `cc.py` remain usable.
Any previously reported cost figure should be treated as an upper bound of unknown
tightness — the factor varies per session, so it cannot be divided out retroactively
without re-running against the transcript.

## Resume

Patch `aggregate_usage` at `cc.py:145-166` in the llm-proxy repo to dedup on
`message.id`; verify against the reproduction (expect `$5.3861` for `23b22760`). Then
decide whether to land the two adjacent fixes in the same change — the `PROJECTS_DIR` one
in particular affects which sessions are discoverable at all, so any re-measurement done
before it lands will be scoped to one profile of three.

## References

- `/home/marius/agents/llm-proxy/.claude/skills/claude-traces/scripts/cc.py:145-166` —
  `aggregate_usage`; `:23-24` — `CLAUDE_DIR`/`PROJECTS_DIR`; `:168-174` — `estimate_cost`
- `.claude/skills/claude-traces` — this repo's symlink into the above
- `docs/issues/archive/2026-08-20-friction-target-omits-command-and-file-path.md` — sibling
  measurement-instrument defect found in the same pass (fixed + archived 2026-08-20,
  `db76f69a`)
