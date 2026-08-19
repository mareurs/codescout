---
status: open
opened: 2026-08-20
closed:
severity: high
owner: marius
related:
  - docs/issues/archive/2026-08-18-clear-leaves-mcp-session-id-stale.md
  - docs/issues/archive/2026-08-16-usage-db-attributes-calls-to-a-shared-session-id-file.md
  - docs/issues/archive/2026-06-14-get-guide-reinjects-on-mcp-restart.md
tags:
  - usage-db
  - telemetry
  - session-identity
kind: bug
---

# BUG: usage.db's session id is frozen at construction while the guide ledger re-keys per call, so 31% of rows may name the wrong conversation

## Summary

The 2026-08-18 session-identity fix gave the guide ledger a per-call rendezvous that
detects a conversation change and re-keys. It deliberately scoped telemetry out:
`cc_session_id` is still resolved once at server construction and cloned unchanged into
every `UsageRecorder`. Any client that attaches to an **already-running** codescout
process — `/clear`, or a subagent reusing a live server — records its calls under whatever
conversation owned that process at birth. Measured on this project's own `usage.db`:
**8,980 of 29,103 rows (30.9%) and 10 of 22 conversation identities (45%)** sit in pools
where one server-side `session_id` carries two or more `cc_session_id` labels.

This is the **third** appearance of one seam. Each previous fix repaired the ledger side
and left telemetry a step behind.

## Symptom (Effect)

One server-side `session_id` maps to multiple `cc_session_id` values. Measured
2026-08-20 against `/home/marius/work/claude/codescout/.codescout/usage.db`:

```
             session_id               n_cc  rows_
------------------------------------  ----  -----
ff553ea5-d39e-41f3-b143-819767ed46e4     2   3410
6e0a2314-53c4-4338-b700-79eb851c0583     2   2792
0db8a1c4-5381-4237-911e-2a012a775332     2    553
1eacc927-ec1a-43cc-a73b-d2cf44fc3de1     2    463
764aab8d-24a4-49ef-a79b-7aab1a077f6e     2    335
d78fa57f-f225-48d1-beb7-ad536c4a22a6     3    303
46d6c103-5f12-4b63-a62e-e2d253735e85     2    262
5c0b1fb8-9f0c-4842-ae03-f7e6ebd5fd0d     5    228
... (16 pooled session_id values total)

pooled_session_ids  pooled_rows  total_rows  affected_cc_ids  total_cc_ids
------------------  -----------  ----------  ---------------  ------------
                16         8980       29103               10            22
```

There is no error, no warning, and no NULL. The rows look like clean per-conversation
data. **Populated-and-wrong is worse than NULL here**, because a NULL cannot be silently
merged into someone else's total.

## Reproduction

```
# HEAD at filing: b4ea12fd989dfc2cbf1604be36090ddd3c99a6a3 (experiments)
sqlite3 -column -header .codescout/usage.db "
  SELECT session_id, COUNT(DISTINCT cc_session_id) n_cc, COUNT(*) rows_
  FROM tool_calls GROUP BY session_id HAVING n_cc > 1 ORDER BY rows_ DESC;"
```

Any row returned is a pool: one codescout process, more than one conversation label.

Live reproduction of the mechanism: open a codescout-backed conversation, run any
codescout tool, `/clear`, then run another tool. `/clear` starts a new conversation with a
new id but does **not** respawn stdio MCP subprocesses, so both calls land under the first
conversation's `cc_session_id`.

## Environment

Linux; codescout on branch `experiments` at `b4ea12fd`; stdio MCP transport; Claude Code
2.1.233–2.1.235 (three profiles in use on this host: `~/.claude`, `~/.claude-sdd`,
`~/.claude-kat`).

## Root cause

Two identities are resolved for one process, under deliberately different rules, and only
one of them is refreshed per call.

- `src/server.rs:58-61` — `ServerEnv::cc_session_id` is documented as
  *"correlation id for `usage.db`. NOT the ledger key: see `session_id_explicit` /
  `harness_session_ids`, which resolve the ledger's identity under different collision
  requirements."*
- `src/server.rs:115-125` — `ServerEnv::from_env()` reads `CLAUDE_CODE_SESSION_ID` **once**
  via `std::env::var`.
- `src/server.rs:169-177` — `CodeScoutServer::cc_session_id: String`, doc comment:
  *"Resolved once on purpose."* The justification cites the 2026-08-16 per-PROJECT-file
  bug — correct as far as it goes, but "once" is precisely what 2026-08-18 identified as
  the defect on the ledger side.
- `src/server.rs:178-180` — `session_key`: *"Resolved conversation identity for the guide
  ledger. Distinct from `cc_session_id`, which is usage-correlation only."*
- `src/server.rs:974-977` — every `call_content` hands `self.cc_session_id.clone()` to
  `UsageRecorder::new`. The rendezvous is never consulted for telemetry.

A running process's environment cannot be mutated by its parent, so the env var can never
reflect a later conversation. The 2026-08-18 fix solved exactly this for the ledger by
polling a pid-keyed rendezvous slot before consulting the ledger on each tool call. That
machinery already exists and already runs per call — telemetry simply does not read it.

measured 2026-08-20: the `sqlite3` query above, plus
`grep(pattern="cc_session_id", path="src/server.rs", context_lines=3)` and
`grep(pattern="CLAUDE_CODE_SESSION_ID|fn from_env", glob="src/**/*.rs")`, read directly
rather than inferred.

## Evidence

### The seam has opened three times

| Bug | What was fixed | What was left |
|---|---|---|
| `2026-06-14-get-guide-reinjects-on-mcp-restart` | ledger persisted per conversation id; single construction-time env read introduced | — (correct for events that respawn) |
| `2026-08-16-usage-db-attributes-calls-to-a-shared-session-id-file` | telemetry stopped reading the per-PROJECT file; server's resolved id threaded in | id still resolved once, at construction |
| `2026-08-18-clear-leaves-mcp-session-id-stale` | ranked key chain + per-call rendezvous re-keying, for the **ledger** | telemetry explicitly excluded — `cc_session_id` untouched |

The 2026-08-16 file's own root cause reads: *"the fix for collision was applied at the
guide-ledger call site and not at the telemetry call site."* That sentence describes this
bug too, one fix later.

### The archived 2026-08-18 file already predicted the telemetry half

Its Summary states both consequences of the single read — ledger suppression **and**
*"`usage.db` misattributes the new conversation's calls to the old session."* Only the
first was addressed; the file was archived as fixed.

### Corroborating analysis (method cited, not independently re-measured here)

A transcript-vs-DB reconciliation pass on 2026-08-19/20 traced one concrete pool: session
`4ba7e23c`'s 33 subagent transcript files (3,220 `tool_use` blocks, all correctly
`workspace(activate)`-ed to this repo) are absent from `4ba7e23c`'s own DB bucket and
appear under `cc_session_id='b8bb058f'`, whose transcript is a 10-line stub with zero
assistant turns. Both share server-side `session_id=ff553ea5` — the 3,410-row pool at the
top of the table above. Reported by that pass; the pooling counts in *Symptom* are my own
measurement, the `b8bb058f` attribution is theirs.

## Hypotheses tried

1. **Hypothesis:** this is a duplicate of `2026-08-18-clear-leaves-mcp-session-id-stale`.
   **Test:** read that file's Fix section, then grep `cc_session_id` through
   `src/server.rs` to see whether the ranked-resolution/rendezvous fix reached the
   telemetry call site. **Verdict:** rejected. The fix is real and scoped to the ledger by
   design; `src/server.rs:178-180` documents the separation explicitly, and
   `src/server.rs:974-977` still passes the construction-time value.
   **Evidence:** *The seam has opened three times*.
2. **Hypothesis:** the overcounts are composed-but-never-round-tripped `tool_use` blocks
   (interrupted or rejected calls).
   **Test:** the corroborating pass checked `23b22760` (94 transcript vs 90 DB) for
   orphaned `tool_use`, errors, and interrupt/reject markers. **Verdict:** rejected — zero
   of each; every call had a clean success result. The 4-call delta was a per-call
   `workspace` override routing to a now-deleted worktree's own DB (see
   `docs/issues/2026-08-20-worktree-removal-deletes-its-usage-telemetry.md`).

## Fix

Not yet implemented. The cheap correct shape: resolve `cc_session_id` **per call** from the
same rendezvous the ledger already polls, instead of cloning the construction-time value at
`src/server.rs:974-977`. The rendezvous is queried on every tool call already, so the
marginal cost is a field read, not new I/O.

Two decisions belong to whoever takes it:

1. **Historical rows.** 30.9% of existing rows carry a possibly-wrong label and nothing
   distinguishes them. Option 3 from the 2026-08-16 file is still unimplemented and still
   applies: a `session_attribution` column, or a documented cutoff date that every
   consumer of `cc_session_id` honours. Without one, fixing the write path leaves a corpus
   that is half-trustworthy with no way to tell which half.
2. **Whether `session_id` should become the primary analysis key.** It is per-process and
   never wrong; `cc_session_id` is per-conversation and sometimes wrong. Analyses that
   group by `session_id` first and disambiguate by timestamp window are correct today.

Record the fix SHA **and** its patch-id (`git show <sha> | git patch-id --stable`).

## Tests added

None yet. A regression test should assert that a `cc_session_id` change observed through
the rendezvous mid-process changes the id written to `tool_calls` — the natural sibling of
whatever test covers the ledger's re-arm on the same signal.

## Workarounds

For any analysis over `usage.db`: **do not group by `cc_session_id` alone.** Group by
`session_id`, and where one `session_id` maps to several `cc_session_id`, attribute rows by
`called_at` falling inside each candidate conversation's own activity window. The pooled
population is enumerable with the *Reproduction* query, so an analysis can at least report
how much of its corpus is affected.

## Resume

Read `src/server.rs:960-990` and the rendezvous poll that precedes the ledger consult in
the same function; determine whether the resolved identity is available at that point or
needs threading. Then decide item 1 under *Fix* — historical rows — before changing the
write path, since the choice affects whether a new column is needed in the same migration.

## References

- `src/server.rs:58-61`, `:115-125`, `:169-180`, `:344-356`, `:974-977`
- `src/usage/mod.rs:16-23` — records the 2026-08-16 fix and its rationale
- `src/tools/session_key.rs:36` — `HARNESS_SESSION_VARS`
- `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` — Phase B,
  the ledger-side fix whose scope excluded telemetry
