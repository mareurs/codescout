---
id: d5acda1995c73ffe
kind: bug
status: investigating
title: Subagents are auto-injected guide topics their parent already holds — 84% of measured subagent sessions, ~2.84 MB for one topic, and the workspace-state guide says it cannot happen
tags:
- cluster/gate-keyed-on-unobservable-event
- guides
- subagents
- guide-ledger
- prompt-surface
- doc-vs-code
unverified: 'ROOT CAUSE NOT FOUND. The 84% double-delivery figure is solid (n=87, two machines, both orders ~50/50). What produces it is not: the 2026-08-31 investigation ELIMINATED four candidate mechanisms -- subagents having their own ledger file (0 of 40 are agent-*), spurious ledger clearing (13 topics survive a rebuild+reconnect in a live ledger), ordering (39 parent-first vs 34 subagent-first), and process overlap (real but only 2 of 7 multi-process conversations, which cannot produce 73 of 87). It also FALSIFIED persist()''s own doc-comment claim that two live writers per session id is ''unreachable'' -- measured reachable, with a concrete overlapping pair, and persist is a whole-map overwrite rather than read-modify-write, so the later writer drops the other''s topics. That is a separate defect in the same mechanism and needs its own fix and test. Remaining hypothesis, UNTESTED: a subagent''s tool calls reach a server process whose in-memory `emitted` was loaded before the parent''s injection was written. Post-fix sample is 5 of 5, far too small to conclude the companion snapshot/restore fix is ineffective -- do not read it as one.'
---

## Symptom

A subagent session receives an auto-injected `tracker-conventions` guide body when its
**parent session already received the same topic**. Measured 2026-08-31 across 87 subagent
transcripts on two machines:

| | n | parent also had it | rate |
|---|---|---|---|
| all subagent sessions | 87 | 73 | **84%** |
| Agent-tool subagents, session start < 2026-08-27 | 76 | 62 | 82% |
| Agent-tool subagents, session start >= 2026-08-27 | 5 | 5 | **100%** |
| Workflow-spawned agents (all pre-2026-08-27) | 6 | 6 | 100% |

At 38,870 B for this topic, the 73 double-deliveries are **~2.84 MB** — for one topic, in
one sampled corpus, over roughly 26 days.

## Why this is filed as a bug rather than a cost

`get_guide("workspace-state")` § *Subagent semantics* states the opposite outright:

> Subagents that share the parent's MCP server share: […] the same `guide_hints_emitted`
> set (**parent-triggered hints don't re-fire for subagents**)

So either the contract is stale or the mechanism is not doing what it documents. Both are
findings; they need different fixes, and nothing distinguishes them today.

## Reproduction / how it was measured

`scripts/probe_guide_section_use.py` builds the session list; the double-delivery join is:
a subagent transcript lives at `<project>/<parent-uuid>/subagents/[workflows/<wf>/]agent-*.jsonl`,
so the parent transcript is `<project>/<parent-uuid>.jsonl`. Check whether the parent file
also contains the **opening** marker
`auto-injected get_guide('tracker-conventions') — first call` — opening form only, per the
2026-08-27 parsing contract. Session dates come from each transcript's own first
`timestamp` field, never mtime.

## Investigation 2026-08-31 — four mechanisms eliminated, one documented assumption falsified

Runtime state first, source second, per the project's own rule.

**Eliminated — subagents do NOT get their own ledger.** `~/.local/state/codescout/guide_hints/`
holds **40 ledgers, 0 of them `agent-*`**; every filename is a session uuid. So a subagent
shares its parent's ledger file, exactly as `workspace-state` documents. This was the most
plausible explanation and it is wrong.

**Eliminated — the ledger is not being spuriously cleared, and it survives `/mcp`.** This
session's live ledger carries 13 topics with stamps from `11:32` still present at `12:58`,
across a rebuild and reconnect. Section-grain keys (`librarian#Filter Syntax`, ...) dedup
correctly alongside whole-topic keys. `project-activation-bootstrap` re-stamped at `12:57`
is the session-opening topic re-arming on reconnect — documented behaviour, not a leak.

**Eliminated — ordering.** The first injection is parent-first in **39** cases and
subagent-first in **34**, near 50/50. A shared ledger should permit neither, so this is not
an artifact of who ran first.

**FALSIFIED — `persist()`'s single-writer assumption.** The method's own doc comment says:

> This assumes a **single writer per session id**; two live processes racing on the same
> fixed `.tmp` name is a distinct, dismissed-as-**unreachable** case. [...] Deliberately NOT
> read-modify-write [...] **last writer wins**. Two live processes sharing one session id
> would need to write simultaneously for that to matter, and an MCP reconnect is
> kill-then-spawn, not overlap.

Measured against `usage.db`, which records `session_id` (per **process**) alongside
`cc_session_id` (per **conversation**):

| | |
|---|---|
| conversations mapping to >1 server process | **7** |
| …of those, with **overlapping** process lifetimes | **2** |
| max distinct processes for one conversation | **28** |

So the case dismissed as unreachable is **reachable**. Concretely, `cc_session_id`
`428b66b8…` has process `a9a52aa3` alive 2026-08-28 16:16 → 2026-08-29 07:18 and process
`057a79c6` alive 2026-08-29 06:36 → 06:37, wholly **inside** that window. Because `persist`
is a whole-map overwrite rather than read-modify-write, the later writer drops every topic
the other process added while it held its own snapshot.

**But rarity means this is NOT the driver of the 84%** — 2 of 7 cannot produce 73 of 87. It
is a real, separate defect in a documented-as-impossible case, found while looking for
something else. Recorded here rather than split out only because it lives in the same
mechanism; it deserves its own fix and its own test.
### The obvious next test does NOT work as written — do not repeat it

The remaining hypothesis is that a subagent's calls reach a server process whose in-memory
`emitted` predates the parent's injection. The natural test is to join a subagent's
injection instant against `usage.db` and read the `session_id` serving it. **It returns a
plausible answer and it is the wrong one.**

Querying `called_at BETWEEN t-120s AND t+120s` for a subagent injection returns an empty
process list, which reads as *"no server was serving this subagent"*. It is not: it means
**no calls landed in that window**. Cross-checked — subagent injection `2026-08-28T09:52:35Z`
is `12:52` local, and process `8911bc18` was demonstrably alive `11:52–14:18`. The process
existed and was idle.

The parent row makes it worse, not better: it *does* return processes, so it reads as a
passing positive control and lends the subagent's empty result false authority. Two further
traps in the same query — it is unscoped by `cc_session_id`, so it returns processes from
**other conversations** (`857a6727` is not this conversation's), and `.codescout/usage.db`
is **project-local**, so a subagent whose server had a different active project writes
nowhere this query can see.

A working version needs: scoping by `cc_session_id`, process **liveness intervals**
(`min/max(called_at)` per `session_id`) rather than point windows, and per-project usage
databases unioned. Also note only **2** of the sampled subagents are codescout-project
sessions under `.claude`, so even a correct query is underpowered on this corpus.
## What is NOT established

- **n = 5 post-fix.** The 100% post-2026-08-27 rate is 5 of 5. It is consistent with no
  improvement, and it is far too small to conclude the fix is ineffective. **Do not read
  it as one.** This is the whole reason the file is `investigating` rather than `open`
  with a diagnosis attached.
- **The mechanism is unverified.** If a subagent genuinely shares the parent's MCP server
  process, it shares the in-memory `guide_hints_emitted` set and re-injection should be
  impossible — which is what the guide asserts. That it happens anyway suggests either a
  separate server process per subagent, or a re-arm on some path. **Not investigated.**
  Reading the source is the next step, and per the project's own rule a claim about tool
  behaviour needs the call run and the real output read, not the source alone.

## Related, and why this is the OTHER direction

`docs/issues/archive/2026-08-26-subagent-guide-fetch-starves-parent.md` (fixed) is the
mirror image: a subagent's fetch marked a topic delivered for the whole session, starving
the **parent**. Its fix shipped 2026-08-27 client-side in the companion plugin
(`codescout-companion:d47dea4`, patch-id `c30242187d28052a672baeadbfd26048861f4fdd`):
`agent-guide-snapshot.mjs` (PreToolUse, matcher `Agent`) copies the ledger file before a
dispatch and `agent-guide-restore.mjs` restores it after, undoing the subagent's writes
while preserving the parent's.

That fix protects the parent. It does not claim to stop the subagent receiving anything,
and this file is not a regression report against it. **Its own text names Workflow-spawned
agents as uncovered** — a different dispatch mechanism, not the `Agent` matcher — and the
table above finds those at 100%, consistent with that exclusion.

**The constraint that shapes any fix** is recorded in the same file and in
`docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` § *Out of scope*:

> no MCP client, Claude Code included, sends any per-request caller identity today, so the
> server cannot tell a subagent's tool call apart from its parent's

So a server-side "don't inject into subagents" rule is **not implementable in codescout**.
Any remedy is client-side, in the companion plugin, where the `Agent` PreToolUse /
PostToolUse hooks already exist and already manipulate this exact ledger file.

## Why it matters beyond the bytes

Measured the same day (`docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md`
§ *The DISTRIBUTION probe ran*), subagents engage **7.5%** of the `tracker-conventions`
bytes they receive against main sessions' ~55%, and 38 of 87 engage none of it. So this is
not merely repeated delivery — it is repeated delivery into the population least likely to
use it.
