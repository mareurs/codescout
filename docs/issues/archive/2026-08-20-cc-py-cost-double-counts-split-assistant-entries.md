---
kind: bug
status: fixed
tags:
- tooling
- measurement
- cross-repo
closed: 2026-08-20
opened: 2026-08-20
owner: marius
related: []
severity: high
unverified: 'No automated test exists — the claude-traces skill in llm-proxy has no test suite. Verification is two manual reproductions matching pre-computed values to the cent, re-runnable from the Reproduction section. Separately: figures published by this script before 2026-08-20 remain wrong and CANNOT be corrected by scaling, since the inflation factor varies per session (2.55x and 2.26x measured). And cc.py still reads only ~/.claude — filed in llm-proxy, not fixed here.'
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

**FIXED 2026-08-20 in `llm-proxy:38d80eb`**
(patch-id `03643e99d03f76f6ee32d5632a40521b47b80d0d`), at
`/home/marius/agents/llm-proxy/.claude/skills/claude-traces/scripts/cc.py`.
Cross-repo by design — this repo consumes the script through the
`.claude/skills/claude-traces` symlink; the defect and its fix live in llm-proxy.

`aggregate_usage` now counts usage **once per `message.id`**. Entries carrying no id
cannot be shown to be duplicates and are still counted — the pre-fix behaviour, kept
deliberately rather than guessed at.

Verified against two sessions, matching independently computed values **to the cent**:

| session | entries | `message.id` | fan-out | before | after |
|---|---:|---:|---:|---:|---:|
| `23b22760` | 222 | 87 | 2.55× | $14.0384 | **$5.3861** |
| `55515bc5` | 1748 | 774 | 2.26× | $236.66 | **$108.4389** |

`tool_calls` is **not** deduped and did not change (101 and 770, before and after). A
`tool_use` block lives in exactly one of the split lines, so it never fanned out. **That
asymmetry is why this survived so long**: the counts were always right and only the tokens
were wrong, and a session whose tool list reads correctly does not invite a second look at
its cost.

`turns` now counts completions — what the label always meant — with the raw line count
moved to `assistant_entries`, and `cmd_stats` printing both plus the ratio. A number that
inflates 2.5× is invisible when only the inflated one is shown; displaying the fan-out is
what makes the next instance of this self-evident rather than plausible.

**Not fixed here, deliberately:** the hardcoded `~/.claude` base and lossy `--project`
encoding, already filed in that repo as
`llm-proxy:docs/issues/2026-07-10-ccpy-config-dir-hardcoded-and-path-encoding.md`. Its fix
direction contains open design choices (a `--config-dir` flag versus an `--all-profiles`
scan) that belong to that repo's owner. It still means **`cc.py` sees only the `~/.claude`
profile**, so any re-measurement is scoped to one profile of three — pass explicit
directories, as `scripts/friction-probe.py` and this repo's analysis code already do.
## Tests added

**None — and this is a real gap, not a formality.** `.claude/skills/claude-traces/` in
llm-proxy has no test suite and no test harness to hang one on; adding both to a sibling
repo was out of scope for a fix this size.

What stands in for it: **two independent reproductions on real sessions, matching
pre-computed expected values to the cent** ($5.3861 and $108.4389, from a separate
`message.id`-dedup implementation written before the fix). Both re-run in seconds via the
*Reproduction* section above, which is the actual regression procedure until a suite exists.

The right test is small and specific, for whoever adds the first one: a fixture with **one**
completion split across three `assistant` entries sharing a `message.id`, asserting the
total equals one completion's cost rather than three. That single case would have caught
this, and `tool_calls` staying constant across the same fixture is the second assertion
worth pinning.
## Workarounds

Do not use `cc.py` for cost or token figures. Use the reproduction snippet above, or dedup
by `message.id` inline. Counts of calls, turns and tools from `cc.py` remain usable.
Any previously reported cost figure should be treated as an upper bound of unknown
tightness — the factor varies per session, so it cannot be divided out retroactively
without re-running against the transcript.

## Resume

N/A — fixed and archived.

Two things this fix does **not** discharge:

1. **Previously published figures are still wrong and cannot be scaled.** The inflation
   factor tracks how often completions happened to be split (2.55× and 2.26× on two
   sessions), so there is no divisor that repairs a past report — each has to be recomputed
   from the transcript. Anything quoting `cc.py` costs before 2026-08-20 should be treated
   as an upper bound of unknown tightness.
2. **`cc.py` still sees only `~/.claude`.** Filed separately in llm-proxy; until it lands,
   any profile-wide claim from this script covers one profile of three. Pass explicit
   directories.
## References

- `/home/marius/agents/llm-proxy/.claude/skills/claude-traces/scripts/cc.py:145-166` —
  `aggregate_usage`; `:23-24` — `CLAUDE_DIR`/`PROJECTS_DIR`; `:168-174` — `estimate_cost`
- `.claude/skills/claude-traces` — this repo's symlink into the above
- `docs/issues/archive/2026-08-20-friction-target-omits-command-and-file-path.md` — sibling
  measurement-instrument defect found in the same pass (fixed + archived 2026-08-20,
  `db76f69a`)
