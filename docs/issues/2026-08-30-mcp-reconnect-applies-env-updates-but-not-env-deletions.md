---
id: '1c5e106ee122f582'
kind: bug
status: mitigated
title: 'BUG: /mcp reconnect applies a CHANGED env var from settings.json but not a REMOVED one — and the change that lands falsely confirms the one that did not'
tags:
- cluster/config-propagation-is-additive
- harness
- mcp-reconnect
- config
- stale-env
- false-confirmation
- not-codescout-source
opened: 2026-08-30
owner: marius
severity: high
unverified: 'Reproduced 2026-09-24 on Claude Code 2.1.282, but LAYER-SPECIFIC: settings.json § env drops a deletion on /mcp (in-phase control observed), while .claude.json mcpServers.<name>.env applies one. The process.env-merge mechanism is a hypothesis the data fits and does not test — no non-codescout child spawned after a phase-1 load was read. Whether a full restart clears the stale key is untested. Claude Code''s source is not in this repo.'
---

# BUG: `/mcp` reconnect applies a CHANGED env var from `settings.json` but not a REMOVED one

> **Not a codescout bug.** `settings.json` → MCP `env` injection is Claude Code
> harness behaviour; its source is not in this repo. Filed here under CLAUDE.md's
> "open a bug file for ANY bug noticed during work — including tool
> quirks/misbehaviors", because the cost lands on codescout's retrieval config.

## Summary

One edit to `~/.claude/settings.json` § `env` made two changes:

- **removed** `CODESCOUT_QUERY_PREFIX`
- **changed** `CODESCOUT_BM25_BOOST` from `"3.0"` to `"5.0"`

After `/mcp`, the newly-spawned server has the **new boost** and the **removed
prefix still set**. The update landed; the deletion did not.

## Symptom (Effect)

`printenv` inside the server spawned 11 s after the reconnect:

```
QUERY_PREFIX: Represent this query for searching relevant code:    <- deleted on disk
BM25_BOOST:   5.0                                                  <- new value, applied
```

## Reproduction

1. In `<profile>/settings.json` § `env`, delete one key and change another.
2. `/mcp`.
3. Read the new server's environment (`/proc/<pid>/environ`, or any child it
   spawns).

Both halves of one edit, one reconnect, opposite outcomes.

## Evidence

**The key is genuinely gone from disk.** `grep -c CODESCOUT_QUERY_PREFIX
~/.claude/settings.json` → **0**. JSON re-validated after the edit (21 top-level
keys, `env` 13→12), and `diff` against the pre-edit backup shows exactly the two
intended lines and nothing else.

**The file WAS re-read on reconnect — this is the load-bearing control.**
`CODESCOUT_BM25_BOOST=5.0` exists in no other config layer: `~/.config/codescout/.env`
(→ `.env.gpu`) says `3.0`, both sibling profiles' `.claude.json` say `3.0`, and the
parent process has no such variable at all. `5.0` could only have come from the file
edited moments earlier. So this is **not** a stale-cache-of-everything story; the
reconnect read the new file and still produced the old key.

**It is not shell inheritance.** The parent `claude` process (pid 801487, unchanged
across the reconnect) carries **neither** variable in its own environ — checked
directly. So the value is held in the harness's own in-memory config, not the
process environment it was launched with.

**Fleet snapshot at the time**, showing the new server against its siblings:

| pid | age | prefix | boost |
|---|---|---|---|
| 3995093 (mine, post-reconnect) | 00:11 | **1** | **5.0** |
| 3911498 | 09:10 | 1 | 3.0 |
| 3885828 | 11:32 | 1 | 3.0 |
| 3882799 | 12:02 | 1 | 3.0 |
| 791075 | 06:36:25 | 0 | 3.0 |

## Root cause

**Inferred, not read** — the harness source is not in this repo. The behaviour is
consistent with the spawn environment being built by **merging** the freshly-read
`settings.json` env over a previously-loaded map, rather than replacing it. Under a
merge, a key whose *value* changed is overwritten; a key that no longer *exists*
has nothing to overwrite it, so the stale entry survives.

This is the same shape as the "sourcing cannot unset" hazard already noted in
`docs/trackers/retrieval-benchmark.md`, one layer up: there it was shell env files,
here it is the harness's own config merge.

### The mechanism made a prediction, and it held

The merge hypothesis is not merely consistent with the observation — it forecast a
**different** outcome for a **different** edit, and that outcome occurred.

Prediction: if the harness merges rather than replaces, then re-adding the key with
an empty value should take effect on the next reconnect, because that is an *update*
rather than a deletion — the same operation that already worked for `BM25_BOOST`.

Run: set `"CODESCOUT_QUERY_PREFIX": ""`, `/mcp`, read the new server (pid 4031908,
age 13 s):

```
QUERY_PREFIX = $        # cat -A: empty string, not the old value
BM25_BOOST   = 5.0
```

So the same key that could not be **removed** by a reconnect was successfully
**changed** by one, minutes later, in the same file. That is the update/delete
asymmetry isolated on a single variable, with everything else held constant — which
is stronger than the original observation, where the asymmetry was split across two
different keys and could have been explained by something specific to one of them.

Still **evidence, not proof**: a merge is not the only implementation that would
produce this, and the harness source remains unread. But any competing explanation
now has to account for one key behaving both ways within minutes.
## Why this is worse than a plain stale-config bug

**The half that works falsely confirms the half that does not.** The natural
verification after a config edit is to check the thing you changed. If the edit
contained any update at all, that check passes — and a reader reasonably concludes
the reconnect applied the edit. Nothing distinguishes "the file was re-read" from
"the file was re-read *and fully applied*" except separately probing a key you
*deleted*, which is not an obvious thing to do.

Had this edit changed only the boost, the verification would have been clean and
correct. Had it only removed the prefix, the failure would have been obvious. The
mixed edit is the case that misleads, and mixed edits are the normal case when
tuning a config.

**It also means any config fix expressed as a deletion silently no-ops on
reconnect.** That is the whole class of "remove the harmful setting" fixes — the
form most correctness fixes to an env block take.

## Suspected prior instance

`docs/trackers/retrieval-benchmark.md` § *2026-07-28* records removing
`CODESCOUT_QUERY_PREFIX` from two profiles and marks it **"real — now fixed"**, with
"takes effect on MCP restart". Those two profiles do read clean today, so the fix
did eventually land there. But if it was verified by reconnect rather than by a full
restart, the verification at the time proved nothing, and the month-long survival of
the same setting in a third profile went unnoticed alongside it. **Suspected, not
established** — no record survives of how that check was run.

## Re-checked 2026-09-24 — not reproducible passively, and the edit surface differs by profile

**No reproduction was possible from inside a session.** It needs a key deleted from config *during* a
Claude process's lifetime followed by `/mcp`, which only the operator can type. The checking session's
`claude` process started 2026-09-24T19:13:53Z; nothing was deleted since.

**New fact, and it changes the Reproduction recipe:** on `~/.claude-sdd` the MCP server's env does not
come from `settings.json` at all. The live server (spawned 19:17:50Z) carries **13** `CODESCOUT_*` keys;
`~/.claude-sdd/settings.json` § `env` defines **0** of them, and the parent `claude` process's own environ
holds **0** (so not shell inheritance). They come from `~/.claude-sdd/.claude.json` →
`mcpServers.codescout.env` (16 keys). `~/.claude-kat/.claude.json` carries 15; `~/.claude/.claude.json`
carries 0, and there `settings.json` is the surface. So step 1 (*"in `<profile>/settings.json` § `env`"*)
holds for `~/.claude` only, and **whether the `.claude.json` layer has the same update/delete asymmetry is
untested**.

For the record, the `.claude-sdd` server's `CODESCOUT_QUERY_PREFIX` is the non-empty
`Represent this query for searching relevant code: ` and `CODESCOUT_BM25_BOOST` is `3.0`. Whether that is
the intended retrieval config is not this bug's question.

Still owed: the operator-run experiment (delete a key → `/mcp` → read `/proc/<server>/environ`, once per
layer), or an upstream report — the mechanism is not in this repo. Checked by sessionId
`e4fbc7ef-27b7-4707-8469-ccdffa8e4e92`.

## Measured 2026-09-24 — the `.claude.json` layer applies a deletion (Claude Code 2.1.282)

The operator-run experiment the section above asked for, on the layer that section found is the
live one for `~/.claude-sdd`: `~/.claude-sdd/.claude.json` → `mcpServers.codescout.env`. Probe keys
were chosen so nothing reads them (`ZZ_MCP_ENV_*`); the file was edited by atomic replace and
re-read before each reconnect; one `claude` process (pid 2834158) throughout, so every reading is a
`/mcp` reconnect and never a restart.

| phase | edit to the file | new server (pid, start UTC) | `PROBE` | `CONTROL` | reads as |
|---|---|---|---|---|---|
| 1 | add `ZZ_MCP_ENV_PROBE=phase1` | 3543619, 19:55:11 | `phase1` | — | the layer IS re-read on `/mcp` — the control that makes phase 2 mean anything |
| (no-op) | none; a "continue" arrived without `/mcp` | still 3543619 | `phase1` | absent | **no reconnect** — separated from "deletion not applied" only by the unchanged pid |
| 2 | ONE write: delete `PROBE`, add `ZZ_MCP_ENV_CONTROL=phase2` | 2826596, 20:56:55 | **absent** | `phase2` | **both halves of a mixed edit applied** |

Phase 2 is the original bug's own shape — one edit, one deletion plus one update, one reconnect —
with the update as its in-phase control. On this layer and this build the deletion **lands**. The file
was restored after: env map back to 16 keys and equal to the pre-experiment backup's, no `ZZ_` key
left. Run by sessionId `e4fbc7ef-27b7-4707-8469-ccdffa8e4e92` with the operator typing `/mcp`.

**What this does and does not settle.** It falsifies *merge-over-replace as the harness's general
behaviour*: whatever built the 2026-08-30 spawn env, it is not what builds the `.claude.json` layer's
today. It does NOT show the 08-30 observation was wrong, and does not re-test it: that was the
`~/.claude/settings.json` § `env` layer, on an older build. Two readings remain and this data cannot
separate them — the asymmetry is specific to the `settings.json` layer, or it was fixed between that
build and 2.1.282. The discriminating run is the same two phases against `settings.json` § `env`.

## Reproduced 2026-09-24 on Claude Code 2.1.282 — the `settings.json` layer drops a deletion; `.claude.json` does not

The discriminating run the section above named: the same two phases against `~/.claude-sdd/settings.json`
§ `env`, same `claude` process (pid 2834158) throughout, every reading a `/mcp` reconnect.

**Precondition, checked first:** this layer reaches the MCP spawn env at all. `PUPPETEER_EXECUTABLE_PATH`
is defined only in that `env` block, and it is present in the codescout server's environ and absent from
both the parent `claude` process's environ and `.claude.json`. Without that, an absent key would mean nothing.

| phase | edit to `settings.json` § `env` | new server (pid, start UTC) | `PROBE` | `CONTROL` |
|---|---|---|---|---|
| baseline | none (after the `.claude.json` restore) | 3190735, 21:00:06 | absent | absent |
| 1 | add `ZZ_MCP_ENV_PROBE=phase1` | 3474525, 21:02:32 | `phase1` | absent |
| 2 | ONE write: delete `PROBE`, add `ZZ_MCP_ENV_CONTROL=phase2` | 4080390, 21:07:52 | **`phase1` — stale** | `phase2` |

**The asymmetry reproduces, isolated on one layer with its own in-phase control:** the update in the
same write landed, so the file was re-read, and the deleted key survived. Beside the `.claude.json` run
above, where the identical edit shape applied both halves, the defect is now **layer-specific** rather
than general: `settings.json` § `env` drops deletions on `/mcp`; `mcpServers.<name>.env` does not.

**The baseline row is load-bearing.** The key names were reused from the `.claude.json` run, so a stale
`phase1` could in principle have come from there. The baseline server, read after that layer's restore and
before any `settings.json` edit, carried no `ZZ_` key, and nothing but `settings.json` changed after it.

**Mechanism — a hypothesis this data fits and does not test.** `settings.json` § `env` is process-wide,
so the harness plausibly assigns it into its own `process.env` (inherited by every child) and never deletes
a removed key there; a server's `.claude.json` env is passed explicitly per spawn and so is replaced. That
would also explain the 2026-08-30 note that the parent's `/proc/<pid>/environ` held neither variable: that
file shows the exec-time block, not later `process.env` writes. **Test it by** reading any NON-codescout
child spawned after a phase-1 load (e.g. reconnect a second MCP server): under this hypothesis it carries the
stale key too. Not run — the only other child (the researcher MCP, pid 2835488) predates every probe.

**Restored:** `settings.json` byte-identical to the pre-experiment backup (`cmp`), after checking nothing but
the `ZZ_` keys had changed meanwhile. If the defect is as measured, the RUNNING `claude` process still
carries both `ZZ_` keys into every codescout spawn until a full restart — inert, and a free replicate on the
next reconnect. Run by sessionId `e4fbc7ef-27b7-4707-8469-ccdffa8e4e92`; the operator typed each `/mcp`.

## Workarounds

- **Full Claude Code restart** for any edit that removes a key. Untested at filing
  time; the next restart is the natural experiment.
- Or **neutralise instead of deleting** — set the key to an empty value if the
  consuming code treats empty as unset. For `CODESCOUT_QUERY_PREFIX` specifically it
  does: `EmbedderHttp::new` reads it with `unwrap_or_default()` and `dense_query`
  short-circuits on empty, so `"CODESCOUT_QUERY_PREFIX": ""` is behaviourally
  identical to absent. **This is the reliable workaround for this key** and it
  survives the merge, because it is an update rather than a deletion.

## Fix

Not ours. The harness should build the MCP spawn environment by **replacing** the
config-derived env rather than merging over the previous one, so that a removed key
is removed.

## Disposition 2026-09-25 — terminal here, and why it was not already

**Status moved `investigating` → `mitigated`.** Nothing in this file is still under
investigation: the behaviour is reproduced, the layer is discriminated, and § *Fix* reads
*"Not ours"* with § *Tests added* reading *"None — no codescout code is involved."*

**The reason it sat in the live pool is worth one line, because it is a routing defect rather
than an oversight.** `investigating` means *worked, no live owner* — which was true — so the
bug kept surfacing in the "what is available?" query that `CLAUDE.md` § *Querying active
trackers* prescribes, and in the `claimable` hint's unclaimed list. A triager therefore reaches
it as available work and pays a 267-line read to arrive at a conclusion the file already
reached. The status vocabulary has no value meaning *"complete, but the fix belongs to someone
else's codebase"*; `mitigated` is the closest, and this section is what the status alone cannot
say.

**The general workaround is MEASURED, not merely the restart one.** § *Workarounds* leads with
a full Claude Code restart, flagged *"untested at filing time"*, and it is still untested —
testing it means ending the session that would observe the result. But § *Measured 2026-09-24*
already establishes a tested alternative: **the `.claude.json` layer applies a deletion; only
the `settings.json` layer drops one.** So a key that may need removing later belongs in
`.claude.json`, and that rests on a measurement taken the same day, on the same harness version,
rather than on an argument about how a restart must work.

**What would reopen this:** a harness release whose changelog names the MCP spawn environment,
or an observation that `.claude.json` has begun dropping deletions too — the control that makes
the workaround a workaround. Re-running the § *Reproduced 2026-09-24* two-phase procedure is the
check; it needs the `PUPPETEER_EXECUTABLE_PATH`-style precondition re-verified first, or an
absent key proves nothing.

*(Dispositioned by sessionId `3aa55c01-9663-44ca-82d2-48b6b8d76d66`, who did not author this file,
added no measurement of their own, and changed no code. The claim taken to do this is released.)*

## Tests added

None — no codescout code is involved.

## References

- `docs/trackers/retrieval-benchmark.md` § *2026-08-30 — D2 resolved* — the edit that
  exposed this, and the fourth config layer it also uncovered.
- `docs/issues/archive/2026-08-17-mcp-reconnect-does-not-refresh-server-instructions.md`
  — same family: `/mcp` refreshes one surface and not another.
- `docs/issues/archive/2026-08-18-clear-leaves-mcp-session-id-stale.md` — same family.
- `docs/issues/archive/2026-08-29-stale-model-dir-env-masked-by-shell.md` — adjacent, but a
  different mechanism (ambient shell masking a repo file, not a harness merge).
