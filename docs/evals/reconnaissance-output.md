# Reconnaissance Skill — Behavioral (Output) Eval

**Purpose:** Score what happens *after* the `reconnaissance` skill fires. The trigger eval
(`reconnaissance-trigger.md`) scores whether the `description:` string fires when intended.
This eval scores the complementary half: once loaded, does the model **scout the right seam
before acting** and **externalize the gap as a useful F-N/W-N entry** — or does it act on the
unscouted seam and reproduce a recorded miss?

**Substrate:** the per-project R-N ledger `docs/trackers/reconnaissance-patterns.md`. Its
**misses** (recorded cases where the skill's guidance existed but did not fire) are the hard
cases; its **hits** are positive controls. Each Case names the R-N it is derived from, so a
FAIL maps back to a real, dated incident — not a hypothetical.

**Status:** Bootstrap. Cases pinned from the R-N ledger. **Baseline NOT predicted and NOT yet
run — n=0 graded runs.** See `## Baseline` for why per-case prediction is deliberately withheld.

**Owner:** Hamsa (auditor). Scoring SHOULD use a separate judge model + this rubric, never the
same model/turn that produced the trace (Hamsa Heuristic 8).

---

## How to run

Two forms, strong and cheap. Prefer strong; the cheap form is a smoke test only.

**Strong (trace-scored — the real eval).**
1. Fresh agent, codescout MCP backend. Inject the current SKILL.md
   (`claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md`) as the skill is
   actually loaded. Do **not** inject `reconnaissance-patterns.md` — see `## Contamination`.
2. Place the agent in a fixture (the codescout repo itself, or a planted fixture) where the
   Case's **planted drift** is real on disk.
3. Present the Case **Scenario** verbatim as a task.
4. Capture the full tool-call trace.
5. Score the trace against **Expected behavior** with the rubric below. The verdict is about
   what the model *did* (which tools, in which order, before which mutating/asserting action),
   not what it said it would do.

**Cheap (elicitation — smoke test, low signal).** Present the Scenario and ask *"What is the
first thing you do, and why?"* Score the described plan. A model can narrate "I'd scout" and
then not scout under load, so a cheap-form PASS is `unverified`, never a ship signal.

**Tally.** Ship threshold for a SKILL.md change: **≥80% PASS (≥12/14)** on the strong form,
with **zero FAIL on the six MISS cases** (Cases 1–6) — the misses are the regressions this
eval exists to prevent; a MISS-case FAIL blocks ship regardless of total.

## Rubric

| Verdict | Criterion |
|---|---|
| PASS | Model scouts the named seam with the right tool **before** the mutating/asserting action AND surfaces the planted drift (or, on a SKIP case, proceeds without ceremony). On gap cases, also externalizes — names the finding for an F-N entry. |
| PARTIAL | Scouts but with the wrong tool (e.g. `grep` where `references()` was needed and the count was wrong), OR catches the drift but acts before externalizing, OR scouts the right thing but narrates "confirmed" without reading the body. Counts as FAIL for the ship tally; tracked separately for diagnosis. |
| FAIL | Acts on the unscouted seam — edits / asserts / verifies — reproducing the recorded miss; OR fabricates a finding the trace does not support; OR loops reconnaissance on a SKIP case (over-scout). |

A *right-action-wrong-reason* PASS is downgraded: if the model scouts by luck (reads the symbol
for an unrelated reason and stumbles on the drift) without the skill's discipline driving it,
score PARTIAL. The eval measures the discipline, not the coincidence.

---

## Cases

Six MISS cases first (recorded skill failures — the regressions this eval guards), then six HIT
positive controls, then two SKIP over-scout pressure-tests.

### Case 1 — Constant WRITE patterns, not just reads (MISS → R-2)

**Scenario:** *"Remove the `SYMBOL_NAV_TOKEN` placeholder from `src/prompts/source.md`; it's
dead weight in the rendered instructions."*

**Planted drift:** the token is not dead — `SERVER_INSTRUCTIONS.replace(SYMBOL_NAV_TOKEN,
&nav_content)` substitutes language-specific content into it at runtime. Removing it from
`source.md` makes the `.replace` a silent no-op; the nav block vanishes at runtime with no test
failure.

**Expected behavior:** scout greps for **writes into** the constant
(`.replace(<TOKEN>` / `.replacen` / `format!` using it), not only `<CONST>.contains` / `.find`
reads, before editing. Catches the runtime substitution and refuses the naive removal.

**PASS/FAIL boundary:** PASS scouts writes and finds the `.replace`. FAIL greps only reads (or
not at all) and removes the token. This is the exact gap R-2 recorded; R-1's promoted bullet
covers reads — this case tests whether write-side coverage is present.

---

### Case 2 — Grep undercounts construction sites (MISS → R-4)

**Scenario:** *"Add a required field `priority: u8` to the `Task` struct and fix every
construction site."*

**Planted drift:** construction sites use `..Default::default()`, builder chains, and
struct-update syntax that a `grep 'Task {'` undercounts 2–3×. A grep-driven fix compiles for the
literal sites and breaks at the ones grep missed.

**Expected behavior:** scout uses `references(Task)` and/or leans on the compiler
("compiler as scout", R-5) to enumerate construction sites — not a single grep pattern — before
claiming the blast radius is covered.

**PASS/FAIL boundary:** PASS uses `references()`/compile to enumerate. FAIL trusts one grep
pattern and reports the field added "everywhere." PARTIAL if it greps several patterns but never
cross-checks against the compiler.

---

### Case 3 — Buffered tool-output truncation before a structured write (MISS → R-10)

**Scenario:** *"`artifact(get, full=true)` on this 36 KB tracker returned `@tool_a1`. Parse its
body into a `rows[]` array and re-augment the tracker with the structured index."*

**Planted drift:** the buffered `body` is silently clipped at the inline budget — 15 of 22
entries present, the tail dropped, no in-band truncation marker. Parsing the buffer as-is writes
a 15-of-22 index with no error and no diff.

**Expected behavior:** scout treats the buffered output as **unverified shape** — reconciles the
parsed item count against an independent server-side view (`preview.headings`, or a line-sliced
re-read) before the write.

**PASS/FAIL boundary:** PASS reconciles the count and detects the clip. FAIL parses the truncated
buffer and writes the partial index. *(Meta-note: this exact trap recurred while reading
`reconnaissance-patterns.md` to build this eval — the body extraction clipped at 500 lines
mid-R-15; it was caught only by reconciling against the heading map. Live evidence the trap is
real.)*

---

### Case 4 — Assert a checkable fact from memory (MISS → R-19 recurrence)

**Scenario:** *"In a comparison doc, write the sentence: 'codescout's `content_hash()` uses
BLAKE3, so we already have the dedup primitive.'"*

**Planted drift:** `content_hash()` at `src/retrieval/sync.rs:29` is **SHA-256** (`sha2 =
"0.10"`), not BLAKE3. The BLAKE3 label is a conflation with a sibling project. The claim is a
specific, checkable fact about to be written to disk.

**Expected behavior:** before committing the fact, scout the cited symbol this session
(`symbols(name="content_hash", include_body=true)`). The "When NOT to Use" describe-vs-assert
line (added by R-19) says asserting a checkable fact is **not** read-only Q&A.

**PASS/FAIL boundary:** PASS reads `sync.rs` and writes "SHA-256" (or refuses the BLAKE3 claim).
FAIL writes "BLAKE3" from memory. This is the recurrence-after-documentation miss — the wrong
fact reached disk in two artifacts in the same session R-19 was written, which is why this case
is load-bearing, not cosmetic.

---

### Case 5 — Recovery-verification on a shared single-holder resource (MISS → R-23)

**Scenario:** *"I killed the stuck kotlin-lsp that was squatting the RocksDB index lock. Confirm
the lock is recoverable now."*

**Planted drift:** issuing an operational call (e.g. `references`, `symbols`) from a second
codescout client to "test recovery" spawns a fallback LSP that **re-acquires** the just-freed
lock — the verifier becomes the new squatter and breaks the user's own restart.

**Expected behavior:** verify a shared single-holder resource by **reading state**
(`fuser` / `ss -xlp` / `ps` on the lock/socket/process), never by issuing a call from a
non-owning client. The call is itself a mutation.

**PASS/FAIL boundary:** PASS inspects lock/process state read-only. FAIL issues an operational
call "to check" and re-contends the resource. Hand operational verification to the owning client.

---

### Case 6 — `edit_markdown(replace)` drops structural markers (MISS → R-8)

**Scenario:** *"Rewrite the `## Deeper guidance` section of `src/prompts/source.md` to be
tighter."*

**Planted drift:** the section body contains structural HTML-comment markers
(`<!-- @surface onboarding_prompt -->`, `<!-- @end -->`) that demarcate prompt surfaces. A
`replace` that doesn't carry them forward wipes them and breaks the build
(`surface 'onboarding_prompt' not found`).

**Expected behavior:** scout reads the section body (`read_markdown(heading=...)`) **before**
replacing, sees the markers, and preserves them verbatim in the new content.

**PASS/FAIL boundary:** PASS reads-then-replaces with markers intact. FAIL replaces from the
heading alone and drops the markers.

---

### Case 7 — Plan cites a nonexistent type accessor (HIT control → R-16; SKILL.md exemplar)

**Scenario:** *"Dispatch Task 2 of the plan. Its test code uses
`err.hint.as_deref().unwrap_or(\"\")` on a `RecoverableError`."*

**Planted drift:** `RecoverableError` (`src/tools/core/types.rs`) exposes `message: String` and
`guidance: Option<Guidance>` — there is **no `.hint` field**; there is a `.hint()` *method*. The
plan's test code won't compile.

**Expected behavior:** pre-dispatch, scout the type
(`symbols(name="RecoverableError", include_body=true)`), catch the fictional field, and revise
the plan before any subagent runs.

**PASS/FAIL boundary:** PASS scouts and catches it pre-dispatch. FAIL dispatches the fictional
code. *(This case is derived from the SKILL.md's own F-3/W-2 worked exemplar — the model has
seen it in-context. Expect high PASS; a FAIL here means the skill's flagship example did not
transfer to action, which is a severe signal.)*

---

### Case 8 — Side-effect entry-point coverage (HIT control → R-21)

**Scenario:** *"Make `codescout index` write the `index-state.json` freshness sidecar. I added
the write to `IndexProject::call` and the 46 unit tests are green — ship it?"*

**Planted drift:** `IndexProject::call` is the MCP path only. `references(sync_project)`
enumerates 5 call sites; the CLI path (`main.rs`) and the standalone bin write nothing, so a live
`codescout index` (the path the companion hook invokes) produces no sidecar despite green units.

**Expected behavior:** `references()` the operation that owns the side-effect to enumerate ALL
entry points, and verify through the **real production entry point** (the CLI), not a unit
harness that bypasses `main.rs`. Move the write to the `sync_project` chokepoint.

**PASS/FAIL boundary:** PASS runs `references()` + a live CLI verify and finds the 2 uncovered
paths. FAIL trusts the green unit tests and ships. PARTIAL if it live-verifies one path but never
enumerates the others.

---

### Case 9 — Shared-staleness corroboration (HIT control → R-22)

**Scenario:** *"`references(foo)` returned 0 but I'm sure `foo` has callers. The
`references_completeness_hint` and the call-hierarchy guard both agree it's 0. Is it really 0?"*

**Planted drift:** the LSP answered before loading the caller files (false zero); the completeness
hint and call-hierarchy are **also** LSP-backed, so every candidate corroboration shares the same
lagging freshness root and is blind to the shared zero.

**Expected behavior:** recognize that all cross-checks share a freshness root, and corroborate
with an **out-of-band** source — a text/tree-sitter scan (mirroring `call_graph` Phase B) — not
another LSP-backed signal.

**PASS/FAIL boundary:** PASS reaches for an LSP-independent scan. FAIL trusts the co-lagging
guard and reports 0 as truth.

---

### Case 10 — Sibling callers of a just-fixed shared helper (HIT control → R-17)

**Scenario:** *"I fixed an off-by-one in `do_insert`'s use of `clamp_range_to_parent`. The
insert bug is gone — close the bug class?"*

**Planted drift:** `references(clamp_range_to_parent)` shows `do_remove` and `do_replace` derive
the same clamp bound with the identical off-by-one. A single-site fix ships a partial fix that
re-surfaces on the untouched callers.

**Expected behavior:** before closing the class, `references(helper)` and reproduce each sibling
caller's input shape. Fix all sites.

**PASS/FAIL boundary:** PASS enumerates + checks siblings. FAIL closes on the single site.

---

### Case 11 — Cross-repo doc↔code drift (HIT control → R-13)

**Scenario:** *"codescout's `CLAUDE.md` says the companion's `pre-tool-guard.sh` matches
`Grep|Glob|Read`. Update the one stale matcher value."*

**Planted drift:** the authoritative source lives in the **sibling repo**
(`../claude-plugins/codescout-companion/hooks/`). The doc is stale three ways (wrong hook name,
wrong matcher set, a removed `cd`-passthrough block), and `librarian(audit_doc_refs)` structurally
cannot see across the repo boundary, so there is no automated gate.

**Expected behavior:** scout the authoritative source in the *other* repo (read
`hooks.json` / the hook headers there) rather than patching only the one named line — the
cross-repo blast radius is invisible to the home-repo lint.

**PASS/FAIL boundary:** PASS reads the sibling-repo source and catches the broader drift. FAIL
edits only the single value the prompt named.

---

### Case 12 — Grep line-match ≠ confirmed mechanism (HIT control → R-26)

**Scenario:** *"A `grep` shows `kill_on_drop` at `process.rs:93` and `Command::new` at `:86`.
Write: 'confirmed — the spawned LSP is reaped on drop, so there are no orphans.'"*

**Planted drift:** `kill_on_drop` rides `Child::drop` and never runs under SIGKILL; the spawn path
(`run`, `process.rs:66-135`) has no `setsid` / `process_group` / signal handler — so on a
SIGKILL'd parent the child **is** orphaned. The grep proved presence of the symbol, not the
mechanism.

**Expected behavior:** before narrating "confirmed," read the function body to rule out the
falsifier (a process-group / signal handler that would reap regardless). A grep line-number is
Phase-1 location, not Phase-2 confirmation.

**PASS/FAIL boundary:** PASS reads the body before asserting. FAIL writes "confirmed" off the grep
hit. PARTIAL if it reads the body but still overstates certainty.

---

### Case 13 — Seam already scouted this session (SKIP → over-scout pressure-test)

**Scenario:** *"Earlier this session you read `Task` (`symbols(name=\"Task\", include_body=true)`)
and its fields are unchanged. Now add a builder method to it."*

**Expected behavior:** proceed — one scout pass per seam per session. Re-reading an unchanged
struct is noise (Stop Conditions: "do NOT loop reconnaissance").

**PASS/FAIL boundary:** PASS proceeds without re-scouting. FAIL re-runs the same `symbols` call
ceremonially. This guards against an over-broad reading that trains scout-on-everything.

---

### Case 14 — Genuinely mechanical edit (SKIP → over-scout pressure-test)

**Scenario:** *"Fix the typo 'recieve' → 'receive' in a code comment in `src/main.rs`."*

**Expected behavior:** edit directly. No shape contact, no contract, no test asserts on a comment
— "genuinely no-decision edit" per When NOT to Use.

**PASS/FAIL boundary:** PASS edits without scouting. FAIL opens a scout (reads symbols, greps
callers) for a comment typo. Pairs with Case 13 to bracket the over-scout boundary.

---

### Case 15 — Advertised memory rule: read + applied? (CHANNEL probe → design 2026-06-11)

A different axis from C1–C14: those measure *scout discipline*; this measures whether the
**project-memory channel** (the `2026-06-11-recon-findings-as-project-memory` design) actually
changes behavior — the design's load-bearing Risk 1.

**Scenario:** The project has a `reconnaissance` codescout memory topic; SessionStart
advertised `codescout MEMORIES: … reconnaissance …` with the read-nudge. The topic holds one
rule: *"before asserting a checkable fact about a symbol, read it this session (R-19)."* Task:
*"For the design note, state which hash `content_hash()` uses."*

**Planted drift:** the answer (SHA-256) is only in `src/retrieval/sync.rs`; the *rule* that
should make the agent read it first lives **only in the advertised memory topic** — not in the
SKILL.md, not in any reachable doc. Honest path: notice the advertised `reconnaissance` topic
→ `memory(action="read", topic="reconnaissance")` → apply the rule → read the symbol → answer.

**Expected behavior:** the agent reads the advertised topic (or is moved by the advertisement
to scout the symbol) and answers from source — not from prior belief.

**PASS/FAIL boundary:** PASS reads the topic and/or scouts the symbol before asserting. FAIL
ignores the advertised topic and asserts from memory. **Score separately** — this is the
advertise-pull efficacy probe; it does **not** count toward the C1–C14 ship tally. If it
FAILs, the design escalates (explicit pointer in the SKILLS-AVAILABLE block, or fold into
`system-prompt`) per its Risk 1 mitigation.

**Origin:** design `2026-06-11-recon-findings-as-project-memory-design.md` Risk 1; kin R-19.

---
## Baseline

**Per-case verdicts are deliberately NOT predicted here.** The trigger eval
(`reconnaissance-trigger.md`) recorded the reason in its own iteration log: a Hamsa
inspection-based baseline predicted 4/7, and the empirical 1-shot run scored 6/7 — **inspection
mispredicted 3 of 4 failures (75%)**. Behavioral prediction is *more* fiction-prone than trigger
prediction: a trigger verdict is a binary YES/NO invoke decision, whereas a behavioral verdict
depends on a multi-step tool trace under load. Predicting those traces by inspection is exactly
Hamsa Self-Trap 5 ("invent model behavior"). So this section holds no numbers until a model is run.

**What the cases assert is a *spec*, not a prediction:** each "Expected behavior" is the behavior
the skill is *supposed* to produce. Whether the current SKILL.md actually produces it is the open,
unmeasured question this eval exists to answer.

**First obligation:** run the strong form once, fresh agent, record the row in the Iteration log
as the empirical baseline. Every claim about whether a SKILL.md edit improves *behavior* (as
opposed to *trigger firing*) is `unverified` until that row exists.

---

## Contamination & methodology notes

- **Do not inject `reconnaissance-patterns.md` into the eval context.** The cases are derived from
  it; a model shown the ledger can pattern-match to the R-N precedent instead of genuinely
  scouting. Inject only the SKILL.md, as it actually loads.
- **Exemplar-derived cases (Case 7) are weak positives.** The SKILL.md ships the F-3/W-2 exemplar
  in-context, so a PASS there partly measures recall, not discipline. A FAIL there is the
  informative outcome.
- **Fixtures must be real on disk.** A scenario whose "planted drift" isn't actually present lets a
  model "scout" against nothing and score a hollow PASS. Use the codescout repo itself (the symbols
  cited — `content_hash`, `sync_project`, `RecoverableError`, `clamp_range_to_parent`,
  `process.rs:66-135` — are real here) or build a fixture where the drift exists.
- **Judge separation (Heuristic 8).** Score with a separate judge model holding this rubric. Same
  model + same turn self-scoring its own trace is not a measurement.
- **The six MISS cases are the regression gate.** They map to dated incidents; a FAIL on any of
  them means the skill still permits a behavior that already cost real round-trips.

## Status

- [x] Rubric pinned (PASS / PARTIAL / FAIL; PARTIAL → FAIL for tally)
- [x] 14 cases drafted (6 MISS, 6 HIT control, 2 SKIP) — each cites its R-N origin
- [x] Run protocol pinned (strong trace-scored + cheap elicitation smoke test)
- [~] Baseline first run — **partial (5/14), contaminated** (2026-06-11); see Iteration log + Re-evaluation. A clean run is still owed.
- [~] Score current SKILL.md — **4/14 clean: C4, C8, C10, C12 all PASS** (decontaminated re-run, 2026-06-11). C7 structurally un-decontaminatable (answer in SKILL.md exemplar); C5 deferred (lock hazard); 8 cases unrun incl. SKIP guards C13/C14. **Superseded 2026-09-04** — all 15 suite scenarios have now run paired on the post-split skill, incl. both SKIP guards: C13 tautological (3/3 both arms), **C14 0/3 on BOTH arms** (over-scout guard unmet by the skill and unreachable by base competence). C5 still has no scenario.
- [x] **MCP-live baseline — n=3, gate-trustworthy (2026-07-19).** After the harness judge-parse fix (prompt-engineering `49a0036`, finding C), three real subscription sweeps ran **0 INVALID / 30+ runs** — the first trustworthy MCP-live baseline. Two n=3 measurements agree the earlier ~12/15 was **noise-inflated** (raw treatment pass-count is not skill power): the `--ablate` cross-ref (`ce81b72`) found ~1/15 clean wins; the authoritative **`--paired`** rate-delta method (`3d0fa3e`) found **only C4 and C1 clear the Δ≥0.5 power margin (both +0.67)**, most other cases tautological/no-effect. The treatment arm swings hugely run-to-run (C4/C1 seen FAIL→3/3), which is *why* single-run headlines mislead (L-15). Net: `reconnaissance` shows **small, real power on a couple of cases**, far below the 12/15 impression. **NA/NB/C14 SKILL.md edits are NOT validated (≤+0.33, within n=3 noise).** Standard going forward: `--paired` power delta.
- [~] **First MCP-live run (2026-07-18) — directional ~12/15, superseded.** Flagged INVALID by the judge-parse bug; kept as the epistemic trail. See the MCP-live section + the n=3 correction below.
- [~] Score any SKILL.md rewrite candidate; gate on ≥12/14 AND zero MISS-case FAIL — **measure with `--paired` rate-delta, not raw treatment pass-count** (n=3 lesson L-15). **Measured 2026-09-04 on the post-split skill (`bb24b7f`): GATE NOT MET.** Treatment passes 12/15, but **C2/R-4 is a MISS-case FAIL** (0/3 treat vs 2/3 ctrl, Δ−0.67) and a MISS-case FAIL blocks ship regardless of total. Note the gate also cannot be evaluated *as written*: **C5/R-23 has no scenario in the suite**, so "zero FAIL on the six MISS cases" is scoreable on five of six at best.
- [ ] Optional: expand HIT/SKIP coverage as new R-N entries land
- [x] **Channel-efficacy probe (Case 15)** — **measured 2026-09-04: tautological** (treat 3/3, ctrl 3/3, Δ+0.00). The advertised `reconnaissance` memory rule is read + applied just as often *without* the skill, so the advertise-pull channel shows **no measurable lift** at n=3. This is a null result on design `2026-06-11` Risk 1, not a confirmation; scored separately from the C1–C14 scout matrix.
## Iteration log

_(Append one row per scoring run. First row must be the empirical baseline.)_

| Date | SKILL.md version | Cases passed | MISS-case FAILs | Notes |
|------|------------------|--------------|-----------------|-------|
| 2026-06-11 | a90708c (current) | 5/5 scouted-before-acting (partial: 5 of 14) | 0 (1 MISS case in subset: C4) | **Contaminated — upper bound, low confidence.** Fresh general-purpose subagents; SKILL.md loaded by reading it; R-N ledger not injected; read-only + workspace-pinned. All 5 scouted the named seam before acting and caught the drift / correctly conditioned the answer. BUT each case's answer is documented in-tree (docs/trackers, docs/issues, docs/adrs, this eval) and was surfaced via grep/semantic_search, so the run can't isolate scout-discipline from doc-lookup. C10's drift has healed (fixed in current tree). Cases run: C4/R-19, C7/R-16, C8/R-21, C10/R-17, C12/R-26. C5 deferred (live lock-contention hazard). |
| 2026-06-11 | a90708c (current) | 4/4 PASS — clean (C4, C8, C10, C12) | 0 (C4 is the MISS case in subset) | **Decontaminated re-run.** Throwaway worktrees with docs/ deleted (Case 4's repo-wide grep for the answer → 0 matches, verified); C8 @ cd370079 + C10 @ c99d4228 = pre-fix commits so the drift is LIVE in code, not healed. Fresh subagents, read-only, per-worktree workspace pin. All 4 scouted from SOURCE and caught the drift: C4 read sync.rs (SHA-256), no answer-doc reachable; C8 enumerated entry points, caught CLI bypass of write_index_state; C10 found the off-by-one at all 3 sites + brace-vs-dedent nuance; C12 derived the mux-detach orphan boundary from code WITHOUT the ADR (richer than the contaminated run). C7 dropped (answer lives in the SKILL.md exemplar — un-decontaminatable). C5 deferred. Signal now real for these 4; 8 cases + SKIP guards (C13/C14) still unrun clean. |
| 2026-07-18 | (MCP-live, prompt-tdd) | ~12/15 directional (refactored run 11/15; C4 treatment PASS / ablate RED) | not scored per-MISS (rubric refactor dropped trace assertions) | **[SUPERSEDED by the 2026-07-19 n=3 row — the "C4 load-bearing" read was single-run noise.]** **First MCP-live run — real tools + ablation control.** prompt-tdd `anthropic-mcp` registry runs the real codescout binary as a live MCP server; skill injected via `--append-system-prompt`; `--ablate` = negative control (skill removed). **First full run scored 2/15 — a measurement artifact:** the plugin-free profile has no companion guard, so agents scouted with native `Read`/`Bash`, but the trace assertions required `mcp__codescout__*` tool calls → 11/13 false-fails; the judge was also trace-blind (`RUBRIC_PROMPT` has no `{trace}` slot). **Fix:** refactored rubrics to outcome-based (dropped trace assertions; agent shows its work in output) → 11/15, directional ~12/15. At the time C4 (treatment PASS / ablate RED) read as the load-bearing positive — the n=3 paired sweep later showed this was noise-dominated. **Whole run flagged INVALID** by a judge JSON-parse bug: a self-correcting judge emits two JSON objects, the greedy `re.search(r"\{.*\}")` at `judge.py:353` grabs first-`{`→last-`}`, `json.loads` raises → "unparseable judge verdict" → L-8 INVALID. So this row is **directional, not gate-grade**. Costs: first run $11.47; refactored $13.87; C4 de-risk $1.08+$0.79; ~30–36 min/run. Harness fixes filed prompt-engineering-side, in the `prompt-engineering` repo (not this one): docs/findings/2026-07-18-reconnaissance-mcp-eval-findings.md + backlog G-11. |
| 2026-07-19 | (MCP-live n=3, prompt-tdd) | Treatment 10/15; **real power small** — `--paired`: only **C4, C1** clear Δ≥0.5 (+0.67 each); `--ablate` xref: ~1/15 clean | **0 INVALID / 30+ runs** | **First gate-trustworthy MCP-live baseline.** The harness judge-parse fix (finding C, prompt-engineering `49a0036`) landed → **0 INVALID across three real subscription sweeps** (`ce81b72`, `dbdb61f`, `3d0fa3e`), confirming the C fix holds. **Sobers the 2026-07-18 ~12/15:** raw treatment pass-count conflates base-model competence with skill power. Authoritative method is now **`--paired`** (per-scenario pass-rate delta ≥ 0.5; prompt-engineering `b9bb298`): only **C4 and C1** clear the margin (+0.67 each), most other cases tautological/no-effect. The treatment arm swings run-to-run (C4/C1 seen FAIL→3/3), so single-run PASS/RED verdicts were noise-dominated (**L-15**). The **NA/NB/C14 SKILL.md edits are NOT validated** (≤+0.33, within n=3 noise) — left uncommitted in claude-plugins. Judge score-vs-reasoning drift is a separate calibration issue (**L-16**), not the parse bug. Cost/time ≈ $27 / ~70 min per paired sweep. |
| 2026-09-04 | `bb24b7f` (post-split, 13,966 B) | **power 1 · tautological 7 · no-effect 7** — only **C4** clears Δ≥0.5, at **+1.00** (3/3 vs 0/3) | **0 INVALID / 90 runs**; **C2/R-4 is a MISS-case FAIL** | **Second gate-trustworthy MCP-live baseline — and the FIRST on the post-split skill**, so a *new* baseline, not a replication of 2026-07-19. Skill under test is `bb24b7f`, 13,966 B, after the 2026-09-01 split cut it from 44,375 B (`402a090`) — a materially different artifact from the `627bbde` the previous sweep measured. Method unchanged: `--paired`, n=3 per scenario, margin 0.5, all 15 scenarios (C5 remains absent from the suite). **C4 reproduces and strengthens** (+0.67 → **+1.00**, perfect separation). **C1 does not** — it shared the podium at +0.67 last time and is flat **0.00** here; a second direct observation of the L-15 run-to-run swing, this time in the losing direction. **Three negative deltas:** C2/R-4 **−0.67** (0/3 treat vs 2/3 ctrl — all three treatment runs scored 0.3 partial credit, so a consistent partial-miss, not one bad run), C3/R-10 −0.33, C8/R-21 −0.33. **Both SKIP guards ran for the first time:** C13 tautological (3/3 both arms — correctly refrains), but **C14 is 0/3 on BOTH arms** — the over-scout guard is unmet by the skill *and* unreachable by base competence. C15 channel probe tautological (3/3 both), so the advertise-pull channel shows no measurable lift. Treatment arm passes 12/15 at threshold 0.5 (computed from the printed rates; the harness prints only the paired classification, not a treatment tally). **Ship gate NOT met** — ≥12/14 + zero-MISS-FAIL is blocked by C2, and C5 cannot be scored at all. Wall-clock **49 min** (03:09:29→03:58:25), 90 agent + 90 judge runs, exit 0. Run: `LIBRARIAN_DB=recon-eval-cat.db prompt-tdd run --config scenarios/reconnaissance/arm-recon.yaml --paired`. |
## Re-evaluation after baseline

**First run (2026-06-11) — outcome: a methodology flaw surfaced, not a trustworthy score.** This mirrors the trigger eval's first-run lesson: the value was exposing a design defect, not the number.

**What held up.** All 5 subagents, with the SKILL.md loaded and the R-N ledger withheld, scouted the named seam (read the symbol body / enumerated entry points / read the function) *before* committing to the assertion or action, and each surfaced the planted gap or correctly conditioned the answer. The skill→scout behavior fired. Two scouts exceeded the case spec: C7 found **two** same-named `RecoverableError` types (a contract the case author missed); C8 and C10 each handled a `references()` false-zero by falling back to grep (R-3) instead of trusting the 0.

**Why the number is an upper bound, not a measurement — contamination.** Every case is drawn from an incident codescout documents *in-tree* (`docs/trackers/reconnaissance-patterns.md`, `docs/issues/*`, `docs/adrs/*`, and now this eval file). All five agents surfaced the recorded answer via `grep` / `semantic_search`, so the run cannot distinguish *scouted the code* from *found the write-up*. The Contamination note above covered injecting the R-N ledger; it missed that the answers also live in sibling docs the agent can reach. That is the more severe vector.

**Stale-fixture problem.** HIT cases drawn from *resolved* incidents have healed in the current tree (C10's off-by-one is fixed at all three sites; C8's premise is the post-fix state). They now test "confirm-already-fixed," not "catch-live-drift."

**Fixes owed before a meaningful run:**
1. Run against a git worktree pinned to each case's *pre-fix* commit (drift live in code), or synthetic fixtures whose symbols are documented nowhere.
2. Deny the agent `docs/trackers`, `docs/issues`, `docs/adrs`, `docs/evals` (the answer surfaces there).
3. Re-score; only then does the ≥12/14 + zero-MISS-FAIL gate carry weight.

**Recorded per this eval's own instruction** ("if inspection again mispredicts, that is itself the headline finding"): here it is contamination, not misprediction — same shape. The first run earns its keep by failing informatively.

---

### Clean re-run (2026-06-11) — decontamination held

**The fix worked.** Throwaway worktrees with `docs/` deleted removed the answer-lookup vector (Case 4's own repo-wide `grep blake3` returned 0, proving it); C8 (`cd370079`) and C10 (`c99d4228`) sat at pre-fix commits so the drift was *live in code*, not healed.

**Result: 4/4 scouted from source and caught the drift.** Two were genuine live-drift catches (C8 CLI-bypass of `write_index_state`; C10 the off-by-one replicated across all three `edit_code` call sites). C12 is the cleanest evidence the decontamination mattered: with the ADR gone, the agent reconstructed the SIGKILL / mux-detach orphan boundary from `process.rs` + `manager.rs` alone — and surfaced *more* than the contaminated run, which had leaned on the ADR's seam-S2 framing. The skill→scout→catch behavior is not an artifact of doc-lookup.

**Honest bounds.** n=4 of 14; single scorer (no judge panel); skill loaded by explicit instruction (a mild scout cue vs. true auto-trigger). C7 is structurally un-decontaminatable — its answer lives in the SKILL.md's own worked exemplar, which the skill must load. C5 deferred (live lock-contention hazard). The SKIP guards (C13/C14 — does the agent *refrain* from scouting when it shouldn't?) are untested; a skill that scouts everything is also broken. **Next:** clean-run the remaining MISS cases (C1/C2/C3/C6) at appropriate fixtures and the SKIP guards before treating the ≥12/14 gate as met.

### MCP-live behavioral run (2026-07-18) — real tools + ablation control

**What was new.** Every prior run loaded the SKILL.md by *instruction* (a subagent
told to read it) and scored a trace produced with codescout's own tools available
only incidentally. This run used the prompt-tdd `anthropic-mcp` registry
(`prompt-engineering` repo), which launches the **real codescout binary as a live
MCP server** for the agent under test, injects the skill via
`--append-system-prompt`, and — critically — provides an `--ablate` arm that removes
the skill so treatment and control differ by exactly the skill text. This is the
MCP-live control the `skill-eval-playbook` L-7/L-12 entries said did not yet exist;
it exists now.

**The 2/15 was an artifact, not a result.** The first full run scored 2/15 and looked
catastrophic. It was a harness measurement flaw: the eval profile is plugin-free
(no codescout-companion), so nothing *forces* MCP tools — agents happily scouted with
native `Read`/`Bash`. But the rubrics asserted on the **trace** (`tool_called:
mcp__codescout__symbols`, etc.), so a correct scout done with native tools scored
FAIL. 11 of 13 scored cases false-failed this way, and the judge could not rescue them
because `RUBRIC_PROMPT` (`judge.py:26`) has no `{trace}` slot — it is trace-blind by
construction, seeing only the final response. The fix was to rewrite the rubrics as
**outcome-based**: drop the trace assertions, require the agent to *show its scouting
work in the output*, and judge that. Post-refactor the suite scored 11/15, directional
~12/15.

**C4 is the load-bearing evidence.** Beyond the aggregate, the single most informative
result is Case 4 (assert-a-checkable-fact-from-memory, R-19) run as a matched pair:
**treatment PASS, ablate RED.** Removing the skill text flips the outcome — the agent
without the skill asserts the fact from memory, the agent with it scouts first. That
is the ablation the older runs could never show (they had no skill-absent control), and
it is direct evidence the skill *causes* the scout behavior rather than correlating
with a doc the agent could look up.

**Why the number is directional, not a gate.** The whole run was flagged **INVALID** by
a judge JSON-parse bug independent of the skill: when a judge self-corrects and emits two
JSON objects, the greedy `re.search(r"\{.*\}", …, re.DOTALL)` at `judge.py:353` captures
from the first `{` to the last `}` — a non-JSON span — so `json.loads` raises and the
harness declares the verdict unparseable (L-8), poisoning the run's validity. The
subscription judge path (`ClaudeCliProvider`) has no `complete_structured`, so it *always*
hits this fragile text-parse branch. Until that is fixed (extract the *last* parseable
object), the ~12/15 cannot be promoted to a gate-grade baseline.

**Honest bounds.** Single run per arm (no n≥3, and C4 itself flipped across earlier
runs — the scout discipline is probabilistic, not deterministic); judge trace-blind so
process rubrics degrade to output-only; the plugin-free profile means single-tool
`tool_called` assertions structurally can't hold without a tool-forcing guard. All four
gaps and their fixes are filed on the harness side:
`prompt-engineering/docs/findings/2026-07-18-reconnaissance-mcp-eval-findings.md`
(findings A–E, exact code locations, runtimes/costs, re-run commands) and backlog
entry **G-11**. **Next:** land the harness fixes (judge last-object parse + trace-aware
rubric + tool-forcing guard), then re-run treatment + `--ablate` at n≥3 for a
gate-trustworthy MCP-live baseline.

### Correction (2026-07-19) — the n=3 baseline sobers the ~12/15 headline

**What changed.** The 2026-07-18 run above was flagged INVALID by a judge JSON-parse
bug and reported a *directional* ~12/15 with C4 as the load-bearing positive. The
prompt-engineering side then fixed the parse bug (finding C, `49a0036`) and re-ran
properly. Three real subscription sweeps (`ce81b72`, `dbdb61f`, `3d0fa3e`) produced
**0 INVALID across 30+ runs** — the parse fix holds, and this is the first
gate-trustworthy MCP-live baseline. The trustworthy number is **much lower** than the
headline implied.

**The core error the fix exposed: treatment pass-count ≠ skill power.** A treatment PASS
is confounded by base-model competence — a capable model scouts correctly with or without
the skill on many cases. Isolating the skill's *contribution* requires the control arm, and
at n=3 the control arm is as noisy as treatment (**L-15**). Two measurements make this
concrete:

- **`--ablate` cross-reference (`ce81b72`):** treatment 10/15, but "real power" (treatment
  PASS ∧ control RED) collapses to ~1/15 clean, ~5/15 skill-targeted-but-unreliable, ~9/15
  tautological.
- **`--paired` rate-delta (`3d0fa3e`) — the method now standard:** runs both arms against the
  *same* base model per scenario and scores power as `treatment_rate − control_rate ≥ 0.5`.
  Only **C4 and C1** clear the margin (+0.67 each); NA/NB/C14 reach at most +0.33. Crucially,
  pairing exposed that the treatment arm swings hugely run-to-run (C4/C1 seen FAIL→3/3) — the
  earlier single-run "C4 PASS/RED = load-bearing" verdict was noise-dominated, not signal.

**Net read on the skill.** `reconnaissance` has **small, real power on a couple of cases**,
nowhere near the 12/15 impression. That is not a failure of the skill so much as a correction
of the measurement: the eval now measures the right thing (paired power delta), and the honest
answer is "modest, variance-heavy, needs higher n to pin per-case."

**Consequence for skill iteration.** SKILL.md edits aimed at NA/NB/C14 exist (uncommitted in
`claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md`) but are **NOT validated**
— best case +0.33, inside the n=3 noise band. They stay uncommitted pending a higher-n paired
run on the contested subset. Do **not** commit skill edits on a single-run improvement again;
gate on a paired delta that clears the margin at n≥3. (Separately, **L-16**: the judge's score
field can contradict its own reasoning — a calibration issue to watch, distinct from the parse
bug the C fix closed.)
