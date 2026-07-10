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

**Resume here:** the high-priority backlog is EMPTY — 9/12 tasks resolved (T-1/2/7/8/9/11 done, T-12 dropped). T-11 closed the saga's methodology goal: protocol P-1..P-8 lives in prompt-hamsa-audit-log § Protocol (+ pointer in `src/prompts/README.md` § Measure before shipping). Remaining open are opportunistic: T-3..T-6 (medium, priors weakened by FND-16 — base-arm-first per P-3, T-6 the most likely to escape ceiling via multi-turn) and T-10 (low, cc.py --config-dir). New prompt-change work should enter via the protocol, not this tracker.

**Open threads:** T-10 (cc.py `--config-dir`) still open; fable tracker updates uncommitted on codescout `experiments` (llm-proxy work is committed as `678778c`).
## History

### 2026-07-07 — index created
Links findings / tasks / research for the Fable tuning work stream.

### 2026-07-10 — ecosystem tracker sync

Swept every tracker the work stream touched across the 4 repos and brought each current with the final 2026-07-07 outcomes: `lf.py mismatches` one-command fallback check (llm-proxy:`b72d0f6`; first run 0 mismatches / 300 traces) recorded in Research + T-9/T-12 notes; base-arm-first → prompt-hamsa **Heuristic 12** (claude-plugins:`5202cca`) cross-referenced from § Protocol and T-2/T-11 notes. Late capture-on-notice: filed `docs/issues/2026-07-10-edit-code-impl-method-selection-range-refusal.md` (edit_code suspicious-range on impl methods, noticed 2026-07-07 in llm-proxy). Noted: `codescout-ecosystem` umbrella is declared globally but the live MCP binary predates the feature — `scope="umbrella"` errors until the next `cargo rb` + `/mcp`.
