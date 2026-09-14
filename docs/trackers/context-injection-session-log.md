---
kind: tracker
status: active
title: "Session Log — Context Injection & Principal Identity"
owners: []
tags:
  - engines
  - guide-ledger
  - subagents
  - session-log
topic: context-injection
entry_prefix:
  - F
  - W
entry_high_water_F: 8
entry_high_water_W: 4
---

# Session Log — Context Injection & Principal Identity

## Purpose

Frictions and wins for the work stream that spans `src/engines/` (the retrieval-engine
registry and coordinator), `GuideLedger`, and the question of **which principal** a
delivery is for. Opened 2026-09-14 out of the architecture review of automatic context
injection; the `_meta` conversation-id reader (`40fb2843`, patch-id
`345c535aa5b549455e34e125800523262961484f`) is this stream's first shipped change.

**Declared `entry_prefix` deliberately** — see `context-injection-session-log:F-1` for the
measurement that settled it, which `docs/TAXONOMY.md`'s open warning asks each new log's
author to make.

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-09-14 | low | plan-prose | open | TAXONOMY's `entry_prefix` tradeoff: one horn is false, the other is not a benefit |
| F-2 | 2026-09-14 | high | plan-prose | fixed-verified | A code comment's scope qualifier was narrower than its phrasing, and it killed a viable design for two turns |
| F-3 | 2026-09-14 | high | architectural | fixed-verified | Adopting a principal moves starvation to the parent's return leg, so the ledger map must precede the hook |
| F-4 | 2026-09-14 | high | release-pipeline | superseded | The hook deploys instantly and the server does not, so shipping it now breaks every unreconnected session |
| F-5 | 2026-09-14 | high | plan-prose | open | The deny_unknown_fields hazard does not exist at the tool surface — 42 occurrences read as 42 gates |
| F-6 | 2026-09-14 | high | architectural | open | The end-to-end win was the shipped guide_rearm path, not this feature — Arm A was confounded |
| F-7 | 2026-09-14 | high | tooling | mitigated | A mutation that never applied is indistinguishable from a surviving mutant |
| F-8 | 2026-09-14 | med | tooling | mitigated | A positive control validates the instrument, never the query |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-09-14 | high | scout the SINK a proposed value must be accepted at, not only the subsystem under design | an ADR would have shipped on a mechanism the `deny_unknown_fields` librarian tools reject, and the one option buildable today would have been wrongly recorded as unavailable | validated |
| W-2 | 2026-09-14 | high | probe a would-be KEY in a universe containing two of whatever it keys | a single-field `agent_id` key would have passed every test and conflated two sessions' parents the first time two sessions shared a server | validated |
| W-3 | 2026-09-14 | high | when the obvious benefit is already provided, measure the property the incumbent cannot have by construction | the ADR's justification would have stayed an argument after F-6 demoted it, with the real benefit unmeasured and unclaimable | validated |
| W-4 | 2026-09-14 | high | ask what proposition a confirming result proves before copying the thing that produced it | `permissionDecision:'allow'` would have shipped on a matcher covering every codescout tool, auto-approving every subagent `run_command` and `edit_code` | validated |

---

## Promotion status

**Audited:** <YYYY-MM-DD>, against the target surface itself — opened and read,
not recalled.

One line per `W-N` (and any `F-N` with a `Fix idea` bound for a permanent
surface). Check the **target**, not the entry: a `Promote-when` that fired is
invisible from inside the tracker, because `Status: validated` reads as healthy
either way. Record one of:

- **already promoted, no action** — quote the promoted text verbatim and name
  where it landed, so the next reader verifies instead of re-deriving.
- **UNFIRED, carried forward** — restate the criterion and the current datapoint
  count.
- **FIRED but not yet applied** — the one that leaks. Name the exact target
  surface and the exact text to add. This is an action item, not a note; set the
  entry's `Status:` to `promotion-due` so a query can find it.

> ⚠️ **Name every instance of the target, not the target's type.** This machine
> runs three Claude Code profiles (`~/.claude`, `~/.claude-sdd`,
> `~/.claude-kat`), each with its own `CLAUDE.md`. An audit that concluded
> *"not found in the user's global CLAUDE.md"* — singular — led to a promotion
> that reached one file of three on 2026-08-18. The session that found the gap
> was running on a profile **without** the rule, and applied it only because
> another profile's copy happened to be injected as project instructions. Three
> files that should be byte-identical have an md5; compare them.

> ⚠️ **For an INSTALLED artifact the target is the SERVING copy — not the repo
> source, and not the other copies.** Measured 2026-08-20: three rules promoted
> into a plugin skill were byte-identical across all three profile caches *and*
> stale against source, because the commit never bumped the version the cache is
> keyed on. Comparing the copies to each other reads **green** there — only
> comparing each copy to the claim catches it. And the session that made the edit
> is the **least representative observer**: its own reload resolved the skill from
> the repo source, so the confirming evidence sitting in front of it was evidence
> about the wrong artifact.

> ⚠️ **Anchor on a back-citation, not a verbatim quote.** A quote goes red when the
> promoted rule is legitimately reworded — a false positive produced by the
> promotion working as intended, observed 2026-08-20 when `codescout:R-89`'s bullet was
> rewritten and the tracker's stored quote had to be edited to match. The durable
> form is the promoted text citing its own entry id —
> *"(codescout:R-1 + codescout:R-7 in codescout's `docs/trackers/reconnaissance-patterns.md`.)"* — so
> verification is a `grep` for the id and survives every rewording. Keep the quote
> as a reading aid; do not make it the predicate.

Run this when the work stream wraps, **and** whenever a criterion fires
mid-stream — an audit that only happens at archive time is one that happens
after the lesson was needed. Prior art:
`eduplanner-ui:docs/trackers/archive/calendar-insight-panel-session-log-2026-08-18.md`, whose
audit correctly caught its own `calendar-insight-panel-session-log-2026-08-18:W-4` as fired-and-unapplied and named the exact
text to promote.

## Category conventions

Use a short kebab-case category to group similar frictions. Prior
sessions have used:

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

Add a new category by writing it as a kebab-case string; no central registry needed.

---

## F-N entry template

Pass this block as `append_entry`'s `body` (without the `## F-N — <title>`
line — the server writes the heading from `title`). Add the matching Index
row afterwards, using the id the call returned. Do not allocate the id
yourself; see *How to use* above.

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates>

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Pass this block as `append_entry`'s `body`, with `id_prefix="W"` — F-N and
W-N have separate counters. A win without a **Counterfactual** is marketing
— name what would have happened without the pattern, with at least one
piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

**Promoted-to:** <surface + section, one per line, line-start — omit until it lands>

**Status:** validated | promotion-due | promoted-to-permanent-docs | archived

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates>

---
```

---

## Status vocabulary

Codified so the Index column means the same thing across sessions.

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. (`fixed` alone is too weak — verification is part of the status.) |
| `promoted-to-bug-tracker` | Moved to a formal tracker (`docs/issues/*`, `docs/TODO-*`, GitHub issue). The session log keeps the pointer; the formal tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. Do NOT close — its job is to remain comparable. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. Default for entries with evidence. |
| `promotion-due` | `Promote-when` has **fired** and the text is not yet on the target surface. An action item, not a resting state. Exists because `validated` cannot distinguish "criterion not yet met" from "criterion met, nobody harvested it" — and both read as healthy, which is how a lesson sits unpromoted while the failure it describes recurs. |
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. Session log keeps the pointer — and, for a multi-instance target, names every instance it landed in. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — TAXONOMY's `entry_prefix` tradeoff: one horn is false, the other is not a benefit

**Observed:** 2026-09-14, creating this log. `docs/TAXONOMY.md` carries a standing
warning asking each new session-log author to *"decide it deliberately before adding
more"* whether to declare `entry_prefix: [F, W]`, and states the tradeoff as: *"declaring
F/W makes them dangling-checked but gives one token many definers; leaving them undeclared
keeps ambiguity."*

**When:** About to create the 20th session log — i.e. standing exactly on the trigger the
warning names. (The warning also records that it said *"One session log"* until 2026-09-13
while thirteen more were added, so it is read only by someone already editing TAXONOMY.)

**Expected (TAXONOMY):** a real tradeoff — declaring buys dangling-checks and costs
definer-multiplicity; declining costs ambiguity and saves definers.

**Got (read from the production path):** in `DefinitionIndex::build`
(`src/librarian/tools/link_scan/resolve.rs`), `active_definers` is populated **only**
inside `for def in &ex.definitions` — the `## F-N — <title>` sections. The separate
declaration loop writes to `known_prefixes` and `declared_by` and never touches
`active_definers`. So:

- **Horn 1 is false as written.** Declaring adds **zero** definers. Definers arrive when
  entries do. Corroborating count at this instant: the 5 session logs that declare nothing
  hold **87** `## F/W-N` sections between them (`release-promotion-session-log.md` alone
  defines 51), and every one is already a definer.
- **Horn 2 is not a benefit either.** Declining does not reduce ambiguity: the ~400
  ambiguous citations the resolver's own doc comment cites (49 of 50 sampled being F/W)
  come from 19 logs each defining `F-1`. Declaring changes none of that.

What declaring *does* do is add a **declarer**, which is what makes `prefix_conflicts`
report the prefix — it fires on `declared_by` ∩ (`active_definers` ≥ 2). But F and W are
already reported: the same doc comment records this, measured 2026-09-02 at six declaring
logs, and 14 declare today. A 15th changes the conflict's membership list, not its
existence.

**Probable cause:** the warning was written when the corpus matched the resolver's older
doc comment (*"Eight session logs define `F-N` and none declares `entry_prefix`"*). Both
the corpus and the guidance moved — `get_guide("tracker-conventions")` now instructs every
ledger author to declare — and the tradeoff sentence was not re-derived against either.

**Workaround / decision taken:** declared `entry_prefix: [F, W]` on this log, deliberately.
At this corpus state the choice is ambiguity-neutral and conflict-neutral, so the only live
term is the write guard, and that argues for declaring. Recorded here rather than in
TAXONOMY because changing that warning is a corpus-policy call over 19 files, not this
log's to make.

**Severity:** low — no tool call fails and nothing reds. Not `med` for exactly that reason.
The cost is a wrong decision: an author weighing horn 1 declines to declare, leaving a
ledger unguarded, to avoid a definer cost that declining does not avoid.

**Status:** open — TAXONOMY's text is unchanged; only this file's own decision is settled.

**Valid:** dated 2026-09-14

**Rests on:** reading `DefinitionIndex::build` and `prefix_conflicts` in
`src/librarian/tools/link_scan/resolve.rs`, not on an observed `link_scan` report — I did
not run one. If the builder's population of `active_definers` ever draws on declarations,
horn 1 becomes true and this entry is void.

**Fix idea / Pointer:** the warning asks for a decision it cannot detect being made. The
mechanism it is owed (§ *Observer Blindness* position 3, which the warning names itself) is
a check keyed on the `entry_prefix` declaration at write time. Until then, the cheap repair
is to correct the tradeoff sentence so the next author weighs the term that is actually
live — the write guard — rather than two that are not.

## W-1 — Scouting the sink, not just the subsystem, inverted two of three design recommendations

**Observed:** 2026-09-14, architecture review of automatic context injection — asked to
design how a subagent (thin context, shares its parent's `CLAUDE_CODE_SESSION_ID`) should
be served guidance. Read-only design work, no dispatch and no edit pending, which is
exactly when scouting feels skippable.

**Pattern:** Before recommending a mechanism, scout the **two ends** of it — the place the
data would come from, and the place it would have to be accepted. Reading only the
subsystem being redesigned (`src/engines/`) answers "where does this plug in?" and leaves
"can the value physically get there?" unasked, which is the half an architecture
recommendation is graded on.

Three shapes surfaced that no amount of reading `src/engines/` would have produced:

1. **`GuideRearmInbox::poll(&self) -> Vec<String>`** (`src/tools/guide_rearm.rs`). Its
   files are named per `(server_pid, agent_id)` — deliberately, to defeat a race the
   sibling snapshot mechanism already lost — and the return type drops the `agent_id`. The
   identity the whole design needs exists one layer down and is discarded at a signature.
2. **`deny_unknown_fields` at 42 sites across 17 files**, concentrated in the librarian
   tools. Any scheme that smuggles a principal id through tool `arguments` is rejected by
   precisely the tools the design most needs, unless stripped at the edge first.
3. **A PreToolUse hook CAN rewrite tool input here** — `explore-inject.mjs:135` uses
   `hookSpecificOutput.updatedInput` in production, in the companion plugin. Observed
   capability, not a documentation claim.

**Counterfactual:** Two of the three recommendations I was about to make were wrong, in
opposite directions, and neither would have failed loudly.

- Without (1)+(2): the natural proposal is *"restore `agent_id` in `poll`, key a ledger per
  principal, and have the companion inject the id into tool arguments."* That reads as
  correct. It fails twice: the re-arm is a **broadcast** with no per-call binding, so under
  concurrent subagent dispatch — which this project's own instructions encourage — no stack
  or last-writer scheme can be right, not merely mistuned; and the injection is refused by
  the `deny_unknown_fields` tools, surfacing as a deserialization error *inside a librarian
  tool*, i.e. attributed to the tool rather than to the design that caused it.
- Without (3), the error runs the other way: I would have recorded input-rewriting as
  unavailable and dropped the one option that is buildable today without an upstream
  dependency. A wrongly-closed option leaves no artifact at all — nobody re-opens a door
  they were told was locked.

Concretely: an ADR would have been written, reviewed and agreed, and the first librarian
tool call under it would have failed. Cost is not a retry — it is a design decision taken
on a false premise and then defended.

**Confirming data points:**
1. This session — three shapes, two of which inverted a recommendation before it was
   written.
2. Pending: any future design pass where the *source* and the *sink* of a proposed value
   live in different subsystems.

**Impact:** high — the class is architectural recommendations, not edits. `codescout:R-19`
already says asserting a checkable fact is not Q&A; this extends it to the mechanism a
recommendation *rests on*, which is checkable the same way and is not read as an assertion
because it is phrased as a proposal.

**Promote-when:** a second design pass where scouting the sink (not the subsystem under
design) falsifies a recommendation. At 2 datapoints, promote to the reconnaissance skill's
*When to Use* as: *"Before recommending a mechanism, scout where the value must be
ACCEPTED, not only where it is produced."*

**Status:** validated — single datapoint, both inversions caught before the review was
delivered.

**Valid:** dated 2026-09-14

**Rests on:** the three readings above being current at this instant on a shared checkout.
(1) and (3) are signatures and a live call site and are stable; (2) is a count and decays —
re-derive it rather than citing 42.

## F-2 — A code comment's scope qualifier was narrower than its phrasing, and it killed a viable design for two turns

**Observed:** 2026-09-14, designing per-subagent guide delivery. I concluded *"the path is
closed — no hook, wire or plugin mechanism can attribute a call to a subagent"* and
recommended dropping the hook-injection option entirely. That conclusion was wrong, and it
stood for two turns until measured.

**When:** Deciding whether the companion could supply a principal id, having just verified
that `PreToolUse` can rewrite tool input (`explore-inject.mjs` uses
`hookSpecificOutput.updatedInput` in production).

**Expected (from the source):** `hooks/lib.mjs` in `codescout-companion` states:

> *"Tool-lifecycle payloads (PreToolUse/PostToolUse with matcher Agent) carry tool_use_id
> and NO agent_id, so a hook wired to the wrong event has nothing to key on."*

I read that as a general claim about `PreToolUse`, concluded the only event that can carry
data into a call has no identity to carry, and declared the option dead.

**Got (measured):** a subagent's `PreToolUse` payload carries **`agent_id` AND
`agent_type`**. The parent's carries neither — absent, not empty. Measured over 10 grep
calls across 4 subagents, 1 parent and 1 peer session, with each call self-labelled via its
grep pattern as an independent control.

**Probable cause:** the sentence is **true and narrower than it reads**. *"with matcher
Agent"* scopes it to the hook firing on the **Agent tool call itself** — issued by the
parent, at dispatch, when no agent exists yet. Correct. It says nothing about `PreToolUse`
on a tool called *from inside* an already-running subagent, which is the case the design
needed. The qualifier is a parenthetical in the middle of a sentence whose subject is
"Tool-lifecycle payloads", so the general reading is the one a reader arrives at.

What makes it expensive rather than merely wrong: the comment is unusually
*well-earned* — it sits on a structural gate (`agentIdOrComplain`, `MISWIRED_MARKER`)
built after a week of silent mis-wiring, and cites its own archived bug file. Its
credibility is exactly why I did not probe behind it. A weaker comment would have been
tested.

**Workaround:** none needed — the probe settled it. Design reinstated, and the withdrawn
option is now the recommended one.

**Severity:** high — no tool call failed and nothing red. The cost was an architectural
recommendation to *stop pursuing the only workable mechanism*, plus a fully-specified
alternative design (bracket-scoped ledger with an in-flight set and a concurrency detector)
invented solely to route around a field that was already on every call. Had it shipped, it
would have been substantial machinery solving a non-problem.

**Status:** fixed-verified — reversed by direct measurement before anything was built.

**Valid:** dated 2026-09-14

**Rests on:** the probe capture in this session's scratchpad
(`pretooluse-payloads.jsonl`, 13 rows) and the quoted comment as it reads today. If the
comment is later re-scoped, this entry records why it needed to be.

**Fix idea / Pointer:** the repair is in `codescout-companion`, not here — move the scope
qualifier to the front of the claim, e.g. *"PreToolUse on the Agent tool itself carries no
agent_id (the agent does not exist yet); PreToolUse on a tool called from INSIDE a running
subagent does carry it."* Two sentences instead of a parenthetical. Cross-repo, so filed
here as friction rather than as a codescout bug file.

## W-2 — An accidental peer capture was the control that stopped a single-field key from shipping

**Observed:** 2026-09-14, measuring whether `PreToolUse` payloads can identify a subagent.
I registered the probe hook in `.claude/settings.local.json` — **project**-scoped rather
than profile-scoped — because it is gitignored and does not reach other profiles. That
choice also meant peer sessions in this shared checkout were captured. I noted that at the
time as acceptable noise to be filtered out by `session_id`.

**Pattern:** When probing an identifier that will become a **key**, run the probe in a
universe where a second principal of the same kind can appear. A single-session capture
cannot distinguish *"this field identifies a principal"* from *"this field identifies a
principal within a scope I happened to hold constant."*

**Counterfactual — concrete, because it nearly happened.** Each probe call self-labelled
via its grep pattern, so I could group by `agent_id` and check no label family mixed. Four
subagent groups came out clean. The parent group printed **`MIXED -> agent_id
unreliable`** — because two rows had no `agent_id`: mine, and one from a peer session
running a grep I never issued.

The naive reading of that verdict is *"`agent_id` is not trustworthy."* The true reading is
that **absent `agent_id` means *the parent of some session*, not *the parent*** — two
sessions' parents are indistinguishable under an `agent_id`-only key. Regrouped on
`(session_id, agent_id)`, all six groups pass.

Without the peer's call in the capture, every group would have been clean, the control
would have printed OK, and I would have derived a **single-field key** — correct in every
test I ran and broken the first time two sessions shared a server, which is the normal
state of this checkout. The failure would have surfaced as cross-session guide suppression:
one session's parent marking topics delivered for another's.

The corpus already knew: `agentGuideSnapshotFile(sessionId, agentId)` and
`guideRearmFile(dir, pid, agentId)` in the companion's `lib.mjs` are both composite. I would
have shipped a worse key than the one already in use two directories away.

**Confirming data points:**
1. This session — 10 captured calls; the composite key resolves 6 principals cleanly, the
   single-field key conflates 2.
2. Pending: any future probe of a value destined to become a map key.

**Impact:** high — the defect class is a key that passes every test because the probe held
its discriminating dimension constant. Nothing downstream would have failed loudly.

**Promote-when:** a second probe where an accidental second principal changes the derived
key shape. At 2 datapoints, promote to the reconnaissance skill as: *"When the thing you
are measuring will become a key, the probe must contain at least two of whatever it keys
— otherwise a constant reads as an identifier."*

**Status:** validated — single datapoint, caught before any code was written.

**Valid:** dated 2026-09-14

**Rests on:** the peer capture being a genuine second session rather than an artifact of my
own dispatch — confirmed by `session_id`, which differs from this session's in the raw
capture. Note honestly that this win was **accidental in origin**: I chose project scope
for blast-radius reasons, not to obtain a second principal. The lesson is the one to keep;
the foresight is not mine to claim.

## F-3 — Adopting a principal moves starvation to the parent's return leg, so the ledger map must precede the hook

**Observed:** 2026-09-14, immediately after committing `5e51e72f` (the per-call principal
reader). Found by reading `GuideLedger::rekey` while planning the next slice — not by a
failing test, and no test in the tree can currently fail on it.

**When:** Deciding what to build next. The obvious answer was "the companion hook, so the
feature becomes live". That answer is wrong, and this is why.

**Expected:** that adopting a principal fixes the subagent-starvation class for the
principals involved.

**Got:** it fixes the subagent and moves the defect to the parent's **return leg**.
`rekey` does `emitted.clear()` — the re-arm is total, deliberately. Trace
parent → subagent → parent:

| step | stamp | effect |
|---|---|---|
| parent call | none | ledger keyed to the session; parent's guides accumulate |
| subagent call | `sess/agent-a` | key differs → `rekey` → `emitted` cleared → subagent served fresh ✓ |
| parent call | none | `adopt_request_conversation(None)` returns early — **no re-key back** |

So the parent's later calls are served from the **subagent's** ledger. The parent's own
delivered-set was cleared at step 2 and is never restored, so every topic the subagent
consumed now reads as delivered for the parent. That is **starvation — the unsafe
direction** — and it is the original defect in mirror image, not its removal.

**Probable cause:** one `GuideLedger` per process. Adoption re-arms; it does not restore.
`adopt_request_conversation`'s own doc comment says exactly this and names the remedy as
"a conversation→ledger map … a separate decision" — but that was written while the tier
was **inert**, so the limitation was unreachable and read as theoretical.

**Workaround / the constraint this produces:** none needed today, and that is the whole
point — nothing stamps `dev.codescout.mcp/agentId` yet, so the path is unreachable. It
becomes reachable the instant a companion hook ships. **Therefore the conversation→ledger
map must land BEFORE the hook, not after.** Building the hook next is the obvious move and
would ship a parent-starvation path.

**Severity:** high — not for what is broken today (nothing is), but because the natural
next step arms it. A one-commit ordering choice between two pieces of work that look
independent and are not.

**Status:** fixed-verified — the parked-ledger map landed in this entry's own commit,
before the hook existed, so the path was never reachable in a shipped state.
`adopt_request_conversation` now treats `None` as "restore the parent" rather than
"leave the ledger alone", and parks the outgoing principal's ledger for restoration.
Mutation-checked: reverting `None` to its old meaning kills
`a_parent_call_after_a_subagent_restores_the_parents_own_ledger` at the assertion it
exists for, and leaves 52 sibling tests green.

**Valid:** dated 2026-09-14

**Rests on:** two bodies read this session — `GuideLedger::rekey` (`emitted.clear()`) and
`adopt_request_conversation` (`let session = asserted?`, so `None` returns before any
re-key). If either changes so that an unstamped call re-keys back to the base session,
this entry is void.

**Fix idea / Pointer:** `HashMap<key, GuideLedger>` replacing the single ledger, so
returning to a principal restores its delivered-set instead of re-arming it. Note the
inverse hazard when designing it: a map that never evicts grows per subagent for the life
of the process, and the safe direction on eviction is to re-deliver, never to suppress.

## F-4 — The hook deploys instantly and the server does not, so shipping it now breaks every unreconnected session

**Observed:** 2026-09-14, about to start the companion hook (the ADR's remaining half)
immediately after `b43e3702` landed the parked-ledger map. Stopped before writing it.

**When:** The server-side work is complete and gate-green, so the hook looks like the
obvious next commit. It is not, and the reason is not in either repo's code.

**Expected:** that with the map in place, the hook could ship whenever.

**Got:** the hook and the server are deployed by **different mechanisms with different
latencies**, and the hook's is instantaneous. Measured earlier this session: editing
hook config takes effect on the *next tool call*, with no restart — that is how the
`PreToolUse` probe worked at all. The server is the opposite: the live MCP process is
whatever binary existed when each session connected, and picking up new server code
requires `cargo rb` **plus** `/mcp` reconnect, **per session**.

So shipping the hook first stamps `dev.codescout.mcp/agentId` into the arguments of
every codescout call in every live session — against server processes that have no
`principal_from_arguments` and therefore never strip it. The key reaches the
deserializer, and `#[serde(deny_unknown_fields)]` at 42 sites (concentrated in the
librarian tools) **refuses the call** rather than ignoring the field. Every `doc`,
`librarian`, `append_entry` call in every unreconnected session would start failing.

**Probable cause:** the two halves of one feature live behind deployment surfaces whose
lag differs by roughly "instant" versus "manual, per-session". Nothing in either repo
expresses the dependency, and the natural build order — server first, then hook — is
also the order that arms it, because the server being *written* is not the server being
*run*.

**Workaround / the required sequence:**
1. `cargo rb`, then `/mcp` in **every live session on this checkout** (6 at last count).
2. Verify the running server strips the key — one stamped call to a
   `deny_unknown_fields` tool that returns normally.
3. Only then add the hook.

**Severity:** high — a cross-session outage affecting every peer on the checkout, from a
commit that is green in its own repo and touches no Rust. The blast radius is other
people's sessions, which is the part no local gate can see.

**Status:** superseded by `context-injection-session-log:F-5` — **the central premise
below is false and was measured so within the hour.** An unknown top-level key is
silently IGNORED at the live tool surface, not refused: `doc(action="find", …)` returns
normally and `doc(action="event_create", …)` reaches the database. The `doc` dispatcher
structurally cannot carry `deny_unknown_fields` — trying it once broke every
`doc(update)` call — and `event_create::Args` receives a fresh map rather than the
top-level blob.

So there is **no cross-session outage**, and the severity below is wrong. What survives
is the much smaller true claim: the two halves still deploy at different latencies, so a
hook shipped before the rebuild would stamp a key that unreconnected servers ignore —
wasted bytes and a silently inert feature until each session reconnects, not broken
calls. Left standing rather than rewritten because the entry is the evidence for F-5,
and an entry quietly corrected to look right teaches nothing.

**Valid:** conditional — closes when every live session runs a server carrying
`principal_from_arguments`, verified rather than assumed

**Rests on:** `deny_unknown_fields` refusing rather than ignoring an unknown key on the
librarian tool inputs — read from the derive sites, **not** executed against a live
old-binary server. That is the one link in this chain I inferred rather than measured,
and it is cheap to settle: one stamped call before the rebuild.

**Fix idea / Pointer:** this is `context-injection-session-log:F-3`'s shape a second
time — two pieces of work that look independent, where the build order is the hazard.
F-3's constraint was internal to one repo and a test now guards it. This one crosses a
repo boundary and no test can reach it, which is what makes it worth writing down rather
than remembering.

## F-5 — The deny_unknown_fields hazard does not exist at the tool surface — 42 occurrences read as 42 gates

**Observed:** 2026-09-14, writing the test F-4 said would settle its one
inferred-not-measured link. The test's **control** failed, which is the finding: the
premise was false, and three documents asserted it.

**When:** Immediately after `b43e3702`. F-4 had named the weak link honestly — *"read
from the derive sites, not executed"* — and the first attempt to execute it inverted it.

**Expected:** that an injected top-level key reaching a librarian tool would be REFUSED,
because `deny_unknown_fields` appears at 42 sites concentrated there. That claim is in
`PRINCIPAL_ARG_KEY`'s doc comment, in `call_tool_inner`'s comment, in
`docs/adrs/2026-09-14-a-subagent-is-a-principal.md` (*"the strip point is forced"*), and
is F-4's whole hazard.

**Got:** it is silently ignored. Two measurements through `call_tool_inner`:

- `doc(action="find", kind="tracker", totallyUnknownField="x")` → returns normally,
  `count: 0`.
- `doc(action="event_create", …, totallyUnknownField="x")` → reaches the **database**
  (`FOREIGN KEY constraint failed` on a bogus id). Deserialization never objected.

**Probable cause — and it was already written down where I did not look.**
`event_create.rs`'s own doc comment says `deny_unknown_fields` *"is safe HERE while
being unavailable on the shared `doc` schema"*: the dispatcher passes the shared
argument blob straight down, so `action` and every sibling action's key would arrive as
unknown fields. `param_probe`'s module doc records the attribute being tried once and
**breaking every `doc(update)` call**. And `event_create::Args` receives a fresh map
from `flatten_event_args`, not the top-level blob, so a stray top-level key never
reaches the strict type at all.

So my 42 was a count of **occurrences** read as a count of **tool-input gates**. They
are config structs and nested arg types. This is `CLAUDE.md` § *Testing Discipline*'s
*"a count of a defect population must arrive with its unit or not at all"* — I had the
number and not the unit, and the number was doing load-bearing work in an ADR.

**Workaround / corrections made:**

- The test is **deleted**, not weakened. Its premise was the control; without a live
  tool-input struct that rejects unknown top-level keys, the control cannot exist and
  the remainder would assert only that a stamped call behaves like an unstamped one —
  satisfied equally by stripping and by ignoring, which is the monotone trap.
- `PRINCIPAL_ARG_KEY` and `call_tool_inner` now state the strip as **hygiene and defence
  in depth**: an internal routing key has no business in a tool's input, and a tool that
  tightens its schema later should not turn the companion into a breaking change.
- `a_malformed_principal_is_still_stripped_so_it_cannot_refuse_a_tool_call` renamed —
  its tail asserted the falsified claim.
- The direct removal assertion in
  `a_principal_token_is_taken_and_removed_from_the_arguments` is unaffected and remains
  the real coverage: it checks the key is gone from the `Value`, which is observable and
  true regardless of what any deserializer would have done.

**Severity:** high — nothing broke, and that is the point. A false premise had reached an
ADR, two code comments and a tracker entry, and it would have been quoted forward by
every reader of any of them. F-4's hazard is materially smaller than filed.

**Status:** open — F-4 and the ADR still carry the overstated form; corrected in code
only so far.

**Valid:** dated 2026-09-14

**Rests on:** two live calls through `call_tool_inner` on this build, plus
`event_create.rs`'s and `param_probe`'s own doc comments. It does **not** establish that
NO tool anywhere rejects an unknown top-level key — only that the two `doc` actions
tested do not, and that `doc`'s dispatcher structurally cannot.

**Fix idea / Pointer:** correct F-4's severity and the ADR's *"the strip point is
forced"* paragraph. The strip point is still right — `call_tool_inner` is the only site
that sees every call — but it is forced by *"one site, every call"*, not by a refusal
that does not happen.

## F-6 — The end-to-end win was the shipped guide_rearm path, not this feature — Arm A was confounded

**Observed:** 2026-09-14, first end-to-end run of the principal feature against a live
rebuilt server, with a disposable prototype of the companion hook stamping
`dev.codescout.mcp/agentId`.

**When:** Immediately after `cargo rb` + `/mcp`. The parent had just consumed
`project-activation-bootstrap` on the fresh server process — precisely the precondition
under which a shared ledger starves a subagent.

**Expected:** Arm A (hook ON) — the subagent receives the guide, because it is adopted as
a new principal and the ledger re-arms. That is what happened.

**Got:** Arm B (hook OFF, otherwise identical, same server process) — **the subagent
received the guide anyway.** So Arm A's delivery is not attributable to the stamp, and
Arm A measures nothing about this feature.

**Probable cause — a shipped mechanism I had read and then argued past.** The companion's
`SubagentStart` hook writes a re-arm request; `poll_guide_rearm()` drains it on the next
request, one line below `adopt_request_conversation` in `call_tool_inner`. It has been in
the tree since August, and its own doc comment cites
`docs/issues/archive/2026-08-31-subagents-receive-guides-their-parent-already-holds.md`
— **archive**, i.e. already fixed. I quoted that same bug as live motivation in the ADR.

**What this does and does not overturn.**

- It does **not** show the feature is wrong or the class imaginary. `guide_rearm` carries
  no identity — it is a broadcast *reset*, never a restore — so it cannot distinguish
  concurrent subagents and cannot return a principal to its own state. True before this
  measurement, still true.
- It **does** overturn the claimed benefit. Subagent guide-starvation is already
  mitigated in practice, so the remaining value is **precision**, not delivery — and I
  have not measured precision. The honest position is that the user-visible improvement
  is *unquantified*, not *demonstrated*.
- A live question I have **not** measured: `poll_guide_rearm` runs immediately AFTER
  adoption, so a stamped subagent call adopts its own ledger and then has that ledger
  re-armed by the inbox. Benign on this evidence, unexamined in general.

**Workaround:** none — nothing is broken. The prototype hook is removed, the tree is
unchanged, and no committed behaviour depends on Arm A.

**Severity:** high — not for a defect, but because an ADR and several commits rest on a
motivation stated more strongly than the evidence supports, and I would have shipped the
hook citing Arm A as proof.

**Status:** mitigated — the replacement experiment this entry called for has been run, at
`context-injection-session-log:W-3`. Measured: one subagent dispatch costs the parent **2**
re-deliveries without the stamp (a full `tracker-conventions` body, then `librarian § Filter
Syntax`) and **0** with it, stable over two dispatches, while the subagent continues to
receive its own guides. So the precision benefit this entry called unquantified is now
quantified. What remains open is the concurrency half — two subagents in flight without
cross-suppression — which is predicted by the same mechanism and still unmeasured.

**Valid:** dated 2026-09-14

**Rests on:** two subagent dispatches differing only in whether the stamp hook was
installed, on one server process, with the parent having consumed the topic beforehand.
The instrument was separately controlled: three synthetic payloads piped through the hook
showed it stamps for an agent in this session and emits **nothing** for the parent or for
another session's id — so a silently-dead hook would not have been mistaken for Arm B.

**Fix idea / Pointer:** the replacement experiment measures PRECISION, not delivery —
whether a parent is re-delivered topics after a subagent runs, and whether two concurrent
subagents can be served without cross-suppression. `guide_rearm` can do neither by
construction, so that is where a difference must show if one exists. Until it is run, the
ADR should say the class is archived-but-recurring and the benefit is precision, rather
than implying subagents are starved today.

## W-3 — Measuring the property the incumbent cannot have: parent over-delivery 2 to 0, subagent delivery intact

**Observed:** 2026-09-14, the replacement experiment `context-injection-session-log:F-6`
called for. F-6 established that subagent *delivery* is already handled by the shipped
`guide_rearm` path and that this feature's remaining value was **precision**, unmeasured.
This measures it.

**Pattern:** When a mechanism's obvious benefit turns out to be already provided by
something else, do not defend the mechanism — find the property the incumbent **cannot
have by construction**, and measure that instead. `guide_rearm` is a broadcast *reset*
carrying no identity, so *restoring a principal to its own prior state* is the one thing
it can never do. That is where the difference had to be if one existed.

**Design that made it discriminate.** The observable had to be a topic the PARENT holds
that the SUBAGENT will not re-trigger — otherwise the subagent re-consumes it, re-marks
it, and both arms look identical. Subagents were therefore restricted to a single `grep`
and explicitly forbidden from touching `doc`, while the parent's observable was a
librarian guide section keyed to one exact `doc(find)` call shape.

**Result.** Identical parent call, before and after one subagent dispatch, from a
verified-silent baseline:

| arm | stamp hook | parent re-deliveries |
|---|---|---|
| 1 | off | **2** — the entire `tracker-conventions` body, then `librarian § Filter Syntax` |
| 2 | on | **0**, and again **0** after a second dispatch |

And the other half, which is what stops this being a trade: under the stamp the subagent
**still received** `project-activation-bootstrap`. Parent over-delivery removed, subagent
delivery preserved.

**Counterfactual:** without the park/restore map, every subagent dispatch wipes the
parent's delivered-set and the parent re-pays it one guide section per call — a full
guide body was ~50 KB in the arm measured, and it took two calls to drain a single
dispatch. On a session that fans out to several subagents, that is the parent's context
window being re-filled with text it already holds, repeatedly, invisibly. `guide_rearm`
cannot avoid it: it has no identity to scope the reset to.

**Confirming data points:**
1. This session — 2 → 0 re-deliveries, stable across two dispatches, subagent delivery
   intact.
2. **The concurrency half, measured 2026-09-14 against the SHIPPED hook** (claude-plugins
   `de7d16a`, released as 1.20.9 and registered via `/reload-plugins`) rather than the
   disposable prototype. Same call shape, same protocol: three identical parent
   `doc(find)` calls with the 2nd and 3rd byte-identical and silent, then **two subagents
   dispatched concurrently in one message**, then the identical call again.
   **0 re-deliveries**, and 0 again on a repeat. Both subagents independently received
   `project-activation-bootstrap` (~2.4 KB and ~2.6 KB), so neither suppressed the other.

   **A positive control was necessary and is what makes that 0 a measurement.** A frozen
   or broken ledger returns silence too, and would have read as a clean result. So the
   same turn issued `doc(action="event_list")`, a surface not yet touched this session:
   `librarian § doc — Event Log` arrived fresh. The parent's ledger was demonstrably still
   *capable* of delivering while declining to re-deliver.

   **What the subagent side does NOT show, and the distinction is F-6's whole lesson.**
   Both probes receiving their own guides is equally what `guide_rearm` produces — it
   re-arms per `(pid, agent_id)` on SubagentStart, so two subagents get two fresh ledgers
   with or without this feature. That observable is a consistency check, not evidence.
   **Only the parent's return leg discriminates**, because restoring a principal to its
   own prior state is the thing a broadcast reset cannot do by construction.

**Impact:** high — it converts the ADR's justification from an argument into a
measurement, after F-6 had correctly demoted the previous one.

3. **Re-derived after a rebuild, and it CONFIRMED** — published because a confirmation is
   a denominator (`CLAUDE.md` § *Testing Discipline*: instrument the doubt, and when a
   re-derivation confirms, say so). Datapoints 1 and 2 were taken against a server process
   that no longer exists; the operator then rebuilt and `/mcp`-reconnected onto a HEAD that
   had moved well past mine, including `a13b31c6`, which touched `src/server.rs` by +114
   lines. Checked rather than assumed: all three of my commits are still ancestors of HEAD,
   that commit's diff mentions **zero** of `adopt_request_conversation` / `parked_ledgers`
   / `principal_from_arguments` / `base_ledger_key`, the strip is still wired at
   `src/server.rs:1259`, and the binary postdates HEAD. One dispatch against the rebuilt
   stack: **0 parent re-deliveries**, subagent received its own
   `project-activation-bootstrap`.

   Incidental confirmation of a documented claim nothing here set out to test: **the ledger
   survived the `/mcp` reconnect**. The repeated parent call came back silent with no
   re-drain needed, and only the session-opening topic re-armed — which is exactly what
   the keyed tier promises in `get_guide("workspace-state")` § *Per-session state reset*.

**Promote-when:** — **met 2026-09-14.** Both halves are measured against the shipped
hook, so the ADR's Consequences may state the benefit in observed terms. What remains
unmeasured is the *shape of the curve*: 1 dispatch and 2 concurrent dispatches both cost
the parent 0, and no run has probed where, if anywhere, that stops holding.

**Status:** validated — single work-stream datapoint, both directions checked.

**Valid:** dated 2026-09-14

**Rests on:** for datapoint 1, a disposable prototype hook scoped to one session id, whose
own behaviour was controlled separately (stamps for an agent in this session; emits
nothing for the parent or another session). One honest gap in arm 1: my own `sleep` ran
concurrently with the subagent, so I cannot say whether the subagent's call or mine
drained the re-arm request. It does not affect the outcome — either drain clears the
shared ledger, which is what arm 1 observed — but the attribution is unestablished and the
run should not be cited as showing *which* call drains.

For datapoint 2, the shipped hook, and **no codescout call was made by the parent between
dispatch and measurement** — the specific confound arm 1 carried. Its control arm is
**historical, not simultaneous**: the "stamp off → 2 re-deliveries" figure comes from arm 1
earlier the same session, under the same call shape and baseline protocol but with one
dispatch rather than two. A within-session A/B would need the hook unregistered and the
plugin reloaded mid-measurement; it was judged not worth disturbing a live deployment,
and that is a judgement rather than a finding.

## F-7 — a mutation that never applied is indistinguishable from a surviving mutant

**Valid:** dated 2026-09-14

**Severity:** high
**Status:** mitigated

**Observed:** Mutation-testing `principal-stamp.mjs`, five `sed` mutations were
applied to the production file, each expected to red a specific assertion. M1 —
inserting `permissionDecision: 'allow'`, the one mutation guarding a permission
escalation — reported **21/21 green**. Read at face value that is a surviving
mutant: the assertion is blind and the guard is decorative.

It was nothing of the kind. The `sed` pattern matched six leading spaces; the
hook indents that line with four. **The mutation never applied.** The suite was
green because the production file was unchanged, and a no-op mutation and a
surviving mutant produce byte-identical output: `Total: N. Pass: N. Fail: 0.`

Re-run with the correct indentation and a `diff` printed before the test, M1
kills `no-permission-decision` cleanly (`expected=null got=allow`).

**Cost if uncaught:** the conclusion available from the green run was "the
absence assertion does not discriminate". The two repairs that invites are
rewriting an assertion that was already correct, or recording in a commit
message that the escalation guard is unverified. Both are worse than no mutation
run, because each carries the authority of having measured.

**Why the existing law does not cover it.** `CLAUDE.md` § *Testing Discipline*
says *demand an observed RED, never an assertion's existence*, and *mutate the
PRODUCTION path, not the test's inputs*. Both were obeyed here. The unstated
third step is that **the mutation's arrival is itself unverified** — a `sed` that
matches nothing exits 0 and prints nothing, so the instrument reports success for
having done nothing. Same shape as the laws it sits beside: the refuting outcome
leaves no artifact.

**Mitigation, cheap and structural:** print `diff <orig> <mutated>` between the
edit and the test run. A no-op mutation shows an empty diff; a real one shows the
hunk. One line, and it converts an invisible failure into a visible one. Used for
the M1 re-run, and M2-M5 carry the same check in the other direction via
`file identical to pre-mutation` after restore.

**Rests on:** `sed` exiting 0 on zero matches — true of GNU sed and POSIX. Any
find-and-replace mutation tool has the same property.

**A SURVIVING MUTANT HAS THREE READINGS, NOT TWO** — noticed 2026-09-14 on reading
`1371ba87`, committed by sessionId `f3c594ce-c424-40d3-a603-9693cfef3f63` shortly after
this entry and arrived at independently. That record publishes two readings of "no test
killed it": **untested** (write the test) and **unreachable by any test that could be
written** (change production code for a seam — no amount of test-writing fixes it). Both
presuppose the mutant EXISTED. F-7 is the third: **the mutation never applied**, so the
green came from a world containing no mutant at all.

The three take three different repairs, which is why collapsing them is expensive:

| reading | what the green means | repair |
|---|---|---|
| untested | mutant ran, no assertion covers it | write the test |
| unreachable | mutant ran, no writable test can drive it | give production code a seam |
| **never applied** | **no mutant ran** | re-run with a diff between edit and test |

Their entry's own warning applies recursively here: reading a survivor as "untested" sends
you to write a test that cannot exist, and the failure of that attempt reads as your own
incompetence rather than a missing seam. Reading a NO-OP as a survivor sends you to repair
an assertion that was already correct — and that failure reads as the guard being
undefeatable, which is the most convincing wrong conclusion of the three.

Check the cheap branch first: a diff costs one line and eliminates the third reading
outright, leaving their two-way decision intact and correct.

**Not claimed:** that this is one promotable class with `1371ba87` and `F-143`
(`39fd478a`, "a red that clears cannot name its own cause"). All three share a shape — two
causes emitting one observable, with a natural reading that picks the wrong one — but that
shape is broad enough to fit much of § *Testing Discipline* already, and a class claimed
from three same-day entries is the kind of tally this repo has been wrong about before.
Recorded as an adjacency to check, not a cluster.

## W-4 — reading the binary stopped a context optimisation shipping as a permission bypass

**Valid:** dated 2026-09-14

**Status:** validated

**Observed:** The disposable prototype that produced the W-3 precision
measurement emitted `permissionDecision: 'allow'` alongside `updatedInput`. The
permanent hook was about to inherit it by copying, and a sibling hook
(`explore-inject.mjs`) sets the same pair — so both precedent and working
evidence pointed at keeping it.

Asking *what proposition does that evidence prove* stopped the copy. The
prototype's success showed **the stamp landed**. It could not distinguish that
from "the stamp landed *and* I disabled the permission prompt", because that
session's permission mode made the two worlds produce identical output. A
confirming result from an instrument that cannot express the failure is not
evidence about the failure.

**Resolved against the shipped binary, not the docs.** `docs.claude.com` lists
the field but never states whether `updatedInput` applies independently, and two
fetches truncated before the `#pretooluse` and `#decision-control` sections.
Grepping Claude Code 2.1.270 settles it three ways:

- the schema declares `permissionDecision` / `permissionDecisionReason` /
  `updatedInput` / `additionalContext` as four **independent** `.optional()`s;
- both composition paths attach `updatedInput` without consulting the decision —
  `O = deny?{…}:ask?{…}:allow?{allow:!0}:{}; if(d) O.updatedInput = d;` and the
  same shape in `hxo()`;
- the precedence map `{deny:3, ask:2, allow:1, none:0}` makes "no decision" a
  real state one rank **below** allow.

In-repo corroboration from an independent source: `lib.mjs::contextPreToolUse`
already emits a PreToolUse `hookSpecificOutput` with no `permissionDecision`, and
the call proceeds.

**Counterfactual — what copying would have cost.** The prototype was scoped to
one session id and matched whatever it was pointed at. The permanent hook matches
`mcp__codescout__.*`: **every** codescout tool, `run_command` and `edit_code`
included. Carrying `allow` there would have auto-approved every tool call any
subagent makes, and outranked every hook that correctly stays silent — the docs
are explicit that silence does not approve while `allow` does. A context-window
optimisation would have shipped as a permission bypass, in a plugin that is
always active in this checkout and installed across three profiles.

Nothing in the test suite as first drafted would have caught it either: the stamp
lands correctly in both worlds, so every predicate assertion stays green. Only
asserting the **absence** discriminates, which is why
`no-permission-decision` / `no-permission-reason` exist and are mutation-verified
(F-7 records how that verification nearly lied).

**Rests on:** Claude Code 2.1.270's hook dispatcher. The field is undocumented in
this respect, so a future version could couple them; the test reds if the
emitted shape ever changes, which is the part that does not decay.

## F-8 — a positive control validates the instrument, never the query

**Valid:** dated 2026-09-14

**Severity:** med
**Status:** mitigated

**Observed:** Before renaming a bug file, I checked what cited it:

```
git grep -l "2026-09-14-pre-edit-dirty-check-claims-this-session-did-not-write-its-own-rename" HEAD
  -> 0 hits
git grep -lc "issue-clusters.md" HEAD          # positive control
  -> 122 hits
```

and reported the zero as conclusive, explicitly citing the control as what made it a
measurement rather than a broken grep. `doc(action="move")` then returned
`inbound_path_citations: ["docs/trackers/issue-clusters/IC-2-..."]` — a citation that was
present in the working tree **and** in HEAD the whole time.

**Cause:** IC-2 names its members by **bare slug**, without the date prefix:
`` `pre-edit-dirty-check-claims-this-session-did-not-write-its-own-rename` ``. I searched
for the full filename. The pattern and the corpus's citation form never overlapped, so the
zero was correct about the string I typed and silent about the question I asked.

**The control was real, and structurally could not catch this.** A positive control
establishes that the **instrument** works — `git grep` reads HEAD, matches, returns hits.
It says nothing about whether the **query** matches how the corpus actually writes the
thing being counted, because the control uses a *different* pattern that happens to be
well-formed. Both halves of the reasoning were sound and the pair still had a hole:

| what was verified | what it covers | what it misses |
|---|---|---|
| control returns 122 | the tool runs, the tree is readable, the method is sound | whether *my* pattern is the form the corpus uses |
| target returns 0 | that exact string is absent | that the same referent appears under another form |

So *"a suspicious zero needs a control"* is necessary and not sufficient. The missing
question is **what does a citation of this thing actually look like** — answered by reading
one known citation, never by adding a control.

**What caught it:** `doc(action="move")`'s `inbound_path_citations`, which **enumerates**
rather than matching a pattern I composed. The standing lesson: when a tool already answers
"who references this", prefer it over a grep — not because grep is unreliable, but because
the tool's pattern is derived from the corpus and mine is derived from my belief about the
corpus. This is IC-6's shape from the query side: I addressed the file by one of its names
and the corpus indexes it under another.

**Cost if uncaught:** the rename would have orphaned IC-2's only pointer to this member,
leaving a cluster row naming a file that no longer exists — and `audit_doc_refs` rates an
unresolvable backticked path `high`, so it would have reddened CI for the *next* session,
reading as their breakage.

**Rests on:** the `move` response's `inbound_path_citations` field, which exists precisely
because path citations decay across moves. Nothing here generalises to corpora with no such
enumerator; there, reading one real citation first is the only available check.

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     doc(action="append_entry", id="<artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...")

     The server allocates the id, writes `## F-N — <title>` at the ledger's
     own level, records the high-water mark and stamps `**Valid:** dated
     <today>` — one write. Then add the Index / Wins Index row with the id
     it returned. Do not hand-allocate; do not pre-write the row. -->
