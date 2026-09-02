---
id: '8a1a5576d005f8d5'
kind: bug
status: open
title: the peer-enumeration regex reads a former session name as the current one
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
---

## Summary

`/codescout-companion:reaching-peer-sessions` Step 1 extracts each peer's name from its
registry JSON with

```
sed -n 's/.*"name":"\([^"]*\)".*/\1/p'
```

POSIX `sed`'s `.*` is greedy, so the capture binds to the **last** `"name":"` on the line.
A session that has been renamed carries `formerNames: [{"name": …, "until": …}]` — a list of
**objects**, each contributing its own `"name":` key — so the last occurrence is a name the
session no longer has. The table prints a plausible peer name and never an error.

This is the disambiguator half of `cluster/addressing-without-an-escape-hatch`: the JSON
namespace *does* carry the disambiguator (nesting — top level vs. inside `formerNames`), and
flat-text matching discards it. Sibling member
`a-git-verb-regex-swallows-longer-subcommands-sharing-its-prefix` is the same half in a shell
gate; this is the same half in the shell *discovery* path that CLAUDE.md § *Reaching a Peer
Session* makes the standing instruction.

## Symptom (Effect)

The skill's Step 3 routes same-profile peers **by name**. Given a stale name, `SendMessage`
answers `No agent named 'X' is reachable` — which this very skill teaches the reader to
interpret as *"switch to the `uds:` form"*. The remediation therefore **absorbs** the failure:
the reader concludes the peer is cross-profile and re-addresses by socket, which works, so
nothing ever surfaces the fact that the name column was wrong. The wrong value survives its
own symptom.

Second effect, worse because it is silent end-to-end: the table is the instrument used to
*report* who is present. A stale name in a report is a misattribution with no error anywhere
in the chain.

## Reproduction

Measured 2026-09-02 on this machine, over the live socket population:

| quantity | value |
|---|---|
| readable registry files (`$CLAUDE_CONFIG_DIR/sessions/<pid>.json`) | 21 |
| with non-empty `formerNames` | 1 |
| where the skill's regex disagrees with `json.load(...)["name"]` | 1 |

The mismatch set is **exactly** the `formerNames` set, which is what the mechanism predicts.

Worked case — pid `4124418`, profile `.claude`:

```
raw line, every occurrence in order:
  "name":"split-issue-clusters-file"          <- the real one, FIRST
  "name":"stop-storing-derived-counts"        <- inside formerNames, LAST

skill regex  -> stop-storing-derived-counts   (former; until=1788370948458)
json.load    -> split-issue-clusters-file     (current)
```

## Environment

`claude-plugins/codescout-companion/skills/reaching-peer-sessions/SKILL.md:31`.
21 live sessions across 3 profiles (`.claude`, `.claude-kat`, `.claude-sdd`).

## Root cause

Greedy `.*` with no anchor, over a namespace where the key is not unique. `sed` has no
non-greedy quantifier, so the expression as written cannot be repaired by a smaller edit to
itself.

## Evidence

The rate is **1 of 21 today and grows monotonically with renames.** A registry name is
re-minted by compaction, resume, or a restart under another profile, and `formerNames`
accumulates rather than rotating — so every rename permanently adds a decoy `"name":` key to
that session's line. A session renamed twice has two.

Note the interaction with CLAUDE.md § *Observer Blindness*, which already rules *"attribute by
sessionId, never by a self-reported name"* because a session quoting its own name is quoting a
belief. That rule treats the **registry** as the ground truth the self-report drifts from.
This bug is in the reader of that ground truth, so the prescribed remedy does not cover it:
consulting the registry through this instrument reproduces the same error by a different route.

## The corrupted input's evidence is destroyed on process exit

Raised 2026-09-02 by session `f13f8169-93a1-4392-95d1-8774d296e0c0` (cited by sessionId, not
name, for the reason this file is about), and measured here rather than taken on report.

The defect corrupts a value whose **ground truth dies with the process**. The two identifiers a
routing decision can be made from have *opposite* persistence:

| identifier | where it lives | survives process exit? |
|---|---|---|
| registry `name` | `$CLAUDE_CONFIG_DIR/sessions/<pid>.json` | **no** — file removed on exit |
| `sessionId` | a path component of `/tmp/claude-*/<project>/<session-id>/scratchpad`, and quoted in message text | **yes** |

Measured on this machine 2026-09-02: **90 scratchpad directories for this project alone, 81 of
them belonging to no live session** — so the sessionId's trace outlives its process by a wide
margin. Against that, pid `4052913`'s registry row is absent from every one of the three
profiles, and its socket is gone.

So a routing decision made from a **name** cannot be audited afterwards *even in principle*: the
only record of what the instrument displayed is deleted by the event that ends the session. A
decision made from a **sessionId** can be.

This is additive to the absorption described under *Symptom*, and distinct from it. Absorption is
about the **symptom** being consumed by correct advice for another cause; this is about the
**evidence** being unrecoverable. A defect can be loud and still unauditable, or silent and fully
auditable; this one is both silent and unauditable, and the second property is what makes the
rate unmeasurable retrospectively — the 1-in-21 figure above is a *current* census and can never
be computed over sessions that have already exited.

### The confirming observation, and what it does not establish

That session independently hit the absorption path from the other side: addressed a peer by the
name its own run of Step 1 displayed, received `No agent named 'X' is reachable`, followed the
skill's advice to switch to the `uds:` form, and succeeded — with nothing at any point
suggesting a name had been wrong. That is this file's *Symptom* section observed in the field by
a second party.

**It does not establish that the regex caused that particular misroute, and it is recorded here
as not establishing it.** The session states it held the target's sessionId and routed by name
anyway, contrary to `CLAUDE.md` § *Observer Blindness*; and the target's registry row is now
gone, so what the instrument displayed for it is unverifiable in the sense above — this section
is its own worked example. Two candidate causes, one operator error and one instrument defect,
and the discriminating record no longer exists. Attributing it to the regex would be the
convenient reading, not the supported one.
## Proposed fix

Parse the JSON:

```
n=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1])).get("name",""))' "$f")
```

**Do not** reach for `grep -o '"name":"[^"]*"' | head -1`. It happens to work today only
because the writer emits `name` before `formerNames`, and JSON key order is a serializer
artifact, not a guarantee — that is a positional fix with the same failure class one layer
down. Prefer a parser; if a shell-only form is required, `jq -r .name` is the honest one.

Every value the script reads from that JSON has the same exposure — `status` is extracted by
the identical greedy shape on the next line and is only accidentally safe because no nested
object in the current schema carries a `status` key. That is a fact about today's schema, not
a property of the expression.

## Status

Not fixed. Filed on notice during unrelated work; the skill remains usable via the `uds:`
addressing form, which does not depend on the name column.
