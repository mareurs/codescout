---
id: 90d32f37ef2d8fc8
kind: bug
status: fixed
title: 'BUG: usage.db records session_id but never agent_id, so every subagent''s calls are its parent''s'
owners:
- marius
tags:
- cluster/attribute-derived-at-container-granularity
- usage-db
- telemetry
- principal
- deep-agent
topic: usage telemetry principal identity
closed: 2026-09-20
opened: 2026-09-20
severity: medium
---

# BUG: `usage.db` records `session_id` but never `agent_id`, so every subagent's calls are its parent's

## Summary

`docs/adrs/2026-09-14-a-subagent-is-a-principal.md` decides that **"A principal is `(session_id,
agent_id)`."** `usage.db`'s `tool_calls` table records `session_id` and `cc_session_id` and has no
`agent_id` column. The server resolves the agent id 84 lines before it constructs the recorder and
does not pass it. Every per-principal figure derived from this table silently merges a parent with
all of its subagents, and the merge is invisible because the rows look complete.

## Symptom (Effect)

No error. Every query succeeds and returns rows that look whole.

A session that dispatched N subagents yields one `session_id` for all N+1 principals. There is no
column, and no value inside `input_json`, that separates them — so the merge cannot be undone after
the fact by any query over this table.

The ADR states the same fact as a measurement. Its *What the wire actually carries* table, from 10
`grep` calls across 4 subagents, 1 parent and 1 concurrent peer:

| property | result |
|---|---|
| subagent `PreToolUse` payload | carries `agent_id` **and** `agent_type` |
| parent `PreToolUse` payload | carries neither — absent, not empty |
| `session_id`, `prompt_id`, `transcript_path` | **identical** between parent and subagent |

The ADR's own gloss on that last row: *"nothing already on the wire separates them."* `agent_id`
does, it is present on **every call**, and it is the one field `usage.db` drops.

## Reproduction

Read-only, against any populated `.codescout/usage.db`:

```
sqlite3 -readonly .codescout/usage.db "PRAGMA table_info(tool_calls);"
```

No `agent_id` row is returned. Then confirm nothing recovers it from the captured arguments:

```
sqlite3 -readonly .codescout/usage.db \
  "SELECT COUNT(*) FROM tool_calls WHERE input_json LIKE '%agentId%';"
```

Expected `0` — see *Root cause*, second half, for why the strip guarantees it.

Not quantified against this machine's database; the defect is read out of the schema and the write
path.

## Environment

Branch `experiments`, HEAD `4098ad3e`. Present since the table was created. The ADR that makes the
omission a defect rather than an absence is dated 2026-09-14; before it, `session_id` was the whole
declared identity.

## Root cause

Two sites, and the first is the surprising one: **the value is in scope and is not passed.**

**The recorder is constructed without it.** `src/server.rs:1353-1358`:

```rust
let recorder = UsageRecorder::new(
    self.agent.clone(),
    self.debug,
    self.session_id.clone(),
    serving_session.unwrap_or_else(|| self.cc_session_id.clone()),
);
let input_for_record = input.clone();
```

Four arguments, no agent id. `UsageRecorder` (`src/usage/mod.rs:9-24`) holds exactly
`agent`, `debug`, `session_id`, `cc_session_id`, and `write_record`
(`src/usage/db.rs:169-240`) INSERTs 17 columns, none of them a principal's second axis. No
migration in `open_db` (`src/usage/db.rs:5-119`) has ever added one.

The agent id was already resolved. `src/server.rs:1269`:

```rust
let asserted_principal = crate::tools::session_key::principal_from_arguments(&mut input);
```

and it is consumed at `src/server.rs:1334` by `adopt_request_conversation`. That is **84 lines
before** the `UsageRecorder::new` above, in the same function body, still in scope.

**And the strip means `input_json` cannot stand in for the column.**
`principal_from_arguments` takes `&mut input` and removes the key
(`src/server.rs:1263-1269` documents the strip and its rationale). `input_for_record` is cloned
from `input` at `:1358` — *after* the strip. So the recorded arguments are post-strip and carry no
`dev.codescout.mcp/agentId`. The debug capture that preserves 97.63% of arguments preserves them
with the identity already removed.

Measured 2026-09-20 by reading the two symbol bodies and the `call_tool_inner` region; not observed
at runtime against a live database.

## Evidence

Schema columns, from `write_record`'s INSERT (`src/usage/db.rs:191-192`):

```
tool_name, called_at, latency_ms, outcome, overflowed, error_msg,
codescout_sha, codescout_dirty, project_sha, session_id,
input_json, output_json, cc_session_id, friction_target,
overflow_tokens, err_family, project_root
```

Every `open_db` migration that touched identity: `session_id` (v0.9),
`cc_session_id` (v0.10). Neither added an agent axis.

The 2026-09-18 baseline (`docs/research/2026-09-18-deep-agent-observation-baseline.md`) reports
"275 distinct session_id values, 64 distinct cc_session_id values" over 26,052 calls and correctly
labels those "coverage indicators only; they cannot identify a person, agent, task, or principal."
That caveat is this bug stated from the consumer side.

## Hypotheses tried

1. **Hypothesis:** `input_json` retains the injected `dev.codescout.mcp/agentId`, so the column is
   redundant.
   **Test:** read the ordering of the strip against the clone in `call_tool_inner`.
   **Verdict:** rejected — `principal_from_arguments(&mut input)` at `:1269` strips the key, and
   `input_for_record = input.clone()` at `:1358` runs after it. The recorded arguments are
   post-strip.

2. **Hypothesis:** `cc_session_id` supplies the missing axis.
   **Test:** read its field doc in `UsageRecorder` (`src/usage/mod.rs:13-23`).
   **Verdict:** rejected — it is the *Claude Code* session id, a second session-grain identifier.
   Both axes recorded today are sessions; neither is an agent.

## Fix

Fixed 2026-09-20.

- **SHA** `1dd363eb35bbca5bb064e22fb627f37c24f0217b`
- **patch-id** `7a661035f24967cfc813cc4af9192fdd95c77887`

A nullable `agent_id TEXT` column, added in the additive `open_db` style. The server splits
the companion's `<session_id>/<agent_id>` stamp at the first `/` immediately after
`principal_from_arguments` — before `asserted_principal` is moved into the ledger adoption —
and threads the agent half into `UsageRecorder::new`. One resolution site, per the precedent
in `cc_session_id`'s field doc. `None` degrades to the previous behaviour for every
unstamped client rather than erroring, as the ADR's product decision requires.

**One claim in this file was wrong, and the correction matters more than the fix.** This
file stated: *"There is no column, and no value inside `input_json`, that separates them —
so the merge cannot be undone after the fact by any query over this table."* The
`input_json` half is right. The first half is **not**: the composed stamp already reaches
the database, in `cc_session_id`.

`serving_session` is `adopt_request_conversation(asserted_principal.or(...))`, which returns
`asserted_principal` verbatim when one was stamped, and that value is passed into
`UsageRecorder::new`'s `cc_session_id` slot at `src/server.rs:1353-1358`. Measured
2026-09-20 against this checkout's db:

```
SELECT COUNT(*), SUM(cc_session_id LIKE '%/%') FROM tool_calls;
-- 68988 | 1873
```

1,873 rows carry the composed `<session>/<agent>` form, including this session's own
subagent calls. So the merge **was** undoable — by sniffing for a `/` that nothing
documents.

That makes the real defect a **conflation** rather than an absence: `cc_session_id` holds a
bare CC session id on some rows and a composed principal token on others, under a name that
states only the first. A consumer grouping by that column files a subagent apart from its
own parent's session and reads a mixed population as one kind of thing — which is what the
baseline's "64 distinct cc_session_id values" counted.

The fix deliberately does **not** change what `serving_session` writes into
`cc_session_id`: that value is also the ledger key, and re-pointing it is a behaviour change
with its own blast radius. `agent_id` is added alongside so telemetry stops being the place
a reader has to un-pick the conflation. The conflation itself is documented at the field and
is a separate, still-open defect — an `IC-24` instance rather than this file's `IC-23`.

## Tests added

`two_principals_in_one_session_are_distinguishable` (`src/usage/db.rs`) — three rows under
one `session_id`: two distinct `agent_id`s and one `None`. Asserts `COUNT(DISTINCT
agent_id) == 2` for that session **and** that the unstamped row records NULL rather than a
fabricated id. The assertion is on the grouping a consumer would actually perform; asserting
only that `agent_id` round-trips would be monotone under a schema that stored the value and
a consumer that could not group by it.

`open_db_migrates_principal_and_start_columns` — a pre-migration row survives and reads
NULL.

**RED observed in an isolated worktree** via `scripts/mutation-probe.sh`, not by arming the
shared tree. Replacing the `agent_id` parameter in `write_record`'s `params!` with a literal
`None` killed it: `COUNT(DISTINCT agent_id)` returned `0` against an expected `2`.

## Workarounds

None. The data is not captured, so no query recovers it. Consumers of `usage.db` must treat any
per-`session_id` figure as an aggregate over a parent **and all its subagents**, and must not
describe it as per-agent, per-principal, or per-worker.

## Resume

This is a precondition for the deep-agent design, whose `Decision` union
(`docs/trackers/local-semantic-evaluator-design.md`) is keyed on `principal` and whose delivery
ledger is described as "principal-keyed". A journal that distinguishes principals cannot be
evaluated against telemetry that does not.

## References

- `docs/adrs/2026-09-14-a-subagent-is-a-principal.md` — the decision this table does not implement.
  Read its *Decision* and *What the wire actually carries* sections; the measurement is what makes
  this a defect rather than a design preference.
- `docs/issues/archive/2026-08-16-usage-db-attributes-calls-to-a-shared-session-id-file.md` — the
  **same table, same axis, one layer up**: two concurrent sessions both recorded under whichever id
  a per-project file was written last, merging every per-session figure. Its fix is quoted in
  `UsageRecorder`'s `cc_session_id` field doc — "the server already resolves this correctly …
  giving the value one resolution site instead of two that drifted." The present bug is that
  sentence one axis short.
- `docs/issues/2026-09-19-file-provenance-answers-at-session-grain-so-sibling-subagents-are-one-writer.md`
  — **the same grain error in a different subsystem** (`scripts/file-provenance.py`, attribution)
  rather than telemetry. Both answer at session grain where the unit of interest is
  `(session, agent)`, and both are correct for the parent. Two subsystems, so `IC-23`'s spread
  bar is met by this pair; the count is the `n` column of that ledger's Index, not restated here.
- **Cluster fit.** `IC-23` is "a per-item attribute is derived at the container's granularity, and
  is correct for the first item". The container is the session, the item is the principal, and the
  row is exactly right for the parent — which is why nothing looks wrong.
