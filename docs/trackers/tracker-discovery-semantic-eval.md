---
id: c7cd64b9230d1548
kind: tracker
status: active
title: Tracker-Discovery Semantic Search Eval — measure semantic vs filter for Phase-0 discovery
owners: []
tags:
- eval
- semantic-search
- tracker-discovery
- project-activation-bootstrap
topic: null
time_scope: null
---

# Tracker-Discovery Semantic Search Eval

**Goal:** Measure how well each discovery primitive surfaces the *right* trackers/artifacts
for a task, so the `project-activation-bootstrap` guide's Phase-0 discovery step is worded
by data — specifically the open question: does semantic search beat keyword-filter discovery
enough to lead the guide, or is filter the workhorse and semantic a bonus?

Feeds: `docs/superpowers/specs/2026-07-10-project-activation-bootstrap-guide-design.md`
(the Phase-0 wording decision). Sibling infra: Retrieval Benchmark tracker `cc4843e5`
(code-retrieval 25-TC) — this eval is the artifact/tracker-discovery analogue.

## Status: active — blocked on embedder reconnect + reindex
**Unblocked 2026-07-10:** embedder wired via `~/.local/bin/codescout-mcp` wrapper, `/mcp` reconnected (env confirmed in-process), `librarian(reindex, force=true, scope=repo)` embedded 808 artifacts (0 backfill errors). Semantic verified live: `find(semantic=…)` top-hit the exact target bug (`fd3694bf`); `context(topic=…)` now returns content (was silently empty — see the two filed bugs). **Ready to run the eval below.**

Prerequisite (2026-07-10): the codescout MCP server was launched with NO embedding env, so
`ctx.embedding == None` and every semantic path was dark. Fixed by re-registering codescout
to launch via `~/.local/bin/codescout-mcp` (sources `.env.amd` + `LIBRARIAN_EMBED_MODEL=CodeRankEmbed`,
`LIBRARIAN_EMBED_URL=http://127.0.0.1:48081/v1`). **Next actions to unblock:** `/mcp` reconnect,
then `librarian(action="reindex", force=true)` to embed artifacts (no vectors exist yet), then run
the eval below.

## What we already measured (2026-07-10, embedder DOWN)

With no embedder, only the filter path worked — and it was high-precision across 5 hand-run
use cases:

| Use case (post-activate task) | `find(semantic=)` | `librarian(context,topic=)` | `find(filter contains…)` |
|---|---|---|---|
| Audit LSP shutdown race | ❌ error (no embedder) | ❌ empty (silent) | ✅ exact open target + related |
| Add a get_guide topic | ❌ | ❌ | ✅ get-guide-topics tracker, prompt-hamsa log, README |
| Refactor / perf | ❌ | ❌ | ✅ code-dupes + legibility backlogs, perf spec/plan |
| Tool-selection went wrong | ❌ | ❌ | ⚠️ right hits on top; broad word "tool" adds noise |
| Cold start (no task) | — | — | ✅ find(kind="tracker") = 40 live + docs/TAXONOMY.md |

Caveats found: `find` results are **recency-ordered, not relevance-ordered** (broad keywords
flood a limit-capped list); scope auto-widens to `repo`; umbrella one param away. Two bugs filed:
`docs/issues/archive/2026-07-10-librarian-context-silent-empty-no-embedder.md` and
`docs/issues/archive/2026-07-10-librarian-semantic-no-like-fallback-doc-drift.md`.

## The measurement to run (embedder UP)

Re-run the same 5 use cases (reuse as the gold set — each with the artifact ids that SHOULD
surface) and add cases, scoring **precision/recall of surfaced artifact ids** for:
1. `artifact(find, semantic="<task>")`
2. `artifact(find, filter={contains on title/rel_path})` — the current baseline
3. `librarian(context, topic="<task>")`

Compare precision, recall, and cost (tool calls / tokens). Decide: does semantic lead, or does
filter lead with semantic as an opportunistic bonus? Record the verdict here and update the guide.

## Parked decision — harness shape (ask user before building)

- **A. Primitive-precision benchmark** — extend Retrieval Benchmark `cc4843e5` with labeled
  {task → gold artifact ids} cases; scores the *primitive*. Home: codescout.
- **B. Behavioral prompt-tdd scenario** — headless `claude -p` runs the candidate guide against
  real codescout tasks; asserts the agent *discovers the right trackers*. Home: prompt-engineering.
- **C. Both, layered.**

## Definition of done
- Semantic vs filter precision/recall measured with the embedder live.
- Verdict recorded; `project-activation-bootstrap` guide Phase-0 wording finalized (semantic-lead
  vs filter-lead-semantic-bonus vs filter-only), including the recency-ordering caveat.
