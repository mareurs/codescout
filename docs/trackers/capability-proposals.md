---
id: '01291679a5ee4707'
kind: tracker
status: active
title: Capability Proposals — codescout (CAP-N)
owners:
- marius
tags:
- capability-proposal
- design
- reflective
- backlog
topic: capability-proposals
---

## Why this exists

A **pre-plan** home for proposed codescout capabilities: an idea with enough grounding to be
judged, but not yet a spec or a plan. `docs/TAXONOMY.md` had no slot for this — a feature idea
was told to go to "`docs/trackers/` or `docs/plans/`" with no named destination, so ideas either
got wedged into a dated landscape doc (`mcp-integration-ideas-2026-04.md`, scoped to an April
read of the Claude Code source) or lived only in a conversation. This is the named destination.

**What belongs here:** a capability codescout does not have, with the substrate check already
done — what exists today, what is actually missing, and what the first decision is.
**What does not:** bugs (`docs/issues/`), tool frictions (U-N), hook designs (H-N), or anything
already scoped enough to be a spec (`docs/superpowers/specs/`) or plan.

**Graduation.** A CAP entry leaves by becoming a spec + plan, or by being marked `rejected` with
the reason kept. It is not a wishlist: an entry with no substrate check is not ready to be here.

## Index

| ID | Opened | Status | Size | Title |
|----|--------|--------|------|-------|
| CAP-1 | 2026-08-15 | proposed | small–medium | Session artifact-touch ledger — expose what this session read and wrote |
| CAP-2 | 2026-08-15 | brainstorm | large | Second arm — a codescout-issued LLM controller that challenges unverified claims |

## CAP-1 — Session artifact-touch ledger

**Ask.** Within a session, record every tracker/artifact the agent touched, classify each as
read-only or read-write, keep a lightweight diff and some statistics, and expose a method that
inspects it. The driving use case is **compaction prep**: at the moment context is about to be
lost, answer "what did I touch, what did I only read, and what did I change?" without
reconstructing it from memory — the least reliable source available at exactly that moment.

**Substrate check — most of this already exists.** `src/usage/db.rs` writes a `tool_calls` row
per MCP call (`write_record`, `src/usage/db.rs:107-151`) carrying `tool_name`, `called_at`,
`latency_ms`, `outcome`, `overflowed`, `err_family`, `project_root`, codescout + project SHAs,
and — decisively — both `session_id` (the MCP server's) and `cc_session_id`, read from
`.codescout/cc_session_id` (`src/usage/mod.rs:99-104`). Retention is 30 days. So per-session
correlation is already there and needs no new plumbing.

**The one real gap.** The row that would identify *which artifact* was touched is `input_json`,
and it is written only under `--debug`:

    let input_json = if self.debug { serde_json::to_string(input).ok() } else { None };
    — src/usage/mod.rs:85-89

In an ordinary session `input_json` and `output_json` are NULL, so the target is unrecoverable.
Turning `--debug` on globally is the wrong fix: it persists the full input **and output** of every
call, which is both large and a disclosure surface (outputs contain file contents).

**Therefore the shape is narrow.** Add an always-on extracted `touch_target` (artifact id or
rel_path) plus an `access` class, rather than storing whole payloads. There is precedent in the
same function: `friction_target` is already an extracted-not-dumped field
(`extract_friction_target`), computed conditionally and stored as a short string. This proposal
is that pattern applied unconditionally to a different field.

**RO/RW classification is derivable, not guesswork.** The tool plus its `action` decides it:
`artifact(find|get|graph|state_at)`, `read_markdown`, `read_file`, `grep`, `symbols` are RO;
`artifact(create|update|move|delete|append_entry|graft|link)`, `artifact_augment`,
`artifact_event(create)`, `edit_markdown`, `edit_code`, `edit_file`, `create_file` are RW.
Worth pinning as a single table with a test, so a new tool added without a class is a
compile-or-test failure rather than a silent RO default — an unclassified write showing up as
"read" is the failure mode that would make the ledger lie.

**Open questions.**

1. **Diff granularity.** "Some sort of diff" spans three very different costs: (a) a byte delta
   from the `field_patch` events the librarian already emits — nearly free, already persisted;
   (b) a per-section list of what changed; (c) real content diffs. (a) is available today via
   `artifact_event(action="list")` and may be enough. Start there and see what is actually
   missing before building (c).
2. **Where does it surface?** A new tool, a `workspace(action=…)` arm, or an argument to the
   existing `usage` tool. Weakly favour extending `usage` — it already owns this table — unless
   the compaction use case wants a distinct, obvious name an agent will reach for under pressure.
3. **Does it need to survive compaction itself?** The ledger is in SQLite, so it does. The open
   part is whether anything should *auto-inject* a summary at compaction time, or whether an
   explicit call is better. Auto-injection has a poor track record here (byte caps, relevance),
   so the default should be explicit.
4. **Scope: session or window?** `cc_session_id` keys the Claude Code session, which survives
   `/compact` — so "this session" is well-defined across a compaction boundary. Confirm that
   holds before relying on it.

**Why it is worth building.** This is the concrete, machine-checkable half of a problem the repo
has already documented from the human side: AB-6 in
[`bistriceanu/agent-behavior-analysis.md`](bistriceanu/agent-behavior-analysis.md), *"the unearned
clean slate"* — the agent starts each session fresh while the human carries every prior mistake,
and the only cure named there is "a damage ledger the next session actually reads". That entry's
ledger is prose the agent must remember to write. This one is a byproduct of doing the work, which
is the difference between a discipline and a record.

## CAP-2 — Second arm: a codescout-issued LLM controller

**Ask.** Let codescout itself issue a separate LLM call (Anthropic) at chosen moments, acting as a
controller that challenges the primary agent — specifically targeting claims it is **not certain**
of, or that rest on code it **did not read**. Explicitly a long brainstorming task, not a plan.

**Substrate check.** codescout makes **no outbound LLM call today** — a search across `src/`,
`crates/` and `Cargo.toml` for `anthropic` / `ANTHROPIC_API_KEY` / `api.anthropic.com` /
`x-api-key` returns zero hits (2026-08-15). Even the librarian's augmentation refresh is
documented as *"run by the agent, not automatic"*. So this would make codescout an **LLM client**
for the first time, not merely an MCP server. That is the single largest thing to weigh: it adds
credentials, cost, latency, non-determinism, and a network dependency to a tool whose current
appeal includes having none of those.

**The evidence this is aimed at is unusually good.** From
[`bistriceanu/agent-behavior-analysis.md`](bistriceanu/agent-behavior-analysis.md):

- **AB-1, the root defect** — a census of one session, verified against the transcript: ten
  corrections, four from user pushback, six from running something, **zero from unaided
  self-review**. The agent's own account: *"I stated the conclusion at the moment I formed the
  hypothesis, instead of at the moment I verified it."*
- **Improvement candidate 3** — *"Institutionalize contradiction, don't wait for it. Self-review
  is worth zero; blind fresh-context review is cheap and effective (5/5, ~30s/agent)."*

That is the case for a second arm stated in measured terms, and it is also the constraint on its
design: the intervention that scored 5/5 was **blind and fresh-context**. A controller that
inherits the primary's context inherits its commitments and will rehearse them — the analysis is
explicit that re-reading one's own reasoning "reads exactly as convincing the second time". The
value is in the blindness, so the hard design problem is *what minimal, unbiased brief* the
controller receives.

**Design axes to work through (none decided).**

1. **Trigger.** What fires it? Candidates: a claim tagged UNCERTAIN; a tool-call pattern implying
   assertion-without-reading (an edit to a symbol never fetched with `include_body`); session
   milestones (pre-commit, pre-compaction); explicit invocation. The tool-call-pattern trigger is
   the interesting one, because codescout is the component that can actually see it — `tool_calls`
   already records the sequence, which is the same substrate CAP-1 wants. **CAP-1 is plausibly a
   prerequisite**, and that dependency is the strongest argument for doing CAP-1 first.
2. **Brief.** Blind by construction (see above). Probably: the claim, the files it concerns, and
   nothing about how the primary reached it.
3. **Authority.** Advisory note, a `RecoverableError`-style block, or an injected instruction?
   Anything that can *block* turns a non-deterministic remote call into a hard dependency of local
   work. Strong prior for advisory-only at first.
4. **Cost and latency.** Per-call price and seconds added, against a measured baseline of how often
   the primary is wrong in a way this would catch.
5. **Offline and restricted hosts.** codescout was just given a fully offline embedding path
   (`local-dir:`, merged `e6484b16`) precisely so a restricted environment can work without the
   network. A controller that silently degrades — or worse, silently *doesn't run* — on those hosts
   would reintroduce the exact failure class that work removed: a guarantee that fails open in
   silence. Whatever this becomes, absence of the controller must be loud.
6. **Who owns the key?** Reusing the host's Claude Code credentials, a separate key, and "no key
   configured" are three different products.

**Anti-goal worth stating early.** Not a general "review my work" pass — the census says
self-review-shaped prompts are dead weight. The target is narrow: *claims asserted without the
check that would settle them.*

**Relationship to what already works.** The maintainer's **Conclude Last** rule and the per-claim
verified-vs-inferred tag attack the same defect at the prompt layer, and AB-1 argues they are two
halves of one fix (the rule names the mechanism; the tag makes compliance observable). A
controller is a third, heavier layer. Before building it, it is worth knowing what the cheaper two
already recover — the natural experiment in that document is pre-registered for exactly this and
has not been run. **Running it is arguably the real first task of CAP-2.**

## Anti-goals

- Not a wishlist. An entry without a substrate check ("what exists today, what is missing") is not
  ready to be here.
- Not a bug tracker. A defect goes to `docs/issues/`, even when the fix is a new capability.
- Not a plan. Once an entry has tasks and a file structure, it graduates to a spec + plan and this
  entry becomes a pointer.

## History

### 2026-08-15 — opened

Created with CAP-1 and CAP-2, both raised in session. Substrate checks done at open time rather
than deferred: CAP-1's `usage.db` reading (the `--debug` gate on `input_json` is the whole gap),
and CAP-2's zero-hit search for any existing LLM client. `docs/TAXONOMY.md` gains a CAP-N row in
the same change.

## Template for new entries

<!-- Insert new CAP-N entries above the "## Anti-goals" heading. Also add an Index row.

## CAP-N — <title>

**Ask.** What the capability is, in the requester's words where possible.

**Substrate check.** What exists today, cited at path:line. What is actually missing. An entry
without this is not ready.

**Open questions.** Numbered, each one a decision someone has to make.

**Why it is worth building.** Evidence, ideally measured, ideally not self-generated.
-->

