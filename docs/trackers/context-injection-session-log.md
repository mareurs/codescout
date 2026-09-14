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
entry_high_water_F: 2
entry_high_water_W: 2
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

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-09-14 | high | scout the SINK a proposed value must be accepted at, not only the subsystem under design | an ADR would have shipped on a mechanism the `deny_unknown_fields` librarian tools reject, and the one option buildable today would have been wrongly recorded as unavailable | validated |
| W-2 | 2026-09-14 | high | probe a would-be KEY in a universe containing two of whatever it keys | a single-field `agent_id` key would have passed every test and conflated two sessions' parents the first time two sessions shared a server | validated |

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

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     doc(action="append_entry", id="<artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...")

     The server allocates the id, writes `## F-N — <title>` at the ledger's
     own level, records the high-water mark and stamps `**Valid:** dated
     <today>` — one write. Then add the Index / Wins Index row with the id
     it returned. Do not hand-allocate; do not pre-write the row. -->
