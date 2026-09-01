---
id: dd576a520be29aba
kind: bug
status: open
title: 'BUG: the tool-call recorder selects MCP calls only, so every usage question answers over a subset with no marker'
tags:
- cluster/selector-narrower-than-its-population
closed: ''
opened: 2026-09-01
owner: marius
severity: medium
unverified: 'RESOLVED 2026-09-01 and the answer was NO — the shell_command_mode eval does not derive its verdict from usage.db, because no such eval arm exists: all ten prompt-engineering scenarios pin shell_command_mode="warn", and usage.db appears once in that harness (claude_code.py:191) as fixture SEED, never as a scoring input. The file''s original urgency framing is withdrawn in ## Summary. STILL UNVERIFIED: (a) the separate claim in memory gotchas:493 that ''Eval data showed Opus performing better with native Bash than with run_command'' has no discoverable eval artifact in either repo — not searched: Langfuse, the headroom repo, or session transcripts, so this is absence-in-my-search rather than absence; (b) no fix is implemented and the write-path fork in ## Fix is undecided; (c) Tier 2''s recorder now lacks a demand argument.'
---

## Summary

`.codescout/usage.db` records **codescout MCP calls only**. Native `Bash` tool calls leave no
row, so every question asked of that database — `/analyze-usage`, a Pika scan, any
`tool_calls` query — silently answers over a subset. The zero it returns for shell-mediated
misuse reads as *"no such misuse"* rather than *"never looked"*. **The urgency framing this file shipped with was WRONG, and is withdrawn here before anyone acted on it.** It read: *"This matters now rather than in general, because `security.shell_command_mode` is a live evaluation arm comparing native `Bash` against `run_command`, and the instrument can see one of the two arms."* Measured 2026-09-01, resolving this file's own `unverified:` field:

- **No arm varies `shell_command_mode`.** All **ten** scenarios in `prompt-engineering` that mention it pin it to `"warn"` — base, treatment, control and positive-control alike. It is fixed setup, not the variable under test, so that harness does not evaluate the shell path at all.
- **The harness does not score from `usage.db`.** `usage.db` appears exactly once across its `scripts/` and `src/`, at `src/prompt_tdd/adapters/claude_code.py:191`, in a comment about *seeding* it as fixture setup before launch. Scoring runs through per-scenario `check_*.py` checkers over run logs. Nothing reads `tool_calls` for a verdict.

So there is no live eval whose verdict this blindness distorts, and the file's slug — `...cannot-see-the-arm-under-evaluation` — is now inaccurate. It is deliberately **not renamed**: a move re-keys the artifact id and would strand `IC-18`'s member citation and `d061c1ee`'s commit message, which is a worse cost than a stale slug that this paragraph corrects on sight.

**What survives, unchanged and still worth fixing.** The blindness itself is measured and real: `/analyze-usage`, any Pika scan, and `docs/trackers/tool-usage-patterns.md` cannot see shell work routed through native `Bash`, and a zero from any of them reads as *"none"* rather than *"never looked"*. What changes is the deadline, not the defect — this is ordinary technical debt on a measurement surface, not something gating an in-flight decision. Tier 1 below (name the scope at every reader) is accordingly the whole of what is clearly owed; Tier 2's recorder now needs a demand argument it does not currently have.

## Symptom (Effect)

```
$ sqlite3 .codescout/usage.db \
    "SELECT COUNT(*), SUM(tool_name LIKE '%ash%'), COUNT(DISTINCT tool_name) FROM tool_calls;"
52769|0|26
```

**52,769** recorded calls, **0** matching `%ash%`, **26** distinct tool names — all of them
codescout tools:

```
run_command 19398 | grep 5694 | artifact 5313 | read_file 5213 | symbols 4394
edit_file 3422 | edit_code 2366 | read_markdown 2229 | edit_markdown 1425
create_file 1059 | workspace 713 | librarian 570 | …
```

No error, no warning, no `NULL`. A well-formed answer over a population that excludes an
entire tool.

## Reproduction

```
git rev-parse HEAD                      # 2026-09-01: bb4688fd
sqlite3 .codescout/usage.db "SELECT COUNT(*), SUM(tool_name LIKE '%ash%') FROM tool_calls;"
```

Then issue any native `Bash` call and re-run. The count does not move.

## Environment

Linux, codescout `v0.15.0`, branch `experiments`, MCP over stdio, `~/.claude-sdd` profile.
Six live sessions had this checkout as cwd at the time of measurement, so the corpus is
multi-session.

## Root cause

The recorder is wired at the **MCP server boundary**. `record_call` in `src/usage/db.rs:191`
is reached from the server's own dispatch path (`src/server.rs`), which by construction only
ever sees calls routed through this MCP server. Native `Bash` is a **harness** tool: it never
enters codescout's dispatch, so no code path exists that could record it. The selector is not
a filter someone wrote too narrowly — it is the boundary the recorder is mounted on, which is
why widening it needs a second capture point rather than a predicate change.

*measured 2026-09-01: the `sqlite3` query above, run in this repo — 0 of 52,769 rows and 26
distinct `tool_name` values, none of them a harness tool.* The mechanism itself
(`record_call`'s call site inside the server) is **inferred from the code and the query
result, not observed at runtime** — nothing here traced a `Bash` call to confirm it reaches no
writer.

## Evidence

### The table is not the problem — the mount point is

`tool_calls` already carries everything a recorder would need. The original `CREATE TABLE`
(`src/usage/db.rs:15`) lists seven columns, but later `ALTER TABLE … ADD COLUMN` migrations
add ten more, including `input_json`, `output_json`, `session_id`, `cc_session_id`,
`codescout_sha`, `project_sha`, `friction_target`, `err_family` and `project_root`
(`src/usage/db.rs:59`, insert at `:191`).

**Reading the `CREATE TABLE` alone gives a confidently wrong picture of the schema** — it was
read that way first here, and produced a false "there is nowhere to put the data" conclusion
that survived about ninety seconds. The live table is the create *plus every migration*.

### The hook surface already exists

`codescout-companion/hooks/hooks.json` wires `PostToolUse` alongside `PreToolUse`,
`SessionStart`, `Stop` and `UserPromptSubmit`. So a capture point for harness tools is
available and does not need inventing.

### CLAUDE.md already states the blindness — as a footnote, not as a defect

> *"`usage.db` records only codescout MCP calls … So shell work routed through `Bash` does not
> appear in `/analyze-usage`, `docs/trackers/tool-usage-patterns.md`, or the
> `pika_observations` table."*

The fact was known and recorded. What was not recorded is that it makes an in-flight
comparison unmeasurable on one side.

## Hypotheses tried

1. **Hypothesis:** the schema lacks columns for a shell command, so recording needs a
   migration. **Test:** read `src/usage/db.rs` past the `CREATE TABLE`. **Verdict:** rejected —
   `input_json` and nine other columns arrive via `ALTER TABLE`. **Evidence:** *The table is
   not the problem*.
2. **Hypothesis:** the companion has no `PostToolUse` surface, so a recorder has nowhere to
   mount. **Test:** `grep` the wired hook events in `hooks/hooks.json`. **Verdict:** rejected —
   `PostToolUse` is wired.
3. **Hypothesis:** the omission is a too-narrow predicate in the recorder. **Test:** locate the
   `record_call` call site. **Verdict:** rejected — it is the mount point, not a predicate;
   nothing filters `Bash` out because nothing ever offers it.

## Fix

Two tiers. **Do the first regardless; the second is the one that needs a decision.**

### Tier 1 — name the scope at every read surface (cheap, no new capture)

This is `IC-18`'s tool-facing remedy and the ADR it promotes to
(`docs/adrs/2026-08-27-negative-results-name-their-scope.md`, Accepted): a suspicious negative
must name the scope it examined. Every surface that reads `tool_calls` — `/analyze-usage`, the
`usage` tool, `doctor://tool-usage`, the dashboard — should state *"codescout MCP calls only;
native harness tools are not recorded"* beside any count or zero. It does not fix the
blindness; it stops the blindness being invisible, which is the difference between a partial
answer and a misleading one.

### Tier 2 — the recorder (`codescout-usage-hookify:H-8`)

A `PostToolUse` hook on native `Bash` that writes a `tool_calls` row and **surfaces nothing to
the agent**.

- **Decision: `record`, never `deny` or `warn`.** `H-7` is the standing precedent — it rejected
  a `read_file`-on-source deny after a `usage.db` sweep showed 82–94% of those calls were
  legitimate, i.e. the threshold had been proposed before the measurement. Do not repeat that
  here. A deny keyed on "Bash touches `*.rs`" would also have blocked the mutation runs that
  produced `bug-fix-session-log:F-94`, which were the work and not the mistake.
- **Row shape:** `tool_name='bash'`, `input_json` carrying the command's first token plus an
  extension classification of its arguments, `outcome` from the exit code, `latency_ms`,
  `cc_session_id`. Deliberately **not** the raw command — see *Open question 2*.

**Open question 1 — the write path.** The insert is a Rust fn; hooks are `.mjs`. Three options:

| option | pro | con |
|---|---|---|
| hook shells out to a `codescout` CLI subcommand | schema ownership stays server-side | one extra process per Bash call |
| hook writes sqlite directly | no process spawn | couples the hook to the schema; concurrent-writer risk |
| server exposes a record endpoint | clean ownership | needs the server reachable from a hook |

Lean is the **CLI subcommand**: `H-8` requires the recorder be silent to the agent, so latency
is off the critical path, and schema ownership is the thing most expensive to get wrong.

**Open question 2 — what of the command to store.** Raw command lines carry paths, hostnames
and occasionally secrets, and `usage.db` is a long-lived local corpus read by scans. Storing
the first token plus an argument classification answers the coverage question without
accumulating shell history. Decide before shipping, not after.

**Do not tune anything on this data until it exists.** `H-8`'s `Promote-when` is deliberate:
ship the recorder, accumulate ~2 weeks, *then* ask whether any warn is justified against a real
denominator.

## Tests added

**N/A — nothing is implemented.** When Tier 2 lands, the regression test must assert a `Bash`
call produces a `tool_calls` row, and must **fail before the hook is installed** — the natural
shape here is an absence assertion, which is monotone under "the recorder does nothing" and
would pass against a stub. Pair it with a positive row-shape assertion, per `CLAUDE.md`
§ *Testing Discipline*.

## Workarounds

None for the data already lost — the calls were never recorded and cannot be reconstructed
from `usage.db`. For a specific question, session transcripts under
`$CLAUDE_CONFIG_DIR/projects/<slug>/<session-id>.jsonl` do record `Bash` calls and can be
counted directly. Note that this is **per-profile**, so a count taken there must sweep all
three profile directories or it reproduces the same class one level up.

## Resume

Decide *Open question 1* (write path) and *Open question 2* (what of the command to store),
then implement Tier 1 first — it is independent, needs no hook, and delivers the honesty half
immediately. Tier 1's concrete next action: find every reader of `tool_calls`
(`src/dashboard/api/usage.rs`, `src/dashboard/routes.rs`, the `usage` tool) and add the scope
sentence to each response.

## References

- `docs/trackers/codescout-usage-hookify.md` § `H-8` — the recorder proposal and its measurement
- `docs/trackers/codescout-usage-hookify.md` § `H-7` — why no gate ships before a base rate
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — Tier 1's Accepted remedy
- `docs/trackers/issue-clusters.md` § `IC-18` — the defect class this instantiates
- `src/usage/db.rs:15` (create), `:59` (migrations), `:191` (insert)
- `claude-plugins/codescout-companion/hooks/hooks.json` — the `PostToolUse` surface
