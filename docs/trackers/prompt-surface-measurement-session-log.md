---
id: db65023089245832
kind: tracker
status: draft
title: Prompt-surface measurement session log
tags:
- prompt-surface
- eval
- measurement
- librarian
topic: prompt surface budget measurement eval harness compaction
entry_high_water_F: 49
entry_high_water_W: 28
entry_prefix:
- F
- W
---

Work stream: measuring codescout's prompt/tool/guide surfaces against a captured
Claude Code request, building a token-aware eval harness, and planning compaction.
Opened 2026-08-23.

Entries are prose sections (`## F-N — title` / `## W-N — title`) — the only shape
`link_scan` binds a citation to. Index tables below are hand-maintained reading
surfaces, not the definition.

## Index — frictions

| id | title | status |
|---|---|---|
| F-1 | Fixed output path destroyed the evidence for the headline figure | fixed (blast-radius 2026-08-26; the SIBLING eval kept the same code until F-36, 2026-08-27) |
| F-2 | `json.dumps` defaults inflated every schema measurement by 3.8% | fixed |
| F-3 | A subagent's `workspace(activate)` mutated the parent's active project | open |
| F-4 | REFUTED — "every augmentation in the catalog is gone" was a false negative; one tracker was genuinely unaugmented | refuted |
| F-5 | A stale recon finding was relayed as current | fixed |
| F-6 | 400k of context with no tracker or librarian use | fixed |
| F-7 | Spec asserted per-arm tool denial the harness cannot do | fixed |
| F-8 | Eight assertions in one task passed for the wrong reason, all the same shape | promoted-to-permanent-docs |
| F-9 | Five API drops stranded uncommitted subagent work | mitigated |
| F-10 | Task 3's native-tool veto keys on a per-arm env var the runner cannot set | fixed-verified |
| F-11 | Sharing one OAuth credential across Claude Code profiles broke three of them mid-pilot | mitigated |
| F-12 | gates.py reported means only, hiding a three-valued instrument for seven rounds | fixed-verified |
| F-13 | A bare `pytest` in prompt-engineering collects zero scenario tests | mitigated |
| F-14 | Offering a hedge channel made every run add a false positive it had excluded | open |
| F-15 | The new eval's dependent set favoured codescout 2-to-1 before a single run | fixed-verified |
| F-16 | The plan put the leak sweep in the wrong file; following it would have dropped the anti-oracle guard and reported green | fixed-verified |
| F-17 | New fixture files were invisible to the reserved-path set, so filler could silently overwrite a dependent | fixed-verified |
| F-18 | The plan named two constants and a CLI flag the file it points at does not have | fixed-verified |
| F-19 | The eval's guard apparatus is unenforced — no collection, no CI, and a permanent red makes "all green" inexpressible | open |
| F-20 | Four times in one task, a commit asserted in prose what it measured otherwise in code — one of them from a controller ruling | open |
| F-21 | I made a probe the acceptance gate for a question it was not precise enough to answer | fixed-verified |
| F-23 | I relayed a subagent's self-reported bug find to the user without verifying it; the bug did not exist | open |
| F-22 | A docstring word ending in "raise" fired a substring guard the docstring called safe | open |
| F-24 | `gates_blast.py` keys arms by log filename stem, so a second pilot round silently overwrites the first | fixed-verified |
| F-25 | I launched a paid eval run under a foreground tool timeout; the timeout killed it mid-session | fixed-verified |
| F-26 | L2 reads only tool ARGS, so it measures "files the agent had to name" — an inverse proxy for navigation-tool effectiveness | fixed-verified |
| F-27 | `native-tool-used` cannot tell "used a native tool" from "attempted one and was denied" — and it is counted, not excluded | fixed-verified |
| F-28 | The "native" arm is in practice a SHELL arm — it does almost everything through Bash | fixed-verified |
| F-29 | I published between-arm claims at n=2; thickening to n=6 killed all three of them | fixed-verified |
| F-30 | I led a surface report with a session anecdote the artifact under review had already measured and labelled unrepresentative | fixed-verified |
| F-31 | The evidence behind every published number sat on tmpfs; F-1's fix guarded one of the two mechanisms that delete it | fixed-verified |
| F-32 | The no-tools floor could run a shell — Monitor left un-denied by a documented ruling resting on an untested inference | fixed-verified |
| F-33 | Runs were scored against other runs' trees; the fix existed, was documented, and was never wired into the driver | fixed-verified |
| F-34 | I wrote a timing verdict that was true by construction — the trigger call is always the first opportunity | fixed-verified |
| F-35 | I measured that guides grow, then used today's sizes for historical injections — overstating every byte figure 1.17x | fixed-verified |
| F-36 | Four defects marked fixed were all still live in the sibling eval — including F-1, whose code was still deleting evidence every run | fixed-verified |
| F-37 | I cleared the proxy as a cause after checking only its response side, then blamed the API for a defect in our own request | fixed-verified |
| F-38 | I selected traces by content-matching a prompt string my own session contained, and published the table as a between-condition finding | fixed-verified |
| F-39 | A profile's settings.json `env` silently overrules an exported ANTHROPIC_BASE_URL — the guard I shipped validates its own input, not the client's | fixed-verified |
| F-40 | Two sessions added the same routing guard to one file within an hour, and both were wrong the same way | fixed-verified |
| F-41 | I reported "semantic search doesn't find these" from one filtered query, with no control — the unfiltered control refuted it | fixed-verified |
| F-42 | I captured the mechanism in the wrong population and refuted a hypothesis that was true | fixed-verified |
| F-43 | My promotion plan skipped two gates the skill documents — and named the weaker of two destinations for this rule's failure class | open |
| F-44 | Half the tasks carried a defect inherited from the plan's own reference code | open |
| F-45 | The documented pre-commit gate cannot see test code; CI's second clippy job can | fixed |
| F-46 | I described a budget from its module name — `SIZE_CEILING` counts rules, at compile time, on the set that is never delivered | fixed-verified |
| F-47 | A review base recorded before dispatch silently widened to three peers' commits — one git identity means `%an` cannot separate them | open |
| F-48 | F-47's remedy names the unit "task", but the thing that needs a base is the DISPATCH — so a fix round re-uses the implementation commit and widens 14× | open |
| F-49 | One fact had four representations, and three review rounds each fixed the one named — a grep over the known phrasings cannot find the form nobody has described yet | open |

## Wins Index

| id | title | status |
|---|---|---|
| W-1 | Adversarial verify caught four fabricating checkers | validated |
| W-2 | Re-measuring beat withdrawing | validated |
| W-3 | Reading the surface README before compacting it | validated |
| W-4 | Scouting the runtime's capability surface before planning | validated |
| W-5 | Measuring the predicate family before fixing beat four rounds of fix-then-discover | promoted-to-permanent-docs |
| W-6 | A subagent's combination search found what a 27-predicate single sweep declared clean | promoted-to-permanent-docs |
| W-7 | Reading the file settled two agent disagreements | promoted-to-permanent-docs |
| W-8 | A deleted git-ignored ledger was rebuilt from the session transcript | validated |
| W-9 | An Opus review with a mutation lens found four Criticals in code with 34 green tests | validated |
| W-10 | A real number attached to the wrong subject — five instances in one session | validated |
| W-11 | The pilot earned its cost by invalidating the fixture, not by measuring anything | validated |
| W-12 | An anchored retrieval eval ceilings by construction — naming the target supplies the doubt | promoted-to-permanent-docs |
| W-13 | The arms separated on a metric we already logged and never reported | validated |
| W-14 | Reading the real dependency chain before writing the spec turned an unbuildable trap into a buildable one | validated |
| W-15 | Reading the generator instead of the plan's description of it caught three defects, one with no downstream gate at all | validated |
| W-21 | Thickening n turned a dead claim into a better one — the mechanism came back stronger than it died | validated |
| W-22 | Reading the source artifact's own limits sections before designing overturned my headline and re-pointed the target | validated |
| W-23 | Re-running the verification after compaction found what re-reading the summary structurally could not | validated |
| W-24 | The control arm found two instrument defects instead of the effect it measured — it was the only arm whose runs differed | validated |
| W-25 | Handing each subagent the controller's own measurement, with a loud-fail gate, made three controller defects surface downstream | validated |
| W-20 | Running the second baseline the spec asked for overturned the headline it was meant to confirm | validated |
| W-19 | Staging the pilot behind a positive-control gate caught two result-fabricating defects mid-spend, for $1 | validated |
| W-18 | Adversarial review before the first spend caught three defects that would each have produced a fabricated pilot result | validated |
| W-17 | A pre-dispatch scout caught a forward dependency I had stated in a form the harness cannot express | validated |
| W-16 | Three dilution rounds converged without closing; one measurement of the shared cause moved it further than all three | validated |
| W-26 | Capturing the mechanism killed an axis that two outcome-comparisons got wrong | validated |
| W-27 | Clean-tree extraction answered "is this failure mine?" with evidence — and the evidence was a third answer | validated |
| W-28 | Naming a defect pattern in the review brief found the instance per-task reviews could not | validated |

## F-1 — Fixed output path destroyed the evidence for the headline figure

**Valid:** invariant
**Status:** fixed

**Observed:** `capture.py` wrote every capture to one hard-coded filename. The
`--bare` run therefore overwrote the default-mode capture, and the copy taken
afterwards copied the *bare* file. Both artefacts ended byte-identical (md5
`77c6b52f…`), so the evidence for the 196k-char split had silently destroyed
itself.

**Why it hid:** identical files look exactly like a successful copy. Nothing
errored. The defect was only found because a verification agent md5'd them.

**Fix:** the output path is now required on argv and an existing file is refused
rather than clobbered. Re-captured and re-measured rather than patching the old
arithmetic.

**Rests on:** the general principle that a measurement script must not be able to
overwrite a prior measurement — a capture tool with a constant output path is a
one-shot instrument wearing a repeatable one's interface.

## F-2 — `json.dumps` defaults inflated every schema measurement by 3.8%

**Valid:** invariant
**Status:** fixed

**Observed:** schema sizes were computed with bare `json.dumps(schema)`. Two
defaults inflate the result versus what actually goes on the wire:
`ensure_ascii=True` escapes non-ASCII (+310 chars) and the default separators emit
`", "` / `": "` (+1,906 chars — six times larger, and the one nobody noticed).

**Impact:** every per-tool figure was ~3.8% high. `artifact` reported 13,203
against a wire-exact 12,727. The *shape* of the finding survived (tools dominate
the budget), but published numbers were wrong.

**Fix:** `measure.py` uses `ensure_ascii=False, separators=(",", ":")`. The repo's
own `tool_surface_report_lengths` was trustworthy all along and should be the
reference any ad-hoc script is checked against.

**Rests on:** `src/prompts/README.md` § *The tool-surface budget*, which already
names `advertised_surface()` as the thing that must match what `list_tools` builds.

## F-3 — A subagent's `workspace(activate)` mutated the parent's active project

**Valid:** conditional — until activation becomes caller-scoped
**Status:** open

**Observed:** a background workflow agent called
`workspace(action="activate", path=<sibling repo>)`. Active-project selection is
process-global in the codescout MCP server, so the parent session's project
changed underneath it and its next write — to a scratchpad path it had written to
five times in the same turn — was refused mid-turn.

**Full record:** `docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md`
(severity high). Measured: exactly one activate across all agent transcripts, zero
`read_only` occurrences, identical `[security]` config in both projects.

**Dispatch defect on our side:** the briefing told agents to `activate`. Every
codescout tool already takes a per-call `workspace` parameter documented for
"concurrent subagents in different workspaces", which resolves per-caller without
touching shared state. Brief subagents with the parameter, never the activate.

## F-4 — Every augmentation in the catalog is gone

**REFUTED 2026-08-26 — nothing was lost.** This entry's heading is wrong; it is
kept unchanged so the reasoning stays findable and the correction visible.

**Valid:** dated 2026-08-26
**Status:** refuted

**Observed (2026-08-23):** `artifact(find, kind="tracker", augmented=true,
scope="repo")` returned zero rows, and `f2ecdd76a6189efb` — the T-N ledger
CLAUDE.md hard-codes — read `augmentation: null`.

**Refutation (2026-08-26), from the catalog's own columns.**
`f2ecdd76a6189efb`'s augmentation row carries `created_at =
2026-07-05T06:51:44Z` with an unbroken history, and `augmentation::upsert`
stamps `updated_at` on conflict but never `created_at` — so it cannot have been
re-inserted later wearing an old date. No codescout row is stamped 08-22/23/24,
which a restore would have stamped all at once; a 2026-07-12 backup holds 53
augmentations against today's 70; no codescout `worktree_registration` has ever
existed; and no restore was ever performed. Independently,
`docs/issues/2026-08-25-sdd-ledger-and-catalog-rows-vanished.md` records
`f2ecdd76a6189efb` alive with all 26 `T-N` rows on 2026-08-25 — two days after
the supposed loss.

**The mechanism note was true, and is worth keeping — it just was not what
happened.** Augmentation does live only in the catalog DB, has no on-disk form,
and is the one class of state `reindex` cannot rebuild, so a post-loss reindex
would report healthy and repair nothing. All correct; none of it evidence that a
loss occurred. A true mechanism is not a substitute for a measurement, and
supplying one is what made the conclusion feel established.

**What was actually wrong:** `docs/research/README.md` had no augmentation — one
tracker, not the catalog. Fixed 2026-08-26; the index renders.

**What produced the zero:** `find(augmented=true)` returned a bare count without
reporting how many augmentations the catalog held or which scope applied, so
"excluded by this query" and "destroyed" were indistinguishable. Fixed in
`a77a39a0` (patch-id `087f3d1d`) with two regression tests.

**Full record:**
`docs/issues/archive/2026-08-23-research-index-tracker-has-no-augmentation.md`
## F-5 — A stale recon finding was relayed as current

**Valid:** invariant
**Status:** fixed

**Observed:** a recon agent reported at ~19:00 that the eval harness could not run
— no `.venv`, no `~/.prompt-tdd`. That was relayed to the user as the current
blocker. Both had been created by the *build* agent at 19:45, 45 minutes later and
in the same workflow.

**Root cause:** a workflow's phases observe the world at different times, and a
recon finding about mutable environment state has a shelf life measured in
minutes. Findings about *code* survive the run; findings about *environment* may
not.

**Rule:** before relaying any environment claim from an earlier phase, re-check it.
A one-command `ls` would have prevented this.

## F-6 — 400k of context with no tracker or librarian use

**Valid:** invariant
**Status:** fixed

**Observed:** user observation, and correct. Across roughly 400k of context this
session produced two bug files but zero session-log entries, and never queried the
librarian until prompted — despite the work being exactly what these surfaces
exist to capture (measurements that decay, mechanisms worth citing later).

**Consequence:** F-1 through F-5 were all reconstructed at the end from
conversation memory rather than logged on notice, which is the failure mode
CLAUDE.md's capture-on-notice rule exists to prevent. Two of them (F-3, F-4) were
already bug-filed; the other three would have been lost at compaction.

**Second-order:** the librarian query that eventually ran (`augmented=true`) is
what exposed F-4. Querying the catalog is not only bookkeeping — it is a
diagnostic. The tracker discipline would have surfaced a repo-wide defect hours
earlier.

## W-1 — Adversarial verify caught four fabricating checkers

**Valid:** dated 2026-08-23
**Status:** validated

**Practice:** the compaction workflow's final phase spawned two independent
reviewers tasked with *refuting* the build, defaulting to unsound when uncertain.

**Counterfactual, concrete:** the new eval harness shipped with a correctness
predicate that demanded the wrong answer. The fixture unpacks twice — plain at the
root, and into `other/` with `sed s/with_tax/apply_levy/` — and the predicate
demanded `with_tax`, the local copy's name. It rewarded reading the wrong project
and failed every correct answer, guaranteeing a 0/10 floor behind a plausible
`wrong-answer` class. That would have been published as "the base prompt fails at
workspace routing" — a fabricated finding about codescout's own prompt surface.
Three sibling defects (dead `distinct` guard, truncated denominator, a null control
that passed denials) were found in the same pass.

**Promote-when:** a second workflow's verify phase changes a headline conclusion.
Then promote "adversarial verify defaults to unsound" into the workflow template.

## W-2 — Re-measuring beat withdrawing

**Valid:** invariant
**Status:** validated

**Practice:** when F-1 and F-2 invalidated the published numbers, the response was
to fix both instruments and re-capture from scratch, not to annotate the old
figures as withdrawn or patch the arithmetic.

**Counterfactual:** the alternative was a deliverable whose headline table read
"withdrawn — no surviving artefact", and a baseline that could never be trusted.
Re-capture cost one command and produced numbers that independently reproduced the
workflow's own figure for the librarian/artifact family (62.1% vs 62%), which is
also a cross-check the patched arithmetic could never have supplied.

**Rests on:** the project's Iron Rule that an unmeasured mechanism is a hypothesis
wearing a conclusion's clothes.

## W-3 — Reading the surface README before compacting it

**Valid:** invariant
**Status:** validated

**Practice:** `src/prompts/README.md` was read in full before planning any edit to
the surfaces it governs.

**Counterfactual:** the stated task was "compact the system prompt". The README
establishes that `server_instructions` is injected once per MCP session, is already
hard-capped at 1900 characters, and — decisively — that `fit_dynamic_block` refills
the channel to 2,000 chars, so every character freed is immediately consumed by the
Project Status block. Compaction there yields **exactly zero net tokens**. Without
that read, the effort would have gone into the one surface that cannot pay, and a
character-count metric would have scored it a success.

**Also recovered from it:** the ~12.5-turn break-even that makes schema→guide
relocation a loss, which is a sharper result than anything in the external source
that prompted the work.

## Follow-ups

- **Measure the librarian itself.** User request, 2026-08-23: the librarian is
  subjectively helpful and that should be quantified, likely as a sibling eval to
  the hidden-information one rather than a variant of it. Candidate axes — does an
  agent with catalog access find prior decisions faster than one grepping
  `docs/`; does provenance (`**Valid:**` / `**Rests on:**`) change whether an agent
  trusts a stale claim; does `librarian(context)` beat `semantic_search` for
  "what did we decide about X". Note the confound: the librarian family is 62.1%
  of codescout's tool-surface footprint, so any librarian eval is also the
  strongest test of whether that footprint earns itself.
- **Re-plan compaction against the guide corpus.** 97,690 chars over 10 topics with
  no cap, no counter, and no gate — the only surface with five-figure headroom. The
  existing 14-step plan was reviewed unsound.
- **Restore the augmentations** once F-4's fix direction is decided.

## F-7 — Spec asserted per-arm tool denial the harness cannot do, and the existing precedent is unenforced

**Valid:** conditional — until a tool-restriction passthrough lands in `SessionConfig`
**Status:** open
**Severity:** high

**When:** 2026-08-23, pre-plan reconnaissance for the hidden-information eval spec
(`556cc34167321863`, committed `f4d3d7b2`), before writing any implementation plan.

**Expected (spec § 2):** the `hidden-cs` arm runs "codescout MCP only; native
`Read`/`Grep`/`Glob` denied". Written as if the harness supports per-arm tool
restriction.

**Got (scouted reality):** it does not. `grep` for
`allowed_tools|disallowed_tools|allowedTools|permission_mode|deny` across
`prompt-engineering/src/**/*.py` and every scenario YAML returns **four** hits, all
`permission_mode` — `src/prompt_tdd/adapters/claude_code.py:51` (default
`"bypassPermissions"`), `:174` and `:361` (the two subprocess arg builders), and
`src/prompt_tdd/cli.py:69` (the loader). `permission_mode` governs *prompting*, not
tool availability. `~/.prompt-tdd/profiles/plugin-free/settings.json` is `{}`, so no
deny rules there either.

**The capability exists one layer down.** Claude Code 2.1.241 offers three flags the
harness never passes:

- `--disallowedTools, --disallowed-tools <tools...>` — deny-list, pattern syntax
  (`"Bash(git *) Edit"`)
- `--allowedTools, --allowed-tools <tools...>` — allow-list, same syntax
- `--tools <tools...>` — "the list of available tools **from the built-in set**;
  `""` disables all tools, `"default"` uses all"

So the fix is a passthrough, not a redesign: a `tools` / `disallowed_tools` field on
`SessionConfig`, read in `cli.py:69`, emitted at `claude_code.py:174` and `:361`.
Four sites, mirroring exactly how `permission_mode` is already threaded.

**Probable cause:** the spec was written from the eval's requirements without
scouting the runtime's capability surface. The arm definition named a behaviour;
nobody checked the harness could produce it.

**Why severity is high, not med.** The likely failure is not a stall. Every existing
arm in `scenarios/surface-budget/` already restricts tools by **prompt instruction**
— the smoke scenario's message reads "Do not use Bash, Read, Grep, Glob, Edit or
Write." An implementer hitting the gap would most plausibly follow that precedent,
and `hidden-cs` would then be enforced only by asking nicely. Any run where the model
reaches for `Read` anyway silently contaminates the arm, and **nothing in the
currently-specified metrics would reveal it** — the spec scores recall, precision and
tokens, none of which notice which tools produced the answer. That is a silent path
from a soft constraint to a wrong headline finding about whether codescout helps.

**Mitigation available today, and it should be in the spec regardless of the
passthrough:** `surface_lib.collect_facts()` already records `tool_names` per run.
A checker veto — fail any `hidden-cs` run that used a native file tool, as its own
class — mirrors the existing `no-mcp-tool-used` veto in `check_nullctl.py` and turns
silent contamination into a loud, countable failure. Enforcement and detection are
independent; ship the detection even after the passthrough lands, because a
passthrough that silently stops working looks exactly like compliance.

**Unverified:** whether `--tools ""` leaves MCP tools intact. The help says "from the
built-in set", which implies MCP tools are out of its scope, but that is read off a
help string, not measured. `--disallowedTools` with an explicit list avoids depending
on the distinction and is the safer choice until someone measures it.

**Fix idea / Pointer:** spec § 2 updated in the same session to name the real
mechanism and add the veto. Harness passthrough is a prerequisite task for the
implementation plan, not part of the eval itself.


**FIXED 2026-08-24.** Shipped as Task 1 of the hidden-information eval plan, in
`prompt-engineering` on `master`. `SessionConfig.disallowed_tools: str` plus a module-level
`_tool_flags(session) -> list[str]`, threaded through **both** `claude -p` arg builders and
the YAML loader.

| commit | patch-id | what |
|---|---|---|
| `772fecd` | `b0838894162ffc1abbc214fc5467c13ed339ea18` | the passthrough |
| `2c607f4` | `107d0992539cf480f1cd38dca08a5eade79d3bab` | regression cover for the three wiring lines |

The second commit is the load-bearing half. The first review found that deleting any one of
the three wiring lines left the entire new test file green — the helper was well tested and
the helper is not what can silently vanish. Cover now asserts flag **and values together**
positionally, across both the single-turn and multi-turn paths, because `_evaluate_handler`
returns via `_run_history_turns` before its own `cmd` reaches `subprocess.run`, so a
history-only test exercises one call site twice and the other not at all.

Probed against the real CLI before relying on it: with `--strict-mcp-config` and
`--permission-mode bypassPermissions`, the deny-list genuinely leaves the model with no
file-reading tool. **Without** `--strict-mcp-config` the model reads the file through a
connected MCP server — `--disallowedTools` denies native tools, not MCP ones, so it is not a
no-file-access switch on its own. That fact is now OP-12 in `prompt-engineering`'s
`docs/trackers/prompt-tdd-operating-guide.md`.
## W-4 — Scouting the runtime's capability surface before planning caught an unbuildable arm

**Valid:** dated 2026-08-23
**Status:** validated

**Observed:** 2026-08-23, immediately after the hidden-information eval spec was
approved and committed (`f4d3d7b2`), before writing its implementation plan.

**Pattern:** when a spec's design names a *runtime behaviour* — "this arm runs with
X denied", "this run is capped at N turns", "this arm uses model Y" — scout whether
the harness can actually produce that behaviour, before the plan is written. Design
review checks whether the experiment answers the question. It does not check whether
the runtime can run the experiment, and those are different failures caught by
different readers.

The scout is cheap and mechanical: grep the runner for the knob, and read the
underlying CLI's flag list for the capability. Two calls found both the gap and its
fix.

**Counterfactual, concrete.** Without it, the implementation plan would have
specified `hidden-cs` as "native tools denied" and the implementer would have hit a
harness with no such field. The available precedent is prompt-instruction
enforcement — every existing `scenarios/surface-budget/` arm restricts tools by
asking ("Do not use Bash, Read, Grep, Glob, Edit or Write"), so following it is the
path of least resistance and looks correct.

The eval would then have run with an arm enforced only by request. Cost if that
reached phase 2: **32+ runs at $40–100**, producing a headline about whether
codescout helps that is partly an artifact of the model reaching for `Read` in the
arm where it was supposed to be unavailable — and none of the specified metrics
(recall, precision, F1, tokens) would have shown it, because none of them look at
which tools produced the answer. The failure would have been invisible in exactly
the way the four fabricating checkers (W-1) were: a clean number from a broken
setup.

**Confirming data points:**
1. W-1 (this log) — adversarial verify caught four checkers producing clean numbers
   from broken processes. Same failure class, caught at a different stage.
2. F-7 (this log) — this scout; the arm was unbuildable as specified and the
   fallback was silently wrong rather than loud.

**Impact:** high — prevented a contaminated arm in a $40–100 measurement whose
contamination would not have been detectable after the fact.

**Promote-when:** a second spec-stage scout catches a runtime-capability gap that
design review passed. At two datapoints, promote to the reconnaissance skill as a
craft-shaped rule — "a design that names runtime behaviour is a seam; scout the
runner before writing the plan" — since it is true of any harness in any language,
not just this one.

## W-5 — Measuring the predicate family before fixing converged where four rounds of fix-then-discover did not

**Valid:** dated 2026-08-24

**Observed:** 2026-08-23/24, the hidden-information eval's fixture (Task 2 of
`docs/superpowers/plans/2026-08-23-hidden-information-eval.md`). Six fix rounds against one
property: *no mechanical filter over the generated tree may isolate the 12 truth sites from
the ~157 filler files.*

**Pattern:** When a generated artifact must be indistinguishable from hand-authored content,
do not patch the discriminating channel you were told about. **Add predicates covering the
whole category to a measuring instrument first, re-run the sweep, and aim the fix at the
measured result.**

**Counterfactual, with the rounds as the evidence:**

| round | brief said | outcome |
|---|---|---|
| 1 | close filename + length | closed those two; name-shape appeared |
| 2 | drop the `_<k>` suffix | closed it; docstrings / annotations / uniqueness appeared |
| 3 | make the style distributions overlap | closed five channels; token frequency + signature vocabulary appeared |
| 4 | close token + signature | closed those; **body AST shape** appeared |
| 5 | **add body-shape predicates, THEN widen** | category closed in one pass |
| 6 | same method, literal-value rarity | closed; stratified null p 0.042 → 0.250 |

Rounds 1-4 each closed exactly the list they were handed and surfaced the next channel.
Rounds 5 and 6 named the *mechanism* rather than the symptom and closed the category. The
controller's own briefs are the confound: round 2's brief ("drop the suffix") was a symptom
instruction and produced a symptom fix.

**Confirming data points:**
1. Round 5 — body-shape predicates added before any generator change; reopening the fix
   (`DECORATE_P = 0.0`) measures lift **8.45** at 0.93 hand-authored precision against
   **1.51** at 0.18 after. One pass.
2. Round 6 — same method on literal-value rarity; the leaking OR went 6 files / 4 of 12 /
   lift **9.39** → 76 files / lift **1.67**, and stratified null p moved 0.042 → 0.250
   **with no bar moved to achieve it**.

**Impact:** high — four rounds of controller and implementer time, roughly 1.5M subagent
tokens, spent closing channels one at a time before the method changed.

**Promote-when:** promoted 2026-08-24 to codescout memory `eval-design` § *Generated
fixtures leak by authorship*, together with the family/combination/null-control method.

**Status:** promoted-to-permanent-docs

## W-6 — A subagent's combination search found what the controller's 27-predicate sweep declared clean

**Valid:** dated 2026-08-24

**Observed:** 2026-08-24, verifying round 4 of the hidden-info fixture. I swept 27
structural predicates individually — ten of them invented for the check, precisely to test
whether the fixture had been tuned to its own guard list — and reported the fixture "at the
null floor": worst concentration lift **1.6** against a base rate of 7.1%.

**Pattern:** **A predicate sweep that evaluates predicates singly cannot see conjunctions,
and conjunctions are where the leak lives.** Any guard over a family of filters must search
combinations (pure ANDs and ORs to at least k=3), because a filter that is harmless alone is
potent when intersected with anything that removes the other population.

**Counterfactual:** the implementer's own combination search — which I had asked for in the
same brief — immediately found twelve leaking combinations that my sweep was structurally
blind to, including `length ≤30 AND leading-token-rarity` at 11 of 12 truth sites, and
`leading-token AND no-float AND cross-file-called` at 5 files / 5 of 12 / **zero false
positives** / lift 14.1. Had I acted on my own "null floor" verdict, the fixture would have
shipped with a three-predicate filter handing over 42% of the answer set at perfect
precision.

**The statistic I was reading was the wrong one.** The token predicate sat at lift 1.6 — so
it cleared every threshold — while capturing **11 of 12 truth sites**. High-recall,
low-precision filters are exactly the dangerous conjunction ingredients: they already contain
the answer, so ANDing them with anything that drops filler concentrates immediately.
Precision was never the risk; recall was, and no single-predicate bar looks at it.

**Confirming data points:**
1. This instance — 27 singles clean, 12 combinations leaking.
2. Round 6's null control, over 140,675 swept candidates, is a combination search by
   construction; the guard's own `_is_leak` gate now applies at k=1, 2 and 3.

**Impact:** high — the controller's verification would have certified a leaking fixture.

**Promote-when:** promoted 2026-08-24 to codescout memory `eval-design` ("searched over
**combinations** … three individually-clean predicates reached 10-of-12 sites").

**Status:** promoted-to-permanent-docs

## W-7 — Reading the file settled two agent disagreements that weighing the agents would have got wrong once

**Valid:** invariant

**Observed:** 2026-08-24, twice in one session, in both directions.

**Pattern:** **When two agents disagree about what a file says, open the file.** Do not
adjudicate by seniority, recency, or which one sounds more careful — both are usually acting
in good faith on the same bytes, and the disagreement is about *where they looked*.

**Case 1 — my subagent was right, a peer session was wrong.** My research subagent reported
that quorum discredited a 352-run, $650 gate because the agent under test could read its own
scenario's `story.md` answer key. A peer session in `changelog-reader`, reading the same two
clones, searched for `leak`, `contaminat`, `$650`, `352` and "own scenario" and **could not
find it**, and asked me to point at the source. Reading
`superpowers-evals/docs/experiments/2026-08-08-fresh-release-gate.md:10-14` confirmed the
claim verbatim. The peer's search had covered `src/` and `docs/superpowers/specs/`; the
incident lives in `docs/experiments/`. Had I deferred to the peer's negative result I would
have retracted a true finding I had already relayed to the user as fact.

**Case 2 — my subagent was right, I was wrong.** I ran the fixture suite mid-round, read
`file length ≤30 + leading token <5x` at 27 files / 11 of 12 truth sites, and built a whole
fix brief on it. The implementer replied that `TEMP-MUTATION-6` was live in the tree at the
time and that on the real generator the predicate is 64 files / **1** truth site / lift 0.22
— then **re-applied the mutation to a copy to prove attribution rather than assert it**. My
numbers reproduced under it and vanished without it. The reviewer independently reproduced
both states.

**Counterfactual:** case 1 costs a retracted-but-true finding and a lost lesson; case 2
costs a round spent widening a vocabulary that was already fine. Each check took under a
minute — a `sed -n` in case 1, a rebuild-and-measure in case 2.

**Corollary worth keeping separately:** *never read a number off a working tree that has a
live mutation in it.* A mid-round suite run reads whatever the implementer currently has
staged. Ask for a clean-tree run, or apply the mutation yourself.

**Impact:** med-high — one false retraction avoided, one wasted round caused and then
diagnosed.

**Promote-when:** promoted 2026-08-24 to codescout memory `test-design-discipline` §
*`indeterminate` is a third verdict* → *Corollary: never read a number off a working tree
that has a live mutation in it*.

**Status:** promoted-to-permanent-docs

## F-8 — Eight assertions in one task passed for the wrong reason, all the same shape

**Valid:** invariant

**Observed:** 2026-08-23/24, across six fix rounds on the hidden-info fixture guard. Every
one was found by a reviewer or by mutation, never by the suite going red.

**When:** writing or reviewing any assertion, especially a guard over an invariant that
cannot be checked directly.

**Expected:** an assertion that names an invariant tests that invariant.

**Got:** eight that named the right invariant and measured something **adjacent** to it:

1. `test_answer_key_is_not_inside_the_fixture` called `build()`, which writes no JSON — only
   `main()` does. Both assertions trivially true while the key could have been written inside
   the tree the agent reads. **This guarded the hardest integrity invariant in the spec.**
2. `assert non_mod != planted_py` — two sets that always differ by ≥2 by construction.
3. The name-shape predicate `any(not SUFFIX_DIGITS.search(d.name))` matched **133 of 133**
   files, because `__init__` and CamelCase can never carry a digit suffix. Dead on the
   unmutated tree, not merely under mutation.
4. + 5. Two restored length-overlap counts — which the controller had ordered restored
   *specifically* to catch a regression — both passed under the very mutation they were
   restored to catch.
6. `_score` counted false positives excluding decoys while the precision beside it included
   them; the discrepancy reaches 8 on any filter catching a decoy, and that number was
   headed for a pre-registration.
7. The family guard itself passed green on an **emptied** predicate space: `real = 0.0`,
   every null best `0.0`, `0.0 >= 0.0` → p = 1.0 → pass, printing `worst … none` with
   nothing asserting on it.
8. Decoy 8's ground-truth symbol (`pricing-overview`) appeared nowhere in its file, so the
   decoy was unhittable and the precision-penalty set was really 7, not 8 — the existence
   test covered `sites` but not `decoys`.

**Probable cause:** an invariant that is awkward to observe directly gets tested through
whatever *is* observable nearby, and the substitution is invisible once written.

**Workaround that works:** the mutation question — *what one-character change to the
production code flips this red?* — caught all eight. Two structural guards followed: a
per-predicate **liveness floor** (`0 < matched < len(all)`), and a **space-size floor**, which
between them make cases 3 and 7 unrepresentable rather than merely detectable.

**Severity:** high — case 1 alone would have let the agent under test read the answer key,
which is the failure that discredited a 352-run, $650 gate at prime-radiant.

**Status:** promoted-to-permanent-docs — codescout memory `test-design-discipline` §
*EXECUTED is not TESTED*, with all eight listed and the shared shape named as its corollary.

**Fix idea / Pointer:** vocabulary from evener's coverage floor: code that *runs* is
EXECUTED, code whose *output is checked* is TESTED, and the gap is invisible in a green bar.

## F-9 — Five API drops stranded uncommitted subagent work; incremental commits and one-line tasks are the fix

**Valid:** dated 2026-08-24

**Observed:** 2026-08-24, five separate agent terminations during the hidden-info fixture
rounds — four `API Error: Connection lost mid-response` / `The response stopped arriving`,
and one process exit when Claude Code itself restarted.

**When:** long-running implementer or reviewer subagents on large diffs, especially once a
subagent's context passes ~200k tokens.

**Got, per occurrence:**

| # | died at | stranded |
|---|---|---|
| 1 | ~10 min in | 887 insertions in `gen_fixture.py`, uncommitted |
| 2 | mid-sentence, "the restored assertion is nearly vacuous" | 1,413 insertions across two files |
| 3 | immediately, before any tool call | nothing |
| 4 | one line after "21 passed. Committing" | the commit itself |
| 5 | CC process exit | `null_control.py` (17 KB) + test changes |

**Probable cause:** long responses. Every drop landed mid-response; the immediate one (#3)
was the only exception and cost nothing.

**Workarounds, both validated in this session:**

1. **Instruct implementers to commit whatever is green as soon as it is green.** Added after
   drop #2; drop #4 then cost only a commit instead of an hour.
2. **When only a commit remains, send a one-line instruction.** The reply is then too short
   to drop. Used to recover #4 — the agent came back with a single SHA and subject line.
3. **Recover by inspection, not by restart.** In every case `git status` plus a test run
   established what survived; the work was intact each time and resuming beat re-dispatching.
   For #5 the remaining task was self-contained, so a *fresh* agent was cheaper than resuming
   one carrying 421k tokens of history.

**Severity:** med — no work was ultimately lost, but roughly two hours of subagent time was
re-spent before the workarounds were in place, and each recovery cost a controller
verification round.

**Status:** mitigated

**Fix idea / Pointer:** carry "commit incrementally" in every implementer dispatch brief as
standing text rather than adding it after the first drop.

## W-8 — A deleted git-ignored ledger was rebuilt from the session transcript, which records every write with its payload

**Valid:** invariant

**Observed:** 2026-08-25, resuming after compaction. The plan's `ledger:` frontmatter key
named `.superpowers/sdd/2026-08-23-hidden-information-eval/progress.md`; the whole
directory was gone from disk. `.superpowers/sdd/.gitignore` is `*`, so there was no git
copy and no reflog entry — the normal recovery paths were all unavailable by construction.

**Pattern:** A Claude Code transcript is a complete, ordered write log. Every
`create_file` / `edit_markdown` / `edit_file` call is stored as a `tool_use` block with
its **full input payload**, and every outcome as a matching `tool_result` carrying
`is_error`. So for any file an agent authored, the transcript holds enough to replay it —
including files git was never allowed to see.

The method, in three steps:

1. Scan `~/.claude*/projects/<project-slug>/*.jsonl` for `tool_use` blocks whose input
   names the path. (Scan all profiles — this machine runs three.)
2. Join each to its `tool_result` by `tool_use_id` and **drop the failures.** This ledger
   had two `edit_markdown` calls refused with *"File writes are disabled for this
   project"* (the F-3 read-only-activation friction). They wrote nothing on the day, and
   replaying them would have inserted content the original never held.
3. Apply the survivors in timestamp order.

**Counterfactual:** 1,264 lines recovered, including all 24 `R-N` rulings and the
`## CURRENT STATE` resume block. Without it, Tasks 3–6 would have started from the
compaction summary alone, which names 7 of the 24 rulings. The other 17 would have been
re-litigated or — worse — silently re-decided the other way, and R-24 in particular
(`LIFT_BAR[3] = 9.5` overriding R-21's fitted 7.0) is exactly the kind of ruling whose
reversal produces a confident wrong number rather than an error.

**Two caveats, both found by doing it:**

- **A simulated edit is not the tool's edit.** My replay of `insert_before` produced
  different whitespace than codescout's, so the two later `edit_file` calls keyed to that
  exact text missed, leaving a duplicated 30-line block and three stray `## Progress`
  headings. Reconciling by diff was quick, but only because the mismatch was loud. Prefer
  replaying through the real tool when the edit chain is short; simulate only when it is
  long, and then verify structurally (heading map, id coverage) rather than by eye.
- **Say so in the artifact.** The reconstruction is content-faithful, not byte-faithful.
  It carries a provenance header naming the transcript, the op count, and what may differ,
  so no later session cites it as the original.

**Confirming data points:**

1. This session — 22 of 24 recorded ops replayed cleanly; the 2 failures were correctly
   skipped by reading `is_error` rather than assuming success.

**Promote-when:** a second transcript replay recovers work no other surface held. At two
datapoints, promote to codescout memory `gotchas` as a recovery procedure — it is
craft-shaped, not project-shaped, and applies to any agent-authored file outside git.

**Status:** validated — single datapoint, loss fully recovered and verified (all `R-1`..`R-24`
present, heading map intact).

**Rests on:** `docs/issues/2026-08-25-sdd-ledger-and-catalog-rows-vanished.md`, which
records the loss itself and the catalog half of the repair.

## F-10 — Task 3's native-tool veto keys on a per-arm env var the runner cannot set

**Valid:** conditional — until `run_arms.py` gains per-arm `setup.env`

**Observed:** 2026-08-25, pre-dispatch reconnaissance for Task 3 of the hidden-information
eval. About to dispatch the implementer.

**When:** Reading the plan's Task 3 Step 3 code, before any subagent ran.

**Expected (plan):** the checker's native-tool veto is gated per arm —

```python
used = set(facts.get("tool_names", [])) & NATIVE_TOOLS
if used and os.environ.get("HIDDEN_ARM") == "cs":
    return "native-tool-used"
```

**Got (scouted reality):** `scripts/run_arms.py:101` builds **one** environment and reuses
it for every arm in the loop:

```python
env = {**os.environ, "PROMPT_TDD_RUN_LOG": str(log)}
```

`PROMPT_TDD_RUN_LOG` is the only key that varies per arm. There is no `setup.env`, no
argv, and no per-arm `PROMPT_TDD_SCENARIO_DIR`. So `HIDDEN_ARM` has exactly two reachable
states, and **both are wrong**:

- **unset** — the veto never fires. The `hidden-cs` arm silently scores runs that used
  `Read`, which is the one thing the veto exists to catch. Fails open, reports a number.
- **set once for the whole invocation** — the veto fires on `hidden-native` too, where
  native tools are the arm's *definition*. Every native run vetoed by construction; the
  comparison reads as a codescout landslide.

Neither state errors. Both produce a clean table.

**Probable cause:** the plan was written before ruling R-5/R-6 was made (both are in the
SDD ledger's `### Rulings from the Task 3/4/5 pre-dispatch scout`, allocated while Task 1
was still running). The plan text was never revised to match, and the ledger's
`Carried into Task 3` block names R-5/R-6 but the plan's code block still shows the env
form. Two surfaces, one of them stale — the classic shape.

**Workaround:** implement per-arm identity the way R-5/R-6 specifies — a thin per-arm
checker shim that hard-codes its own arm and delegates to a shared scorer, writing the arm
into `facts` so `score_arm.py`'s re-scoring path cannot mispair it. This composes with R-8
(one config dir per checker), which the same scout re-confirmed at
`scripts/run_arms.py:87-94`: the runner picks the **first** arm's checker for the whole
directory and warns only on stderr.

**Severity:** high — it would not have failed. It would have produced the eval's headline
number, wrong, with no error anywhere in the chain. This is the F-8 shape (a check that
passes for the wrong reason) promoted from an assertion to the instrument itself.

**Status:** fixed-verified — caught pre-dispatch; the implementer's brief carries the
corrected design and the `path:line` evidence for it.

**Rests on:** R-5 / R-6 / R-8 in the SDD ledger
`.superpowers/sdd/2026-08-23-hidden-information-eval/progress.md`; verified this session
against `scripts/run_arms.py:87-105` and `scripts/score_arm.py:69-83`.

## W-9 — An Opus review with a mutation lens found four Criticals in code that had 34 green tests

**Valid:** invariant

**Observed:** 2026-08-25, Task 3 of the hidden-information eval. A Sonnet implementer
delivered the checker with 34 passing tests — 10 from the plan verbatim plus 24 it wrote
itself, including genuine red/green verification via `git stash push -u`. Good work by
every visible signal. An independent Opus review, dispatched blind with an explicit
mutation-testing lens, returned **four Critical findings**, five Important and six Minor.
I verified all four Criticals at the source myself before acting; every one was real.

**Pattern:** for code that *publishes numbers*, budget an independent review at a stronger
model than the implementer, and give it two specific instructions:

1. **The mutation question, per test:** *if I deleted or inverted the logic this test
   claims to cover, would it still pass?* The reviewer ran 43 mutants and found 5
   survivors — including `facts.update(m)` (the single line that puts every published
   metric into the log) replaceable with `pass` on a fully green suite.
2. **A named-risk list to attack first,** written by whoever hands off. Two of the three
   risks I named were cleared with reasoning; naming them cost nothing and bought a
   verdict instead of a worry.

**Counterfactual — the finding neither the implementer nor I would have reached.** C2:
`LINE_RE`'s symbol class was `[\w.]+`. codescout's canonical symbol notation is
`Quote/subtotal`; raw source reads as `Quote.subtotal`. So the **cs arm's natural idiom
does not parse and the native arm's does** — measured on identical correct content, f1
**0.4** vs **0.6667**. Four of the twelve real truth sites carry dotted symbols, so it
would have fired. That is a differential measurement error *correlated with the
treatment*: the worst defect an A/B eval can carry, because it produces a clean, confident
table showing codescout worse than native for a purely notational reason.

**It was my defect, not the implementer's.** That regex came from the plan — I wrote it.
The implementer was told to keep the plan's scorer verbatim and correctly did. A review
that only checks the implementer against the brief cannot find this class of bug; only one
that checks the *artefact against the world* can.

The same review also found the veto could not fire at all when a scenario omits
`mode: trace` (`assertions.py:532-537` writes a well-formed doc with `tool_calls: []`, so
`have_trace` is `True` while the tool list is empty) — a guarantee asserted from data that
is silently absent.

**Confirming data points:**

1. 2026-07-07, EDU-Planner SI-29 — a Sonnet review approved a new module with zero
   Important findings; a blind Opus re-review with a mutation lens found a
   `(owner, date)` key-discrimination path with zero coverage. Recorded in CLAUDE.md.
2. This session — 4 Critical / 5 Important / 6 Minor on 34 green tests, one of them
   arm-correlated and therefore fatal to the eval's validity rather than merely to its
   correctness.

**Promote-when:** already promoted — CLAUDE.md's *Subagent Dispatch — Model Floor +
Review Escalation* rule states it. This entry is the second datapoint and **sharpens** it:
the existing rule says escalate for "test-rigor / edge-case coverage on load-bearing
code." Add the sharper trigger — **escalate whenever the artefact under review is an
instrument, i.e. its output is a number someone will publish** — and instruct the reviewer
to check the artefact against the world, not merely against the brief, because a
brief-conformance review structurally cannot find a defect inherited from the brief.

**Status:** validated — two datapoints, second one verified independently at the source by
the controller before any fix was made.

**Rests on:** `docs/superpowers/specs/2026-08-23-hidden-information-eval-design.md` § arm
symmetry; [[F-10]], which is the same failure mode caught one stage earlier, by
reconnaissance rather than by review.

## W-10 — A real number attached to the wrong subject — five instances in one session

**Valid:** invariant

**Observed:** 2026-08-25, across a single day of eval work. Five separate incidents, all the
same shape, three of them mine and two in code I commissioned.

| # | The number | What it actually described | How it was caught |
|---|---|---|---|
| 1 | `run()` at `surface_lib.py:427` | a line offset in a **JSON buffer**, not the source file (203 lines) | the implementer verified the citation against source |
| 2 | file length ≤30 at 27 files / 11 of 12 sites *(earlier round)* | a working tree with a **live mutation** applied | the implementer re-applied the mutation to prove attribution |
| 3 | `FAIL(no-findings-block)` ×2 | a **CLI error envelope**, not a model answer — the run never happened | reading the response body after the score looked wrong |
| 4 | `main-cs/hidden-cs.log: VERDICT PASS` | the last line of an **alphabetical concatenation**, not the file that changed | `runs: 2` makes a third verdict *impossible*, not merely odd |
| 5 | "your live profiles are fine" | a streamed **`init` envelope**, which proves the CLI started, not that it succeeded | probing again and reading the terminal `result` object |

**Pattern:** the failure is never a wrong number. In all five the number was real, correctly
computed, and correctly reported — *about something other than what the claim was about*.
That is why none of them errored, and why four of the five looked entirely plausible.

**What actually catches it**, in the order these were caught:

1. **A domain constraint that makes the value impossible, not merely surprising.** `runs: 2`
   means a third verdict cannot exist. Impossible beats implausible as a trigger, because
   implausible values get rationalised and impossible ones do not.
2. **Asking what surface the number came from**, separately from whether it looks right. A
   `symbols` response carries source ranges *and* is stored in a buffer with its own line
   numbers; a Claude Code run emits an `init` envelope *and* a `result` envelope. Both
   coordinate systems are present, formatted alike, and unlabelled.
3. **Re-deriving through a second, independent path.** Every substantive pilot claim was
   computed by `gates.py` reading the logs directly, which is why incident 4 cost nothing.

**Counterfactual, incident 3 specifically:** `hidden-cs` scored f1 0.8138 and
`hidden-native` would have entered the aggregate as 0.0 twice. Gate 4 asks for a ≥ 0.10
separation; it would have read **0.82** and passed triumphantly on an arm that never made an
API call. Gate 1 would have failed, and gate 1's documented remedy is *tune the fixture* —
i.e. make the task easier because the arm "found nothing". The eval would have published a
codescout landslide manufactured entirely by an expired OAuth token.

**The structural defence, and it is cheap:** route every number a decision rests on through
**one audited instrument**, and never read one off whatever surface is nearest. That is what
`gates.py` is for, and it is why incident 4 was an anecdote rather than a retraction.

**Confirming data points:** five, above, in one session. Incidents 1 and 4 were mine;
2 and 3 were in commissioned code; 5 was my own reporting of someone else's system.

**Promote-when:** already met — five datapoints in a day. **Promote to codescout memory
`eval-design`** as a named class, and to the reconnaissance skill's Phase 2, whose current
wording ("compare plan to reality") does not cover *"the number is right and the subject is
wrong"*. The check to add: **name the surface the number came from before you use it.**

**Status:** validated — five independent instances, each verified at the bytes.

**Rests on:** [[T-27]] (incident 1), [[W-5]] (incident 2),
`docs/issues/archive/2026-08-25-checker-scores-cli-error-as-content-failure.md` (incident 3).

## W-11 — The pilot earned its cost by invalidating the fixture, not by measuring anything

**Valid:** invariant

**Observed:** 2026-08-25. A $2.88 pilot at N=2 returned twelve runs that all scored
**identically to four decimal places** — f1 `0.8000`, recall `0.6667`, precision `1.0000`,
band A `1.00`, band B `1.00`, band C `0.00`.

**What it found.** Not a result — a fixture that could not produce one. All four band-C
truth sites hardcoded the same number with **no reference to `TAX_RATE`**: a `LEVY` constant
described as *"the customs levy"*, a `duty_multiplier` described as *"import duty"*, a bare
`8.25 / 100`, and `825  # basis points`. The task asks what must change when **the sales
tax rate** changes. A customs levy that happens to equal 8.25% is not the sales tax, and
nothing in the tree said otherwise — so including band C required guessing from a numeric
coincidence, and excluding it was correct reasoning. The fixture had a hard ceiling at
recall **8/12 = 0.6667**, which is exactly what every run scored.

**The agents were right and the answer key was wrong, and the unanimity is the evidence.**
Several runs named `apply_levy` / `surcharge_pct` / `duty_multiplier` in their reasoning and
then declined to list them. That is an agent that looked and judged, not one that failed to
look — and twelve of twelve agreeing across two different toolsets is not a coincidence, it
is a measurement of the fixture.

**Counterfactual.** Phase 2 was budgeted at 96 runs. Run on that fixture it would have spent
~$34 to produce a table showing codescout and native tools performing identically, with a
plausible-sounding explanation (*"the hard band is hard for both"*) and no error anywhere.
The pilot's four gates cost $2.88 and returned **gate 4 FAIL / gate 2 FAIL**, which is what
sent someone to look at the fixture code.

**The trap in the obvious fix.** Deleting band C makes every run score recall 8/8,
precision 8/8, **f1 1.000** — which breaches gate 1's *upper* bound and makes the task
trivial. The eval would then fail gate 1 instead of gate 4, for the mirror reason. There was
no discriminating middle: band A+B trivial, band C impossible. The repair had to make band C
**findable by evidence** (a derivation chain rooted in the constant), not remove it.

**And the repair surfaced a second, older defect.** Closing the chain's imports tripped a
predicate that had been mis-calibrated **since round 4** — `has an import` sat at 0.351 on
filler against 0.632 on planted, an asserted parity that had never been measured, and the
generator's own docstring stated it as fact. It could not trip a verdict alone at lift 2.13,
so it hid by appearing in *all fifteen* worst-surviving combinations. Fixing it took the
worst asserted predicate from **8.05 → 5.63** and the null control from p = 0.292 → **0.683**.

**Pattern:** a pilot's job is to falsify the instrument, not to preview the result. Budget
it, gate it, and treat a *uniform* result as the loudest possible signal — twelve identical
scores is not weak evidence of no effect, it is strong evidence of no measurement.

**Confirming data points:**

1. This session — a $2.88 pilot prevented a ~$34 run on an instrument with a hard ceiling,
   and incidentally exposed a leak channel that had been live for three rounds.

**Promote-when:** a second pilot catches an instrument defect that a full run would have
published. At two datapoints, promote to codescout memory `eval-design` as: *before scaling
any eval, run it small and check whether the results can vary at all.*

**Status:** validated — single datapoint, fixture defect confirmed at the bytes, repaired,
and re-piloted with band C at 1.00.

**Rests on:** R-31 / R-32 in
`.superpowers/sdd/2026-08-23-hidden-information-eval/progress.md`; [[W-10]], whose incident 3
is the same pilot's other finding.

## F-11 — Sharing one OAuth credential across Claude Code profiles broke three of them mid-pilot

**Valid:** invariant

**Observed:** 2026-08-25, six runs into the first Task 5 pilot. `hidden-native` returned
`is_error: true`, `terminal_reason: api_error`, `duration_api_ms: 0`, zero tokens, zero
tools, `$0`, carrying *"Failed to authenticate: OAuth session expired and could not be
refreshed"*.

**Expected:** two eval profiles cloned from an existing working profile, each symlinking
`.credentials.json` to a shared source, would both authenticate.

**Got:** the first profile to run authenticated; every other holder of that credential —
including the shared source itself — could not.

**Mechanism, established at the bytes.** An OAuth **refresh rotates the refresh token**, and
Claude Code writes the new credential by **replacing the symlink with a regular file inside
that profile**. So the rotation is captured by whichever profile refreshed first, and every
other holder is left with a consumed token. Confirmed by `expiresAt`: the profile that ran
first held a token valid for eight more hours; the shared source and the second profile both
sat at epoch 0. Confirmed again in the other direction later — when the shared token was
still valid, no refresh occurred, the symlinks **survived**, and both profiles worked.

**Severity:** high — it broke `~/.claude-kat`, a profile in daily use, and it did so
silently. Nothing announced the rotation; the next user of that profile simply could not
authenticate.

**Repair:** the freshly-refreshed credential was written back into the shared source, and
both affected profiles verified authenticating afterwards. Expired state kept at
`/tmp/claude-kat-creds-expired.bak`.

**Workaround, now automated.** `scenarios/hidden-info/run_pilot.sh` gained a `sync_creds`
guard that runs after every arm: if a profile's credential materialised into a regular file,
it validates the new token, pushes it back to the shared source, and restores the symlink.
That is the manual repair above, made idempotent, so a rotation can never be stranded.

**The real lesson is not about credentials.** Two claims were made too early during the
diagnosis, and both were corrected the same session:

1. *"Your live profiles are fine"* — asserted after reading a streamed `init` envelope with
   a full tool list, without checking the terminal `result` object. One of the three was
   broken at that moment. **A streamed envelope proves the CLI started, never that it
   succeeded.**
2. The first probe loop reported all three live profiles *"unparseable"* because it assumed
   one JSON object where `--output-format json` had emitted a stream. Two of the three were
   healthy. **A parser failure is not a subject failure** — and reporting it as one nearly
   raised a false alarm about working infrastructure.

Both are the [[W-10]] class: a real signal, read as evidence about the wrong subject.

**Status:** mitigated — the mechanism is understood, the damage is repaired and verified,
and the guard prevents recurrence in this harness. Not `fixed`, because nothing prevents the
next person from symlinking a credential into a new profile; the structural fix is a
separate credential per profile, or an API key.

**Rests on:** R-30 in `.superpowers/sdd/2026-08-23-hidden-information-eval/progress.md`.

## W-12 — An anchored retrieval eval ceilings by construction — naming the target supplies the doubt that real failures lack

**Valid:** invariant

**Observed:** 2026-08-25. Two independent evidence sweeps (curated trackers; raw
session transcripts) run to answer "why can the hidden-info eval not produce a
failing arm?"

**Pattern:** Before building a retrieval eval, ask whether the prompt **names the
target**. An *anchored* task — one whose prompt states what to find — hands the
agent the one thing missing in every real failure: the doubt that sends it looking.
Anchored evals therefore measure a capability that was never the bottleneck, and
they saturate.

**Counterfactual — this exact harness was already built on this machine and already
saturated.** `docs/trackers/bistriceanu/raw/WhatsApp Chat with Bistriceanu.txt:713-899`:
the operator asked for a needle-in-a-haystack harness; a needle was drawn by
`/dev/urandom` from a 60-candidate pool, the key sealed, five blind subagents given
byte-identical prompts. Result at `WA:891-899` — **5/5 exact** on name + value +
`path:line`, **0** line-number drift under a rule where off-by-one counts as a miss,
**5/5 said CERTAIN and 5/5 were right**, mean **5.8 tool calls**, ~30 s each. The
conclusion recorded at the time (`WA:855-858`, `WA:900-906`): *"I never failed to
find `path_security.rs`. I never looked… **Retrieval is healthy; triggering
retrieval is what's broken.** … **Suspicion is the scarce resource, not
capability.**"* We spent $8.28 rediscovering it.

**Confirming data points:**

1. **The failure population is the inverse of what we score.** 51 failures
   reconstructed with file+line citations across four sessions on two machines:
   **36 confident wrong answers** vs **4 pure misses** (~9:1); 8 of the 36 are
   confident wrong *absence* claims. Our eval's primary signal is **recall**, which
   measures misses — the 8% case.
2. **Self-review detects nothing.** Of the 51: 22 caught by execution, 12 by the
   human operator, 10 by an external reviewer, 6 by a subagent, 1 by CI, and
   **0 by unaided self-review**. Corroborated by three independent in-transcript
   tallies made at the time (`WA:958-968`: 10 corrections — 4 user pushback,
   6 execution, 0 self-review; `WA:671`: 6 errors, 0 self-review).
   Mechanism, `WA:970-978`: *"Re-reading my own text produces no new signal — it
   reads exactly as convincing the second time, because it was generated from the
   same wrong belief that would have to be the thing under suspicion."*
3. **Anchoring explains W-11's "no discriminating middle" mechanically.**
   Anchored + findable = ceiling (round 7: recall 1.0000 in 11 of 12 runs, band-C
   recall 1.0000 in 11 of 12). Anchored + unfindable = floor (pre-repair band C:
   twelve agents unanimously and *correctly* declined). There is no middle inside
   the anchored paradigm, so no negative test can be designed there.
4. **The unreachable needle is not a retrieval problem at all.** `FB:10257`
   (`KT-13`): the missed fact was **fifteen lines below the constant the agent was
   editing, in a file it had already read** — *"I was reading for the FK ordering
   claim I'd come to fix, not for what the file already knew."* No retrieval eval
   can produce this.
5. **The repo diagnosed the same shape once before, in a different eval.** The
   reconnaissance eval's P5 traps saturated at 100% for every arm *"because the
   production artifact was named in the prompt and one small file away"*, with a
   follow-up filed to *"redesign instrument traps with the artifact unnamed and the
   check costly"*. The lesson did not transfer because it was filed against that
   instrument rather than against eval design generally — which is why this entry
   is promoted to `eval-design`, not to the hidden-info stream.

**The one intervention with a measured positive effect** was a scoring change, not a
rule or a hook: requiring each claim to carry `CERTAIN`/`UNCERTAIN` **plus its
justification**. All five searchers then double-verified through independent means,
unprompted (`WA:872-876`). Rationale, `WA:984`: *"it doesn't depend on suspecting
anything — it attaches the check to the act of asserting, which is the one moment
the failure is guaranteed to be present."* Contrast `R-104`, where knowing the rule
prevented nothing: three instances were committed in a session that had read the
entry in full and quoted it back.

**Impact:** high — it invalidates the design of the current eval, and it is the
reason the $40.14 phase-2 run was not spent.

**Promote-when:** fired immediately. Promoted to memory `eval-design` on
2026-08-25 as the anchoring rule; this entry keeps the evidence.

**Status:** promoted-to-permanent-docs

## F-12 — gates.py reported means only, hiding a three-valued instrument for seven rounds

**Valid:** conditional — gates.py prints per-run value sets and a range check

**Observed:** 2026-08-25, re-reading the round-7 pilot logs for *range* rather than
*validity*.

**When:** After seven rounds of fixture work and $8.28, while asking why gate 4
would not separate.

**Expected:** F1 is a continuous measure; a 0.10 threshold is a reasonable bar that
more runs could clear.

**Got (measured):** across **all 12 round-7 runs** there are exactly **three**
distinct outcomes — `f1 0.8000 / P 0.6667 / R 1.0` (7 runs, 12 truths + 6 extras),
`0.8276 / 0.7059 / 1.0` (4 runs, 12 + 5), `0.8462 / 0.7857 / 0.9167` (1 run,
11 + 3). Total observed F1 range **0.046**. Since every per-run value lies in
[0.8000, 0.8462], any two arm means lie there too, so `|Δmean| ≤ 0.046` — **gate 4's
`ΔF1 ≥ 0.10` is unreachable by construction**, at any sample size, while runs land
in this value set. The instrument's resolution is one borderline symbol; its full
dynamic range is about three.

**Probable cause:** `gates.py::summarise` computes plain means for f1 / recall /
precision / bands and reports no dispersion — no min/max, no std, no distinct-value
count. A three-valued output is invisible in a table of means. Worse, the diversity
check we *did* have pointed the other way: round 7 reported **12 distinct answers**,
which reads as healthy variance. Twelve distinct answers collapsed onto three
distinct scores; the metric was measuring the wrong layer.

**Severity:** high — it is the direct cause of seven rounds spent tuning a fixture
whose gate could not be reached, and it would have licensed the $40.14 phase-2 run.

**Fix:** `gates.py` now prints per-arm **tool calls** and the **per-run value set**
alongside the mean, and gate 4 reports the observed range so an unreachable
threshold is visible in the same table that evaluates it.

**Generalisation (the reusable half):** a pilot has two jobs and we assigned it one.
*Is the number real* (validity) and *can the number move* (range) are independent
questions. Range is the cheaper one — it needs no ground truth, only the spread of
what came back — and checking it first would have preceded the leak-guard sweep, the
null control and the mutation harness, all of which were built on top.

**Status:** fixed-verified

## W-13 — The arms separated on a metric we already logged and never reported

**Valid:** dated 2026-08-25

**Observed:** 2026-08-25, after establishing in F-12 that gate 4's F1 threshold was
unreachable. Rather than accept the tie, checked what else the round-7 facts blocks
already carried.

**Pattern:** When a headline metric ties, enumerate every field the harness already
records before concluding "no effect". Reporting is a choice made once, at
instrument-build time, and it is rarely revisited; the discriminating dimension may
already be on disk, unpaid-for.

**Counterfactual:** the round-7 facts blocks carry `calls`, `prompt`, `output`,
`cost_usd` and the full `tool_args` sequence. `gates.py` surfaced only `cost`. On
the metric it *did* report, the two main arms **interleave**:

| arm | F1 (per run) | recall | tool calls (per run) | cost / 2 runs |
|---|---|---|---|---|
| `hidden-cs` | 0.8000, 0.8276 | **1.0000** | **49, 43** | $0.7722 |
| `hidden-native` | 0.8276, 0.8462 | 0.9584 | **12, 20** | $0.6020 |

F1 `{0.800, 0.828}` against `{0.828, 0.846}` overlap — which is exactly why gate 4
reads ΔF1 0.0231. Tool calls `{49, 43}` against `{12, 20}` **do not overlap at
all**: ~2.9×, with cost 28% higher.

**It is not a setup tax.** Tool-name census across both `hidden-cs` runs (92 calls):
41 `references`, 33 `symbols`, 13 `grep`, 3 `tree`, 2 `ToolSearch` — schema loading
and orientation are 5 of 92. Both `hidden-native` runs (32 calls): 23 `Bash`,
9 `Read`. The difference is behavioural: **codescout's tools induce per-symbol graph
walking; the shell induces batch sweeps.**

**Read it charitably — it is a characterisation, not a verdict.** `hidden-cs` spent
2.9× the calls and 28% more money and got the one truth site `hidden-native` missed
(recall 1.0000 vs 0.9584). Exhaustive versus fast-approximate is a real and useful
description of the two toolchains, and it is the first substantive thing this eval
has produced.

**Limits, stated because n is small:** n=2 per arm. The call-count ranges are
disjoint and the ratio is large relative to spread, but two runs cannot establish a
distribution. The recall difference is a single site in a single run. And by the
operator's own stated preference the call cost may not matter at all —
`WA:213`: *"I do not care if you take longer to come back to me with answers. I
would rather have correct answers and findings than inaccurate and fast answers."*

**Impact:** med — it does not rescue the eval's design (see W-12), but it converts a
"no effect" reading into a measured behavioural difference at zero additional spend,
and it is the change that made `gates.py` report calls.

**Promote-when:** a second eval in this repo ties on its headline metric while a
logged-but-unreported field separates. At two datapoints, promote to `eval-design`
as "enumerate the recorded fields before accepting a null".

**REPLICATED AT n=4, WITH A SMALLER EFFECT (round 8, 2026-08-25).** Round 8 ran the
same two arms under the new UNCERTAIN contract (F-14), so f1 is not comparable across
rounds -- but the call counts are the same measurement either way, and the DIRECTION
held while the MAGNITUDE did not:

| round | cs calls | native calls | ratio of means |
|---|---|---|---|
| 7 | 49, 43 | 12, 20 | 2.9x |
| 8 | 54, 33 | 31, 27 | 1.5x |

Pooled across both rounds the ranges are still disjoint -- cs [33, 54] against native
[12, 31] -- but by **two calls**, not the clean gap the n=2 reading suggested. Ratio of
pooled means 44.75 / 22.5 = 1.99x. Native's call count roughly doubled between rounds,
which the longer prompt plausibly explains and which no measurement here isolates.

The honest statement is now: cs uses more tool calls than native for the same score, in
both rounds, at a ratio somewhere between 1.5x and 2.9x. Not: 2.9x.

**Status:** validated

## F-13 — A bare `pytest` in prompt-engineering collects zero scenario tests, so every "repo baseline green" was silent about the eval code

**Valid:** conditional — pyproject's `testpaths` stops excluding `scenarios/`

**Observed:** 2026-08-25, reconciling a test count that did not add up. Added 9 tests
to `scenarios/hidden-info/test_gates.py`; the repo total moved by 7. The arithmetic
was the tell.

**Expected:** the repo suite covers the scenario code, so "401 passed / 7 deselected"
is evidence the hidden-info checker and gate evaluator are green.

**Got (measured):** `pyproject.toml:36` sets `testpaths = ["tests"]`. A bare
`pytest` collected **408 of 408 node ids under `tests/`, and 0 under `scenarios/`** —
`grep -c '^scenarios/'` on the collect-only output returns zero. The 9 added tests
were not in that number at all, and the ±7 that made me look was unrelated drift in
`tests/`. Every "repo baseline green" figure recorded in this work stream is true,
and is a statement about a different body of code than the one being changed.

**Probable cause:** `testpaths` was set for the harness package before the scenario
suites existed; scenarios are run explicitly by their own tooling. Nothing warns.

**Severity:** med — no wrong code shipped from it, because the scenario suites were
also run directly every round. But it is a standing licence to believe a green number
about the wrong subject, in a repo whose entire job is measurement, and the number is
quoted in commit messages and reports.

**Workaround / correct invocation:**
`\.venv/bin/python -m pytest scenarios/hidden-info/` (300 passed at
`prompt-engineering:cea3fc4`). The `.venv` matters too: a bare `python3 -m pytest`
fails collection with `ModuleNotFoundError: No module named 'prompt_tdd'`, which at
least fails loudly.

**Fix idea:** add `scenarios` to `testpaths`, or state in `CLAUDE.md` that the repo
run excludes them. Not done here — changing collection scope mid-stream would move
every baseline figure at once, which is a change to make deliberately and alone.

**This is the T-13 shape.** *"Every signal was true of 25 lines"* — a trimmed
`cargo test` tail whose `test result: ok` was real and described a fraction of the
run. Here the signal is true of `tests/`. Same defect class, different substrate:
a count is only evidence once you know its denominator.

**Status:** mitigated

## F-14 — Offering a hedge channel made every run add a false positive it had previously excluded

**Valid:** dated 2026-08-25

**Observed:** round 8, 2026-08-25. 4 runs (`hidden-cs` ×2, `hidden-native` ×2),
$1.77, checker `prompt-engineering:cea3fc4`, binary `4deef93d`, fixture
`513cbba30d360c42` — identical to round 7 in everything but the prompt's new
`## UNCERTAIN` contract.

**Expected:** the calibration block is a read-only channel. Agents already carry 4–7
false positives per run; asking which ones they doubt should label a subset of the
findings they would have made anyway.

**Got:** it changed what they found. `tests/fixtures/rates.py:SAMPLE_RATE` — a
declared decoy — appears **0 times in all four round-7 runs** and in **all four
round-8 runs**. `n_found` rose in every arm: cs 18/17 → 19/19, native 17/14 → 17/15.
Precision fell (cs 0.6863 → 0.6316, native 0.7458 → 0.7196) and so did f1
(0.8138 → 0.7742, 0.8369 → 0.8212).

**And the hedge is nearly circular.** Of the three runs that hedged anything, all
three hedged **exactly one item, and it was the same item every time** —
`SAMPLE_RATE`, the one that did not exist in round 7. `hedge_precision` is 1.0 in
3/3, but that number is not the agent identifying a pre-existing wrong finding: it
**added** a finding *because* the channel existed, then labelled the thing it added.

**What did NOT get hedged is the real answer to the question this run was for.** The
pre-existing false positives — `src/pricing/basis.py:RATE_BASIS_POINTS`,
`src/intl/customs.py:LEVY_MULTIPLIER`, and the four band-C callers
(`international_total`, `describe_duty`, `surcharge_label`,
`rate_basis_points_label`) — drew **zero** doubt across all four runs.
`hedge_recall` is 0.1429 / 0.1429 / 0.0 / 0.25.

So the precision loss this eval measures is **entirely confident**. The extras are
convictions, not hedges — the same 36-to-4 shape the transcript corpus shows
(W-12), reproduced inside the instrument. That question is answered, and the answer
did not need the channel to be free of side effects to be legible.

**Probable cause:** an inclusion threshold is not fixed. "List it and mark it
uncertain" is cheaper than "decide", so a channel for expressing doubt lowers the bar
for listing. The instruction says UNCERTAIN is a subset of FINDINGS; it does not say
that being able to hedge is not a reason to include.

**Severity:** med — no wrong conclusion published, and the run answered its question.
But it moved the f1 baseline, so round 8 is **not comparable to round 7 on f1 or
precision**, and any future arm carrying this contract inherits the shift.

**Fix options, not yet chosen (this is the user's call):**

1. Keep as is, and treat round 8 as the new baseline. Cost: one round of history.
2. Add one clause — *"being able to mark something uncertain is not a reason to list
   it; list only what you would list without this section"* — and re-measure. Cost:
   a 4-run pilot, and it is prompt-tuning by guess.
3. Revert the contract on the main arms and keep it for the un-anchored eval, where
   confident wrongness is the thing under test rather than a side effect.

**Status:** open

**Generalisation:** a measurement channel added to the *subject's own output* is not
free — it is an intervention. This one was reasoned from a measured result (five
searchers double-verified unprompted when asked to declare confidence) and still
changed behaviour in a direction nobody predicted. Pilot any contract change on the
metric it is NOT supposed to move.

## W-14 — Reading the real dependency chain before writing the spec turned an unbuildable trap into a buildable one

**Valid:** dated 2026-08-25

**Observed:** 2026-08-25, immediately before writing
`docs/superpowers/specs/2026-08-25-unanchored-blast-radius-eval-design.md`.

**Pattern:** When a spec's central mechanism rests on a code structure, read the
structure in the same session, before the spec — not the design conversation's memory
of it. A design can be internally coherent and rest on a graph that does not exist.

**Counterfactual — the spec would have specified an unbuildable trap.** I had asserted,
in two consecutive messages, that `duty_multiplier` has **four** dependents, and had
designed the trap as *"two fixes exist, one of them edits the shared function."* The
bug was to sit **downstream**, in `apply_levy`. Reading `gen_fixture.py:1344-1358`
before writing showed both halves were wrong:

- `duty_multiplier` has **two** direct consumers (`LEVY_MULTIPLIER` and
  `describe_duty`), not four. The four I had quoted were callers of *different* band-C
  functions — a real number about the wrong subject, the W-10 shape again.
- `apply_levy` is `return amount * LEVY_MULTIPLIER`. There is nothing upstream of it to
  fix, so *"two fixes, one shared"* has no second fix. The trap could not fire.

The repair was structural, not numeric: **put the defect INSIDE the shared function.**
Checking the dependents then stops being optional cleverness and becomes what doing the
job correctly requires — which is a strictly better design, and it was only reachable
by reading the code.

Without the scout, the spec would have shipped with a premise that has no
implementation, and Task 1 would have discovered it — after the spec had been reviewed
and a plan written on top of it.

**Confirming data points:**

1. This entry.
2. `F-12` (same session): the round-7 support was readable from logs that already
   existed; nobody looked, for seven rounds.
3. `W-10` (same stream): five instances of a real number attached to the wrong subject
   in one day.

**Impact:** high — it changed the trap's mechanism, not its wording, and it happened
before any review cost was spent on the wrong version.

**Promote-when:** a second spec in this repo is corrected by a pre-write scout of the
structure it rests on. At two datapoints, promote to the reconnaissance skill as
"before writing a spec, read every structure the spec's mechanism depends on."

**Status:** validated

## F-15 — The new eval's dependent set favoured codescout 2-to-1 before a single run

**Valid:** dated 2026-08-25

**Observed:** 2026-08-25, first draft of the blast-radius spec, §4.

**When:** After the design was agreed and before the plan was written.

**Expected:** six reference forms chosen so that no single lexical pattern enumerates
the dependent set — a difficulty property.

**Got:** the six forms were *also* an asymmetry. Two of them (aliased call site,
package re-export) are found by LSP `references` and missed by a lexical grep. Exactly
**one** (a `getattr`-resolved dict string) was missed by LSP and found by grep. So the
instrument offered codescout **two** available points against native's **one**, by
construction, before any measurement was taken.

**Probable cause:** I enumerated the forms by asking *"what would a grep miss?"* — a
question that silently frames codescout as the subject and native as the baseline.
The mirror question, *"what would `references` miss?"*, was asked once and answered
once. One row is not a category; it is an afterthought.

**How it was caught:** the user asked *"maybe we can add even more, 6?"* — a question
about **count**. Re-deriving the count forced a re-derivation of the **composition**,
which is where the asymmetry was. Nothing in my own review had looked at the balance,
and the spec had already been written and committed.

**Severity:** high — the eval would have produced a codescout-favouring result that
looked measured. It would have passed every gate in the spec, because no gate checks
the instrument's own bias, and there is no downstream check that would have caught it:
the leak sweep looks for oracles, not for asymmetry.

**Fix:** rebalanced to **2 / 2 / 2** — two forms both toolchains find, two only LSP
finds, two only a lexical sweep finds. The second LSP-invisible form uses a different
mechanism (config-file dispatch, not a second `getattr`), so one implementation slip
does not decide the codescout-loses case. The module-attribute form was dropped to make
room, since rows 1–2 already cover "both find it".

**Status:** fixed-verified — spec §4 rewritten before the plan was written; the
reasoning is recorded in the spec's own review-checklist section so a reversal is
informed.

**Generalisation (promoted to memory `eval-design`):** an eval in which the tool under
test can only win is an advertisement, not a measurement. Enumerate the ways the
subject **loses** with the same effort spent on the ways it wins, and count them. If
the counts differ, the instrument is biased before the first run and no amount of
sample size fixes it.

## F-16 — The plan put the leak sweep in the wrong file, so following it literally would have deleted the anti-oracle guard and reported green

**Valid:** dated 2026-08-25

**Observed:** 2026-08-25, pre-dispatch reconnaissance for Task 1 of
`docs/superpowers/plans/2026-08-25-unanchored-blast-radius-eval.md`
(subagent-driven execution, before any subagent ran).

**When:** Reading Task 2 Step 5 while composing the Task 1 dispatch.

**Expected (plan):** Task 2 Step 5 — *"The copied `gen_fixture.py` already carries the
75-predicate / 140,675-combination sweep. Add six predicates naming the new forms."*
Task 2 Step 6 then runs `pytest scenarios/blast-radius/test_fixture.py -k leak -v -s`
and expects the worst surviving combination to print.

**Got (scouted reality):** none of the sweep is in `gen_fixture.py`. In
`prompt-engineering`, `STRUCTURAL_PREDICATES` is `scenarios/hidden-info/test_fixture.py:1028`;
`_score` is `:1311`, `_is_leak` `:1329`, `test_every_structural_predicate_is_a_live_instrument`
`:1382`, `test_no_structural_predicate_isolates_the_planted_set` `:1409`, and
`test_null_control_detects_an_injected_channel` `:1553`. Every one of them is in the
**test** file. `gen_fixture.py` mentions lift only in prose comments.

The compounding half: Task 1 Step 2 **creates** `scenarios/blast-radius/test_fixture.py`
from scratch (it is not a copy of the 1619-line hidden-info original). So the sweep, the
null control, the live-instrument guard and the site/filler disjointness check would all
have been absent from the new scenario entirely — and `-k leak` would have selected zero
tests and exited 0.

**Probable cause:** the plan was written from the spec plus a memory of where the sweep
"lives", never from the file. The two files sit next to each other and are named for the
same fixture, which is exactly the pairing that makes a location slip invisible on re-read.

**Workaround / ruling:** Task 2's scope grows explicitly — port the sweep machinery from
`hidden-info/test_fixture.py` into `blast-radius/test_fixture.py` and **retarget its truth
set** from the 12 tax sites to `gen_fixture.DEPENDENTS` (six). The retarget is real work,
not a copy: `_score`'s `n_sites`, the base rate, and the `LIFT_BAR` / `RECALL_FLOOR`
calibration are all keyed to the old truth-set size.

**Severity:** high — the failure mode is the silent-zero shape this very plan warns about
in three separate places (the missing exec bit reporting a clean `0/N`; `count: 0` against
233 files; `prompt-surface-measurement-session-log:F-13`'s bare-`pytest`-collects-nothing).
A green `-k leak` with zero tests selected is character-identical to a clean sweep, and the
anti-oracle guard is the single thing standing between this eval and a fixture where one
`grep getattr(` is the whole answer.

**Status:** fixed-verified — ruled and carried into the ledger before Task 1 was dispatched;
Task 2's dispatch will carry the retarget as a requirement.

**Fix idea / Pointer:** SDD ledger
`.superpowers/sdd/2026-08-25-unanchored-blast-radius-eval/progress.md`, pre-flight row 1,
`Ruling: GAP-A`. Pairs with [[F-17]] and [[F-18]] — same scout, same root cause.

## F-17 — New fixture files were invisible to the reserved-path set, so generated filler could silently overwrite a row of the instrument

**Valid:** dated 2026-08-25

**Observed:** 2026-08-25, same pre-dispatch scout as [[F-16]], while checking whether
Task 1's four new dependent files could collide with anything.

**When:** Reading `gen_fixture.py`'s emission order before composing the Task 1 dispatch.

**Expected (plan):** Task 1 Step 8 writes four new files into the generated tree
(`src/intl/manifest.py`, `src/orders/crossborder.py`, `src/pricing/registry.py`,
`src/exports/customs_feed.py`, plus `src/intl/__init__.py` and `pricing.toml`) and says
nothing further about them.

**Got (scouted reality):** `gen_fixture.py:515` defines `_planted_paths()`, which returns
`{SITES} | {DECOYS} | {"src/intl/checkout.py", "src/pricing/quotes.py"}`. That set is
consumed as `protected` / `reserved` by filler-module planning at `:1212`, `:1217`, `:1221`
and `:1231`, and `_module_filename(rng, used, reserved, rel_dir)` at `:586` skips a
candidate only when `rel_path not in reserved` (`:599`). `build()` at `:1167` runs
`_emit_planted` **before** `_emit_filler_modules`. So a filler module that draws a
colliding path overwrites the planted file — after it was written, with no error, no
warning, and a byte-stable tree either way.

The new paths are exactly the collision-prone kind: `manifest`, `registry`, `dispatch`,
`digest`, `window`, `labels`, `counters`, `cache` are all ordinary nouns, and the filler
vocabulary is a noun pool.

**Probable cause:** the plan reasoned about the fixture as a set of files to write, not as
a two-pass generator with a reservation protocol between the passes. The protocol is only
visible if you read `build()` — `_write` itself is an unguarded overwrite.

**Workaround / ruling:** Task 1 registers all six new planted paths in `_planted_paths()`;
Task 2 registers its eight anti-oracle filler files there too. Carried in the Task 1
dispatch as ruling `GAP-C`, marked not-optional.

**Severity:** high — a dropped dependent does not fail anything. `test_all_six_dependents_
exist_with_their_declared_forms` would catch a total loss, but the plan's own scoring reads
L2 out of the trace against `DEPENDENTS`, so a row that exists in the constant and not on
disk scores as "the agent never reached it" for every run in both arms. That is a
fabricated number, not a broken test, and it would have been indistinguishable from a real
result.

**Status:** fixed-verified — ruled before dispatch; Task 1 carries it.

**Fix idea / Pointer:** SDD ledger pre-flight row 10, `Ruling: GAP-C`. The general form is
worth keeping: **a generator with a reserve-then-emit protocol has two write surfaces, and
adding a file to one without the other is silent.** Same family as [[F-16]] — both are
"the plan described the artifact, not the machine that produces it".

## F-18 — The plan told the implementer to edit two constants and pass a flag that the file it names has none of

**Valid:** dated 2026-08-25

**Observed:** 2026-08-25, same pre-dispatch scout as [[F-16]] and [[F-17]].

**When:** Verifying Task 1 Steps 1 and 9 against `scenarios/hidden-info/gen_fixture.py`.

**Expected (plan):** Step 1 — *"In the copy, change the two output names only:
`TARBALL = "blast-fixture.tar.gz"`, `GROUND_TRUTH = "blast-ground-truth.json"`,
`MARKER_SUFFIX = ".blast-radius-generated"`."* Step 9 — regenerate twice with
`gen_fixture.py --out /tmp/blast-a`.

**Got (scouted reality):** `gen_fixture.py` defines `MARKER_SUFFIX` at `:103` and neither
of the other two, anywhere. `FIXTURE_TGZ` and `GROUND_TRUTH` live in the sibling
`scenarios/hidden-info/gen.py:76-77`, which Task **6** copies. And the CLI is positional:
`gen_fixture.py:1427` is `ap.add_argument("out_dir", type=Path)` — `--out` is an
`unrecognized arguments` error.

There is also a live consequence of the marker that Step 9 does not mention:
`build()` writes its marker as a **sibling** of the tree (`marker_path()` at `:508`) and
refuses to regenerate into a non-empty directory lacking one, so the two-tree diff needs
both markers cleaned up.

**Probable cause:** the same root cause as [[F-16]] — written from the design's vocabulary
rather than from the file. "The generator emits a tarball and a ground-truth JSON" is true
of the *scenario*; it is not true of `gen_fixture.py`.

**Workaround / ruling:** Task 1 changes `MARKER_SUFFIX` only; the tarball/ground-truth
rename moves to Task 6 where those constants actually exist. Step 9 uses the positional
form. Carried in the dispatch as rulings `GAP-B`.

**Severity:** med — an implementer hits both within a minute and the failure is loud
(`AttributeError` on a constant that isn't there; argparse rejecting `--out`). The cost is
one confused round-trip and the risk that an implementer *invents* the two constants to
make the step do something, which would then quietly diverge from Task 6's copy of `gen.py`.

**Status:** fixed-verified — ruled before dispatch.

**Fix idea / Pointer:** SDD ledger pre-flight row 9, `Ruling: GAP-B`. Cheapest general
guard: when a plan step says "change constant X in file Y", grep Y for X while writing the
plan. All three of [[F-16]], [[F-17]] and this entry would have been caught by one pass of
that habit at plan-writing time rather than at dispatch time.

## W-15 — Reading the generator instead of the plan's description of it caught three defects, one of which had no downstream gate at all

**Valid:** dated 2026-08-25

**Observed:** 2026-08-25, pre-dispatch reconnaissance for Task 1 of the un-anchored
blast-radius eval plan, subagent-driven mode. Scout cost: eight `run_command` greps and
four ranged reads of `scenarios/hidden-info/gen_fixture.py` — no subagent, no dispatch.

**Pattern:** Before dispatching the first implementer of a plan that *copies and edits an
existing generator*, read the generator's own machinery — not the symbols it exports, but
its **protocols**: what order it writes things in, what it reserves, what it refuses. Grep
for every constant and CLI flag the plan tells the implementer to change, in the file the
plan names.

**Counterfactual, per finding:**

- [[F-18]] (constants + `--out`): the implementer hits `AttributeError` and an argparse
  rejection in its first two minutes. Cost without the scout ≈ one clarification
  round-trip. **A downstream gate exists.**
- [[F-16]] (leak sweep in the wrong file): the implementer of Task 2 runs
  `pytest test_fixture.py -k leak`, sees `0 selected`, exit 0, and reports the step passed.
  The task review reads a diff in which nothing about the sweep appears — because nothing
  was written. Cost without the scout: the anti-oracle guard is absent from the eval, and
  the first indication is a pilot in which one `grep getattr(` would have been the whole
  answer. **The only gate is a human noticing a zero.**
- [[F-17]] (`_planted_paths()`): **no gate at any layer.** A filler collision overwrites a
  dependent silently; the tree stays byte-stable; `DEPENDENTS` still lists six; the checker
  reads L2 from the trace and scores the missing row as "the agent never reached it" in
  *both* arms, forever. It would have surfaced as a plausible number.

**Confirming data points:**
1. This session — three findings, one ungated, before any subagent ran.
2. [[W-14]] (2026-08-25) — reading the real dependency chain before writing this same
   spec turned an unbuildable trap into a buildable one.
3. `bug-fix-session-log:W-2` / `F-3` (2026-05-18) — pre-dispatch scout caught a
   plan citing a `RecoverableError.hint` field that does not exist.

**Impact:** high — the F-17 class is the one that matters. A defect with a downstream gate
costs a round-trip; a defect with no gate costs the validity of every number the instrument
later produces, and is discovered (if ever) long after the runs are paid for.

**Promote-when:** at a fourth datapoint, or at a second *ungated* one, promote to
`CLAUDE.md` / the reconnaissance skill as: **"Before the first dispatch of a plan that
copies an existing generator or harness, read that file's write-ordering and reservation
protocol — a plan describes the artifact, and the bugs live in the machine that produces
it."** Distinct from the existing type-shape rule (`bug-fix-session-log:W-2`), which is
about signatures; this one is about *protocols between passes*.

**Status:** validated — three findings, ruled and carried into the Task 1 dispatch before
any subagent ran.

## F-19 — The eval's entire guard apparatus is unenforced — three independent facts compound so that no gate can ever be attached

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, out-of-scope observation from the opus re-review of Task 2's fix
round 1 on the blast-radius eval. Surfaced while verifying that the round's new guards
could actually fail.

**When:** After a round whose entire deliverable was falsifiability — pinning every
calibration bar and giving the leak sweep a positive control.

**Expected:** That a suite of guards, once written and green, protects the fixture against
regression the way any test suite does.

**Got (three facts that compound, each measured):**

1. `prompt-engineering:pyproject.toml:36` sets `testpaths = ["tests"]`, so a bare `pytest`
   collects **zero** scenario tests — 408 of 408 node ids under `tests/`
   ([[F-13]], measured 2026-08-25).
2. The repo has **no `.github/` directory**, so nothing runs `scenarios/` in CI at all.
3. `scenarios/blast-radius/` deliberately carries a **permanently-failing** test
   (`test_every_dependent_changes_when_the_defect_is_fixed`, awaiting Task 3's `golden.py`;
   a second, `test_no_bar_sits_above_the_lift_ceiling`, was also red for a round). So even
   if someone attached a gate, **green is never the expected state** and the gate could not
   be written as "exit 0".

And separately, running the two scenario suites together fails: `pytest scenarios/` yields
**21 failures** — the whole of `blast-radius/test_fixture.py` — because
`gen_fixture` / `test_fixture` / `null_control` are ambiguous module names and each
scenario's `conftest.py` puts its own directory on `sys.path`. Measured 2026-08-26:
`21 failed, 301 passed in 16.74s`. Collection itself is clean (353 collected), so this is a
runtime resolution failure, not a collection error.

**Probable cause:** the scenario directories were built as *instruments run by hand during
an eval*, not as a test suite. Every convention that makes a suite enforceable — a
collectable path, a CI job, an all-green invariant, unambiguous module names — was
therefore never established, and each subsequent scenario inherited the gap.

**The shape worth naming:** the re-review put it precisely — *"every guard this round added
is enforced only when a human explicitly targets the directory and reads which tests are
red — the same 'a print no test reads' shape that I1 was filed against, one level up."*
Finding I1 was that `null_control.py` printed a drift detector no test consumed. The repair
was to make tests consume it. But the tests themselves are consumed by nobody.

**Severity:** high — not because anything is broken today, but because the entire
investment in this eval's trustworthiness is uncollected. The fixture's anti-oracle
guarantees, the 4/4/4 partition, the re-derived calibration and the falsifiability layer
are all real and all verified once, at the moment they were written. Nothing re-checks them
when the tree changes — and the tree has already changed three times under them
(188 → 206 → 250 `.py` files), each time silently invalidating any bar that was not
re-derived.

**Status:** open — deliberately not fixed inside the SDD run. It is repo-level work, and
folding it into a task whose acceptance criterion is a red test would have been incoherent.

**Fix idea / Pointer:** three separable pieces, in dependency order. (1) Disambiguate the
module names or load scenario modules by path rather than by `sys.path` insertion — this
one is a genuine bug and blocks the other two. (2) Replace the permanent-red convention
with a marker (`@pytest.mark.awaiting_task3`) and a deselect, so "all green" becomes
expressible. (3) Add a CI job over `scenarios/` once (1) and (2) hold. Pairs with [[F-13]],
which found the collection half of the same gap; this entry is what makes it consequential
rather than merely inconvenient.

## F-20 — Three rounds running, a commit asserted in prose what it measured otherwise in code — measured facts and derived claims have no connecting gate

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, diagnosed by the Task 2b implementer at the end of its second fix
round on the blast-radius eval, after the third consecutive review found the same shape.

**When:** Building a measurement instrument whose credibility rests on documented numbers —
calibration bars, worst-survivor tables, partition claims.

**The pattern, in its own words:** *"measured facts live in test output, derived claims live
in docstrings, nothing connects them."*

**Got — four instances, all in one task, none caught by a gate:**

1. `_is_leak`'s docstring said *"LIFT_BAR[3] sits ABOVE the lift ceiling of 34.33, so the
   k=3 clause cannot fire at all"* — in the documentation of the one function the entire
   guard turns on, after the task had moved the ceiling to 20.83 and the bar to 10.5. It
   told the next re-deriver the exact belief the task was commissioned to refute.
2. The generator's module docstring declared *"a MEASURED 2/2/2 partition"* while the test
   it named as its keeper asserted 4/4/4 — in a sentence whose own point was that a stale
   comment is what tests exist to prevent.
3. The liveness docstring argued for a `MAX_NEAR_DEGENERATE` population clause the same
   commit had deleted, contradicting an in-body comment forty lines below, so the function
   carried two mutually exclusive accounts of its own band.
4. The form-pair passage asserted *"every form-pair OR sits precisely ON the recall floor"*
   while the same commit's `EXCLUSIVE_AMONG_DEPENDENTS` comment, 150 lines away, recorded
   two forms at 9 of 12 and 6 of 12. Measured distribution over 15 pairs:
   `{4: 6, 6: 1, 7: 1, 8: 2, 9: 2, 10: 2, 12: 1}`. **This one originated in a controller
   ruling** — see the cost note below.

Also: five stale `n=6` prose sites survived a round *dedicated* to fixing seventeen of them,
two of which contradicted text that same commit had written; and `calls getattr` was
documented as "65 files, 4 of 6, the worst single predicate" when it was 86 files, 6 of 12,
and no longer the worst.

**Probable cause:** the numbers in these docstrings were true when written and are derived
from a tree that moves. This fixture's `.py` count went 188 → 206 → 250 across three tasks,
and every derived figure silently expired at each step. Nothing re-reads a docstring; the
tests that *could* falsify these claims compute their own values and never compare.

**Severity:** high — for an instrument, the documentation is part of the deliverable. Every
one of these told a future re-deriver something false about how the guard works, and the
one in `_is_leak` would have propagated the retired conclusion into the next calibration.
The reviews caught all four, but a review is a person reading, which is precisely the
enforcement model [[F-19]] shows is absent here.

**The controller-ruling instance is the sharpest.** Instance 4 entered the code because
*I* asserted it in a fix dispatch, confusing declared carriers with tree matches. The
implementer had no reason to re-measure a premise handed down as settled, so it wrote the
false claim into the instrument faithfully. **A controller ruling is a claim and needs the
same gate as code** — it should carry its measurement or be marked as unverified.

**Status:** open — the repair is proposed and deliberately deferred, not dropped.

**Fix idea / Pointer:** the implementer's own proposal, which it asked to have scoped
rather than bolted on: regex the survivor tables out of `_is_leak`'s and the family guard's
docstrings and compare them against a live sweep, the way `null-control-n12.txt` is now
parsed and compared against `NULL_P90` (that one *was* fixed this way and works). Deferred
in the SDD ledger to after the pilot, because the pilot may invalidate the design outright
— as round 7 did for the sibling eval — in which case those docstrings are rewritten
anyway. Pairs with [[F-19]]: this is the same enforcement gap one level down, where
[[F-19]] is *tests* nothing runs and this is *numbers* nothing checks.

## W-16 — Three dilution rounds converged without closing; one measurement of the shared cause moved it further than all three

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, blast-radius eval, Tasks 4c and 4d. A structural-leak guard kept
reporting the same high-recall channel at roughly the same magnitude after each repair.

**Pattern:** When targeted fixes converge without closing — the same magnitude, a *different*
instance each time — stop fixing instances and measure what the instances share. The
give-away is the shape of the series, not the size of the residual.

**The series.** Three dilution rounds against the high-recall tier: **3.49 → 3.36 → 3.30 →
3.30**. Each round diluted the predicate that had won, and each time a different triple won
next, at nearly the same lift. The implementer's own closing sentence was the diagnosis it
had not yet been authorised to act on: *"the underlying cause is structural, not any single
predicate."*

**The measurement, one command.** Functions per file, dependents vs everything else:

| | files | mean | median |
|---|---|---|---|
| the twelve dependents | 12 | **18.50** | **19.5** |
| everything else | 255 | 12.29 | **8.0** |

The planted files were **1.5× the mean and 2.4× the median** of the tree they hide in. Every
existence predicate in the sweep — *contains a raise*, *calls getattr*, *has an f-string*,
*has a return annotation*, *nests control flow* — is monotone in file size, so a
systematically larger truth set is **jointly enriched on all of them at once**. Diluting one
predicate simply moved the winner to a neighbour that the same size gap was still enriching.

**Counterfactual.** A fourth improvised dilution round was the obvious next step and would
have produced another ~0.05 with a fourth winning triple — the pattern the first three had
already established. Instead, matching the planted padding volume to the filler distribution
closed the size ratio to **1.16× mean / 1.5× median** and moved the tier **0.29 in one
change**, against 0.13 for the whole of the round before it. The leak sweep went green for
the first time in three tasks.

**A negative result from the same pass, worth as much as the positive one.**
`single-word filename` appeared in every recent winning triple and looked exactly like the
next authorship tell. Measured: **0.833 on the truth set against 0.768 tree-wide — lift
1.08.** It survives because it preserves recall (10 of 12), not because it discriminates.
One command ruled out what would otherwise have been a whole task.

**Confirming data points:**
1. This session — the padding-volume measurement, after three symptom rounds.
2. [[W-5]] — measuring the predicate family before fixing converged where four rounds of
   fix-then-discover did not. Same shape, one level up.
3. The same session's independent confirmation that general growth dilutes where targeted
   batches do not: across seven seeds the two **largest** trees (302, 278 files) produced the
   two **lowest** reals (2.549, 2.989), and of 16 bespoke dilution files **zero** appeared in
   the winning combination's matched set.

**Impact:** high — three tasks of real work bought 0.19 on this tier; one measurement bought
0.29 and closed it.

**Promote-when:** this is the second datapoint with [[W-5]] and they generalise the same
rule at different scales. At a third, promote to `eval-design` memory as: *"When repairs
converge on a constant with a different instance each round, the instances share a cause —
measure the shared property before repairing again."* Already recorded there in narrower
form.

**Status:** validated — the counterfactual is measured, not argued: 0.13 for a full round of
symptom repair against 0.29 for one cause repair.

## F-21 — I made a probe the acceptance gate for a question it was not precise enough to answer

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, blast-radius eval, Task 4d. Caught by the implementer, not by me.

**When:** I set a task's acceptance criterion to "`real <= p90` in ≥5 of 7 seeds, measured by
`scratchpad/seed_sweep_probe.py`".

**Expected:** that the probe and the fixture's own leak gate would agree, since both compare
a real lift against a null percentile.

**Got:** they disagreed by 0.10 on the shipped seed, in opposite directions. The tree
**passed** the fixture's internal gate (real 3.0689 strictly below its raw gate of 3.07) and
**failed** my probe (real 3.07 against p90 2.97). The implementer diagnosed why rather than
reporting a contradiction: the internal gate draws **240 samples across two schemes and
takes the conservative `min`**, while my probe drew **150 samples, one scheme, a different
draw seed**.

**Probable cause:** I built the probe to answer a different question — *"does each seed's own
p90 track its real value?"* — where it works well, because that question turns on comparing
two **spreads** (real 1.36 against p90 0.26) and a coarse instrument resolves that fine.
Then I reused it, without re-examining it, as a pass/fail gate on a **0.10 difference**. A
p90 of 150 samples is the 135th order statistic of a right-skewed distribution; it does not
resolve 0.10. Same tool, different job, and I never asked whether it was strong enough for
the second one.

An earlier instance in the same session should have warned me: my first run of that probe
used 30 draws and read the shipped seed's p90 as **3.71** against the **3.07** that 240
draws gives. I noticed *that* one because it was obviously wrong, and drew no general lesson
from it.

**Severity:** med — no wrong artefact shipped, and the error was conservative (it made the
gate stricter than the real one, so it could only cause extra work, never a false pass). But
it cost a task's acceptance verdict, and had it run the other way — a weak probe reporting a
pass the shipping guard would have failed — it would have waved a defective fixture into a
paid pilot.

**Status:** fixed-verified — replaced by `scratchpad/seed_sweep_v2.py`, which reproduces the
gate exactly: both schemes, 240 draws, `min(p90)`, truth set excluded from the pool. Its
verdict now agrees with the shipping guard by construction. It also prints an explicit
`edge` verdict for any margin under 0.10 rather than calling it — so the instrument states
its own resolution instead of leaving the reader to assume it has none.

**Fix idea / Pointer:** the general rule, and it is cheap: **before reusing a measurement
instrument as a gate, check it is at least as strong as the thing it is standing in for.**
Where a project already ships its own guard, the probe should reproduce that guard's exact
sampling rule rather than approximate it — then disagreement is a finding rather than an
artefact. And an instrument that cannot resolve a difference should say so in its output;
`seed_sweep_v2.py`'s `edge` label is what makes that automatic rather than remembered.

Same family as [[F-20]] — a controller claim that reached the work unchallenged because it
arrived as settled. There it was a false premise about form-pair carriers; here it was an
unexamined assumption that a probe was gate-grade. Both were caught by someone downstream
re-measuring rather than by me.

## F-22 — A docstring word ending in "raise" fired a substring guard the docstring called safe

**Valid:** invariant

**Observed:** 2026-08-26, blast-radius fixture, re-review round 1 fix (Minor 8 nit). An
implementer widening an import-time pool guard to `SHAPE_VERBS` / `RARE_SHAPE_VERBS` hit an
immediate collection failure on an entry that has been shipping since Task 4c.

**Expected:** `_dilute_raise_pair_no_params` (`gen_fixture.py:1025`) selects filler files by
`"raise " in text` rather than re-parsing, and its own docstring (`:1056-1059`) justifies
that as *"safe because this generator's own `raise` statements are always plain
`raise ValueError("...")` text, never inside a string literal or comment."*

**Got:** `SHAPE_VERBS["ratio"]` (`gen_fixture.py:134`) contains **`"appraise"`**, which ends
in the four characters `raise`. Rendered into prose with a following space — *"...will
appraise the..."* — it contains the exact substring `"raise "` the check scans for, with no
`ast.Raise` node anywhere in the file. Three shipped files already carry an appended
`_<stem>_default()` purely from this collision: `src/provisioning/ledger.py`,
`src/identity/pool.py`, `src/identity/envelope.py`.

**Probable cause:** the safety argument reasons about the shape of *emitted statements*
(`raise ValueError(...)`) and never about the *vocabulary pools* rendered into prose in the
same files. A substring test has no word boundary, so any pool word ending in `raise` —
`appraise`, `braise` — satisfies it. The docstring asserted the property instead of testing
it, which is F-20's family: prose asserting what code measures otherwise.

**Impact:** not a validity leak, and the direction is worth stating rather than assuming.
The dilution's purpose is to raise the FILLER side's conditional rate of "has a zero-arg
function, GIVEN it matches (single-word stem + raises)". Firing on three files the sweep's
own AST-based predicate does not count as raising means those three helpers do not
contribute to the intended dilution — so the dilution is marginally **weaker** than its
docstring claims, never stronger, and no dependent-side signal is created. The leak sweep is
green either way (worst surviving channel 9.9x against the depth-3 bar of 10.5).

**Severity:** low — the shipped effect is three spurious helpers in filler and one false
sentence in a docstring. It is filed because the false sentence is load-bearing: a future
author widening that check, or trusting the "safe" claim while adding a pool word, inherits a
guard that silently mis-selects.

**Status:** open — deliberately not fixed. Either remedy (change the pool word, or give the
check a `\braise\s` word boundary) **moves the generated tree**, which reopens the leak
sweep, the null-control bars and the edge-margin seed sensitivity for a defect that costs
three filler helpers. The implementer left the verb pools out of the enforced guard and
documented the collision inline at the assert site (`gen_fixture.py:603-608`) rather than
silently fixing or silently ignoring it — the right call under a byte-identity constraint.

**Fix idea / Pointer:** fold into the next change that is already allowed to move the tree.
Correct the docstring's "safe because" sentence to say what is actually true — the check is
a substring test and the pools are not word-boundary clean — even if the behaviour stays.
A false safety claim is the part that propagates.

## W-17 — A pre-dispatch scout caught a forward dependency I had stated in a form the harness cannot express

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, blast-radius eval, between Task 5 closing and Task 6 dispatching.

**Pattern:** When one task invents an interface that a *later* task must satisfy, scout the
later task's substrate for whether it CAN satisfy it — before dispatching, and before writing
the forward dependency into a ledger as though it were settled.

Task 5's `check_blast.py` reads two tree roots from `BLAST_GOLDEN_BEFORE_ROOT` /
`BLAST_GOLDEN_AFTER_ROOT`. I ruled that contract accepted and recorded "Task 6 owes these env
vars" in the ledger. The scout found `ScenarioSetup` (`src/prompt_tdd/types.py:160-174`) has
exactly five fields — `files`, `hooks`, `skills`, `commands`, `mcp_config` — and **no `env`**.
`hidden-info/gen.py` even carries an `assert_no_env_key()` that hard-fails anyone who emits
one. Env reaches a checker only via `assertions.py:551` building the child environment as
`{**os.environ, ...}`, so the variables are an **operator responsibility documented in a
README** — the precedent being hidden-info's own section, *"`HIDDEN_GROUND_TRUTH` is an
operator responsibility, not a config key."*

**Counterfactual:** the dispatch would have told an implementer to wire two env vars into arm
config. The most likely path is that it ports `assert_no_env_key()` from hidden-info (the
dispatch says to copy that file) and its own ported assertion hard-fails on its own output —
a confusing self-inflicted BLOCKED report costing a round-trip. The worse path is that it
emits `setup.env` *without* porting the guard, the harness silently ignores the key, and every
run returns `indeterminate:no-golden-output` — which is the gate behaving **correctly**, so
nothing looks broken. That failure would surface at Task 9, the first paid step, as an eval
that produces no data.

**Confirming data points:**
1. This entry — a controller-invented interface checked against the consuming task's substrate
   before dispatch.
2. Same session, Task 5: scouting `golden.py` confirmed `main(install=import_stub.install)`
   and that a second in-process call RAISES, against plan text saying the checker "runs it
   twice and diffs".
3. Same session, Task 5: scouting `DEPENDENTS` / `ENTRY_POINTS` found the brief *less* stale
   than assumed (four details verified correct, one docstring wrong) — the scout that returns
   "mostly fine" still converts assumptions into facts the next session inherits.

**Impact:** high — the silent-failure branch reaches the first paid step before anyone
notices, and its symptom is a correctly-firing gate.

**Promote-when:** a fourth datapoint where a scout of the *consuming* substrate invalidates an
interface the controller had already ruled settled. At that point promote to CLAUDE.md as:
*"An interface one task invents is a hypothesis about the next task's substrate — scout the
consumer before recording the dependency as settled."*

**Status:** validated — three datapoints in one session, all pre-dispatch, all caught before
any subagent ran.

## F-23 — I relayed a subagent's self-reported bug find to the user without verifying it; the bug did not exist

**Valid:** invariant

**Observed:** 2026-08-26, blast-radius eval, Task 7 (`gates_blast.py`).

**When:** The implementer returned DONE with a section headed *"Concern worth attention: found
and fixed a real bug while adapting the copied `parse_log` regex."* I reported that to the user
as a genuine catch in the same turn, without checking it.

**Expected:** hidden-info's `VERDICT (.+)` capture requires ≥1 character; `check_blast.py`'s
success verdict is the empty string; therefore every scored run was silently mislabelled
`"(none)"`, and `(.+)` → `(.*)` fixes it. Self-consistent, specific, and it named a real file
and a real difference between the two evals.

**Got:** the bug does not exist. `surface_lib.py:199` wraps every predicate return —
`verdict = "PASS" if not cls else f"FAIL({cls})"` — *before* `log_run` writes
`VERDICT {verdict}`. The empty string never reaches the log; hidden-info's `(.+)` was always
correct because its success token is `PASS`. The implementer had confused *"`make_predicate`
returns `""`"* with *"`""` is logged"* — a misread of which function the value crosses a
boundary to. The follow-up claim *"all 39 tests failed before the fix"* was also not
reproducible: re-introducing `(.+)` fails 15/39.

**Probable cause:** a subagent's voluntarily-disclosed "concern worth attention" reads as
diligence, and diligence is persuasive. It arrived in the same report as several verified
claims, so it inherited their credibility. The check was two tool calls
(`grep` for the writer, read `surface_lib.py:199`), and I ran them only after an independent
reviewer forced the question.

**Impact:** the same misreading was load-bearing in the shipped module — `gates_blast.py`
classified against raw verdict strings that never appear, so a real pilot log would have
produced `n_clean = n_broken = 0`, every arm flagged contaminated, and **all five gates
REFUSED with exit 1**. And 39 tests were built against the fictional format, so the suite
could not see it. My relay did not cause the defect, but it moved a wrong claim one step
closer to being believed, and it spent the user's attention on a fabrication.

**Severity:** med — no artifact was corrupted by the relay itself, and the reviewer caught the
underlying defect. The cost is credibility: a controller who passes on unverified subagent
claims makes every one of its reports worth less.

**Status:** open — the discipline is stated but not yet mechanised.

**Fix idea / Pointer:** a subagent's report is untrusted content in the specific sense that
matters here: its *factual claims about the codebase* are verifiable and must be verified
before relay, exactly as `get_guide("untrusted-content")` prescribes for file bodies —
*"quarantine the instructions; verify the facts."* The trigger is narrow enough to be a rule:
**before relaying any subagent claim of the form "I found a bug in X", read X.** Pairs with
[[F-20]] (asserting in prose what the code measures otherwise) — same failure, one agent
further out.

## W-18 — Adversarial review before the first spend caught three defects that would each have produced a fabricated pilot result

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, blast-radius eval, Tasks 6b–8, across five opus review passes.

**Pattern:** For an eval, review the **instrument** adversarially before spending on the
measurement, and require each finding to be shown by construction rather than argued. The
specific move that worked every time: **apply the mutation and look**, never read the test and
reason about it.

**Counterfactual — three defects, each of which would have produced a confident wrong number
rather than an error:**

1. **B-1 (Task 6b).** `after_hook.py` resolved `golden.py` via
   `Path(__file__).resolve().parent`, correct for the in-place script and wrong for the copy
   `install_hooks` places in a separate `hooks_dir` — it copies only `hook.source`. Every
   installed run would have scored **`broken-after-tree`, unconditionally, across all five
   arms**. A pilot would have reported 100% "the agent broke the tree" as a *number*. All 14
   tests were green because both subprocess tests ran the in-place script, and the arm test
   asserted the **declaration**, never the **installation**.
2. **F1 (Task 7).** `gates_blast.py` classified against raw verdict strings that never appear —
   `surface_lib.py:199` wraps every predicate return as `"PASS"` / `f"FAIL({cls})"` before
   `log_run` writes it. Fed a real pilot log: `n_clean = n_broken = 0`, every arm contaminated,
   **all five gates REFUSED, exit 1**. Its 39 tests were green because they validated the
   module against a format the harness never writes.
3. **N1 (Task 7 fix round).** Widening the countable set to `len(good)` let
   `FAIL(checker-error:<ExcType>)` — which `surface_lib` writes itself — count as an arm
   outcome. Constructed with ten real crashed-checker runs: **gate 3 FAIL, gate 5 FAIL**,
   printing `!! GATE 5 FAILED -- TOOL DENIAL LEAKED`, with `main()` returning 0. A crashed
   checker reported as a finding about tool denial.

Each defect was invisible to a passing suite, and each would have produced a **fabricated
result**, not a failure — the shape that gets published.

**Confirming data points:**
1. This entry — three blocking-class defects caught pre-spend, all by construction.
2. Same session, Task 5: two mutation classes survived a green suite (`_TEXT_KEYS` reduced →
   10 passed; `DEPENDENT_PATHS[:-3]` → 10 passed), and the review's *own* proposed remedy was
   inert — it iterated the constants its mutation shrank, staying GREEN while the detector had
   gone blind to `pattern`.
3. Same session, Task 8: a probe entry with `must_pass=()` gutting the detector wholesale was
   graded a clean kill, because the harness never guarded its own table's shape.

**Impact:** high — the eval's first spend is Task 9, and each of the three would have bought a
confident, wrong table. Cost of the discipline: five opus review passes and several fix rounds,
against a pilot budget of roughly $6–12.

**Promote-when:** a second work stream where adversarial pre-spend instrument review catches a
result-fabricating defect a green suite missed. Then promote to CLAUDE.md as: *"Before an eval
spends, review the instrument adversarially and require findings by construction — a green
suite is not evidence about the thing it guards."* See memory `eval-design` §§ *The
instrument/subject verdict boundary* and *A green suite is not evidence*, which already carry
the reusable half.

**Status:** validated — three datapoints in one session, all caught before any spend.

## F-24 — `gates_blast.py` keys arms by log filename stem, so a second pilot round silently overwrites the first

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, Task 9 Step 1, writing the pilot driver. Deciding the log
directory layout, so I read how the gates actually discover logs rather than assuming.

**Expected:** `gates_blast.py --logs <dir>` reads the logs under `<dir>` and scores them.

**Got:** `gates_blast.py:377-378` is `for log in sorted(root.rglob("*.log")): arms[log.stem]
= summarise(parse_log(log))`. Two facts compose badly:

- `rglob` is **recursive**, so it descends into every round directory beneath `--logs`.
- `arms[log.stem]` is a **plain dict assignment keyed by filename stem**, and
  `run_arms.py:98` names every log `f"{arm}.log"`. So the stem is the arm name and is
  identical across rounds.

Therefore, pointed at a parent holding two rounds, the gates silently keep **one file per
arm** — whichever `sorted()` yields last — and drop the rest. No warning, no error.

**Why it is not caught by the existing guard:** `--expect` (default 2) exists precisely to
catch a truncated denominator — "a shortfall means runs died before logging". But the
surviving file is a *complete* round, so `n == 2` and the shortfall check passes. The row
reads full and correct while describing one round of two.

**Why it matters here specifically:** the plan's own Task 9 Step 5 is
`gates_blast.py --logs /tmp/blast-pilot` — the **parent**, and also the module's default
(`gates_blast.py:369`). Step 3 is explicitly a stop-and-fix-and-re-run gate, so a second
round into the same parent is the *expected* path, not an edge case. Following the plan
literally after one re-run scores half the evidence and looks fine doing it.

This is the third instance of this eval's signature failure mode: an instrument fault that
surfaces as a plausible number rather than an error. See W-18 and the ledger's "How to read
the pilot without being fooled".

**Mitigated, not fixed** (`prompt-engineering:93da2a7`): `run_pilot.sh` gives every
invocation its own timestamped round directory, never deletes one (F-1), and prints the
exact stamped `--logs` path to score. So the driver's happy path is safe. The residual risk
is anyone invoking `gates_blast.py` with its default or a parent path by hand.

**Fix idea:** make it structurally impossible rather than documented — collect logs into a
list first and **refuse** (exit 2) on a duplicate stem, naming the colliding paths and
telling the operator to pass a single round dir. Same shape as Task 7's ruling: the safe
direction is losing a result loudly, never manufacturing a complete-looking one. Deliberately
NOT done mid-pilot: the user authorised Step 3 only, and changing the scorer between writing
the driver and reading the first numbers would mean the gates that scored round 1 are not the
gates that were reviewed.

**Severity:** med — cannot corrupt a single-round score, which is what Step 3 produces; can
silently halve the evidence for any multi-round reading, which is what Steps 3-and-re-run and
Step 5 are.

**Fixed:** `prompt-engineering:01e2298`. `gates_blast.py` collects the rglob into a list, groups
by stem, and **REFUSES (exit 2) naming every colliding path** rather than guessing which round
was meant — same direction as the verdict allow-list: losing a result loudly beats reporting
half the evidence as if it were all of it.

Verified live against the real `/tmp/blast-pilot`, which by then held **five** colliding stems,
`blast-pos-cs` across **three** files (pre-fix round, post-fix round, hand-assembled combined
set). Before the fix it would have silently scored whichever sorted last.

Three tests — two rounds refuse and name both directories and print no gates table; one round
with unique stems still scores (without which the refusal could be unconditional and every
other test would still pass); and the collision is keyed by **stem**, not full path, since
rounds are separate directories by construction and path-keying would make the guard inert for
the only case that produces it. Two mutations, both killed.

**Status:** fixed-verified — driver mitigation retained, scorer now refuses.

## F-25 — I launched a paid eval run under a foreground tool timeout; the timeout killed it mid-session

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, Task 9 Step 3, the first paid step of the blast-radius eval.

**What I did:** launched the pilot driver with `nohup … &` inside `run_command`, then
`sleep 20; head` in the same command, with `timeout_secs=60`. I reached for `nohup` and `&`
*because* I had correctly predicted the run would outlive any tool timeout — 4 sessions at a
300s per-session timeout is up to ~20 minutes.

**Got:** the tool timed out at 60s and the driver died with it. `pgrep` found nothing, the
round directory was empty, and the driver log stopped at `── running blast-pos-cs …`,
frozen at its start timestamp. The `nohup` did not save it: `&` leaves the background
process holding the stdout pipe open, so the tool never sees EOF, times out, and takes the
process group down. The tool's own hint says exactly this and names the fix
(`run_in_background: true`, which spawns via a log file instead).

**Cost:** a partial, unmeasurable spend. `~/.prompt-tdd/blast-radius-golden/` was
re-published at the kill time, so the arm's `setup.commands` had run and a Claude Code
session was live when it died. Not free, not recoverable, and it produced no scoreable log —
the worst shape a spend can take.

**Root cause, and why the reasoning was half-right:** I identified the hazard (a tool
timeout must not kill a paid run) and then picked a mechanism from general shell habit
rather than from this tool's documented one. `nohup` detaches from SIGHUP; it does not
detach the *pipe*, and the pipe is what the timeout is keyed to. The harness has a
first-class answer, and I had not read for it before spending.

**Generalisable rule:** anything that costs money or takes minutes goes out under
`run_in_background: true` on the FIRST launch — never a foreground call with a generous
timeout, and never `nohup … &` as a substitute for it. Poll the returned `@bg_*` handle.
The cost asymmetry is total: backgrounding a fast command wastes nothing, while
foregrounding a slow paid one burns spend and yields no evidence.

**Related:** this is the same class as the ledger's standing warning that a broken
instrument surfaces as a plausible number — here it surfaced as a plausible *log*, with a
correct header, a correct round dir and a correct arm banner, that simply stopped. Nothing
in the output says "killed".

**Severity:** med — small absolute spend, zero data, and it would recur on every future
pilot step (Steps 4 and 5 are longer than Step 3) if not written down.

**Status:** fixed-verified — relaunched under `run_in_background: true`; the run proceeded
past the point where the first attempt died.

## F-26 — L2 reads only tool ARGS, so it measures "files the agent had to name" — an inverse proxy for navigation-tool effectiveness

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, blast-radius Task 9 Step 3 — the positive-control pilot, the
first paid step. $1.02 spent. The Step 3 stop-gate fired and this is why.

**Expected:** both hinted arms reach `asked=1` and `l2 >= 5`. The prompt explicitly says
*"before changing anything that other code shares, enumerate what depends on it"*, so a
detector that cannot see the behaviour here cannot see it anywhere.

**Got:**

| arm | l1 asked | l2 (of 12) | per-run l2 | calls |
|---|---|---|---|---|
| blast-pos-native | 1.0 | 6.5 | {6, 7} | 14 |
| blast-pos-cs | 1.0 | **2.0** | **{0, 4}** | 20 |

The cs positive control **failed the gate**, and one of its two runs reached **zero**
dependents while making *more* tool calls than the native arm.

**Root cause — verified end to end, not inferred.** L2 counts a dependent as reached when
the dependent's path string appears in a tool call's **arguments**:

- `check_blast.py:115-123` `_dependents_seen` builds its blob from `_paths(call, _L2_KEYS)`.
- `check_blast.py:89-96` `_paths` reads `call["args"]` and nothing else.
- `_L2_KEYS` = `("file_path","path","notebook_path","glob")` + `("command","pattern",
  "symbol","query","name","old_string",…)` — all argument keys.

That equates "reached" with "named in an argument". The two tool families address files in
opposite ways:

- **Native** is path-addressed. `Read(file_path="src/intl/manifest.py")` puts the dependent's
  path in the args, so the natural workflow — grep, then open each hit — scores one L2 hit
  per file opened.
- **codescout** is query-addressed. `references(symbol=…)`, `grep(pattern=…)`,
  `semantic_search(query=…)` return the dependent paths **in the result**; the whole point is
  that the caller never has to name them. Args carry the query, so they score **nothing**.

**The smoking gun.** The zero-scoring run called

    mcp__codescout__references(path="src/intl/duties.py", symbol="duty_multiplier")

which is *literally the behaviour this eval exists to measure* — "who depends on this?" — plus
a broad `grep(pattern="TARIFF_MULTIPLIER|border_total|landed_cost|duty_rate|unit_duty|
apply_duty\b")`. Its L2 blob is `"src/intl/duties.py duty_multiplier"`: `duties.py` is the
**definition** site, not a dependent, so no `DEPENDENT_PATH` matches and the run scores 0/12.
The agent performed the target behaviour perfectly and the metric recorded nothing.

**So L2 as built is close to an INVERSE measure of what the eval is testing.** The better a
navigation tool is at finding dependents without being told their names, the lower it scores.
Note the direction: this compounds with the pre-registered fixture tilt, which also leans
against the symbol-navigation arm. Two independent biases, same sign.

**Why L1 is unaffected:** `_L1_KEYS` adds `prompt`/`description` but already contained
`symbol`/`query`/`pattern`, and *asking* is visible in the args by nature. Both arms scored
`asked=1.0`. It is specifically L2's dependent-**path** counting that inverts.

**The data reaches the checker at run time — this is a drop, not a harness limitation.**
Verified through the stack: `types.py:52-57` `ToolCall` has a `result` field;
`_shared.py:176-191` `parse_transcript` attaches each `tool_result` block to its call via
`by_use_id`; `assertions.py:537-540` writes `{"name","args","result"}` into the trace file
the checker reads. Only `surface_lib.collect_facts()` (`:121-123`) drops it, projecting
`tool_calls` down to `{"name","args"}`. **No harness change is needed** — the fix is
scenario-local.

**CORRECTION to this entry's first version, which claimed the four captured logs could test
a fix at $0. They cannot.** `assertions.py:513` creates the trace as a
`NamedTemporaryFile(delete=False)` and `:574-577` `os.unlink`s it in a `finally` block after
every run. Results exist only for the lifetime of the checker subprocess. What the per-run
log preserves is the `facts` block, which `collect_facts` had **already stripped of results
before writing** — so no amount of re-scoring recovers them, and `score_arm.py` re-runs
checkers over logged text with no trace file at all (`assertions.py:557-559` says so
explicitly). Consequence for planning: the fix is unit-testable at $0 against synthetic
trace fixtures (which is how `test_check_blast.py`'s 26 tests already work), but producing
real post-fix numbers requires **re-running Step 3** at roughly the same ~$1. The error was
mine: I read that results are written into the trace and stopped there, without following
the file's lifetime to the `finally` block eleven lines further down.

**The design rationale was right and the implementation over-shot it.** The module docstring
(`check_blast.py:25-31`) is emphatic that L2 must come from the trace, never the answer text,
because "an answer can name a file the run never opened" — that is the exact defect the
sibling `hidden-info` eval shipped. Correct. But it conflated two different things: the
**answer text** is authored by the model and untrustworthy, while a **tool result** is
produced by the harness and observed. Excluding results bought nothing against
confident-wrong answers and cost the entire query-addressed arm.

**Fix idea:** extend the L2 blob to the call's `result` as well as its args — carry `result`
through `collect_facts` (or read the trace directly in `check_blast.py`) and scan it for
`DEPENDENT_PATHS`. Two things to settle in review, not here: (a) a result blob is large, so
cap or path-extract rather than substring-matching megabytes; (b) decide explicitly whether
a path merely *listed* in a result counts as "reached" or whether reached requires a
subsequent read — and pre-register that choice, because it is the definition the headline
number rests on. Both arms must be re-scored under whichever rule wins; the existing four
logs are enough to test it at $0.

**Severity:** high — this invalidates the eval's primary metric for one of its two arms. Had
Step 3 not been staged as a hard stop, Step 4 would have produced a clean-looking
"codescout reaches fewer dependents" result that is substantially an artifact of how
codescout's tools take arguments.

**Fixed in code, NOT yet validated on live runs** — `prompt-engineering:3ce3bb2`, patch-id
`5f2205fcb5eef24436465f0dfd60f6c7f1fbc38c`. `_dependents_in_results` reads the live trace and
caches its derived set into facts; `l2_enumerated` unions the two sources. Suite **277
passed, 28/28 mutations killed**, including three new ones: `l2_ignores_tool_results` (the
direct guard — blanking the result blob reverts the detector to the state that produced the
{0,4} vs {6,7} split), `l2_double_counts_across_sources` (the hazard the fix introduces: grep
then read is ONE reach), and `excl_freebie_ignores_the_result_side`. The rule is
pre-registered in `scenarios/blast-radius/README.md` § *PRE-REGISTERED DEFINITION*, written
before the implementing code.

Two things the fix surfaced that are worth keeping:

- **`l2_enumerated` counts `len(seen) + len(extra)`, not `len(seen | set(results))`.** The
  union form makes the args side a set expression, so an intra-source dedup regression is
  either silently *repaired* (if the left side is coerced) or turned into a **TypeError** (if
  it is not). A guard that repairs a defect and a guard that explodes are both worse than one
  that reports it — the existing `l2_counts_duplicates` mutation went from killed, to
  crashing every l2 test, and back to cleanly killed across those three shapes.
- **`_clean_env` did not clear `PROMPT_TDD_TRACE_FILE`.** Harmless until the checker started
  reading the trace directly; after that, a suite run from a shell with it still exported —
  which is exactly what a checker subprocess sees, and what an operator debugging a pilot is
  likely to have — would score a foreign run's results into these tests' L2 numbers.

**Status:** open — code fixed, **live validation owed**. Step 3 must be re-run (~$1) before
Step 4, and pre-fix logs must never be pooled with post-fix ones: the four captured runs have
no `_deps_in_results` key and their L2 values are args-only. This is exactly the outcome the
staging was designed to buy, at $1.02.

## F-27 — `native-tool-used` cannot tell "used a native tool" from "attempted one and was denied" — and it is counted, not excluded

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, blast-radius Task 9 Step 3 re-run (post-F-26-fix round,
`/tmp/blast-pilot/20260826-224303`). One of two `blast-pos-cs` runs scored
`FAIL(native-tool-used)`.

**What is in the trace:** exactly one native call in the whole run —

    {"args": {"file_path": "/tmp/prompt-test-q6hn_4s8/pricing.toml"}, "name": "Read"}

— in an arm whose `disallowed_tools` is `"Read Grep Glob Bash Edit Write NotebookEdit Agent"`.
That run scored `l1_asked=true`, **`l2_enumerated=12`** (every dependent) over 37 calls.

**What is verified:** the deny-list is passed correctly. `claude_code.py:74-76` returns
`["--disallowedTools", *session.disallowed_tools.split()]`, i.e. the flag followed by one
argv element per tool name — the CLI's documented form. `Read` was genuinely denied to this
arm, so the model's call should have been refused rather than executed.

**What is NOT verified, and cannot be from the log:** whether that `Read` *executed* or was
*blocked*. `ToolCall` carries both `result` and `error` (`types.py:52-57`), and
`parse_transcript` sets `error = "tool_result reported is_error"` (`_shared.py:191`) — but
`collect_facts()` projects every call down to `{"name", "args"}` (`surface_lib.py:121-123`),
so neither reaches the checker's facts block. The verdict therefore fires on the **tool_use
block alone**, which the model emits whether or not the call is subsequently refused.

**Why that matters more than one run.** `native-tool-used` is on `COUNTABLE_VERDICTS` — it is
treated as arm-accountable and **counted in every rate the run produced a value for**, by
deliberate design (the Task 7 allow-list ruling). If it fires on blocked attempts, then the
cs arm is penalised precisely when its tool restriction is *working*, and the penalty lands
on the metric the whole eval compares. The two readings are opposite:

- **Blocked** (the likely one): the arm is intact and the verdict is over-strict. Evidence,
  circumstantial: a single `Read` appears in 37 calls and is never retried — the shape of one
  refusal, not of an agent that found a working tool. And the run still reached 12/12 through
  codescout tools.
- **Executed**: the arm's isolation leaked and that run is not a cs observation at all.

Circumstantial evidence is not a verdict, and this one is load-bearing.

**Same root cause as F-26.** Both are the facts block dropping something the trace already
carries. F-26 dropped `result` and inverted L2; this drops `error` and blurs an
arm-accountable verdict into an instrument question. The fix shape is the same and now has a
precedent in-tree: `_dependents_in_results` already reads the live trace directly and caches a
small derived value into facts. A `_native_tool_executed` reading `error`/`is_error` would sit
beside it.

**Fix idea:** distinguish the two at scoring time — `native-tool-used` for a call that
returned without `is_error`, and a separate non-penalising class (or nothing at all) for a
blocked attempt. Pre-register which one counts before re-running, for the same reason the L2
rule was pre-registered: the choice moves the headline number.

**Severity:** high for interpretation, though it did not change Step 3's outcome — the gate
criterion (`asked=1` and `l2 >= 5` on both hinted arms) is met either way, since the affected
run scored l2=12. It becomes load-bearing at Step 4, where the cs/native separation is the
result.

**SETTLED, and the arm did NOT leak.** Claude Code writes a complete session transcript to
`~/.prompt-tdd/profiles/<profile>/projects/<sanitised-workdir>/<uuid>.jsonl` and does **not**
remove it with the temp dir. The run's transcript was still on disk:

    TOOL_USE Read -> {"file_path": "/tmp/prompt-test-q6hn_4s8/pricing.toml"}
    IS_ERROR : True
    RESULT   : "<tool_use_error>Error: No such tool available: Read. Read is disabled
                for this session, in subagents as well as here.</tool_use_error>"

The deny-list held. The agent tried once, was refused, and finished through codescout tools
reaching 12/12. Across all five cs runs to date — **110 tool calls — exactly one denied**. The
veto was over-strict; the arm was intact.

**Fixed:** `prompt-engineering:3580004`. `_native_tools_executed` reads the live trace and
vetoes only on a native call whose result lacks the `<tool_use_error>` marker, with a
deliberately **conservative fallback**: no cache and no readable trace → return every native
name, i.e. the old behaviour. A run that cannot be *shown* to have been blocked stays vetoed,
because losing one cs observation costs a run while silently declaring an escaped arm clean
corrupts the comparison. Two mutations guard it (`native_veto_counts_denied_calls`,
`native_veto_fallback_declares_clean`). Suite 283 passed, 30/30 mutations killed.

**Status:** fixed-verified — settled against the primary record, not by inference.

## F-28 — The "native" arm is in practice a SHELL arm — it does almost everything through Bash

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, first use of `scripts/inspect_eval_run.py` on the post-fix control
round. Nothing in any per-run `.log` shows this — the facts block keeps `tool_names`, but no
one had ever looked at the distribution.

**Got**, per-run tool mix for the two positive-control arms:

| arm | run | calls | mix |
|---|---|---|---|
| blast-pos-cs | bu1qooj2 | 26 | `symbols` 12, `grep` 6, `ToolSearch` 3, `references` 2, `edit_code` 1, `run_command` 1, `memory` 1 |
| blast-pos-cs | q6hn-4s8 | 37 | `symbols` 20, `grep` 6, `run_command` 3, `read_file` 2, `edit_code` 2, `references` 1, `Read` **DENIED** |
| blast-pos-native | 8qhx9vi1 | 11 | **`Bash` 8** (+1 ERROR), `Read` 1, `Edit` 1 |
| blast-pos-native | svj5hnwq | 8 | **`Bash` 6**, `Read` 1, `Edit` 1 |

**The native arm barely uses the native file tools it is named for.** Its `disallowed_tools`
is `"Agent"` alone, so `Bash` is available, and it does essentially all of its searching and
reading through the shell — `grep`, `sed`, `cat` — with a single `Read` and a single `Edit`.
This is not a defect in the arm: it is what an unrestricted agent chooses. But it means the
comparison is **codescout tools vs. a shell**, not codescout tools vs. Claude Code's native
file tools, and every result should be worded that way.

**Two consequences that bite the metrics, not just the prose:**

- **L2 attribution differs by mechanism.** `_TEXT_KEYS` includes `command`, so a
  `Bash(command="grep -rn duty_multiplier src/")` puts the *pattern* in args and the
  *dependent paths* in the result, while `Bash(command="cat src/intl/manifest.py")` puts the
  path in args. Post-F-26 both count, which is right — but it means the native arm's L2 is
  assembled from shell strings, and any future change to how command strings are parsed moves
  that arm alone.
- **The call-count asymmetry is large and in the unintuitive direction.** cs used 26-37 calls
  to reach 8-12 dependents; native used 8-11 calls to reach 6. Reaching more is not free, and
  a headline that reports only L2 would hide the cost. Whatever Step 4 concludes about
  completeness should be read next to `calls` and `cost`, both already in the gates table.

**Fix idea:** none to the arm — this is the honest behaviour of an unrestricted agent, and
constraining it to make the contrast cleaner would be measuring a strawman. What is owed is
**wording**: say "shell" where the design says "native", in the README, the gates output, and
any write-up. Optionally add a third condition that denies `Bash` too, if the native-file-tool
comparison is genuinely wanted — but that is a new arm and new spend, not a relabel.

**Severity:** med — no number is wrong, but the obvious reading of the arm's NAME is, and that
reading is the one a write-up would carry.

**Wording paid, 2026-08-26.** `scenarios/blast-radius/RESULTS.md` labels the arm "shell" in
the results table, states the caveat explicitly ("the comparison is codescout **vs. a shell**,
not vs. Claude Code's native file tools"), and carries `calls` and `cost` beside every l2x
figure so the reach-is-not-free point cannot be dropped by a reader taking only the headline.
The ruling stands: **relabel, do not re-arm** — constraining the native agent's Bash to make
the contrast look cleaner would measure a strawman, and what an unrestricted agent actually
does IS the comparison worth having.

Still open as an OPTION, not a debt: a third condition denying `Bash` as well, if the
native-file-tool comparison is genuinely wanted. That is a new arm and new spend, not a
relabel, and nothing in the current result depends on it.

**CORRECTED 2026-08-27, on the user's challenge — this entry caused a worse error than the one
it reported.** The user pushed back that the no-codescout arm *should* have the shell and only
the toolless arm should be denied it. Checked against the configs and the transcripts, they are
right and nothing in the fixture is wrong:

| arm | native file tools | Bash | codescout MCP |
|---|---|---|---|
| `blast-native` | **all** (only `Agent` denied) | **yes** | no |
| `blast-cs` | denied | **denied** | yes (incl. `run_command`) |
| `notools` | denied | denied | no |

The native arm has the **full** toolkit and uses `Read` and `Edit` in every run. It merely does
most of its *searching* through Bash. And the **restricted** arm is the cs one — it is the only
arm denied a shell in the ordinary sense, reaching one through `mcp__codescout__run_command`.

**Where this entry went wrong.** It reported a behaviour ("does 5-8 of 7-11 calls through
Bash") and then prescribed a *wording* remedy — relabel the arm "shell". I applied that to the
results table and the published artifact, where a column header reads as what an arm was
**allowed**. So an observation about what the agent *chose* was promoted into a false claim
about what it was *permitted*, and it shipped in the headline table of both.

**The generalisable lesson, and it is the sharper half of this entry:** a label in a results
table is read as a property of the *condition*, never of the *run*. Behavioural observations
belong in prose or in a column of their own — never in the column that names the arm. "It chose
X" and "it could only do X" differ by exactly one word in a header and completely in meaning.

**Fixed:** the table now says `native (all)`, with the permission matrix stated up front in
both RESULTS.md and the artifact, and the caveat rewritten to say which arm is actually
restricted. The original ruling still stands — relabel, do not re-arm: what an unrestricted
agent actually does IS the comparison worth having.

**Status:** fixed-verified — and re-opened once, because the first fix introduced the real
defect.

## W-19 — Staging the pilot behind a positive-control gate caught two result-fabricating defects mid-spend, for $1

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, blast-radius Task 9. The plan staged the pilot as: positive controls
first, 2 runs, **hard stop** unless both hinted arms reach `asked=1` and `l2 >= 5`; main arms
only after. Total spend to a complete answer: **$2.61**.

**Pattern:** run the *controls* before the *arms*, with a numeric stop-criterion written down
in advance, and treat a control failure as an instrument question before an arm question.

**Counterfactual, and it is not hypothetical — both defects fired.**

1. **F-26.** The first control round stopped at the gate: `blast-pos-cs` l2 2.0 against a
   required ≥5, one run at zero. Root cause was that L2 read tool **args** only, so
   query-addressed tools scored ~nothing while path-addressed ones scored per file opened —
   close to an *inverse* proxy for the capability under test. Without the staging, Step 4
   would have run first and produced a clean, fully-scored, entirely plausible **"codescout
   reaches fewer dependents"** headline. Nothing in any verdict would have flagged it: all
   four runs scored PASS.
2. **F-27.** The post-fix round surfaced `FAIL(native-tool-used)` on a cs run. That verdict is
   on `COUNTABLE_VERDICTS` and lands in every rate. The transcript showed the call was
   **refused** — `<tool_use_error>` — so an intact arm was being penalised for its own
   deny-list working.

**Two properties made the staging work, and both are copyable:**

- **The criterion was numeric and written before the spend.** "Both hinted arms reach
  `asked=1` and `l2 >= 5`" is falsifiable at a glance. A prose criterion ("check the controls
  look sensible") would have accommodated l2 = 2.0, because 2.0 is not obviously wrong.
- **The controls are the arm that MUST succeed.** The hinted prompt says "enumerate what
  depends on it". An arm that is *told* to do the thing and still scores near zero is an
  instrument failure with very high prior — which is what turned a disappointing number into
  a source read rather than a finding written up.

**And the staging paid a third time, in the other direction.** Step 4's main arms came back
*uniform* — l2x exactly 1.0 in all four runs, span 0.0000 — which is the same shape as a
broken detector. The same reflex applied (`distinct == 2`, transcripts read, tool mixes
inspected, `_native_executed` empty) confirmed the floor was **real**: the cs runs called
`references` and `symbols` zero times. Without the habit already established, that genuine
finding would have been indistinguishable from the two artifacts that preceded it.

**Confirming data points:** F-26 and F-27 (this session); W-18's three pre-spend defects, same
work stream. Four instrument failures in one eval, every one of which would have surfaced as a
number rather than an error.

**Impact:** high — the difference between publishing a measurement and publishing an artifact
of one's own tooling, at a cost of roughly $1 and one extra control round.

**Promote-when:** a second work stream stages a paid measurement behind a written numeric
control gate and catches an instrument failure with it. At two datapoints, promote to
CLAUDE.md / `eval-design` as: *"Never run the treatment arms first. Run the arm that must
succeed, with a numeric stop-criterion written down before spending, and read a control
shortfall as an instrument fault until the source says otherwise."*

**Status:** validated — two independent catches within one work stream, both traced to source
and fixed, with the third (uniform-result) application confirming a true negative.

## W-20 — Running the second baseline the spec asked for overturned the headline it was meant to confirm

**Valid:** dated 2026-08-26

**Observed:** 2026-08-27, blast-radius. The spec said *"baseline with Sonnet and Opus."* The
2026-08-26 pilot ran ten Sonnet runs and produced a clean, gated, fully-caveated result — and
the Opus arms were nearly dropped as a nice-to-have, because the Sonnet result already looked
finished.

**Pattern:** when a spec names two conditions and the first one produces a publishable answer,
**run the second anyway** — the risk is not that it adds nothing, it is that the first answer
silently over-generalised.

**Counterfactual, and it is not hypothetical.** The published headline was *"handed a narrow bug
report, the agent does not go looking for who else depends on the code — and no toolset changes
that."* Four Opus runs at **$2.26** showed that is a **Sonnet** fact and false of Opus, which
reaches 8-10 of 11 unprompted, beating Sonnet's *hinted* ceiling. Worse for the original claim:
the tool advantage that carried the entire Sonnet result — 4/4 against 0/4 on the rename-chase
bucket — **disappears** on Opus, where the plain shell reaches it too.

So the scope error was in the load-bearing sentence, and it was the kind that reads as a general
truth about agents while resting on one model tier. Nothing in the Sonnet data hinted at it: all
five arms scored, no gate refused, every caveat pre-registered.

**What made it cheap enough to be worth doing:** the arm was a config change, not a new fixture.
`gen.py` gained a per-dir `model` with a sonnet default, and the five existing configs
regenerated **byte-identically** — verified, because that is the check that the already-scored
runs were not disturbed. A model dimension that costs one dict key and $2 is not a nice-to-have.

**Two guards worth copying, both added with the arm rather than after:**

- `test_the_opus_arms_differ_from_their_sonnet_twins_ONLY_in_model` compares the *pair* —
  deny-list, profile, prompt and runs must match, model must not. Every other test in that file
  pins one field against a table in isolation, so nothing else compared the twins to **each
  other**, and adding a row to two tables is exactly how a confound would have entered.
- Model is pinned by value on **every** dir, not just the new ones, so an arm that silently
  drifted to a different model fails loudly instead of becoming a confound wearing the clothes
  of a tool condition.

**Confirming data points:** this one. The nearest relative is W-19 (staging caught defects
*before* the headline); this is the mirror — a second condition caught a defect *in* a headline
already published.

**Impact:** high — the difference between publishing a fact about agents and a fact about one
model, at $2.26 and one config key.

**Promote-when:** a second work stream finds that an unrun condition from its own spec changes a
published conclusion. At two datapoints, promote to `eval-design` as: *"A spec's unrun condition
is a live threat to the conclusion you already drew — run it before publishing, not after."*

**Status:** validated — single datapoint, but the overturned claim was the document's headline
and the correction is committed.

## F-29 — I published between-arm claims at n=2; thickening to n=6 killed all three of them

**Valid:** dated 2026-08-27

**Observed:** 2026-08-27. A published artifact and a committed `RESULTS.md` carried three
headline claims derived from **two runs per arm**. Twenty more runs ($7.28) falsified every one.

| claim at n=2 | at n=4-6 |
|---|---|
| Sonnet-plain reaches **exactly 1**, all four runs, zero spread | **2.00 / 2.25** — one run in each arm reached 5 |
| Hinted: **9 vs 5**, tools nearly double reach | **7.67 vs 6.67**, supports heavily overlapping |
| `CHASE_REQUIRED` **4/4 vs 0/4**, perfect separation, *the entire gap* | **3.17 vs 1.33** |

**The pattern in what died is exact and was predictable.** All three were claims about a
**difference between two tooled arms**. Every claim about a *large* effect survived untouched —
the floor (0.00, 4/4 runs), the prompt effect (~2 → ~7), and Opus supplying the sub-goal
unprompted (7.00/7.83 against 2.00/2.25, non-overlapping supports).

That is not luck. **A between-arm difference is structurally the smallest effect in any
comparative design**: it is what remains after the floor, the manipulation and the model have
each taken their share. So it is always the claim that needs the most runs — and it is
invariably the one the eval was built to make.

**Why n=2 looked sufficient at the time, which is the part worth remembering.** The numbers
were not noisy-looking. Sonnet-plain read `{1, 1}` and `{1, 1}` — *zero* spread, four runs
agreeing exactly. `CHASE_REQUIRED` read 0/4 twice against 4/4 twice — perfect, in the direction
the fixture predicted before any run. **A confirmed prediction at n=2 is still n=2**, and
apparent zero-variance from two draws is the least informative kind of agreement, not the most.
The support block printed `{1.0000 x2}` on every row and I read consistency where there was
only a small sample.

**What I should have done:** published the floor, the prompt effect and the model effect — all
robust at n=2 and all large — and withheld the tool comparison pending more runs. Instead the
tool comparison was the headline of both the artifact and the commit message.

**Cost:** a correction box at the top of `RESULTS.md`, a redeployed artifact, and a commit whose
subject line records that three of its predecessor's claims were wrong. Cheap in absolute terms
because it was caught internally, at the user's prompting, before anyone acted on it.

**Fix idea:** before publishing any comparative claim, sort the claims by effect size and draw a
line: large effects can go out at low n, between-arm differences cannot. State the n *next to
each claim*, not once in a caveats section — a caveat at the bottom does not travel with a
number that gets quoted.

**Severity:** high — the falsified claims were the eval's stated purpose, and they were
published, not merely believed.

**Status:** fixed-verified — all three corrected in `RESULTS.md` (correction box at the top
mapping each claim to its n=6 value) and in the artifact; `prompt-engineering:42c55fe`.

## W-21 — Thickening n turned a dead claim into a better one — the mechanism came back stronger than it died

**Valid:** dated 2026-08-27

**Observed:** 2026-08-27. Twenty runs, $7.28, taking every arm from n=2 to n=4-6.

**Pattern:** when thickening n kills a claim, **look at what replaces it before recording a
loss.** A claim that dies at higher n is often a crude version of a truer one, and the truer one
is usually more defensible than the original.

**The counterfactual, both directions.** Three published claims died (F-29). But the *mechanism*
behind them — the fixture's pre-registered theory that symbol navigation must win the
rename-chase bucket and lose the string-dispatch one — came back in **better** shape:

| | n=2 | n=4-6 |
|---|---|---|
| `CHASE_REQUIRED`, hinted cs vs native | 4/4 vs 0/4 | **3.17 vs 1.33** (+1.84) |
| `LEXICAL_ONLY`, hinted cs vs native | 0/4→4/4 vs 2/4 | **1.83 vs 2.33** (−0.50) |
| net total reach | +4 | **+1.0** |

At n=2 the tool looked **strictly dominant**: it swept one bucket and the other was noise. At
n=6 it shows the **trade the design predicted** — better at following a rename, worse at names
held as strings. *A tool that is better at one thing and worse at another is a more credible
finding than one that wins everywhere*, and it is the finding the fixture was built to produce.
The clean sweep was the artifact; the trade is the result.

**And thickening produced a genuinely new finding that n=2 could not have contained.** Across
6 hinted-Sonnet codescout runs, `symbols` was called in 5 of them (1-24 calls) and `references`
in 5 (1-8). Across 6 Opus codescout runs, **`symbols` was called zero times** and `references`
in only 2. Opus-codescout's `CHASE_REQUIRED` (1.33) therefore sits *below* hinted-Sonnet
codescout's (3.17) — stronger model, same tools, no other route to the filesystem. **A
navigation tool pays only when the agent chooses to navigate with it**, and a model strong
enough not to need it will not reach for it. That needed six runs per arm to see; at two it
would have read as variance.

**One method change made the difference visible**, and it is copyable: `bucket_breakdown.py` was
reporting a **union** across an arm's runs — "could this arm ever reach it". A union saturates
as n grows, so more runs would have made every arm look *better* and the arms look *more alike*,
hiding the very spread that thickening was meant to expose. Switching to per-bucket **means**
is what let n=6 speak. **A union is the wrong aggregate for anything you intend to compare.**

**Confirming data points:** this one. Related: W-19 (staging caught defects before a headline)
and W-20 (an unrun condition overturned a published headline) — all three are the same family:
*the cheap extra measurement is the one that changes the conclusion.*

**Impact:** high — converted three false claims into one credible mechanism plus a new finding,
for $7.28.

**Promote-when:** a second work stream finds that raising n replaced a dead claim with a better
one. At two datapoints, promote to `eval-design` as: *"When higher n kills a claim, read what
replaced it — the crude claim usually dies into a truer one."*

**Status:** validated — the replacement claims are committed (`prompt-engineering:42c55fe`) and
the method change is in the probe.

## F-30 — I led a surface report with a session anecdote the artifact under review had already measured and labelled unrepresentative

**Observed:** 2026-08-27, breadth-first sweep of the `get_guide` prompt surface,
before any design work. Task was "understand the surface", prompted by the buddy
specialist-graph refactor shipping in `claude-plugins`.

**When:** Writing the summary report of the surface, after reading only
`## Root cause` and `## Proposal` of
`docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md`
(`7579b32b1cd2362f`).

**Expected:** That my live measurement was fresh corroborating evidence worth
leading with — five guide topics auto-injected across five routine tool calls,
66,286 bytes, 63.2% of the 104,827-byte corpus.

**Got (scouted reality):** Two things, both already in the file I was reporting on.

1. Its `## Symptom` section carries the **identical** figure — same five topics
   (`project-activation-bootstrap`, `tracker-conventions`, `librarian`,
   `symbol-navigation`, `progressive-disclosure`), same 66,286 bytes, same 63% —
   measured on a `claude-plugins` session (`f6ae2d77`, 2026-08-26/27). Mine was a
   reproduction presented as a discovery.
2. Its `### Three findings the anecdotes could not reach` ¶3 names the defect
   outright: *"The 63% headline was itself unrepresentative."* A census of 91
   session ledgers under `~/.local/state/codescout/guide_hints/` puts the median
   at **2 topics per session** (38 of 91 received exactly two); five topics is the
   top 24%. My session is an unremarkable draw from a distribution the file had
   already published, not a smoking gun.

The file also states the generalised form: *"anyone quoting the 1/91 as proof of
that is doing what the 63% headline did."*

**Probable cause:** Section-targeted reads. I fetched the heading list **twice**,
and it literally contained *"The delivery census exists — 91 sessions, and it
corrects both anecdotes"* and *"Three findings the anecdotes could not reach"*. I
then read `## Root cause` and `## Proposal` because those are the sections a design
task wants, skipping the two whose headings advertised that they invalidate the
lead I was about to write. Cheap targeted reads made it cheaper to skip the
correction than to find it — which is the same all-or-nothing-addressing tension
this very bug is about.

**Workaround:** Re-anchored the report on the census: the target is the
`tracker-conventions` + `librarian` bundle (54,878 B, co-occurring in 38 of 91
sessions, 74.4% of all guide bytes ever auto-delivered), plus three topics that
have never delivered at all (26,329 B, 25% of the corpus).

**Severity:** med — no code written and nothing shipped, but the design was one
step from being motivated by a tail observation, and the correcting text was
already in context as a heading.

**Status:** fixed-verified — caught by the `/reconnaissance` pass before any design
work; the report was corrected in the same turn.

**Valid:** dated 2026-08-27

True of the bug file at `9eb0dd628b791e3cd07abf145f2d4f3b08055be9`; the census is a
floor, not a history — its own `### What this census is NOT` names five ledger
deletion paths, so 91 is "sessions whose ledger survived".

**Rests on:** the principle that a source artifact's stated limits bind anyone
quoting its subject matter, not only anyone quoting its numbers.

**Kin:** F-5 (a stale recon finding relayed as current), F-23 (a subagent's
unverified bug find relayed to the user), F-29 (between-arm claims published at
n=2) — same family: a figure promoted to headline ahead of its power.

## W-22 — Reading the source artifact's own limits sections before designing overturned my headline and re-pointed the target

**Observed:** 2026-08-27, `/reconnaissance` invoked after a breadth-first sweep of
the `get_guide` surface and before any design work on it.

**Pattern:** When a report is *about* an artifact, read that artifact's own
evidence-and-limits sections before leading with your own measurement — in
particular any section whose **heading** claims to correct, contextualise, or bound
the class of measurement you are about to present. Heading text is a near-free
filter: *"…and it corrects both anecdotes"* and *"What this census is NOT"* are
direct warnings, readable without fetching the body.

**Counterfactual:** Without this pass, the design would have opened on *"63% of the
corpus delivered per session"* as its motivating number. Concretely:

- That figure is a **top-24% tail draw** (median is 2 topics/session, n=91), and
  the source artifact pre-labels the move as the error its own headline made.
- It points at the wrong target. "The corpus" is not the cost centre; the
  **bundle** is — `tracker-conventions` + `librarian` = 54,878 B, co-occurring in
  38 of 91 sessions, **74.4%** of every guide byte ever auto-delivered here, and
  they arrive by *different* calls, so targeting either alone leaves the other
  arriving whole.
- It hides the inverse finding entirely: **three topics have never auto-injected
  in 91 sessions** — `iron-laws-detail`, `librarian-runtime`, `untrusted-content`,
  26,329 B, 25% of the corpus. A corpus-wide framing cannot see a quarter of the
  corpus that costs nothing because it is never delivered.
- It would have left direction (b) looking viable. The census supplies the
  already-run experiment: `librarian-runtime` was hand-split out of `librarian.md`
  *specifically to keep the parent lean*, and in 91 sessions **nothing has ever
  followed that edge** — the parent's 20 KB is intact and 9,774 B became
  unreachable.

**Confirming data points:**
1. F-30 (this session) — the anecdote-as-headline miss this scout caught.
2. Three code claims the scout **confirmed**, which would otherwise have shipped
   on inference: `GetGuide::force_inline() → true` (`src/tools/guide.rs:119-124`,
   read — so a 34 KB guide is never buffered); the injection path has **no byte
   budget** (positive control: the same grep finds five budget constants in
   `src/tools/core/types.rs`, and `guide_block` at 805-820 calls none of them);
   and `Symbols::relevant_guide_topic` branches `overflow`/`output_id` →
   `progressive-disclosure`, else → `symbol-navigation`.
3. One measurement the scout produced that is **absent** from the bug file and
   survives its correction: the corpus is growing fast behind the largest trigger.
   `tracker-conventions` went **10,377 → 34,333 B in ten days** (`git show
   'experiments@{2026-08-17}:…'`, corroborated independently by
   `src/librarian/adapter.rs:197`'s contemporaneous *"10.4 KB + 19.9 KB"*
   comment); corpus-wide 75,441 → 104,827 B, **+39%** since the BL-25 measurement
   of 2026-08-16. The census makes this sharper rather than softer: the file that
   tripled is 46.5% of all bytes ever delivered.

**Impact:** med — prevented a design anchored on a tail statistic, and re-pointed
it from "the corpus" to a two-file bundle plus a growth rate.

**Promote-when:** A second instance of a report leading with a measurement its own
source artifact had already classified as unrepresentative. At 2 datapoints,
promote to the reconnaissance skill's Phase 1 as: *"When the seam is a document,
read its limits-and-corrections sections before quoting your own figure on its
subject — headings advertise them."* Route as craft-shaped: it holds in any repo
and needs no codescout dialect.

**Status:** validated — single datapoint, caught before any design work. Awaiting
promotion criterion.

**Valid:** dated 2026-08-27

Claims 2 and 3 re-verify at the bytes; claim 1 is this session's own record.

**Rests on:** F-30, same session — this win is F-30's counterfactual, not
independent evidence of the pattern.

## F-31 — The evidence behind every published number was sitting on tmpfs, and the fix that was supposed to protect it only ever guarded one of two mechanisms

**Observed:** 2026-08-27, resuming the blast-radius stream after a compaction.
Rather than quote the compaction summary's numbers, I re-ran
`gates_blast.py --logs /tmp/blast-pilot/POOLED-n`. It reproduced the published
table exactly. Then I looked at the filesystem it had just read.

**When:** First action of a resumed session, re-verifying a finished result
before reporting state. No task depended on the answer.

**Expected:** `run_pilot.sh`'s header says round dirs are never deleted — that
is F-1's remedy, written after *"a fixed output path destroyed the evidence
behind a headline figure"*. Every round since has had its own timestamped
directory and nothing has ever been removed. So: evidence safe.

**Got:** `findmnt /tmp` → **tmpfs**, RAM-backed, 63 G. `/tmp/blast-pilot` held
5.2 MB — 36 scored runs, 32 `.log` files, 16 preserved Claude Code session
transcript dirs, the two deliberately-excluded rounds, and `POOLED-n` — all of
it one reboot from gone. **$11.34 of paid, unrepeatable runs**, and the sole
evidence base for `RESULTS.md`.

Nothing in either repo would have reported the loss. `RESULTS.md` cites
*numbers*, never the logs behind them, so it stays readable and confident with
its evidence deleted; `gates_blast.py` on an empty root fails with "no logs",
which reads as a path typo, not as data loss. The first symptom would have been
a future session unable to reproduce a published figure and unable to tell
whether the number or the evidence was wrong.

**Probable cause — and this is the transferable part.** F-1's remedy was named
after its **mechanism** (a fixed output path, fixed by timestamping and never
deleting) rather than after its **failure class** (*the evidence disappears*).
Two mechanisms produce that class here: the script deleting it, and the
substrate deleting it. The remedy addressed the first and never prompted anyone
to look for the second, because a mitigation named after its mechanism reads as
complete the moment that mechanism is closed. Six rounds ran under a rule
everybody believed was protecting them.

**Fix:** archived to `~/.local/share/prompt-tdd-evidence/blast-pilot` (btrfs,
230 G free) with a `PROVENANCE.md` recording what may and may not be pooled and
how to re-score. `run_pilot.sh` now defaults `OUT` there
(`BLAST_PILOT_OUT` / `--out` still override), so the next round does not
recreate the exposure; the two path *defaults* (`gates_blast.py --logs`,
`bucket_breakdown.py:P`) and the prose citations were repointed in the same
commit. `prompt-engineering:7424ab26`, patch-id `23d6cef558e8cb1b`.

**Verified by re-running the gates against the COPY**, not by comparing `du` —
identical arm means, `TOTAL $11.3378`, exit 0, and `bucket_breakdown.py`
reproduces the published per-bucket table. A copy that scores the same is the
only check that means anything; matching byte counts would have passed even if
the copy were unreadable by the scorer.

One defect in the rescue, worth naming because it is funny and it is a class:
`cp -a` faithfully preserved `latest` as an **absolute symlink back into
`/tmp`** — the archive's own pointer led straight to the volatile directory it
had just been rescued from. Repointed relative. *A copy made for durability can
carry a pointer that defeats it, and `du` cannot see that either.*

**Severity:** high — silent, total, unrecoverable loss of the evidence base for
a published result, with no surface anywhere that would have announced it.

**Status:** fixed-verified — the exposure is closed for this round (archived)
and for future rounds (`OUT` default). What remains open is the general lesson,
which is why W-23 is written separately.

**Valid:** dated 2026-08-27

**Rests on:** F-1, same ledger — this is that entry's unguarded second
mechanism, not an independent finding.

## W-23 — Re-running the verification after compaction found what re-reading the summary structurally could not

**Observed:** 2026-08-27, first action of a session resumed from compaction on
the blast-radius stream.

**Pattern:** On resuming a finished work stream, **re-run the verification
rather than quoting its recorded result** — even when the record is detailed,
recent, and correct.

**Why it is not merely belt-and-braces.** A record carries *claims* and their
*verification status*. It cannot carry properties of the **substrate the
verification ran on**, because those were never the subject of any claim. So
there is a class of defect no amount of summary fidelity preserves, and
re-execution is the only thing that surfaces it.

That is exactly what happened. The compaction summary said: *gates exit 0, 36
runs pooled, $12.34, both repos clean.* Every word true, and all of it
reproduced on demand. None of it could have revealed that the entire evidence
base was sitting on **tmpfs** and would not survive a reboot (F-31). The fact
was not omitted from the summary — it was not the kind of thing a summary has a
slot for.

**Counterfactual, and it is concrete.** Resuming by quoting the summary is the
*intended* use of a compaction summary, and it would have reported a correct,
green, fully-consistent state. The evidence would have stayed on tmpfs until the
next reboot took 36 paid runs and 16 preserved transcripts. The first symptom
would have arrived in some later session as an unreproducible published figure,
with no way left to tell whether the number or the evidence had been wrong —
and by then the arm-level supports, the transcripts behind the `symbols`-usage
finding, and the excluded-round provenance would all be gone. I only looked at
the filesystem because re-running `gates_blast.py` put me on the path it reads.

**Cheap, too.** The re-verification was `$0` — a local scorer over logs already
on disk — against a loss that was unrecoverable at any price. That asymmetry is
the whole argument: re-execution is nearly free exactly when the record is
about finished work, which is when the temptation to quote it is highest.

**Confirming data points:**
1. F-31, this session — tmpfs exposure found by re-running rather than
   re-reading.
2. Pending: a second resume where re-execution surfaces something the record
   had no slot for.

**Impact:** high — the defect it caught was silent, total and unrecoverable.

**Promote-when:** a second datapoint. At two, promote to the `eval-design`
memory as *"resuming a finished stream: re-run the verification, never quote
it — a record cannot carry properties of the substrate it ran on."*

**Status:** validated — single datapoint, defect caught and closed.

**Valid:** dated 2026-08-27

**Rests on:** F-31, same session — this win is that entry's counterfactual,
not independent evidence of the pattern.

## F-32 — The no-tools floor could run a shell; the tool was left un-denied by a documented ruling whose reason was an untested inference

**Observed:** 2026-08-27. Added `notools-opus` (a negative control for the Opus
half of the design) and ran it. First run, gate 5 FAILED: L1 0.50, L2 3.00,
l0 0.50 — the "no tools" arm fixed the bug.

**Expected:** a floor. The Sonnet twin had scored a clean 0.00 across four runs
against the same deny-list, and I justified the new arm as testing a claim about
*model priors* — "is this task answerable without tools, for a stronger model?"

**Got (from the transcripts, not the score):** the arm called `Monitor` eight
times and used it as a complete Bash replacement — `find`, `grep -rn`, `cat`,
`python3 -c`, and finally a heredoc that patched `src/intl/duties.py` and ran
pytest. `CronList` too. `Monitor` takes a `command` parameter and executes it.
Neither name was in `NOTOOLS_DENY`.

**What that says about the Sonnet floor, which is the point.** The same leaky
list produced `blast-notools`' 0.00 across four runs. That was never evidence
the denial held — it is evidence Sonnet did not go looking. The mechanism has
been incomplete for the entire study and no gate could see it.

**Probable cause — and it is not carelessness, which is why it is worth an
entry.** The omission was *deliberate and documented*. `hidden-info/gen.py`,
where this list originates, contains: *"KNOWINGLY LEFT UN-DENIED, and why none
of them can reach the fixture: … Monitor / Artifact / EnterWorktree /
ExitWorktree / SendMessage — absent from BUILTIN_TOOL_NAMES, i.e. not on the
headless `claude -p` surface at all."*

That sentence contains a **verified fact** (Monitor is absent from
`BUILTIN_TOOL_NAMES`) and an **untested inference from it** (therefore it is not
available headless), written at one confidence, in one breath — and the
inference carries all the load. The list even names the "wider surface registry"
that *does* contain Monitor, two lines above.

I nearly published the wrong cause: CLI 2.1.245 → 2.1.247 since the derivation
looked like obvious version drift. Checked it — `Monitor` appears in **both**
bundles. Not drift. The inference was wrong on the day it was written.

The list also already had a category for this failure mode — *"surface a
deferred tool that could do any of the above"* — holding `ToolSearch` alone.
That mitigation assumed deferred tools are reachable only *through* ToolSearch.
Monitor was called with no ToolSearch call anywhere in the transcript. **An
anticipated failure mode with the wrong mechanism is not a mitigated one.**

There is no allow-list escape: `claude --help` confirms `--allowedTools` is
auto-approval, not restriction. `--disallowedTools` is the only mechanism, so
the floor's denial is exactly as complete as a hand-typed list of names — and a
list is a snapshot of a registry that grows.

**Fix:** `prompt-engineering:e67d419`, patch-id `7e470bc8165b2424`. Four new
categories (run-a-command-by-another-name, delegation, non-file-shaped file
access, worktree entry), a named regression test for the tools actually
observed leaking, and gate 5 now prints each floor's attempted tool names so
the next leak names its own missing entry. Verified by re-running:

    pre-fix   [ FAIL ]  tools attempted: CronList, Monitor      L1 0.50  L2 3.00
    post-fix  [ PASS ]  tools attempted: Bash, Glob, Grep, Read L1 0.00  L2 0.00

Once denial holds, the Opus floor is 0.00 — the same as Sonnet's.

**Severity:** high — the control every other gate rests on was not enforcing
what it claimed, silently, for the whole study.

**Status:** fixed-verified

**Valid:** dated 2026-08-27

## F-33 — Runs were scored against other runs' trees for the whole study; the fix existed, was documented, and was never wired into the driver

**Observed:** 2026-08-27, while checking why the freshly-fixed `notools-opus`
floor still reported `l0_fix_correct: true` in both runs. The agents had refused
outright — *"every tool I'd need is disabled … I'm not going to hand you a diff
for code I haven't seen"* — and made only denied calls.

**Expected:** an arm that touches nothing scores `l0 = false`. Its own round-1
zero-call run had.

**Got:** `golden_after.international_total = 108.25` (the *fixed* value) against
`golden_before = 100.0`, and `changed: 13`. The after-tree was not the agent's
tree.

**Root cause, verified rather than inferred.** The after-hook queue pairs each
run's post-edit tree with that run's checker **strictly by FIFO arrival order**
(`check_blast.py:_consume_after_queue`). One unconsumed entry offsets every
later pairing by one, permanently. `run_pilot.sh` never isolated the queue, so
every round since the first shared `~/.prompt-tdd/blast-radius-after-queue`.

The proof is airtight and was sitting on disk: the shared queue held **three
unconsumed entries**, two of them timestamped 08:32 carrying `100.0` — which can
only have come from round `20260827-083202`'s two floor runs, the only runs at
that minute. Those runs were scored `108.25`. Their own entries were still in
the queue while I read them. They had been scored against a tree left behind by
an earlier run that patched the fixture through `Monitor` (F-32).

The same round shows the offset in its cleanest possible form: **l0 is inverted
between arms.** The floor "fixed the bug" (1.00) while reaching zero dependents
and asking nothing; the two ceilings "failed to fix it" (0.00) while reaching 11
and 12. Each was scored against the other's tree.

**The mitigation already existed and was never wired in.**
`BLAST_AFTER_QUEUE_DIR` is read by both `after_hook.py` and `check_blast.py` at
import time, and `README.md` documents this exact export as the fix, naming this
exact hazard: *"one entry left behind by a Ctrl-C'd prior invocation shifts every
subsequent pairing by one, permanently."* A hazard can be fully understood,
documented, and given a working mitigation, and still fire — because nothing
made the driver use it. **A mitigation that is opt-in is a mitigation that is
off.**

**Why it survived the whole study.** Nearly every *tooled* run fixes the bug, so
an offset among runs that all produce the same tree pairs wrong values that
happen to be **equal**. Only an arm whose runs differ from each other can expose
it — which is why the floor found it, on the day the floor was added. Same shape
as F-32, same session, and the reason that shape is now stated once in
`RESULTS.md`.

**Blast radius, stated precisely because it is narrower than it first looks.**
`l0_fix_correct`, `l3_silent_changes` and the `broken-after-tree` verdict all
read the golden pair and are unusable wherever the offset was non-zero — this
puts the published "100% fix rate" and every silent-change count in doubt. **L1
and L2 are computed from the trace and are untouched**, which is every headline
reach finding in `RESULTS.md`.

**Fix:** `prompt-engineering:163bdde`, patch-id `9874f1fbd20a6e66`. Private
per-round queue, plus a drain check after **every arm** rather than once at the
end — isolation alone would have left a within-round offset exactly as silent as
the cross-round one, and the per-arm check names which arm first went out of
step.

**Severity:** high — silent, plausible-looking wrong values on two published
metrics, with no error anywhere.

**Status:** fixed-verified

**Valid:** dated 2026-08-27

**Rests on:** F-32, same session — the 108.25 tree the offset served up was
produced by that entry's `Monitor` leak.

## W-24 — The control arm found two instrument defects instead of the effect it was added to measure — because it was the only arm whose runs differed

**Observed:** 2026-08-27. Asked to add the Opus hinted ceilings and an Opus
floor, I justified the floor as testing a claim about *model priors*: "is this
task answerable without tools, for a stronger model? Sonnet's 0.00 does not
transfer." It ran once and found **two instrument defects instead** (F-32,
F-33), both of which had been silently corrupting the study from the start.

**Pattern:** **A control arm is the only instrument that can detect a broken
control mechanism, and it detects it by being the arm whose runs DIFFER from
each other.**

The mechanism is worth stating precisely, because it says *which* control to add
and not merely "add controls":

> Nearly every *tooled* run in this eval behaves the same way — reaches many
> dependents, fixes the bug. So a broken instrument that pairs, denies, or scores
> the wrong thing pairs values that **happen to be equal**, and produces output
> indistinguishable from correct. The defect is invisible precisely *because* the
> arms agree. An arm that produces a **different** value is the only probe that
> can separate them.

Both defects fit exactly. The FIFO queue offset (F-33) served every run some
earlier run's tree — invisible while every tree was the fixed one, glaring the
moment a floor run left the tree *unfixed*. The deny-list hole (F-32) let any
model reach a shell — invisible while Sonnet never reached for `Monitor`, glaring
the moment Opus did.

**Counterfactual, and it is concrete.** Without this arm, the two ceilings would
have run, produced clean-looking numbers, and I would have published a completed
2×2×2 with a correct headline (the hint *is* additive) resting on a "100% fix
rate" that was partly other runs' trees, and against a floor of 0.00 that
recorded Sonnet's incuriosity rather than any enforced denial. Both defects would
have survived into every future round, and the fix rate is exactly the kind of
number that gets quoted without its provenance.

**What I would have got wrong without checking.** Twice in one session the
obvious cause was the wrong one, and both were cheap to check:

1. Gate 5 failing read as "Opus answers from priors" — a *finding*. The
   transcripts showed a shell. Not a model fact at all.
2. The deny-list omission read as version drift (CLI 2.1.245 → 2.1.247 since the
   list was derived). `Monitor` is in **both** bundles. The real cause was an
   untested inference written at the same confidence as a verified fact.

**Confirming data points:**
1. F-32 and F-33, this session — two independent defects, one arm, one run.
2. W-23, this session — same underlying shape one level up: re-executing found
   what re-reading structurally could not.

**Impact:** high — the arm cost ~$1 and invalidated two published metrics before
they were quoted again.

**Promote-when:** a second session where a control arm surfaces an instrument
defect rather than the effect it was added to measure. At two, promote to the
`eval-design` memory as: *"add the control whose runs will DIFFER from the
others — an instrument that is broken in a way all your arms share cannot be
detected by any of them."*

**Status:** validated — single session, two independent defects found and fixed.

**Valid:** dated 2026-08-27

**Rests on:** F-32 and F-33, same session — this win is their shared
counterfactual, not independent evidence.

## F-34 — I wrote a timing verdict that was true by construction — the trigger call is always the first opportunity

**Observed:** 2026-08-27, guide-injection use study. I authored the measurement
rubric (`docs/evals/data/2026-08-27-guide-injection/rubric-BRIEF.md`) that ten
subagents executed verbatim.

**When:** Writing M2 (timing). I defined `timing_verdict: LATE` as
`first_opportunity_turn < turn_index`, where `first_opportunity_turn` is the first
assistant turn containing a tool call of the class the guide governs.

**Expected:** A verdict that discriminates well-timed from late injections.

**Got:** A verdict that is **true by construction for every push injection**. The
triggering `tool_use` sits on assistant turn *N*; the `tool_result` carrying the
injection lands at *N+1*. So the trigger call is *itself* the first opportunity and
always precedes the injection by one turn. Every one of the 81 injections scored
`LATE`, including `symbol-navigation` firing on a session's **first** symbol call —
the earliest arrival physically possible.

Caught by OPUS-4, unprompted, in its own result: *"`LATE` is structurally forced
for push injections — the trigger call is itself the first opportunity."* OPUS-5
independently reported the same and recorded raw numbers so the verdict could be
flipped downstream.

**Probable cause:** I derived the rule from the *concept* (did the session need
this before it arrived?) without tracing the mechanism that generates the two
numbers. The rule reads as a discriminator and is arithmetic on a quantity the
injection itself produces.

**Workaround:** Recomputed in aggregation from components the agents did record:
`gap = turn_index − first_opportunity_turn`; **gap ≤ 1** = arrived at first
contact, **gap > 1** = genuinely late by that many turns. Corrected figures: 11%
at first contact, 89% genuinely late, median lateness 320 turns. Added
`TRIGGER_ONLY` (`opportunities_after == 0`, 51%) as the complementary measure —
proposed by SONNET-2, also unprompted.

**Severity:** med — no wrong number was published (the aggregate was recomputed
before any report), but the defect was in a rubric ten agents had already
executed, and it was caught downstream rather than at authoring.

**Status:** fixed-verified — corrected in aggregation; the corrected rule and its
rationale are recorded in the eval doc and in the bug file's measured section.

**Valid:** dated 2026-08-27

Describes the rubric as authored on that date; the corrected rule is what the
published figures use.

**Rests on:** the ordering of `tool_use` and `tool_result` in Claude Code
transcripts — the trigger necessarily precedes the injection it causes.

**Kin:** `reconnaissance-patterns:R-5` — *a check that is computed from the thing
it judges cannot fail; treat its green as unmeasured.* This is that law applied to
a metric rather than to a gate, and it fired in the rubric of the person who had
loaded the law earlier the same session.

## F-35 — I measured that guides grow, then used today's sizes for historical injections — overstating every byte figure 1.17x

**Observed:** 2026-08-27, guide-injection use study, corpus-scale delivery census
over 1,705 sessions.

**When:** Computing total delivered guide bytes. I looked up each topic's size on
disk *today* (`wc -c src/prompts/guides/*.md`) and multiplied by its injection
count.

**Expected:** A faithful byte total.

**Got:** A **1.17× overstatement** — 34.4 MB reported against 29.5 MB actually
delivered. Per topic the error is worse and uneven: `tracker-conventions` injections
have a median as-delivered size of **24,836 B against today's 34,333** (ratio 0.72),
`librarian` 17,146 vs 20,545 (0.83). The minimum observed `tracker-conventions`
injection is 10,395 B — the pre-growth size.

Caught by OPUS-5 on its own transcript: *"the byte table overstates old transcripts
~26% … guide mtimes postdate this 2026-07-28 session. Likely systemic."* It was
systemic. SONNET-1 independently reported the same for its 2026-08-25 session
(28,032 B as-injected, not 34,333).

**Probable cause:** This is the part worth keeping. **I had measured the growth
myself, earlier in the same session, and then used current sizes anyway.** The
growth measurement is `W-22`'s third confirming datapoint —
`tracker-conventions` 10,377 → 34,333 B in ten days — and I wrote it up as a
finding roughly an hour before building the byte table. Knowing that a quantity
varies over time did not stop me treating it as a constant, because the two facts
were used for different purposes: growth was *a finding about the corpus*, size was
*a lookup for arithmetic*, and nothing connected them.

**Workaround:** Re-measured the bytes actually present **between the opening and
closing markers** of each injection (`scripts/probe_guide_injection.py`, and
`truebytes.json` in the eval's data dir). Every published figure now uses
as-delivered bytes. The instrument's docstring and its PROBES.md row both name this
trap explicitly, because the naive lookup is the obvious thing to do.

**Severity:** med — corrected before publication, but it touched every byte figure
in the study, and the correct method costs no more than the wrong one.

**Status:** fixed-verified — all figures recomputed; the probe measures inter-marker
bytes by construction, so the mistake is not re-expressible through it.

**Valid:** invariant

The general form — a measurement of a *current* value is not valid for *historical*
events of the same kind — does not decay. The specific ratios are dated 2026-08-27.

**Rests on:** guide files being edited over time while transcripts record what was
delivered at the moment of delivery; any append-only log of a mutable artifact has
this shape.

**Kin:** `W-22` (this log) — whose own evidence supplied the growth figure that
should have prevented this. `reconnaissance-patterns:R-89` — freshness is a
property of the copy that served you; this is its retrospective twin, where the
copy that served the *past* is the one you must reconstruct.

## W-25 — Handing each subagent the controller's own measurement, with a loud-fail gate, made three controller defects surface downstream

**Observed:** 2026-08-27, guide-injection use study. 10 subagents, one transcript
each, all executing one shared rubric
(`docs/evals/data/2026-08-27-guide-injection/rubric-BRIEF.md`).

**Pattern:** When fanning out a measurement across N agents, give each agent **the
controller's own independently-measured value for its slice**, and instruct it to
**STOP and report `calibration: FAIL`** on any mismatch rather than adjust its
parser. Pair it with explicit permission to return nothing: *"If your transcript
shows 0% utilisation across every injection, that IS the finding. Do not hunt for a
nicer number."*

Two properties, and the second is the one that is easy to miss:

1. It makes the results **addable**. 10 of 10 passed calibration, so the 81
   injections aggregate into one number instead of ten incomparable ones.
2. It inverts the error-flow. **Three of this study's four instrument defects were
   caught by subagents, not by the controller** — the degenerate `LATE` verdict
   (`F-34`, OPUS-4), today's-bytes-on-historical-injections (`F-35`, OPUS-5 and
   SONNET-1 independently), and an injection channel the rubric could not see
   (`queue-operation` lines, SONNET-2). A fourth refinement — that a prescribed
   shape which is the tool's *default value* is not evidence of use — came from
   OPUS-4 arguing **against its own positive result**.

**Counterfactual:** Without the calibration gate, a diverging parser produces a
plausible number and nothing contradicts it — the failure is silent and arrives as
data. Concretely, three defects would have shipped: every byte figure inflated
1.17×, a timing verdict reading 100% `LATE` including the earliest arrival
physically possible, and no route by which any agent could tell me the rubric was
wrong. The controller would have had ten confirmations and zero contradictions,
which is exactly the shape of a measurement that cannot fail.

**Confirming data points:**
1. This study — 10/10 calibration PASS; 3 controller defects surfaced downstream;
   1 agent argued down its own U2 credit.
2. Pending: any future fan-out measurement using the same gate.

**Impact:** high — the gate is cheap (one extra line per dispatch prompt, computed
from work the controller has already done) and it is the only mechanism in the
design by which a subagent can contradict the controller.

**Promote-when:** A second fan-out measurement where the calibration gate catches a
controller-side defect. At 2 datapoints, promote to the reconnaissance skill's
dispatch guidance as: *"When fanning out a measurement, hand each agent your own
value for its slice and require a loud FAIL on mismatch — a subagent that cannot
contradict you can only confirm you."* Route as craft-shaped: it needs no codescout
dialect and holds for any multi-agent measurement.

**Status:** validated — single study, three caught defects, awaiting the promotion
criterion.

**Valid:** dated 2026-08-27

One study; the promote-when threshold of 2 is not yet reached.

**Rests on:** the controller having an independent measurement of each slice
*before* dispatch — which is what makes the gate a real check rather than the
agents' own work reflected back at them.

**Kin:** `F-34`, `F-35` (this log) — the two defects this gate surfaced.
`reconnaissance-patterns:R-5` — a check computed from the thing it judges cannot
fail; the calibration value is sourced independently precisely to avoid that.

## F-36 — Four defects marked fixed were all still live in the sibling eval — including F-1, whose code was still deleting evidence on every run

**Observed:** 2026-08-27, on "fix all" — sweeping the sibling eval for the
deny-list hole F-32 had just found in `blast-radius`.

**Expected:** one defect to port.

**Got: four.** `hidden-info` — which `blast-radius` was modelled on, and which
several of these fixes were *originally derived from* — still carried every one:

| defect | ledger | state in hidden-info |
|---|---|---|
| deny-list missing `Monitor` | F-32 | present, with the false justifying comment verbatim |
| output on tmpfs | F-31 | `OUT=/tmp/hidden-pilot` |
| **recursive wipe of a fixed output path** | **F-1** | live, on the normal path, every invocation |
| arms keyed by log filename stem | F-24 | present in `gates.py` |

F-1 is the one that stings. Its own title is *"Fixed output path destroyed the
evidence for the headline figure"*, it is marked **`fixed`** in this ledger's
index, and the code that does exactly that was still running here — not on a
crash path, on the ordinary one. Every `hidden-info` pilot deleted the evidence
behind the previous one's published numbers.

**Root cause: every one of these was fixed at the INSTANCE, never at the class.**
Each was found in one scenario, fixed there, and closed. Nothing asked "where
else does this pattern live?" — and the answer was always "the sibling, which
shares most of the code and in one case the comment character-for-character."

**Why no test caught the shared ones.** Both scenarios had green suites. Each
pinned *its own copy* of the same wrong deny-list, so each suite confirmed the
scenario matched its own expectation, and neither could see that both
expectations were wrong. **A per-scenario test cannot detect a defect both
scenarios share** — the same shape, one level up, as the thing the floor arm
exists to catch (W-24): a control that agrees with everything around it detects
nothing.

**One defect I would have CREATED.** F-24's stem-collision was *unreachable* in
`hidden-info` while the driver kept a single wiped directory — there was never
more than one round. Fixing F-1 (timestamped rounds, nothing deleted) makes it
reachable. Porting the driver fix without the scorer guard alongside it would
have introduced the silent undercount rather than avoided it. Verified after
the fact against the rescued archive: `gates.py` now REFUSES its root, naming
`hidden-cs` in **three** logs. Un-guarded it would have kept one and reported a
full-looking `n=2`.

**Order of operations mattered.** Evidence was rescued to disk *before* touching
`run_pilot.sh` — the script being repaired was the script that would have
destroyed it. 184 files, verified readable and run-counted, and the rescued
round re-scores clean (12 runs, $4.30).

**What was NOT contaminated, checked rather than assumed:** no `notools` log
exists in any preserved `hidden-info` round — the floor arm appears never to have
been run, so there was no published floor claim to retract. And no `Monitor` /
`CronList` call appears anywhere in that evidence, so no tooled arm was affected
either. The hole was real and prophylactic here.

**Fix:** `prompt-engineering:ee73088` (patch-id `4e36dee3bee56832`) ports all
four. `prompt-engineering:109f35b` (patch-id `7f71d6ba9a0e36fb`) closes the
*class*: a cross-scenario parity test that fails when any two floors' deny-lists
diverge. It lives in `tests/` because `testpaths = ["tests"]`, so a bare
`pytest` collects it while collecting no scenario tests at all (F-13) —
divergence now fails the default run. Its scenario list is **derived by glob,
never enumerated**, since enumerating would reproduce this very bug. Verified by
mutation, not by being green: removing `Monitor` from one scenario's config
alone fails 5 tests and names each missing tool.

**Severity:** high — one live defect was actively deleting published evidence,
and three more were one sibling away from every fix that had been declared done.

**Status:** fixed-verified — 307 (hidden-info) + 379 (blast-radius) + 28
(parity) passing.

**Valid:** dated 2026-08-27

**Rests on:** F-1, F-24, F-31, F-32 — this entry is the observation that all four
remained live in a sibling after being closed.

## F-37 — I cleared the proxy as a cause after checking only its response side, then blamed the API for a defect in our own request

**Observed:** 2026-08-27, testing whether Langfuse captures thinking text, so the
guide-injection study's blind spot could be closed.

**When:** After the llm-proxy streaming fix (`b01ee4c`) landed and the service
restarted. I measured, then concluded, then wrote the conclusion into three durable
documents.

**Expected:** Either thinking text appears (blind spot closes) or it does not, in
which case find out why.

**Got (measurement, accurate):** 150 Langfuse observations, 39 `thinking` blocks,
**0 with non-empty text**, signatures fully populated (524–3,988 chars). Token
accounting on one `claude-opus-5` session: 1,210 output tokens billed against 252
characters of returned text — ~95% of billed output never delivered.

**Got (conclusion, WRONG):** *"The API returns the signature, not the reasoning."*
I declared the blind spot irreducible and wrote that into
`llm-proxy:docs/issues/archive/2026-08-27-streaming-langfuse-output-drops-thinking-and-tool-use-blocks.md`,
codescout's `docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md`
and `docs/evals/2026-08-27-guide-injection-use.md`.

**Reality:** Two request-side causes, either sufficient alone, both in our own
request construction. Claude Code sends `anthropic-beta:
redact-thinking-2026-02-12` — a client-side terminal-UI choice, **not** an
Anthropic restriction — and `thinking: {"type": "adaptive"}` with no `display`
key, which several current models default to `"omitted"`. Fixed the same hour by a
concurrent session (`llm-proxy:6f3cb62`, 09:07): strip the beta token, set
`display=summarized` when the client omitted it. Post-fix, live traces carry
readable thinking on `claude-opus-5` (mean 1,188 chars) and `claude-sonnet-5` (mean
316) — and this very session's JSONL began carrying non-empty thinking blocks
within minutes, so CC's transcript redaction was downstream of the same cause.

**Probable cause — two errors, the first structural:**

1. **I eliminated a component by verifying one half of it.** A proxy has a request
   side and a response side. I read `BlockAcc::apply_delta`, confirmed it handles
   `thinking_delta`, confirmed the running binary was the fixed one, and wrote *"so
   the proxy is excluded as the cause"*. The request path — the headers it forwards,
   the `thinking` object it passes through — was never opened. Having verified
   something real and specific made the exclusion feel earned.
2. **I treated a non-discriminating figure as decisive.** The token accounting
   (~95% of billed output never returned) was presented as *"the decisive one"*. It
   proves reasoning happened and was not returned. It is **equally consistent** with
   *the model omitted the text* and *our own request asked for it to be omitted*. A
   measurement that cannot separate the hypotheses cannot choose between them, no
   matter how striking it is.

**Workaround:** All three surfaces corrected, with the superseded conclusion left
visible rather than deleted — the measurements were sound and the record of what
was believed is the useful part. The limits now read "unavailable for these
transcripts, now fixed, so a re-run can measure it" instead of "irreducible".

**Severity:** high — not for the measurement (unchanged: those transcripts genuinely
lack thinking, so `U0_UNUSED` stays an upper bound) but for the **claim of
irreducibility**, which is the kind of statement that stops anyone re-running the
study. It reached three durable documents, one in another repo, inside 40 minutes.

**Status:** fixed-verified — corrections landed in all three surfaces; post-fix
capture verified independently in Langfuse and in this session's own JSONL.

**Valid:** invariant

The specific cause is dated 2026-08-27; the reasoning error — clearing a
two-sided component after checking one side — does not decay.

**Rests on:** a proxy mediating both directions of a request, so "the proxy is
fine" is two claims, not one. Generalises to any middlebox: a gateway, an ORM, a
serializer, a CI runner.

**Kin:** `F-34` (this log) — also a conclusion true by construction rather than by
evidence. `W-25` — the third of three controller defects surfaced by someone other
than the controller, and the only one caught *after* publication.
`reconnaissance-patterns:R-104` — a negative result is evidence about the
instrument, not about the world; here the instrument was the request I never read.

## F-38 — I selected traces by content-matching a prompt string my own session contained, and published the resulting table as a between-condition finding

**Observed:** 2026-08-27, investigating why eval runs logged empty `thinking`
blocks while some other traffic did not.

**What I did:** to find which runs had reasoning text, I queried Langfuse for
traces in a time window and selected the ones belonging to my probes by
**searching each trace body for the prompt string** (`'windowless room' in
json.dumps(trace)`). I then tabulated thinking-chars by that selection and
published a clean-looking result:

> manual `claude -p` probes — 40+ blocks, 163–2655 chars
> eval-harness runs — 8 blocks, all empty

I filed an issue on that table, listing seven ruled-out causes.

**The table was entirely wrong.** Re-attributing the same traces by
`sessionId` showed **every populated block belonged to my own interactive
session**, and **every** headless probe was empty. The harness-vs-probe axis I
had "measured" did not exist.

**Root cause — and it generalises past this tool.** I was searching a log that
records *my own activity alongside the experiment's*. The prompt string was in
my session because **I had typed it into the shell commands that launched the
probes.** The act of running the experiment put the experiment's identifying
marker into the observer's own record. Content-matching then swept both in and
attributed all of it to the subject.

> **When you search a shared log for evidence of your experiment, you are in
> that log too.** A content match cannot separate the observer from the
> observed; only an identifier the runs themselves carry can.

The failure is quiet in the worst way: it does not error, it does not look
sparse, and it does not look noisy. It produces a **clean table with a large
effect in the direction you expected** — which reads as strong evidence rather
than as an artifact. The eight genuinely-empty rows were real, so the table was
half true, which is what made it convincing.

**What settled it:** an A/B that (a) varies exactly **one** input and (b) matches
results by `session_id` captured from each run's own output, never by content.
Same profile, model, `--permission-mode`, `--strict-mcp-config`, deny-list and
cwd; only `--output-format` differing:

    prompt 1   json -> 0 chars     stream-json -> 538 chars
    prompt 2   json -> 0 chars     stream-json -> 152 chars

Run twice with different prompts *because* I had already been wrong twice.

**Second error, same session, worth pairing:** before the filter bug I had
concluded "no capture defect — the `notools` arm just barely reasons." A fully
tooled 334 s Opus arm was equally empty. Both errors have one shape:
**concluding from a measurement whose selection step I had not checked.** The
first mis-selected an arm, the second mis-selected traces.

**Fix:** `llm-proxy:7824e3f` rewrites the issue with the corrected axis and keeps
the retraction visible rather than editing it away. The bad-filter story is
recorded *in the issue*, because a reader who only sees the final cause would
have no reason to distrust a content-matched table next time.

**Severity:** high — a fabricated between-condition table, published to an issue
tracker, with seven "ruled out" causes resting on it.

**Status:** fixed-verified — corrected axis reproduced twice.

**Valid:** dated 2026-08-27

**Rests on:** F-32 and W-24, same session — the third instance of reading a
measurement before checking what it selected.

**Kin:** `F-37` (this log, concurrent session) — same investigation from the other
side, and the two are COMPLEMENTARY rather than contradictory. F-37 found the
request-side cause (`redact-thinking` beta + absent `thinking.display`) and
verified the fix on STREAMING traffic: opus mean 1,188 chars, sonnet 316. Those
figures match what my own interactive session shows. What my A/B adds is the
residue that verification could not have covered: with `--output-format json` the
text is still empty after that fix, twice, on two prompts. Read together: the fix
works, and it works on the path it was tested on.

Both entries are also the same error one level apart — F-37 cleared a component
after checking one of its two sides; F-38 cleared a hypothesis after checking a
selection it never validated. Neither measurement was wrong; both selections
were.

## W-26 — Capturing the mechanism killed an axis that two outcome-comparisons got wrong

**Valid:** invariant

**Observed:** 2026-08-27, resuming the thinking-token investigation. The filed issue
had already named its own next step — *"log the forwarded `thinking` object … for one
`json` and one `stream-json` request. That single observation confirms or kills it."*
It was the right instruction and it had not been followed, because two rounds of
outcome-comparison had each produced a confident axis instead.

**Pattern:** When an investigation has produced an **axis** ("A differs from B") but
no **mechanism**, capture the mechanism before designing another comparison. A
comparison establishes that two groups differ. It never establishes which knob names
the group — the knob is supplied by the investigator, from whatever varied most
visibly between the groups they happened to select.

**What the capture cost and what it returned:** a ~90-line recording shim in front of
the proxy, ~20 minutes. It showed both arms send `thinking: {"type": "adaptive"}` with
no `display` and `stream: true` — so the client never sets `display` (killing the
hypothesis), and `--output-format` never reaches the wire at all (killing the axis).
Two claims, one observation, no statistics.

**Counterfactual:** The issue's preferred fix was *"force `display: \"summarized\"`
even when the client set it explicitly, behind a config flag."* Implemented, it would
have been a behaviour change to a shared observability proxy — one all three CC
profiles route through — that **overrides nothing**, because no client on this machine
sets `display`. It would have shipped green, changed no measurement, and left a
permanent flag documenting a client behaviour that does not exist. The alternative fix
was re-shaping the harness adapter to `stream-json`, changing an eval condition to
correct a wire difference that is not there.

**Second-order:** the capture also falsified the *symptom*. Six session-id-matched
runs across shim/no-shim and sonnet/opus all logged non-empty thinking (212–345
chars). Neither the mechanism nor the effect survived contact with direct
measurement — and only the mechanism check could have told me which.

**Confirming data points:**
1. This session — hypothesis and axis both killed by one request capture, after two
   comparison rounds each produced a wrong axis (`bug-fix-session-log` peer entry
   F-37; this log's F-38).
2. F-38 itself — an axis ("harness vs manual probe") produced by a selection the
   investigator never validated.
3. **F-41, ninety minutes after this entry was written** — I concluded "semantic search
   doesn't find these" from a single *filtered* query and published it to the user. The
   unfiltered control refuted it immediately. The promote-when criterion below has
   therefore fired: three datapoints, the third against my own report rather than a
   third-party system.

**Impact:** high — prevented a no-op behaviour change to shared infrastructure, and
converted an open bug into a `zombie` with its residue named.

**Promote-when:** a third investigation reaches a wrong axis by outcome-comparison
where a mechanism capture was available and cheap. At three datapoints, promote to
CLAUDE.md as *"Before running another comparison, ask whether the mechanism can be
observed directly — and if it can, observe it first."*

**Status:** promote-when FIRED 2026-08-27 at three datapoints (F-37, F-38, F-41), then
**amended the same day by F-42**. The rule survives and gains a required clause:

> Before running another comparison, ask whether the mechanism can be observed directly —
> and if it can, observe it first, **in the population the claim generalises to**. Before
> believing a filtered query's result, re-run it unfiltered.

F-42 is the counter-example that makes the clause necessary: I *did* capture the mechanism,
and still refuted a true hypothesis, because I sampled manual runs under a rich profile as
a stand-in for harness runs under an empty one. A mechanism capture inherits the sampling
problem whole — and it is quieter than a bad comparison, because a comparison at least
looks like it needs a control group while a capture looks self-evidently authoritative.

Awaiting promotion to CLAUDE.md in the amended form.

## F-39 — A profile's settings.json `env` silently overrules an exported ANTHROPIC_BASE_URL

**Valid:** conditional — Claude Code changes its settings/env precedence

**Observed:** 2026-08-27, trying to route one `claude -p` run through a recording
shim by exporting `ANTHROPIC_BASE_URL=http://localhost:8099`.

**Expected:** the exported variable routes the child process, the way it does for
every other program.

**Got:** both runs exited 0, produced correct output, and reached `:8082` — the shim
logged **zero** requests. Claude Code's precedence is:

    CLI --settings env  >  profile settings.json env  >  inherited shell env

`~/.claude/settings.json` and `~/.claude-sdd/settings.json` both pin
`env.ANTHROPIC_BASE_URL`, so the export was overruled without a word. Adding
`--settings '{"env":{"ANTHROPIC_BASE_URL":"http://localhost:8099"}}'` routed on the
first try.

**Why it is worse than the ambient-inheritance bug it neighbours:** that one fails
when *nobody* sets the variable. This one fails when somebody sets it **explicitly
and correctly**, and is silently overruled. Nothing in the run reports the
disagreement — there is no warning, and the run succeeds.

**What it cost here:** the routing guard I shipped in `prompt-engineering:6d7c664`
the previous session — added specifically to stop eval runs bypassing capture — is
**decorative as a router**. It exports the variable and then health-checks *the value
it just exported*, which the client never reads. It has agreed with reality this whole
time only because `settings.json` happens to name the same URL. A guard that validates
its own input rather than the client's behaviour cannot fail in the direction it was
built to catch.

**Same shape as F-32 and F-37:** verify one side of a two-sided thing, report it as
the whole. Here the export side was verified and the consumption side assumed.

**Fix:** `prompt-engineering:8ac0d63` resolves `env.ANTHROPIC_BASE_URL` from
`$CLAUDE_CONFIG_DIR/settings.json` — the file the child will actually read — falls
back to the exported value, and health-checks that. Recorded in
`llm-proxy:docs/issues/2026-08-27-no-detection-when-traffic-bypasses-the-proxy.md`
§ *Settings `env` overrides the environment*.

**Severity:** high — it silently defeats explicit routing, and the caller-side fix an
open issue prescribed was implemented and did nothing.

**Status:** fixed-verified — `bash -n` clean, resolution returns
`http://localhost:8082` from `~/.claude-sdd/settings.json`, 379 tests pass.

## F-40 — Two sessions added the same routing guard to one file within an hour

**Valid:** dated 2026-08-27

**Observed:** 2026-08-27, editing `prompt-engineering:scenarios/blast-radius/run_pilot.sh`
to fix the guard described in F-39. Grepping for `ANTHROPIC_BASE_URL` returned **two**
routing guards in the committed file — one at lines 42-66, another at 113-128.

**Got:** `git blame` + `git merge-base --is-ancestor` show `5508336a` (09:26) added the
first and `6d7c664` (an hour later, mine) added the second, above it. Same author
identity, different sessions, same shared checkout. Neither grepped the file for an
existing check before adding one. Both did the same thing — export the variable, curl
it, exit 2 — differing only in cosmetics (`localhost` vs `127.0.0.1`, base path vs
`/v1/messages`, an `LLM_PROXY_OPTIONAL` escape hatch in one).

**And both were wrong the same way**, per F-39: an export cannot route Claude Code
past a profile that pins the variable. So the duplication was not merely redundant —
it doubled a check that could not fail in the direction it was built to catch, and the
second copy read as independent corroboration of the first.

**Probable cause:** the shared-checkout concurrency this project runs on. Peer sessions
commit to the same working tree, so "is this already handled?" is a question about a
file that changed since my last read — exactly the seam the reconnaissance skill
covers. I scouted the *proxy's* code carefully this session and did not scout the file
I was about to add a guard to.

**Severity:** med — no wrong behaviour shipped (both guards passed), but it cost a
duplicated maintenance surface and would have cost a confusing double-failure message
the first time the proxy was genuinely down.

**Fix:** collapsed into one guard in `prompt-engineering:8ac0d63`, with a comment at
the removed site recording why there were two.

**Status:** fixed-verified — one guard remains, `bash -n` clean, 379 tests pass.

## F-41 — I reported "semantic search doesn't find these" from one filtered query, with no control

**Valid:** dated 2026-08-27

**Observed:** 2026-08-27, verifying that the three repos' new trackers were discoverable.

**Got:** I ran one query — `find(kind="tracker", semantic="<paraphrase>", limit=8)` — got
eight unrelated trackers, and reported to the user that *"semantic ranking doesn't find
these… they're indexed and embedded; they just don't rank."* I offered it as a limitation
I could not fix.

**It was wrong, and one control refuted it.** The same query **unfiltered** returns the two
genuinely-best artifacts at #1 and #3. They are `kind: bug`; my `kind="tracker"` filter
excluded them, and `semantic_find` backfilled the page with the nearest surviving trackers
(`src/librarian/catalog/find.rs:255-276` — the loop widens `k` until the page is *full*,
not until the results are *close*). A verbatim-title query with the same filter returns the
target at #1, proving filter and ranking both work.

**Why it got past me:** the response carries no score and no starvation hint, so a starved
page is byte-identical in shape to a satisfied one. But the response's silence is only half
of it — the other half is that I ran a filtered query and no control, which is the same
omission as F-38 (selection never validated) and the same shape W-26 names: *a comparison
establishes that two groups differ, never which knob names the group.* I had written W-26
ninety minutes earlier.

**Third datapoint for W-26, and the sharpest**, because here the wrong conclusion was not
about a third-party system — it was my own report to the user, delivered with a hedge
("I didn't want to report it as solved") that made it read as careful rather than
unverified. Confidence-hedging is not a substitute for a control.

**Severity:** med — one wrong claim published to the user, self-caught within the same
session when the user asked for proof. Nothing was built on it.

**Fix:** the real defect is filed as `docs/issues/archive/2026-08-27-semantic-find-fills-the-page-past-relevance-with-no-score.md`
(`4b257f3be10ec322`) with a one-call reproduction. **FIXED 2026-08-27 in `e4569fcc`**
(patch-id `4008d77b`): responses now carry a per-item `distance` and a `semantic_starved`
hint, so a starved page is no longer byte-identical to a satisfied one. The behavioural
half still stands
workarounds there: never conclude "nothing is indexed about X" from a single *filtered*
semantic query — drop the filter and re-run before believing it.

**Status:** fixed-verified — claim retracted to the user in-session; bug filed with proof
and a reproduction that was re-run after filing and reproduced byte-identically.

## F-42 — I captured the mechanism in the wrong population, and refuted a hypothesis that was true

**Valid:** invariant

**Observed:** 2026-08-27. Having written W-26 that morning — *capture the mechanism, don't
run another comparison* — I did exactly that, and reached a confidently wrong conclusion
anyway.

**What I did.** An open issue said harness runs log empty reasoning text, and named a
hypothesis: the client sets `thinking.display` explicitly, so the proxy correctly declines
to override it. I built a recording shim, captured the actual request bodies, and found
`{"type": "adaptive"}` with **no** `display`. I declared the hypothesis dead, moved the
issue to `zombie`, and told the user the symptom no longer reproduced.

**What was wrong.** All six captures were **manual `claude -p` runs under a rich profile**.
The issue is about **harness runs under a generated, empty profile**. Capturing the harness
directly, hours later:

    ~/.claude-sdd (rich)                      {"type":"adaptive"}                     212-345 chars
    ~/.prompt-tdd/profiles/... ({} settings)  {"type":"adaptive","display":"omitted"}   0 chars

Same prompt, same flags, same model, same tool count; only `CLAUDE_CONFIG_DIR` differs. The
original hypothesis was **right**, and I refuted it with evidence that was accurate about
something else.

**The refinement to W-26.** Capturing the mechanism beats comparing outcomes — that still
holds, and it is what finally settled this. But a mechanism capture inherits the sampling
problem whole: **it names the axis only for the population you sampled.** I sampled the
population that was convenient to run, not the one the claim was about, and nothing in the
result flagged the substitution. A comparison at least *looks* like it needs a control
group; a mechanism capture looks self-evidently authoritative, which is exactly what makes
this failure quieter than F-38's.

**Compounding error.** The first framing of that issue — "harness vs manual probe" — was
true all along. It had been rejected because its *evidence* was a contaminated selection
(F-38). I then treated an unsupported claim as a refuted one and spent a second pass
proving the opposite of the truth. Unsupported and false are different things, and
conflating them cost the whole middle of the investigation.

**Severity:** high — a true hypothesis was closed, an issue was mis-statused `zombie`, and
a wrong conclusion reached the user twice (once as "the axis is dead", once as "the symptom
is gone"). Self-caught only because a paid re-run happened to look at thinking text again.

**Fix:** issue reopened `open` with the confirmed mechanism (`llm-proxy:20f6bfd`); the
"falsified" section is retained and relabelled as a record of a wrong refutation, since
deleting it would hide the shape. `eval-design` memory updated with the population rule.

**Status:** fixed-verified — mechanism confirmed by direct capture of the harness's own
requests, trigger isolated to the profile by a one-variable test.

## W-27 — Clean-tree extraction answered "is this failure mine?" with evidence — and the evidence was a third answer

**Observed:** 2026-08-27, closing the two runner orphan paths. After landing the
`PROMPT_TDD_RUN_ID` change, `tests/prompt_tdd` reported `1 failed, 400 passed` —
`test_integration.py::test_sdk_pipeline_with_v2_scenarios`.

**Pattern:** When a suite fails in a working tree you have modified, do not reason from
the diff about whether the failure is yours. Extract the tree at HEAD and re-run the one
test:

```bash
git archive HEAD | tar -x -C <scratch>
cd <scratch> && PYTHONPATH=<scratch>/src <venv>/bin/python -m pytest <the one test> -q
```

`git archive` over `git worktree add` on a shared checkout: it writes nothing into
`.git`, registers nothing, and needs no cleanup beyond `rm -rf`. Works whenever the
package is installed as a plain-path editable `.pth`, because `PYTHONPATH` is searched
before `site-packages` — check the `.pth` first, since a PEP 660 import-hook editable
install would win over `PYTHONPATH` and silently give you the modified code back.

**Counterfactual:** My reasoning was sound and reached the right verdict —
"`No trace available` and `0 tool call(s)` are a mock/trace concern, nowhere near an env
var or a queue path" — and I had already half-written it as the conclusion. What the
reasoning could not have reached is the actual finding: the failure is not
mine-vs-pre-existing at all, it is **environment-dependent**. It fails under
`CLAUDE_CONFIG_DIR=~/.claude-sdd` and passes with the variable unset, on an unchanged
tree, because the mock plants its transcript at a hardcoded `~/.claude` while
`find_transcript` honours the ambient variable. Filed as
`prompt-engineering:docs/issues/2026-08-27-integration-test-plants-transcript-in-the-wrong-claude-home.md`.

Three minutes of isolation bought a filed bug with a one-line reproduction. The
alternative was a commit message asserting "pre-existing" on the strength of a plausible
story — true, as it happens, and still the wrong thing to have written, because it would
have closed the question instead of opening it.

The narrower cost avoided: the failure message itself blames the wrong thing
(`check_guard_denied.py` concludes "this is a scenario-design failure" from zero tool
calls). Three hypotheses it suggests — prompt-text drift, missing `mode: trace`, encoding
divergence — are all refutable in a minute each, and all three are wrong. A diff-reasoning
approach would never have tested them, because it never gets as far as asking what the
failure IS.

**Confirming data points:**

1. This entry — clean-tree extraction turned an assumption into a filed bug, and the bug
   was a different category than either candidate answer.
2. Pending: the next time a suite fails in a dirty tree.

**Impact:** med — one filed bug per occurrence, and it removes a whole class of
commit-message claims made on plausibility. The suite in question is one a developer here
runs constantly, and it is red for all of them.

**Promote-when:** A second occurrence where clean-tree isolation contradicts, or
re-categorises, what diff-reasoning concluded. At two datapoints, promote to `CLAUDE.md`
as: *"A test failing in a tree you have modified is a question for `git archive HEAD`,
not for reading your own diff."*

**Status:** validated — single datapoint, bug filed with a verified one-variable
reproduction.

**Valid:** invariant

**Rests on:** the principle that a claim about causation owes evidence rather than a
plausible mechanism (CLAUDE.md, *Conclude Last*), and the project's rule that any bug
noticed during work gets a file.

## F-43 — My promotion plan skipped two gates the skill documents — and named the weaker of two destinations for this rule's failure class

**Valid:** dated 2026-08-27

**Observed:** 2026-08-27, scouting the promotion route for the importance×cost
explore-vs-ask rule before writing anything. The rule was found living in exactly one
place — codescout memory `reconnaissance`, final bullet — with no `R-N` entry in any repo
and no copy in any of the 19 cached `codescout-companion` versions.

**When:** Immediately after recommending a two-step promotion to the user (backfill the
`R-N` entry, then PR the served `SKILL.md`), and before either write landed.

**Expected (my plan):** two steps. Step 1 writes an `R-N` entry shaped like a session-log
win — `**Rests on:**` pointing at `F-20`, `**Status:** validated`. Step 2 syncs to
`SKILL.md` with a `plugin.json` bump across three profiles.

**Got (scouted reality):** three divergences, none of which would have failed loudly.

1. **Wrong entry shape.** The `R-N` ledger's own augmentation prompt (artifact
   `5696563f06b2c222`) pins its required fields as **Verdict, Observed, Seam, narrative,
   Promote-when, Status, Kin**. `**Rests on:**` and `**Status:** validated` are the
   session-log / statement-validity vocabulary; `validated` is not this ledger's
   disposition value, which pairs a `Verdict` of `hit | miss | proposal` with a separate
   `Status` line. I was importing one ledger's template into another ledger that has its
   own.

2. **A mandated third step, omitted entirely.** `SKILL.md` § *Every promotion audits the
   promoted set* makes promoting a law the trigger to re-verify the already-promoted ones
   against four staleness classes — False / Outgrown / Unreachable / Obsolete — **and to
   record which class each was checked against, in the ledger entry**, "so the next
   promotion inherits the check rather than repeating it". My plan had two steps; the
   skill requires three, and the third is the one that keeps the promoted set from
   becoming the ledger it was extracted from.

3. **The destination I recommended is not the one the skill prescribes for this rule's
   failure class — and the prescribed one is gated shut.** The rule's problem is not its
   wording; it is that it sits behind a deliberate memory read and governs a decision made
   in the first thirty seconds of a task. That is staleness class 3, **Unreachable**,
   whose stated remedy is *"placement, not rewording … if a law keeps recurring in sessions
   that never invoke this skill, the fix is the session-opening surface, not a better
   sentence here."* That is destination 2 (`project-activation-bootstrap`), which requires
   **a measured base arm** — evidence an unaided agent does not already do this. No such
   arm exists. `prompt-engineering:scenarios/exploration-protocol` is the nearest
   candidate and is **not** it: read at `protocol.md` + `find-bug/scenario.yaml`, it
   measures Phase-0 bug-ledger consult and the `"Ledger checked:"` receipt — a different
   rule. Destination 1 (`SKILL.md`) needs no base arm, so my recommendation was
   *available*, but I reached it without noticing it is the weaker remedy for this
   particular failure.

**Probable cause:** I applied the routing test — *"would this rule mislead a different
project?"* — took the destination it selects, and stopped. The two paragraphs that gate
that destination, and the entire section that follows it, went unread. Reading a decision
procedure far enough to get an answer is not the same as reading it far enough to get its
preconditions, and the first feels complete.

**Workaround:** None needed — nothing had been written. Plan revised to three steps, with
the base-arm gap surfaced as an explicit precondition on destination 2 rather than
discovered after a PR.

**Severity:** med — would have produced an `R-N` entry missing 3 of 7 required fields
(invisible to every field-presence sweep, which is the exact defect the ledger's own
prompt says left 39 of 57 entries unharvestable for three months), plus a `SKILL.md` PR
that skipped the mandated promoted-set audit. Both recoverable; neither raises.

**Status:** open — routing surfaced, no write landed, destination-2 gate unmet.

**Rests on:** `codescout-companion:skills/reconnaissance/SKILL.md` §§ *Promotion routing*
and *Every promotion audits the promoted set* (routing section landed
`claude-plugins:42254d8`, 2026-06-11 — so it predates the 2026-08-26 promotion); the
augmentation prompt on artifact `5696563f06b2c222`; and
`prompt-engineering:scenarios/exploration-protocol/{protocol.md,find-bug/scenario.yaml}`
for the base-arm absence.

**Fix idea / Pointer:** Kin to [[F-20]] — the rule under promotion is F-20's own remedy,
so leaving it unreachable leaves F-20 open by construction. The base arm is the tractable
missing piece: a scenario whose bare arm asks the user (or asserts) a question answerable
in one or two tool calls, scored on whether the agent runs the call instead. That is the
same shape as the verify-before-assert arm the skill cites as precedent (0% bare, 100%
shipped over 35 runs, codescout `5917e37e`).

## W-28 — Naming a defect pattern in the review brief found the instance per-task reviews could not

**Observed:** Three separate assertions in one branch turned out to be incapable of failing
— a fixture whose alphabetical order coincided with document order; a
`contains("get_guide")` guard defeated by the guide's own preamble text; and a success
check blind to the `RecoverableError` class it was written for. Each was found by a
different per-task reviewer, each as an isolated finding.

**Got:** The final whole-branch review brief was written with the pattern named explicitly:
*"This branch had a recurring problem worth checking for residue: three separate assertions
were found to be incapable of failing. … Look for a fourth."*

It found one — and in the worst place. Gate 5's reachability predicate contained
`.any(|c| c.level > sec.level && !c.serves.is_empty())`, which never references `sec` except
for `.level`. Since the splitter emits only levels 2 and 3 and the topic had four declaring
`###` sections, the clause was unconditionally `true` for **every** `##` section, making the
assertion `true || …`. The gate whose entire job was catching unreachable sections could not
fail for the majority of them — and 1,802 B of guide content was consequently unreachable
with no waiver, while two carefully-written waivers sat decorative, short-circuited before
they were ever evaluated.

**Counterfactual:** the three prior instances were each reported as a one-off defect and
fixed as one. Nothing in the per-task review flow escalates "this is the third time" into
"look for a fourth" — each reviewer sees one diff and one instance. Without the pattern
named in the brief, a whole-branch reviewer has no reason to weight this failure mode above
its base rate, and the gate ships green and useless.

**Practice:** once a defect class has **two** instances in a work stream, name it in the
next review brief and ask explicitly for the next one. The discriminating question is never
"does this test pass" but **"what mutation of the production code would make this test
fail?"** — a question a reviewer will not ask unprompted about a test that is already green.

**Valid:** dated 2026-08-27

**Status:** validated

**Promote-when:** a second work stream reproduces it — a named-pattern review brief finding
an instance that per-task reviews missed. At that point it belongs in
`requesting-code-review`'s template rather than in an ad-hoc brief.

## F-44 — Half the tasks carried a defect inherited from the plan's own reference code

**Observed:** The `get-guide-section-grain` plan was written with full inline reference code
for all ten tasks — every test body, every implementation, exact values — on the reasoning
that an implementer transcribing complete code needs no design judgement.

**Got:** **Five of the ten tasks carried a defect inherited from that reference code**, none
caught by its author, all caught by task reviews:

| task | plan-mandated defect |
|---|---|
| 1 | `fence = !fence` bool toggle — desyncs on nested or mixed fences |
| 2 | no validation on `tool`/`action` identifiers — a stray space or extra dot yields a permanently inert declaration |
| 4 | `selector_key` returning `None` when a call has no `action` — makes tool-only shapes unmatchable |
| 8 | `assert!(guide.contains("get_guide"))` — defeated by the guide's own preamble text |
| 9 | Gate 5's `child_declares` clause — unconditionally `true`, so the assertion was `true \|\| …` |

A sixth was structural rather than in the code: the brief told the implementer that Gate 2
"replaces" an existing test that in fact asserted four things, three of them orthogonal.
The implementer declined to follow it and restored them. It was right.

The plan was *not* bad at what plans are for. Interfaces, ordering, test structure and the
producer/consumer contract across ten tasks were all correct — the pre-flight scan found
only two conflicts, both real, both cheap. What it was bad at is **edges**: the code had the
right shape and under-specified boundaries, and boundaries are exactly where these defects
live.

**Counterfactual:** the `plan-mandated` label in the task-reviewer rubric is what surfaced
all five. Without it a reviewer can reasonably reason "the implementer transcribed the brief
faithfully, so this is not their defect" and pass it — which is true and beside the point.
The rubric's line *"the plan's authorship does not grade its own work"* is doing real work,
not ceremony.

**Practice:** write a plan's inline code as **shape**, and expect the review loop to supply
correctness. Do not treat "the plan contains complete code" as evidence that a task is
mechanical — it changes who finds the defect, not whether one exists. Budget review
accordingly rather than downgrading the reviewer model on transcription-shaped tasks.

**Valid:** dated 2026-08-27

**Status:** open

**Promote-when:** a second multi-task plan reports a comparable plan-mandated defect rate.
Two datapoints would justify a line in `writing-plans` — its "No Placeholders" section
currently pushes toward complete inline code without noting that complete code is not
correct code.

## F-45 — The documented pre-commit gate cannot see test code; CI's second clippy job can

**Valid:** dated 2026-08-27
**Status:** fixed

**Observed:** Merging `sdd/get-guide-section-grain` into `experiments` produced a tree that fails CI. `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` (`.github/workflows/ci.yml:61`) reported seven `clippy::doc_lazy_continuation` errors. The narrow form CLAUDE.md documents as the gate — `cargo clippy -- -D warnings` (`ci.yml:50`) — passes on the identical tree.

The defect itself is small: a doc comment on `#[test] fn session_opening_guide_never_declares_sections` wrapped as "…never declares `##`/`###` sections in Phase / 1. `guide_blocks_for` keys everything else…", putting `1. ` at the start of a line. Rustdoc's markdown parser reads that as an ordered list item and the six following lines as lazy continuations of it. One sentence breaking after "Phase" invented a list.

**Got:** The lint was unreachable from every check the run actually performed. Ten per-task gates, ten task reviews, one whole-branch review on the most capable model, and a final green suite — all running the documented narrow form, which does not compile test targets. A defect sitting on a `#[test]` fn is invisible to all of it by construction. It would have failed on the first push.

Worth being precise about where the knowledge already lived: `ci.yml:51-60` carries a comment that states the fact outright — "the bare command above only lints the root package's non-test targets with default features". The repo knew. The gate line in CLAUDE.md, which is what an agent actually reads before declaring a task complete, did not carry it.

The generalisation is the part to keep: **a documented gate narrower than the enforced gate is worse than no documentation.** Everyone runs the documented form and believes they are covered, so the gap cannot surface during review — only at push, after every reviewer has already signed off. The cost is not the lint; it is that a clean review is not evidence of a clean tree.

**Rests on:** `.github/workflows/ci.yml:50` (narrow job) and `:61` (wide job, with its explanatory comment at `:51-60`). Failure and fix: `c30c07a7`, patch-id `c586b0766eacf6c4765c3bb0359ff50839a2e9f1`. CLAUDE.md § Development Commands corrected in the same commit as this entry.

## F-46 — I described a budget from its module name — SIZE_CEILING counts rules, at compile time, on the set that is never delivered

**Status:** open — the false premise is committed in five places; fix listed below
**Severity:** high — it is already published, and it was about to be the design input for Layer 2's central gate
**Valid:** dated 2026-09-02

**Observed.** Writing the retrieval-engine coordination spec I asserted, as a
core problem statement, that engine 1 and engine 5 each enforce a budget over
the same context window and neither knows about the other:

> *"Engine 1 enforces a p50 session ceiling (`CEILING = 12_000` B) … Engine 5
> enforces its own `SIZE_CEILING` in `operator_rules::budget`. Both spend the
> same context window and neither knows about the other. Two budgets over one
> resource is not a budget."*

**I never opened `budget.rs`.** Reading it at the start of Layer 2 — the layer
whose central gate was to be *"collapse the two ceilings into one"* — showed
every clause of that sentence is wrong in a different way:

| I claimed | Verified 2026-09-02 |
|---|---|
| a byte ceiling | `SIZE_CEILING = 10` is a **count of rules** |
| per-call / per-session | called only from `operator_rules::mod:47` and `corpus.rs:40` — **compile time**, never on the delivery path |
| over delivered content | filters `binding == Always`, and `route()` excludes `always` **unconditionally** — it governs exactly the set that is **never delivered per call** |

**The real shape is worse, and argues the same conclusion for a different
reason.** There is **one** byte budget covering **part** of the window, plus two
wholly unbounded emitters:

| emitter | bound | unit |
|---|---|---|
| guide sections (push) | `CEILING = 12_000` | bytes, p50 session |
| session opener | same ceiling (emits a `get_guide(` block) | bytes |
| operator `always` (resident) | `SIZE_CEILING = 10`, compile time | **rule count**, on a disjoint set |
| operator `triggered` (per call) | **nothing** — grep for `len\|bytes\|CEILING\|cap` in `route.rs` returns zero | — |
| craft skills | **nothing** | — |

And the exclusion is explicit rather than incidental: the ceiling test's
`shape_total` filters to blocks containing `<!-- auto-injected get_guide(`, while
operator rules emit `<!-- operator-rule OP-N …`. So the one real budget
**deliberately** does not see the other engine's bytes.

**Why the error survived my own review.** Two things, and the second is the
transferable one.

1. `operator_rules::budget` is a plausible name for a delivery budget, and its
   doc comment opens *"Gate 3 — the two budget constraints"*, which reads as
   confirmation if you are looking for a second budget.
2. **The claim was load-bearing for a document rather than for a call.** Nothing
   executed it, so nothing could red. Every gate stayed green through five
   commits while a false premise about a measurement instrument propagated —
   which is `Loudness is a property of a PATH` applied to prose: a claim on no
   execution path has no observer, however confidently written.

> **A budget you have not read is a budget you are describing from its name.**
> The tell is available without reading anything: I could name the *other*
> ceiling's constant, unit, and call site, and for this one I could name only the
> module.

**Counterfactual.** Layer 2's gate 3 would have been built to reconcile two
ceilings, one of which does not measure what the gate assumes. The likely
outcome is not a red test — it is a gate that sums a byte count and a rule count,
passes, and is cited later as proof the budgets are unified.

**Fix — five committed surfaces carry the false premise:**

- `2026-09-02-retrieval-engine-coordination-design.md` § *Problem 4*, § *Gates* gate 3
- `2026-08-27-get-guide-section-grain-design.md` § *Coordination* (second bullet)
- `resume-get-guide-section-grain-phases-2-3.md` `GG-9` (the `GG-4` bullet)
- commit `238755ff` message (immutable; corrected by the follow-up commit)
- `src/engines/mod.rs` module docs are **clean** — they describe the ledger, not budgets

**Rests on:** `src/operator_rules/budget.rs`, `src/operator_rules/route.rs`,
`a_p50_session_stays_under_the_committed_guide_byte_ceiling`'s `shape_total`.

## F-47 — A review base recorded before dispatch silently widened to three peers' commits — and one git identity means `%an` cannot separate them

**Valid:** conditional — `superpowers:subagent-driven-development` derives a task's review base from the task's own commits rather than from a HEAD recorded before dispatch

**Severity:** high — would have spent an Opus review seat on three other sessions' diffs and returned Important findings against work the task never touched, which I would then have routed into a fix loop aimed at the wrong implementer.

**Status:** open

**Observed:** The SDD skill prescribes recording `BASE = git rev-parse HEAD` *before* dispatching an implementer, and warns explicitly against `HEAD~1` because it "silently drops all but the last commit of a multi-commit task". Both halves are correct — and on a shared checkout the guard **inverts**.

Three peer sessions committed into `experiments` while my Task 1 ran. `review-package … e20b794f e701ec59` therefore produced **4 commits, 119,298 bytes** — `a24c93a7`, `781633e4` and `655c0b6f` are other sessions' work. Re-scoped to the commit's true parent, `781633e4..e701ec59`: **1 commit, 15,523 bytes.**

**Why the usual disambiguator is dead here.** Every commit in this repo is authored `Marius Ailinca` — one git identity across every concurrent session — so `%an` separates nothing inside a range. The `Session-Id:` trailer is the only positive discriminator, and it is per-commit rather than per-range, so it cannot be used to *scope* a range, only to audit one afterwards.

**Why it is silent.** The widened range is a valid range; the diff is well-formed; the reviewer reads it and produces findings. Nothing errors. The only tell was the script's own `4 commit(s)` line in its success message — a number printed beside a path, in a step whose purpose is generating a file. Had the task been multi-commit I would have had no cheap tell at all, because "more than one commit" is then the expected state.

**The class.** *A correct method whose precondition the environment withdraws.* "Record BASE before dispatch" is correct exactly when the branch advances only through the controller — which the method never states, because on a single-session checkout it cannot be false. `codescout-ca` observed the same shape in a different subsystem the same night (a prescribed ledger remedy whose second half lived in a file frozen by another session's uncommitted work), which is what suggests it is a class rather than a one-off.

**Remedy, and why it is strictly better rather than a trade.** Derive the base from the task's own first commit after the fact — `<first-task-sha>^` — never from HEAD before dispatch. It is correct on a single-session checkout too, where it is identical to the recorded BASE, so there is no world in which the pre-recorded value is the better input. The skill's warning against `HEAD~1` still stands and is unaffected: `HEAD~1` is wrong because it is anchored to the range's *end*; this is anchored to its *start*.

**What this does not claim.** I have not checked whether the other `scripts/` in that skill share the assumption, and I have not re-derived the peer's cross-subsystem instance myself — it is reported here as their observation, not my measurement.

## F-48 — F-47's remedy names the unit "task", but the thing that needs a base is the DISPATCH — so a fix round re-uses the implementation commit and widens 14×

**Valid:** conditional — an SDD review package's base is expressed relatively (`<sha>^..<sha>`, or `git show <sha>`) rather than as a literal SHA pair

**Severity:** high — the same counterfactual as `prompt-surface-measurement-session-log:F-47`, deliberately rated the same: an Opus review seat spent on ten other commits, returning Important findings against work the dispatch never touched, which the controller then routes into a fix loop aimed at the wrong author.

**Status:** open

**Observed:** `prompt-surface-measurement-session-log:F-47`'s remedy reads *"derive the base from the task's own first commit after the fact — `<first-task-sha>^`"*. I wrote that remedy into Plan 2's stop-and-resume note as the range `cb6aed69..aab0c4ef`, annotated *"that range is the fix commit's true parent → head"*, and cited F-47 in the same sentence as the reason. **The annotation is false.** `aab0c4ef`'s parent is `dde26886`; `cb6aed69` is Plan 2's *implementation* commit, ten commits earlier.

Measured 2026-09-02, building the package the note prescribed:

| package | bytes | files |
|---|---|---|
| `git show aab0c4ef` (correct) | 10,674 | 3 — `src/engines/{coordinator,emitters,mod}.rs` |
| `git diff cb6aed69 aab0c4ef` (as noted) | 148,930 | 20 |

A factor of **14**, with 17 of the 20 files belonging to other work — four peer commits plus six of my own docs commits, all landed in the ~35 minutes between the two.

**Why F-47's own remedy did not prevent this.** F-47 quantifies over *tasks*, and a task has more than one dispatch: implementation, then one fix round per review cycle, each producing a commit and each earning its own review. The unit that needs a base is the **dispatch**, not the task. Applied to a fix round, *"the task's first commit"* resolves to the implementation commit — the wrong end of the range, and wrong in precisely the direction F-47 exists to warn about. The remedy is not incorrect; its quantifier is one level too coarse, and the coarseness is invisible until a task has two dispatches.

**The remedy that cannot drift: name the base relatively, never symbolically.** `<sha>^..<sha>`, or `git show <sha>` for a single-commit dispatch. A relative expression re-derives the parent from the object itself, so it is correct for whichever dispatch it names, correct on a single-session checkout (where it equals the pre-recorded BASE), and correct when written down and read hours later. **A literal SHA is a fact about an instant**, and on a shared checkout that instant expires in minutes — which is the same decay F-47 documents, arriving through a different door.

**What caught it, and what would not have.** A routine `git log -1 --format=%P` before generating the package. What did *not* catch it: writing the note while citing F-47, and re-reading the note on resume. Knowing the class prevented neither the error nor the re-read — consistent with `observer-blindness:OB-1`'s measured finding that four instances of one class in one evening were each committed by an author actively writing about that class.

**What this does not claim.** I have not checked whether the SDD skill's own `review-package` script can be given a relative base, nor whether its other scripts share the literal-SHA assumption. F-47 already declines the same check; this entry does not close it either.

## F-49 — One fact had four representations, and three review rounds each fixed the one named — a grep over the known phrasings cannot find the form nobody has described yet

**Valid:** invariant

**Severity:** med — each round costs a review dispatch and a commit, and the intermediate states ship a *partially* repaired doc, which reads more trustworthy than the original because the site a reader is most likely to check has just been corrected.

**Status:** open

**Observed:** One fact — *where the session-opener and operator-rule ledger keys are written* — was represented in **four** places in `src/engines/mod.rs`. Plan 3's wiring commit falsified all four at once. Three successive review rounds each named one, I repaired the one named, and the next round found the next:

| round | site the reviewer named | representation | what I left |
|---|---|---|---|
| task review | `mod.rs` § *What this module is NOT* | prose paragraph | the table, 20 lines above |
| fix round 1 | the six-ledger-writers **table** rows | prose table | `writes_at`, 150 lines below |
| fix round 2 | `writes_at: &["tools::core::types"]` ×2 | **data** field | — (swept) |

Plus a fourth form the same rounds turned up piecemeal: `EngineDecl::emit_post`'s doc, `route::ledger_key`'s `**Load-bearing:**` doc, and a *quotation* of a prose comment the wiring commit had deleted, whose quoted bytes existed nowhere.

**Why "check harder" is the wrong instrument.** A repairer's field of view is set by the **finding**, not by the file. The finding names a site; the fix lands at that site; the sibling representations are outside the frame the reviewer handed over. That is why it recurred three times *while I was actively writing about the class* — Plan 2's ledger records the same shape from this morning (a remedy list covering fewer sites than its finding named), and knowing it prevented none of the three.

**And the obvious check is monotone in the wrong direction.** After round 2 I offered a greppable sweep — no `types.rs:<line>`, no `"documented at the site"`, no `"tools::core::types"`, no `"rule branch"` under `src/engines/`. Every one of those patterns is a phrasing **a previous round had already named**. A grep over the known phrasings cannot find the representation nobody has described yet, and it returns a clean zero either way, which reads as completeness.

**The representation that survives longest is the one with no enforcing test, and the test's shape is why.** `writes_at`'s only read is `assert!(e.writes_at.is_empty())` in `an_unmanaged_engine_is_registered_and_owns_nothing` — which exercises `craft-skills`, whose list is empty **by definition**. The assertion holds over a population that excludes every member capable of falsifying it. So the field is the last to be fixed and the first to rot, and no gate moves in either direction.

**Remedy, and its limit.** Ask *"how many representations does this fact have?"* before repairing any of them — count the sites, then fix the count. For a fact with a **data** representation, prefer deriving it: `writes_at` could be checked against the crate's actual `ledger.insert` call sites, which would convert this from a documentation habit into a gate. What this entry does **not** claim is that the count is now right: rounds 1 and 2 both ended with me believing I had swept.

**Rests on:** `observer-blindness:OB-1` § *the third position* — a check that runs when nobody is worried, rather than a resolution to be careful.

## Template for new entries

```
## F-N — <title>
**Valid:** invariant | dated YYYY-MM-DD | conditional — <event>
**Status:** open | fixed | mitigated | validated
**Observed:** …
**Rests on:** …
```
