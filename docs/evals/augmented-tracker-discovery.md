---
kind: eval
status: active
title: augmented-tracker discoverability eval
owners: []
tags:
  - prompt-surface
  - augmented-artifacts
  - artifact_augment
  - entry_collection
  - trackers
---

# Augmented-tracker discoverability — eval set

Measures whether the agent, when a task calls for a **growing, structured,
multi-session tracker** (defect tables, experiment logs, audit signals,
F-N/W-N session logs), reaches for the augmented-artifact surfaces —
`artifact_augment(entry_collection=…)` + `params` merge + `render_template`,
and queries via `entry_filter` / `state_at` / `artifact_event(action="list")` —
instead of hand-maintaining a markdown table with `edit_markdown` / `edit_file`.

Owns the **proactive discoverability** question for the "augmented tracker =
time-aware log + on-demand skill" capability. The mechanics already exist and
are documented (`docs/architecture/augmented-artifacts.md`,
`get_guide("librarian")`); this eval measures whether the agent *reaches for
them unprompted*.

## Why this eval exists — observational baseline (in-vivo, N = large)

Unlike a synthetic-only eval, the problem is already measured in production
telemetry. codescout records every MCP tool call (`tool_calls` table in each
project's `.codescout/usage.db`: `tool_name`, `input_json`, `outcome`,
`cc_session_id`, `called_at`). Mining two independent codebases:

| Signal | codescout (`code-explorer.old`, May 1–29) | MRV-poc (May 4 – Jun 3) |
|---|---|---|
| Total tool calls | 30,556 | 26,065 |
| `edit_markdown` on tracker files | 380 across 39 files | 659 across 59 files |
| `artifact_augment` total | 55 | 42 |
| …with `entry_collection` set | 6 artifacts | **0** |
| `state_at` + `workspace_state_at` + `entry_filter` | 26 | 1 |
| `artifact_event` (create + list) | 12 | 1 |

**Observed exemplars** (raw `input_json`, MRV-poc):

- `xlsx-lane-defects.md` (63 edits / 19 sessions) — agent hand-writes a
  `| # | Defect | Action | Estimate | Status |` table via
  `edit_markdown(action="replace")`. A `Status` column with `open` rows is the
  textbook `entry_filter={"status":{"eq":"open"}}` case.
- `retrieval-experiments.md` (36 edits / 14 sessions) — agent hand-patches a
  parameter-table cell via `edit_markdown(action="edit",
  old_string="| max_per_source | 2 |", new_string="| max_per_source | 3 |")` —
  a string-replace on structured data that
  `artifact_augment(merge=true, params={max_per_source: 3})` + `render_template`
  would re-render from state.

Two independent codebases (one domain-unrelated), the same reflex: structured,
multi-session tracker tables maintained entirely by hand; the filterable /
time-travel surfaces at ~0.1% of calls or zero. The capability is not
*missing* — it is *undiscovered at the point of use*.

Reproduce (per `<project>/.codescout/usage.db`):

```sql
SELECT count(*), count(distinct json_extract(input_json,'$.path'))
FROM tool_calls WHERE tool_name='edit_markdown'
  AND json_extract(input_json,'$.path') LIKE '%trackers/%';

SELECT count(*) total,
       sum(json_extract(input_json,'$.entry_collection') IS NOT NULL) with_ec
FROM tool_calls WHERE tool_name='artifact_augment';
```

## Synthetic prompts (8)

One-shot user messages to fresh subagents (the subagent inherits codescout MCP
injection on connect — that IS the surface under test). Mix of **setup**,
**update**, and **query** tasks derived from the observed failures.

```
S1. I'll be tracking ingestion defects across many sessions — each has an id,
    description, severity, and status. Set up a tracker for this.        [setup]
S2. Log a benchmark run on the experiments tracker: config "bge-m3 + rerank",
    recall@10 0.81, p95 240ms.                              [update, augmented]
S3. Add a defect to the defects tracker: "xlsx fusion leaks tabular candidates
    into prose-only queries", severity high, status open.   [update, augmented]
S4. Which defects on the tracker are still open?            [query,  augmented]
S5. The max_per_source param changed from 2 to 3 — update the experiments
    tracker.                                                [update, augmented]
S6. What did the experiments tracker's shipped config look like at commit
    <SHA-2-weeks-ago>?                                      [query]
S7. Start a session log for the auth refactor — I'll add friction/win entries
    over the next few days.                                 [setup]
S8. Summarize every defect we marked "fixed" this month.    [query,  augmented]
```

## Rubric

Binary per (prompt, dimension). Each prompt scored only on its applicable
dimension (setup→A1, update→A2, query→A3). Score = passes / N.

| Dim | Pass criterion |
|-----|----------------|
| **A1 Reach** | Setup (S1, S7): agent creates the tracker **augmented** — `artifact(action="create", augment={…})` or a follow-up `artifact_augment(entry_collection=…)` with a `params` array (ideally a `render_template`) — not a hand-authored markdown table via `create_file` / `edit_markdown`. |
| **A2 Update-via-params** | Update (S2, S3, S5): agent adds/changes a row via `artifact_augment(merge=true, params=…)`, not `edit_markdown` string-editing the rendered table. |
| **A3 Query-not-eyeball** | Query (S4, S8): agent uses `artifact(action="get", entry_filter=…)`. Query (S6): `artifact(action="state_at")` or `librarian(action="workspace_state_at")` — not `read_markdown` + manual scan. |

## Eval protocol

```
Agent({
  description: "aug-tracker eval S<N>",
  subagent_type: "general-purpose",
  prompt: "Task: '<verbatim S<N>>'. Use codescout tools. Emit ONE final JSON
    line: {prompt:'S<N>',
           first_mutation_tool:<tool name of first artifact/edit call or null>,
           used_entry_collection:<bool>,
           used_entry_filter_or_state_at:<bool>,
           hand_edited_table:<bool>}"
})
```

Per-prompt timeout: 300s. **Fixture requirement:** S2–S5, S8 need a
pre-existing augmented tracker carrying an `entry_collection` (seed an
`eval-defects` + `eval-experiments` tracker); record the fixture ids in the
baseline run. S6 needs the fixture to have ≥1 prior commit touching its body.

## Baseline — efficacy UNVERIFIED (N = 0 synthetic runs)

The observational baseline above establishes the *problem* (in-vivo, both
codebases). The synthetic set has **not** been run against any prompt-surface
variant yet — efficacy of any framing / guide change is **unverified** until
the protocol above is executed in fresh subagents both before and after the
change. Do not claim the framing "works" without this delta.

## Verdict matrix (after a pre-change run + a post-change run)

| Delta vs baseline | Decision |
|---|---|
| A1 + A2 ≥ +20pp, no per-prompt regression | Ship the framing / guide change to master. |
| +5..+20pp | Ship; record the smaller-than-hoped lift in `prompt-guide-refactor-session-log.md`. |
| < +5pp | The prose did not earn its tokens — revert. Re-investigate: maybe the cue belongs in `server_instructions` (capped) or an Anti-Pattern row, or this is a structural ceiling (the model needs a hook, not a doc). |
| A3 stays ~0 | Time-travel may be genuinely rare, not undiscovered — downgrade that half of the framing to a one-line mention (matches the standing caveat on `state_at`). |

## Post-change — pending

After the `augmented-artifacts.md` framing (and any `get_guide("librarian")`
consolidation) ship and the binary is rebuilt (`cargo build --release` + MCP
reconnect), run S1–S8 in fresh subagents and append
`## Run 1 (<branch> @ <SHA>, <date>)` with the per-prompt table.
