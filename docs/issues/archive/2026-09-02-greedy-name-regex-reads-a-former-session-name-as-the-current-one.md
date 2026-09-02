---
id: 8dbb3ff3cb9a89f7
kind: bug
status: fixed
title: the peer-enumeration regex reads a former session name as the current one
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-09-03
unverified: 'The regression test covers the name read only. The self-identification half of the same commit is verified by hand, not by test — see the sibling bug file. The test is newly reachable: `tests/run-all.sh` globbed hooks only until 2026-09-03, so a skill-colocated test was discovered by nothing; the glob was widened in the same commit and has not yet run in CI, which invokes two named targets rather than the runner.'
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

**How the live count was derived, because the obvious cross-check is not independent.** Session
`f13f8169-93a1-4392-95d1-8774d296e0c0` flagged that a live-session count taken from
`~/.claude*/sessions/` shares a substrate with the socket enumeration and so corroborates only
partially, by this repo's rule that *two instruments returning the same number is evidence only if
their scopes differ*. Correct, and it applies to the first derivation of the 81 above. A
registry-free instrument does exist and was run: liveness from `/proc/<pid>`, project attribution
from `readlink /proc/<pid>/cwd`, population from `/tmp` — **no `sessions/*.json` read on that
path**. It returns 9 live for this project, agreeing with the registry-derived 9.

**The residual, which neither instrument closes:** both start from
`/run/user/<uid>/cc-socks/*.sock`, so a live session with no bound socket is invisible to both and
would be miscounted as an orphan. That biases the orphan figure **upward**, i.e. in the direction
that flatters the claim — so read **81 as an upper bound**, not a measurement. The claim does not
need it: it needs only that scratchpads greatly outnumber live sessions, and 90 directories
against at most 21 live sessions machine-wide holds under any correction the residual could
produce.

**The slack in that upper bound has since been measured: 7.** Enumerating structurally by
`readlink /proc/<pid>/exe` rather than by process name gives 28 live claude binaries, 21 of them
socket-bound, so **7** live sessions carry no socket machine-wide. At most 7 of the 81 can
therefore be live-but-socketless, putting the true orphan count at **≥ 74** — the claim clears
the residual with room to spare.

That number was itself wrong the first two times it was taken, and the reason is a second defect
in the same script: a peer derived it as **2** via `pgrep -x claude`, and the `comm` filter misses
every session whose binary is version-pinned. Filed separately as
`docs/issues/archive/2026-09-02-comm-filter-misses-version-pinned-claude-processes.md`
(`cluster/selector-narrower-than-its-population`). Noted here because it is the same lesson this
file is about arriving one layer up: **a value read by pattern-matching a name the schema never
promised**. There it was `"name":` matched by position in a line; here it is `claude` matched
against `comm`. Both return a plausible number rather than an error, and the second one
undercounted the blast radius of the first.

The same reasoning is why this section reports a derivation rather than a value. A reader who
re-runs it later gets a different 90 and a different 9, and should.

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
## A second site, which this file did not name

This file located the defect at `reaching-peer-sessions/SKILL.md` Step 1 and stopped there. There
was a **second** occurrence of the identical greedy expression, in a `reconnaissance-patterns.md`
R-N entry's sessionId-lookup snippet, found by grepping the tree while fixing the first — not by
reading this file, which named one site and read as complete.

That is `bug-fix-session-log:W-102` holding about the bug file that helped produce it: *checking
the named site confirms the named site.* A bug file is a claim about a population of occurrences,
and naming one is the same shape as a ledger naming one stale citation when three existed.

The second site is corrected in place, with the broken original deliberately preserved and marked
*do not copy* — the entry records a method that was actually run and a conclusion drawn from it,
so rewriting it would falsify the record rather than repair it. Worth reading for its own reason:
its author had validated the script **on their own sessionId as a control**, and the control
passed because that session was neither renamed nor version-pinned — a control drawn from the
population both defects exclude.
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

**Fixed** at `claude-plugins:bb14719`, patch-id `87019883ed5b6a85ae30999f6cc3381522fc73dc`
(cross-repo: the fix is in `claude-plugins`, not this repo, so the SHA is prefixed and the
patch-id is what survives that repo's own history rewrites).

The shipped line is now a structural read — `json.load(...)["name"]` — which cannot be reached by
the decoy key at all, rather than a regex taught to avoid it.

**Verified 2026-09-03, discriminatingly.** Against the same live registry file that reproduced it:
the old greedy form still returns `stop-storing-derived-counts` and the shipped form returns
`split-issue-clusters-file`. The reproduction has not gone stale, so the check still separates a
fixed world from a broken one rather than merely agreeing with both.

**Regression test:** `codescout-companion/skills/reaching-peer-sessions/reaching-peer-sessions.test.sh`,
four cases, and it **extracts the command from `SKILL.md` rather than re-typing it** — a copy would
pass forever against a skill edited back to the greedy form. Two mutations of the production path
were run and both were killed: changing the line's *shape* reds the extraction guard, and keeping
the shape while making the *program* return `formerNames[-1]["name"]` reds case 1 with
`got 'idle|stop-storing-derived-counts'` — the defect's own signature. Case 2 asserts the fixture
still reproduces the bug, so case 1 cannot quietly become vacuous.

See `unverified:` for what the test does **not** cover and for the reachability caveat.
