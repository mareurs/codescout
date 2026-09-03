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
unverified: 'ROOT CAUSE NOT FOUND. The 84% double-delivery figure is solid (n=87, two machines, both orders ~50/50), but the 2026-08-31 measurement never checked whether the RECIPIENT context actually already had the content -- only whether the parent transcript did. For non-fork subagents (zero inherited context, the harness default), that is the wrong question: re-delivery to an empty context is not redundant. 2026-09-04 live-tested that an Agent-tool subagent shares the parent''s already-running codescout process (no new process spawns), which refutes the session''s own ''fresh process reads stale disk ledger'' guess for that dispatch path, but is now in tension with the 84% figure itself (shared in-memory state should suppress deterministically). Untested: whether Workflow-spawned agents (100% in the original sample) go through a genuinely separate process, which would let the stale-disk-ledger mechanism still explain that population specifically. Also untested from 2026-09-04: a live redelivery probe from a fresh, unsaturated session (this session''s ledger was already fully saturated, making its probe uninformative on the redelivery question itself). Concrete unexploited lever for a client-side fix: `SubagentStart`''s hook payload already carries `agent_type` (fork vs fresh) and `agent-guide-snapshot.mjs` currently ignores it.'
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
## Investigation 2026-09-04 — the premise itself was under-examined, plus one mechanism ruled out

**The redundancy premise needs a caveat the 2026-08-31 measurement never applied.** The
double-delivery rate was computed purely from "did the PARENT transcript also contain this
topic's opening marker" — it never asked whether the RECIPIENT's own context already had
the content. That distinction is not academic: of the two ways a subagent is dispatched in
this harness, only one (`subagent_type: "fork"`) inherits the parent's conversation
context. Every other type starts with **zero** context — the parent having received a
guide is irrelevant to whether that fresh context has, because it categorically does not.
For those, re-delivery is not redundant; it is the only way the guide ever reaches that
context. Suppressing it — which is literally what `workspace-state` documents as the
intended behavior (*"parent-triggered hints don't re-fire for subagents"*) — would be the
actively harmful direction for a fresh dispatch: silent withholding, not wasted bytes. This
reframes the fix target: not "stop the bytes", but "stop conflating fork (correctly
suppressible) with fresh (must not be suppressed) when the server cannot tell them apart
at all" — which the 2026-08-31 investigation's own "constraint that shapes any fix" section
already named as the reason no server-side fix is possible, without drawing this
consequence from it.

**A concrete, already-available lever for a client-side fix.** Per
`claude-plugins/.codescout/memories/agent-dispatch-hooks.md` (measured 2026-08-27,
independent of this investigation): `PreToolUse:Agent` receives `tool_input.subagent_type`
directly, and — more usefully, since that event fires before the subagent exists and closes
before it runs a single tool call (measured async gap: 2-8s) — **`SubagentStart` already
receives `agent_type` in its own payload**, the same lifecycle event
`agent-guide-snapshot.mjs` already hooks. The current hook reads `input.cwd`,
`input.session_id` and `agent_id` from that exact payload and ignores `agent_type`
entirely. No new plumbing is needed to distinguish fork from fresh at the one point in the
lifecycle that already brackets this exact ledger.

**Mechanism check, live-tested 2026-09-04: an `Agent`-tool subagent does NOT spawn a new
codescout process.** Dispatched two probe subagents (`subagent_type: "general-purpose"`)
from this session — one doing nothing, one calling `workspace(action="status")` once — and
polled `ps -eo pid,lstart,cmd | grep 'codescout start'` every second across each dispatch's
full lifecycle (launch through completion). No new `codescout start` process appeared in
either case; the subagent's tool call landed on the SAME already-running process as the
parent. This REFUTES the "fresh process loads a stale on-disk ledger" hypothesis this
session initially proposed — there is no fresh process, so no stale-disk-read moment exists,
at least for this dispatch path.

**But that finding is now in tension with the 84% figure, not a resolution of it.** A truly
shared process sharing one in-memory `Mutex<GuideLedger>` should suppress a same-topic
re-delivery deterministically — predicting ~0% redelivery for topics the parent already
triggered, not 84%. The live probe above could not actually test this: this session's own
ledger was already saturated (every relevant topic had fired from the parent's own prior
calls before either probe ran), so "the subagent got nothing new" is uninformative — it is
consistent with correct shared-state suppression AND with there being no live redelivery
bug on this path at all. **A real test needs a fresh, unsaturated session**: trigger a
topic as the parent, immediately dispatch a non-fork subagent, and check whether IT also
receives that topic (with the ordering matching the 2026-08-31 measurement's parent-first
cases).

**Candidate reconciliation, UNTESTED:** the 2026-08-31 data separates "Agent-tool
subagents" (82-100%, small post-fix n) from "Workflow-spawned agents" (100%, n=6, all
pre-2026-08-27) as distinct populations. If Workflow-spawned agents run through a genuinely
separate process invocation (the `workflow` skill's own dispatch mechanism, not the `Agent`
tool this session's probe used), the original "fresh process, stale disk ledger" hypothesis
may still be correct for THAT population specifically, while `Agent`-tool subagents —
confirmed process-sharing tonight — need a different explanation for their share of the
84%. Not established either way; next session should check `workflow`'s own dispatch code
for whether it spawns a codescout subprocess per agent.

**Status released to `investigating`** (not concluded) — both the fork/fresh reframing and
the process-sharing finding are real and load-bearing for whoever picks this up next, but
neither closes the bug, and no fix was attempted.

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
