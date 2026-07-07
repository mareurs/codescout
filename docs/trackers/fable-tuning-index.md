---
id: ca8c26fecbbc4f37
kind: tracker
status: active
title: Fable Tuning — Index
owners: []
tags:
- fable
- prompt-tuning
- index
topic: null
time_scope: null
---

## Fable Tuning — work-stream index

Recovering "early-Fable" (`claude-fable-5`) quality in codescout via prompts / tools / trackers. This index links the set; each entry is a full librarian artifact.

| Tracker | id | Role |
|---|---|---|
| [Findings (FND-N)](fable-tuning-findings.md) | `35de33286cd34f87` | What we learned, filterable by `dimension`/`status` |
| [Tasks (T-N)](fable-tuning-tasks.md) | `ad1af8262fdce357` | Eval-gated recommendations |
| [Research](fable-tuning-research.md) | reflective | Sources + local-trace evidence |

## How to use

1. Start in **Findings** — filter to `status=confirmed|plausible` on the `dimension` you care about (especially `exploration`, `behavior`, `mechanism`).
2. Cross to **Tasks** — take the highest-`priority` `open` task; if `eval_gate` is true, run it as a pre-registered A/B arm through `prompt-hamsa-audit-log` (id `59ebeebb6ed05c89`) against the "original Fable captures" baseline before shipping.
3. **Research** holds the sourcing + local-trace evidence behind both.

Query examples:
- `artifact(action="find", tags=["fable"])`
- `artifact(action="get", id="35de33286cd34f87", entry_filter={"dimension":{"eq":"exploration"}})`
- `artifact(action="get", id="ad1af8262fdce357", entry_filter={"and":[{"status":{"eq":"open"}},{"priority":{"eq":"high"}}]})`

## Related

- `docs/trackers/prompt-hamsa-audit-log.md` (id `59ebeebb6ed05c89`) — the prompt-TDD apparatus that gates every prompt-surface change in this work stream.

## Session passover (2026-07-07)

**State (session 2, 2026-07-07):** all three do-next items resolved. **T-8 done** — `lf.py` key auto-discovery fixed (first-existing-`.env` shadowing bug; skill-frictions F-001 updated). **T-9 done** — silent-Opus-fallback **REFUTED locally** (FND-14): full-corpus JSONL served-model scan, 81 fable sessions / 3 profiles, 0 refusals, 0 per-call Opus interleaves; Langfuse lane was impossible at the time (llm-proxy logged request-side model only) — served-model logging then SHIPPED same day on user go-ahead (llm-proxy:`678778c`, deployed 11:01Z; requested vs served on every call, lf.py SERVED column + MISMATCH marker). **T-12 dropped** as moot. **T-1 CLOSED as zombie-born** — the CLAUDE.md cut had already shipped 2026-06-21 (`b603d86f`, on master, 42 KB → 12.5 KB); the remaining A-2 measurement was run this session and HELD: 0 dead-name calls / 4,743 post-cut tool calls, relocated rules still followed, audit A-2 closed.

**Work-stream reframe:** with the fallback mechanism dead locally and the CLAUDE.md diet shipped+measured, the remaining lever is FND-8/FND-9 (Fable defaults + over-prescriptive prompts).

**Resume here — next open (`high`):**
- **T-7** — audit prompt surfaces for reasoning-extraction foot-gun + token-countdown surfacing (no eval gate).
- **T-2** — A/B anti-tidying / anti-over-engineering snippet (eval-gated via `59ebeebb6ed05c89`).
- **T-11** — codify the subtract-and-measure protocol.

**Open threads:** T-10 (cc.py `--config-dir`) still open; fable tracker updates uncommitted on codescout `experiments` (llm-proxy work is committed as `678778c`).
## History

### 2026-07-07 — index created
Links findings / tasks / research for the Fable tuning work stream.
