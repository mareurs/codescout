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
entry_high_water_F: 11
entry_high_water_W: 11
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
| F-1 | Fixed output path destroyed the evidence for the headline figure | fixed |
| F-2 | `json.dumps` defaults inflated every schema measurement by 3.8% | fixed |
| F-3 | A subagent's `workspace(activate)` mutated the parent's active project | open |
| F-4 | Every augmentation in the catalog is gone | open |
| F-5 | A stale recon finding was relayed as current | fixed |
| F-6 | 400k of context with no tracker or librarian use | fixed |
| F-7 | Spec asserted per-arm tool denial the harness cannot do | fixed |
| F-8 | Eight assertions in one task passed for the wrong reason, all the same shape | promoted-to-permanent-docs |
| F-9 | Five API drops stranded uncommitted subagent work | mitigated |
| F-10 | Task 3's native-tool veto keys on a per-arm env var the runner cannot set | fixed-verified |
| F-11 | Sharing one OAuth credential across Claude Code profiles broke three of them mid-pilot | mitigated |

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

**Full record:** `docs/issues/2026-08-23-subagent-activate-mutates-parent-active-project.md`
(severity high). Measured: exactly one activate across all agent transcripts, zero
`read_only` occurrences, identical `[security]` config in both projects.

**Dispatch defect on our side:** the briefing told agents to `activate`. Every
codescout tool already takes a per-call `workspace` parameter documented for
"concurrent subagents in different workspaces", which resolves per-caller without
touching shared state. Brief subagents with the parameter, never the activate.

## F-4 — Every augmentation in the catalog is gone

**Valid:** dated 2026-08-23
**Status:** open

**Observed:** `artifact(find, kind="tracker", augmented=true, scope="repo")`
returns zero rows. `f2ecdd76a6189efb` — the T-N ledger CLAUDE.md hard-codes and
prescribes `append_entry` / `update_entry` against — has `augmentation: null`.

**Mechanism:** augmentation lives only in the catalog SQLite DB and has no on-disk
form, so it is the one class of state `reindex` cannot rebuild. Any event that
recreates the DB destroys every augmentation at once while leaving all artifact
rows intact — the observed state exactly.

**Why it stayed invisible:** reindex *preserves* augmentation rather than
regenerating it, so a post-loss reindex reports healthy and repairs nothing. No
gate notices; `artifact(get)` returns `augmentation: null` without comment.

**Full record:** `docs/issues/2026-08-23-research-index-tracker-has-no-augmentation.md`
(escalated low → high, scope corrected from one tracker to repo-wide).

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

## Template for new entries

```
## F-N — <title>
**Valid:** invariant | dated YYYY-MM-DD | conditional — <event>
**Status:** open | fixed | mitigated | validated
**Observed:** …
**Rests on:** …
```
