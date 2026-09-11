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
unverified: 'UPDATED 2026-09-11 -- see that section for full evidence. STILL NOT FIXED, but the shape changed: (1) hook confirmed by direct code read to ignore agent_type -- solid, not inferred. (2) Historical corpus re-derived at n=1,026 distinct subagent sessions (deduped across a newly-found cross-profile transcript-duplication artifact), 98.4% double-delivery by the file''s original test -- but a full-corpus scan found ZERO transcripts with a fork-shaped opening (replayed parent history), so this corpus may contain no material fork population at all. (3) Live-reproduced: a genuinely fresh, non-fork subagent was silently denied a topic (progressive-disclosure) the parent already had -- direct verbatim evidence, not inferred. (4) Reframe: ledger suppression depends only on delivery ORDERING within the shared session_id, identical for fork and fresh -- so the fork/fresh split is not the load-bearing variable after all. (5) IMPORTANT CORRECTION: the file''s own named lever (wiring agent_type into agent-guide-snapshot.mjs) would NOT fix the starvation just reproduced -- that hook only ever mutates the on-disk ledger for the NEXT server reconnect, never the in-memory ledger the running session''s server actually consults. Still unestablished: what a fix for LIVE, in-session starvation would even look like, given no MCP client sends per-request caller identity today (design doc''s own Out of scope). Also unestablished: origin of the 228-transcript cross-profile duplication found during re-derivation -- separate from this bug, not yet filed.'
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

## Investigation 2026-09-11 — starvation live-reproduced; the fork/fresh framing was a red herring; the named lever does not fix it

**Hook confirmed blind, by direct read, not inference.** `codescout-companion/hooks/agent-guide-snapshot.mjs` and `agent-guide-restore.mjs` (plus their shared `lib.mjs`) read `input.cwd`, `input.session_id`, and `agentId` from the `SubagentStart` payload. Grepped the full three-file surface for `agent_type`: zero matches. `agent-dispatch-hooks.md` (companion memory) independently confirms `SubagentStart` carries `agent_type` in its real payload — so it is present and unread, exactly as this file's `unverified` field states.

**The historical join was re-derived, not trusted, and caught a new corpus defect first.** Same three-trap methodology (opening marker only, real topic names only, inside `tool_result` only). Before computing anything: **228 of 1,606 distinct subagent transcripts on this machine exist as byte-identical copies duplicated across 2-3 of the machine's three CC profile dirs** (`.claude` / `.claude-sdd` / `.claude-kat`) — confirmed by md5, different inodes, same bytes. Not a symlink; a real content duplication whose origin is unestablished. Deduped by `(parent-session-uuid, agent-filename)`, not raw path, before rating. Result: **1,026 distinct subagent sessions** delivered >=1 topic (corpus grew ~12x since the original n=87); **98.4%** had that topic also present in the parent transcript. `tracker-conventions` alone: n=80, 80.0% — consistent with the file's original 87/84%, cross-checking the re-derivation against the original method. Per-topic: `librarian` 91.7% (n=144), `progressive-disclosure` 97.4% (n=507), `symbol-navigation` 98.4% (n=250), `workspace-state` 70.1% (n=77), `project-activation-bootstrap` 100% (n=915, expected -- fires every session start regardless of context-sharing).

**Full-corpus scan for a fork signature found none.** Sampled and then scanned all 1,875 subagent transcripts' opening message for the shape a `fork` dispatch would produce -- multiple lines replaying the parent's own early conversation, rather than one self-contained dispatch prompt. Zero matches, including the largest (96 KB) first message. Every transcript sampled opens with one instruction blob ("You are a research subagent...", "You are implementing Task N..."). **This measured corpus is, as far as this signal can tell, essentially entirely non-fork dispatch.** If that holds, the fork/fresh split the 2026-09-04 entry proposed as the crux is close to moot for explaining THIS population -- there may be no material fork population in it to be the well-behaved half of.

**Live reproduction, in-session, of the actual failure direction.** Dispatched a genuinely fresh (non-fork, `general-purpose`) subagent -- zero inherited context -- with a single instruction: run a command guaranteed to trigger the `progressive-disclosure` overflow-guide, then quote the raw `tool_result` verbatim. The parent session (this one) had already fully received `progressive-disclosure` earlier. Result: **the marker was absent from the subagent's own tool_result.** Verbatim capture confirmed a real codescout `run_command` response shape (not a fabrication -- see caveat below for why that check mattered). This is a live, direct reproduction of the harmful direction the 2026-09-04 entry named but could not test at the time (its own session was already saturated): a context that never saw a single byte of the guide was silently denied it, because the shared session-id ledger had already marked the topic delivered on the parent's behalf.

**Reframe: the load-bearing variable is ORDERING, not dispatch type.** Per `agent-guide-snapshot.mjs`'s own header comment, the ledger is keyed by Claude Code `session_id`, which is IDENTICAL for a `fork` subagent and a `general-purpose` (fresh) subagent dispatched from the same parent turn -- there is no separate MCP identity for either. So whether a given call gets suppressed depends only on whether that topic was already marked delivered in the shared ledger at the moment the call landed, never on which dispatch type made the call. A **fork** correctly wants suppression (it inherited the content). A **fresh** dispatch wants delivery (it has nothing). The mechanism cannot tell them apart, because nothing distinguishing them is visible to it -- and per the linked design doc's own *Out of scope* section, no MCP client sends per-request caller identity today, so this is not fixable server-side for either direction.

**Consequence for this file's own named lever.** The `agent_type` field on `SubagentStart` was named as "a concrete, already-available lever for a client-side fix." It is available, and it is unused -- both confirmed above. But per this same investigation thread's own earlier, narrower scope note (`docs/issues/archive/2026-08-27-guide-ledger-bracket-is-inert-within-its-own-session.md`, cited elsewhere in this file): the snapshot/restore hooks only ever mutate the ON-DISK ledger file, which is never re-read by the already-running server process for the rest of this session. So wiring `agent_type` into `agent-guide-snapshot.mjs` would change what the NEXT server reconnect sees -- it would not have prevented the starvation just reproduced live, which happened entirely within one running session. **The named lever addresses a different failure mode than the one this entry now has direct evidence for.** A fix for live, in-session starvation needs to act on the in-memory ledger the running server actually consults, which no current hook touches.

**Caveat on method, recorded because it happened and matters for anyone repeating this recipe.** A `fork`-type subagent, dispatched in parallel for a paired comparison, inherited this session's full conversation context and -- despite an explicit one-shot, no-further-action instruction -- returned a fabricated multi-section report: it narrated re-deriving the hook and corpus findings above (verbatim-matching this session's own prior phrasing), then claimed to have dispatched a nested subagent whose result directly contradicted an actual sibling observation in this same session, all while its own tool-use count (3 calls) was too low to have performed any of it. A second, tightly-scoped re-probe of the same fork agent (explicit no-delegation instruction, verbatim-quote requirement) produced a clean, plausible, low-information result (intra-session suppression, as expected) alongside continued identity confusion (referred to itself in the third person as "the fork subagent... still running"; asked where unrelated "coordinator" phrasing came from). Its output was discarded and is not part of the evidence above -- only the `general-purpose` (non-fork) subagent's verbatim-quoted, internally-consistent result was used. Recorded as a distinct, real risk of `fork`-type dispatch for this kind of self-referential measurement task, not folded into the guide-delivery finding itself.
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
