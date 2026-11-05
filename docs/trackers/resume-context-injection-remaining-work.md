---
id: '585c6883b1c24062'
kind: tracker
status: draft
title: Resume queue — Context injection, remaining work (CIW-N)
owners:
- marius
tags:
- resume-queue
- context-injection
- operator-rules
- engines
topic: context-injection
entry_high_water_CIW: 21
entry_prefix: CIW
snapshot_anchor: '| ID | Owner | Size | Status | Title |'
---

# Resume queue — Context injection, remaining work (CIW-N)

Work left after the 2026-11-04 audit of the context-injection plan family
(`context-injection-session-log:F-19`): operator rules, retrieval engines, guide ledger and
section grain, the subagent-principal ADR, and the System 1 measurement chain. Everything that
could be done without the operator was done that day (commits `a80c8eaec`, `b697e19bd`,
`c6159eb7f`, `4516d674c`, `3addb3e7d`, `e474f7bc5`, `1dbc7e033`); what is below is what was left.

Each entry names its source, so nothing here needs re-deriving. **Owner `operator`** means the
next step is a decision only the operator can make; **owner `any`** means a session can pick it
up. Statuses: `open | in-progress | done | declined | deferred`.

## Index

| ID | Owner | Size | Status | Title |
|---|---|---|---|---|
| CIW-1 | operator | S | open | System 1: label pilot-2 and give the k=11 ruling |
| CIW-2 | operator | S | open | System 1: decline or schedule base-rate tasks 11–12 and the prospective window |
| CIW-3 | operator | S (decision) | open | Retrieval engines: build or decline steps 2b, 3, 2c and 4 |
| CIW-4 | operator | S (decision) | open | Section grain: build or decline Phase 3 (GG-2) and the derived tables (GG-5) |
| CIW-5 | operator | S | open | OP-6: keep `always`, demote to `triggered`, or re-measure with every skill installed |
| CIW-6 | operator | S–M | open | OP-3: what the agent does when codescout is absent, and whether to reopen CAP-10 |
| CIW-7 | operator | M | open | Operator-rules ledger scope (per machine or per user) and the CM-10 per-host check |
| CIW-8 | operator | S | open | Superpowers marketplace skew across the four profiles blocks `release.sh` |
| CIW-9 | operator | S | open | Push the local commits in three repos |
| CIW-10 | any | M | done | Attribution scan: store each operation's write targets in the cache entry (approach 2) |
| CIW-11 | any | S–M | done | Route `Onboarding::call_content` through `run_post` |
| CIW-12 | any | S–M | done | Emission ceiling coverage: the session opener and a real p50 session |
| CIW-13 | any | S | done | GG-11: `librarian` § Reference sits 14 B under its 2,500 B cap |
| CIW-14 | any | S | open | GG-8: re-run the section-use probe against its pre-registered predictions |
| CIW-15 | any | M | open | A write-time check on an `entry_prefix` declaration (F-1's owed mechanism) |
| CIW-16 | any | S | done | Eval runner: re-link the credential cell every round |
| CIW-17 | any | S | done | A-46's "median run < 5 s" health rule misfires on short control prompts |
| CIW-18 | any | M–L | open | Principal identity: the edges the ADR left unmeasured |
| CIW-19 | any | S | done | Guide-ledger residuals S-01, S-02 and S-04 |
| CIW-20 | any | M | done | Classify the 55% repeat deliveries by re-arm cause |
| CIW-21 | any | M | done | Record the re-arm cause where the ledger re-arms |

## CIW-1 — System 1: label pilot-2 and give the k=11 ruling

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** operator. **Source:** `docs/superpowers/plans/2026-09-29-system1-labelled-sample.md` Task 6–7 (≈L428); handoff `docs/trackers/2026-09-29-system1-pilot-walkthrough-handoff.md` (last entry 2026-09-30).

**Observed.** The labelled-sample tooling is built (sampler, packet, label, estimate, run.py). Three pilot sets were drawn under `~/work/claude/measurement-corpora/` (`2026-09-29-labelled-pilot`, `2026-09-30-labelled-pilot-1r`, `2026-09-30-labelled-pilot-2`); none has a `labels.jsonl`. The spec's "11 or more → go" is false under the current code, so Amendment 1 needs a ruling first.

**Next:** label pilot-2 in your own terminal and return format feedback; rule on k=11. Then the main draw (60+20) and about 80 labels follow, and the intervention-judge gate spec can use those labels as held-out gold. This is the only route that measures whether injected context changes behaviour outside the lab.

## CIW-2 — System 1: decline or schedule base-rate tasks 11–12 and the prospective window

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** operator. **Source:** `docs/superpowers/specs/2026-09-26-system1-base-rate-measurement-design.md`; gate result `docs/evals/data/2026-09-27-system1-base-rates/gate-notes.md` (INCONCLUSIVE, 14/21 against 16).

**Observed.** After the INCONCLUSIVE gate the spec requires a prospective 21-day window; it was never frozen or read, and `usage.db` keeps 30 days, so its rows are partly gone. Tasks 11 (`readout.py`) and 12 (the real-corpus run) are stopped ("stays stopped", Codex review) but no document declines them. Also owed by the record: an off-machine backup of the frozen corpora and a token rotation.

**Next:** decline both on record, or schedule the window with a frozen start date.

## CIW-3 — Retrieval engines: build or decline steps 2b, 3, 2c and 4

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** operator. **Source:** `docs/superpowers/specs/2026-09-02-retrieval-engine-coordination-design.md` § Rollout.

**Observed.** Steps 0, 1 and 2a are built (`src/engines/`). Not built, never declined: 2b (Pre phase at `call_tool_inner`, advise only, M), 3 (`preview()` and the `codescout engines list|preview|check` CLI, M), 2c (block disposition and `CODESCOUT_BLOCK`, gates 6–8, M–L, needs 2b and 3), 4 (`/api/engines` dashboard routes, M; the graph route L, needs GG-7). The spec still lists them as pending; `src/server.rs:17954` says "The PRE phase does not exist yet (Rollout 2b)".

**Next:** a decision per step. A decline goes into the spec's Rollout table with its date and reason.

## CIW-4 — Section grain: build or decline Phase 3 (GG-2) and the derived tables (GG-5)

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** operator. **Source:** `docs/trackers/resume-get-guide-section-grain-phases-2-3.md` GG-2, GG-5.

**Observed.** Phases 1 and 2 shipped. Eight guide topics declare no `serves:` sections; four of them are still pull-only. A stopgap ships any topic over 16 KiB as a pointer (`MAX_AUTO_INJECT_GUIDE_BYTES`, `5d4ee2393`). GG-2's own plan says to measure next-topic traffic first.

**Next:** decide whether Phase 3 is wanted; if yes, start with that traffic measurement (L in total).

## CIW-5 — OP-6: keep `always`, demote to `triggered`, or re-measure with every skill installed

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** operator. **Source:** `prompt-hamsa-audit-log:A-44`, `:A-46`; `docs/trackers/operator-rules.md` OP-6.

**Observed.** A-44: OP-6 does not dilute OP-1. A-46: with the skill as the only installed skill, architecture prompts invoke it 0.95 without the rule and 1.00 with it, so OP-6 is redundant in that harness. Production has about 60 skills and the `buddy:` prefix, which the harness does not reproduce.

**Next:** a decision. If you want evidence first: re-run A-46's op6 cells with the full plugin set installed in the cell (instrument in prompt-engineering `scenarios/operator-rules/`).

## CIW-6 — OP-3: what the agent does when codescout is absent, and whether to reopen CAP-10

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** operator. **Source:** `prompt-hamsa-audit-log:A-46`; `docs/trackers/operator-rules.md` Index (CAP-10 note).

**Observed.** OP-3 works: built-in memory writes 20/20 → 0/20. But with no codescout tool the agent then stores the fact nowhere; it ignores a planted `docs/trackers/` file and hands the note back. Separately, OP-3's routed copy fires on `memory.write`, which never sees the violation; CAP-10 kept the hook option rejected on 2026-09-02 with the reopen condition "a rule that matters is provably harness-only", which OP-3 meets.

**Next:** decide (a) whether OP-3 should name a plain tracker file as the fallback, and (b) whether to reopen CAP-10 for a companion hook on built-in memory writes. Either change to the ledger is a re-N=0 wording and needs its own arm.

## CIW-7 — Operator-rules ledger scope (per machine or per user) and the CM-10 per-host check

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** operator. **Source:** `docs/superpowers/specs/2026-08-27-operator-rules-engine-design.md` § Not in scope (UNDECIDED bullet); `docs/trackers/resume-cross-machine-catalog-restore.md` CM-10.

**Observed.** The ledger is compiled per build, while the rules are "the human's". Three options are on record (per machine; per user, synced; a per-user base with a per-machine layer); none chosen since 2026-08-29. CM-10's cheaper half — run `operator-rules check` on every host from a hook or CI — was never built.

**Next:** choose the scope; separately, say whether a SessionStart hook or CI should run the check (S–M once chosen).

## CIW-8 — Superpowers marketplace skew across the four profiles blocks `release.sh`

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** operator. **Source:** claude-plugins `scripts/check-profile-parity.sh` output during the codescout-companion 1.20.29 release.

**Observed.** `MARKETPLACE SKEW: superpowers-marketplace — different git HEADs across profiles: 5c90728 87b5911`. Claude Code auto-updated `superpowers` to 7.0.2 in `~/.claude` only, at 2026-11-04T10:48Z. The release script refuses to repair it; its suggested repair is an rsync from the newest HEAD after a symlink check, which writes into every profile.

**Next:** approve the repair (or choose another), then re-run `scripts/release.sh` to reach its push step.

## CIW-9 — Push the local commits in three repos

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** operator. **Source:** this session (sessionId `732f1c09-df3f-48d4-be63-d9326860edda`), which pushed nothing.

**Observed.** codescout: `a80c8eaec`, `b697e19bd`, `c6159eb7f`, `4516d674c`, `3addb3e7d`, `e474f7bc5`, `1dbc7e033` (plus this file's commit). claude-plugins: `836aadd`, `8f230f7`, `696a592` (codescout-companion 1.20.29, already cached in all four profiles). prompt-engineering: `9bc8e4e`, `c2025d6`, `70e9d19`.

**Next:** push when ready; claude-plugins via `scripts/release.sh` after CIW-8.

## CIW-10 — Attribution scan: store each operation's write targets in the cache entry (approach 2)

**Status:** done — 2026-11-04, commits `c754e00b3` and `d8f3512c8`
**Valid:** dated 2026-11-04

**Owner:** any (operator chose this option on 2026-11-04). **Source:** bug `20927ccfe293c34e` § Remaining items (reopen trigger fired: a real red took 19,917 ms against a 15,000 ms trigger); `docs/superpowers/specs/2026-11-01-policy-loop-speedup-design.md` § Approaches (approach 2, deferred).

**Observed.** A warm `scan_provenance` takes about 13 s, of which 5.81 s is `write_targets` (phase timers, estimated not measured). Approach 2's recorded costs: the cache digest must cover `file-provenance.py`, so an edit to that file invalidates every entry (2,355 files here); a cold refill was measured at four calls killed at 30 s before the fifth answered; doc-tool targets stay dynamic; equivalence must be re-proved on the real corpus.

**Result (measured, 2026-11-04).** Four equal digests over the real corpus (old, new, old, new, one process); lifetime scan 121.5 s warm before, 8.9 s warm after; `attribute-red.py` warm median 1.66 s before, 0.95 s after (42.8%). Gate green (6,010 lean, 8,874 default). Records: bug `20927ccfe293c34e` § Remaining items. **Left open:** the first scan after deploy misses every entry (the cache key changed) and a cold refill is about 120 s here, over the 30 s ceiling, so the first red after a deploy may show no hint; the `timed_out` reopen trigger stands.

## CIW-11 — Route `Onboarding::call_content` through `run_post`

**Status:** done
**Valid:** dated 2026-11-04

**Owner:** any. **Source:** `src/engines/mod.rs` `POST_PHASE_BYPASSES` (added `4516d674c`).

**Observed.** `Onboarding` overrides `call_content` and never calls `run_post`, so an `onboarding` call gets no session opener, guide block, operator rule or ledger stamp. It is now declared, "recorded as found, not ruled deliberate".

**Next:** route it through `run_post` and remove the entry; `every_call_content_override_reaches_run_post_or_is_declared` then fails on a stale entry, which is the check that it worked.

**Result (2026-11-04, `c8e0fa428`).** `Onboarding::call_content` now calls `run_post` in its own body on the success path (the gate test reads the body, so a helper does not satisfy it). `POST_PHASE_BYPASSES` is `&[]`. New test `server::guide_hint_tests::a_session_that_opens_with_onboarding_gets_the_opener` failed first (1 block) and now passes. Ten existing `run_command/tests.rs` tests asserted exactly one block from `onboarding`, so they had encoded the bypass; they now call `onboarding_own_blocks`, which marks the opener as delivered and keeps asserting onboarding's own blocks. Gate: lean 6,012 passed, default 8,878 passed.

**Left as a ruling, not measured:** the emission's `_guide_hint` is dropped, because `onboarding` builds text blocks and the hint is a field in a primary JSON block. `overflowing` is `false`. If onboarding ever returns an oversized envelope, revisit both.

## CIW-12 — Emission ceiling coverage: the session opener and a real p50 session

**Status:** done (cold ceiling added; real-transcript probe not run)
**Valid:** dated 2026-11-04

**Owner:** any. **Source:** `src/server.rs` `a_p50_session_stays_under_the_committed_emission_byte_ceiling` (`CEILING = 12_244`) and its comment (≈L17926–17934).

**Observed.** The fixture warms the ledger, which masks the session opener; the fixture triggers no operator rule; and the comment estimates a real p50 session at about 15,351 B, over the ceiling, with no test. Re-deriving the ceiling against real transcripts is called "a spec amendment".

**Next:** a fixture variant without `warm_ledger`; one probe run of p50 bytes on real transcripts.

**Partial result 2026-11-04 (cold fixture measured; real transcripts not run).** A throwaway probe ran the fixture's six `doc` shapes with NO `warm_ledger`, fixture root normalised as the fixture does, and was removed afterwards (`git diff` on `src/server.rs` empty). Bytes after the primary block: `create` 3,552 (3 blocks), `get` 2,733, `update` 2,685, `append_entry` 1,634, `find` 2,649, `move` 2,290. **Cold total 15,543 B against `CEILING = 12_244`: 3,299 B (27%) over.** The comment's "roughly 15,351 B" was a conservation model, not a measurement; the measurement is 192 B higher. So the ceiling holds only because the fixture hides the opener: a real session's first call draws the bootstrap guide on top. Decision for the operator, not made here: raising `CEILING` is a spec amendment by its own comment. Options are (a) keep the warm ceiling as the section-grain guard and add a second, cold ceiling at about 15,543 B plus margin, or (b) leave it recorded. The permanent cold fixture variant is NOT added: it needs that ceiling value.

**Resolved 2026-11-04 (operator chose option (a)).** Added `guide_hint_tests::a_cold_p50_session_stays_under_the_committed_cold_emission_byte_ceiling` with `COLD_CEILING = 15_671` (15,543 B measured + the 128 B margin the warm ceiling uses; the margin is my ruling). It runs the same six `doc` shapes with no `warm_ledger` and asserts the first call carries the opener (`ALWAYS VERIFY`), so it cannot silently turn warm. Two mutants killed: ceiling lowered to 15,000 reds with `emitted 15543 B ... margin -543 B`; a `warm_ledger` call reds on the opener assertion. Gate: default 8,879 passed, lean 6,012 passed. **Not done:** the one probe run of p50 bytes over real transcripts; the cold ceiling prices the fixture's six shapes, not a measured real session.

## CIW-13 — GG-11: `librarian` § Reference sits 14 B under its 2,500 B cap

**Status:** done (remedy 1)
**Valid:** dated 2026-11-04

**Owner:** any. **Source:** `docs/trackers/resume-get-guide-section-grain-phases-2-3.md` GG-11.

**Observed.** § Reference ≈2,486 B (naive byte count, not the production parser); § doctor repairs grew to ≈2,338 B, so `librarian.doctor` draws ≈4,824 B. GG-11 lists three remedies, none adjudicated. Raising the cap is allowed only for a harness reason (`src/prompts/README.md` § Caps follow the harness).

**Next:** pick a remedy (the re-measure below is done).

**Re-measured 2026-11-04 with the production selector** (`GuideIndex::match_sections`, so the `requires` closure is included; a throwaway probe, removed afterwards, `git status` clean). § *Reference* is **2,486 B against `MAX_DECLARED_SECTION_BYTES = 2500`: margin 14 B**, and the naive count above equals the production count. All eight `librarian.*` shapes draw it. `librarian.doctor` draws **4,824 B** (2,486 + 2,338 for § *doctor repairs*); the other seven draw 2,486 B. The six `doc.*` shapes draw 2,215 (`create`, `get`), 2,123 (`update`), 1,328 (`append_entry`), **2,429 (`find`)** and 2,042 (`move`). So `librarian.doctor` is 1.99× the largest `doc.*` shape, and GG-11's "largest doc.* shape is `doc.update` at 2,123 B" is stale: `doc.find` (§ *Filter Syntax*, 2,429 B) is now the largest. Section bytes only; block wrappers add more. The remedy is still open and is the operator's call.

**Resolved 2026-11-04 (operator chose remedy 1).** Added `guide_hint_tests::librarian_doctor_stays_under_its_committed_emission_byte_ceiling` with `DOCTOR_CEILING = 5_735`: it calls `librarian(action="doctor")` on a warm ledger and counts every block after the primary, fixture root normalised. Measured **5,607 B** (783 B above the 4,824 B of section bytes: block wrappers and seams), plus the 128 B margin the sibling ceilings use (my ruling). Two discriminators run before the ceiling check, so the fixture cannot silently price another shape: the blocks must contain `doctor repairs` and the `Reference` heading. Proofs: ceiling 0 reds with `emitted 5607 B ... margin -5607 B`; calling `reindex` instead of `doctor` reds on the repairs discriminator. Gate: default 8,880 passed, lean 6,012 passed. **Not changed:** § *Reference* itself is still 2,486 B, 14 B under its per-section cap; this test prices the shape's draw, it does not widen that margin. The next byte added to § *Reference* trips the per-section cap first.

## CIW-14 — GG-8: re-run the section-use probe against its pre-registered predictions

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** any. **Source:** GG-8 in the section-grain queue; probe `scripts/probe_guide_section_use.py` (rekeyed by topic and heading, `ec439798a`, `01b1a82bf`).

**Observed.** Four predictions have no held/falsified record; prediction 4 is moot since Phase 2 shipped. Blocked on sample size and on re-copying the desktop transcripts.

**Next:** re-run when the sample allows; record each prediction's verdict in GG-8.

## CIW-15 — A write-time check on an `entry_prefix` declaration (F-1's owed mechanism)

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** any. **Source:** `context-injection-session-log:F-1` (mitigated); `docs/TAXONOMY.md` F/W warning.

**Observed.** The TAXONOMY sentence was corrected on 2026-11-04. The warning still asks each author to "decide before adding more", an instruction with no trigger. The mechanism it names is a check keyed on the declaration at write time (Observer Blindness position 3).

**Next:** design the check (it is a corpus-policy question over about 19 logs, so the policy half may need the operator).

## CIW-16 — Eval runner: re-link the credential cell every round

**Status:** done
**Valid:** dated 2026-11-04

**Owner:** any (prompt-engineering). **Source:** `prompt-hamsa-audit-log:A-44` runner repair; `src/prompt_tdd/profiles.py` `link_credentials` docstring.

**Observed.** On 2026-11-04 the flat `profiles/plugin-free` cell held an expired credential copy (every run exited 1 in about 1 s). After the repair, Claude Code 2.1.318 replaced the account cell's symlink with a regular file once (11:18), which the docstring says cannot happen (it was written against 2.1.252). Tokens were still identical, and `prompt-tdd profiles link` restored the link.

**Next:** call `link_credentials` at the start of every `run_arms.py` round, and correct the docstring.

**Result (2026-11-05, `prompt-engineering:997b4cd`).** Ruling: the call lives in `prompt-tdd run`, not in `run_arms.py`. `run_arms.py` starts one `prompt-tdd run` per arm, so that is the start of every round, and only `run` knows each arm's account and isolation class. New `profiles.refresh_cell_credentials(account, isolation_class, config_dir)` returns None for a `config_dir` or ambient profile (no account source, so nothing to link) and otherwise calls `link_credentials`. A missing account source now refuses the run with the existing remedy text. The docstring's claim that a linked cell cannot materialize a file is marked as true for 2.1.252 only. Tests: `test_refresh_restores_a_link_that_claude_code_replaced_with_a_regular_file`, `test_refresh_touches_nothing_unless_the_cell_is_chosen_by_account_and_class`, both RED first.

**Found on the way, fixed in `prompt-engineering:bd67360` (2026-11-05).** Three tests already failed at `prompt-engineering` HEAD `70e9d19`, checked in a clean worktree: `test_sync_creds_and_snapshot_projects_both_walk_the_round_cells`, `test_sync_creds_and_snapshot_projects_behave_correctly_under_a_stub_round` (both in `test_run_pilot_wiring.py`, `sync_creds` in `run_pilot.sh` now calls `profiles link` and no longer walks `round_cells`) and `test_no_scenario_file_imports_an_ambiguous_module_by_bare_name` (bare `il3_shapes` imports in three il3 scenarios). Full suite with my CIW-16 change: 731 passed, those 3 failed. After `bd67360`: 735 passed, 0 failed. The two `sync_creds` tests had a bug file, `docs/issues/2026-09-09-sync-creds-rewrite-left-two-tests-asserting-the-pre-rewrite-shape.md`, now `fixed`. The il3 guard failure is fixed by loading `il3_shapes` by path in four checkers; the il3 test scripts (23/23, 43/43, 55/55) and mutation probes (6/6, 28/28, 45/45) all hold.

## CIW-17 — A-46's "median run < 5 s" health rule misfires on short control prompts

**Status:** done
**Valid:** dated 2026-11-04

**Owner:** any (prompt-engineering). **Source:** `prompt-hamsa-audit-log:A-46` outcome.

**Observed.** Two control cells ran at a 3.9 s and 4.3 s median with 8–10 distinct genuine answers, so the frozen dead-instrument rule fired on healthy cells.

**Next:** key the duration rule on distinct answers plus an auth/refusal pattern, or apply it to treatment cells only, in `scenarios/operator-rules/` before the next run.

**Result (2026-11-05, `prompt-engineering:892d90c`).** Ruling: the rule was prose in the unregistered decision tables of `gen_oprules.py`, with no code behind it, so the amendment is prose too. A cell is dead on distinct answers == 1, or on a median under 5 s together with an auth or refusal pattern (every run exits 1, or the answer is a login or refusal text). A short median alone is not a tell. OP-6 row 1 and the OP-3 row now say so, and the OP-4 row still reads "OP-11 tells" and so follows. Regenerated with `gen_oprules.py --runs 10`: 25 `scenario.yaml` files changed, `description:` field only. Cost if wrong: a dead cell that answers fast with varied text is now read as alive; the auth or refusal pattern is the only guard, and it is stated, not coded.

## CIW-18 — Principal identity: the edges the ADR left unmeasured

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** any. **Source:** `docs/adrs/2026-09-14-a-subagent-is-a-principal.md` § Deliberately out of scope, § Revisit-when.

**Observed.** Not measured: whether `agent_id` survives a subagent's own compaction or resume; nested dispatch beyond the fork-seed guard (companion `3b94e5f`); clients other than Claude Code. The cross-repo key-drift guard is local only: codescout now pins the full literal (`4516d674c`), but no CI job sees both repos.

**Probe result (2026-11-05, Claude Code 2.1.318, headless, Sonnet).** One persisted `claude -p` session driven with `--resume` in the isolated `plugin-free` cell, with a logging hook on `PreToolUse`, `SubagentStart`, `SubagentStop`, `PreCompact` and `SessionStart`. Four turns: dispatch a subagent (it ran one `Bash`), `SendMessage` to resume it (one `Bash`), `/compact` on the parent, `SendMessage` to resume it again (one `Bash`). Cost about $0.55.

| step | subagent `Bash` call `agent_id` | parent calls |
|---|---|---|
| dispatch | `a2ea815e9d940ed22` | no `agent_id` |
| resume by `SendMessage` | `a2ea815e9d940ed22` | no `agent_id` |
| resume after the parent's `/compact` | `a2ea815e9d940ed22` | no `agent_id` |

`agent_id` survives a `SendMessage` resume and a parent compaction. `session_id` stayed `c2b187b5-e0a4-43f9-be66-2d9d5982fc19` throughout. Each resume fired a fresh `SubagentStart` with the same `agent_id`, so `guide_rearm` re-arms on every resume by design. The compaction itself ran as a hidden agent: one `SubagentStop` with `agent_id` `a4ae8eb857838b3c1` and an empty `agent_type`, no `SubagentStart`, no tool call. `agent-guide-restore.mjs` treats a missing snapshot as a silent no-op, so that event is harmless to the companion.

**Limits.** One agent and one resume path (`SendMessage`). The probe reads the Claude Code hook payload, which is what `principal-stamp.mjs` stamps from, but it did not run the codescout MCP server, so the stamped key and the ledger file were not observed. A subagent's own compaction was not measured: it cannot be forced cheaply, and the ADR line stays open for it. The scripts lived in the session scratchpad and are not kept.

**Next:** a CI job that checks out both repos (M–L). The subagent's own compaction needs a subagent long enough to auto-compact.

## CIW-19 — Guide-ledger residuals S-01, S-02 and S-04

**Status:** done (S-02's before/after premise cannot be met; the current level is recorded, and the classification question moved to CIW-20)
**Valid:** dated 2026-11-04

**Owner:** any. **Source:** `docs/trackers/2026-08-18-guide-ledger-residuals.md`.

**Observed.** S-01 and S-02 (measurements) are recorded nowhere; 2,328 ledger files are on disk against an estimate of about 1,050. S-04: GC still filters on `.json` only (`src/tools/guide_ledger.rs`), with 0 `.tmp` files live.

**Next:** take the two measurements; decide whether S-04 needs code.

**Partial result 2026-11-04 (S-01 measured, S-04 evidence in; S-02 still open).** `$XDG_STATE_HOME/codescout/guide_hints/` held 2,355 files, all `.json`, 0 other, 419,248 bytes (about 178 B per file). 1,851 were older than 7 days and 113 were newer than 1 day. The oldest was 35 days old (2026-09-30), which matches `GC_MAX_IDLE_DAYS = 35`, so the GC is deleting on schedule. Steady state is `starts/day × 35`, so 2,355 files means about 67 ledger-creating starts per day here, against the spec's estimate of 30. Reading and parsing every file once takes 67.7 ms in Python (an upper bound for the Rust reader). That is the cost S-01 worried about, and it stays in the tens of milliseconds at 2.2× the estimated population. S-04: 0 `.tmp` files are live, so there is no evidence that `gc`'s `.json`-only filter strands anything; no code is warranted by this measurement. S-02 (Phase A against `usage.db`) still needs a re-injection measurement over `usage.db`.

**S-02 measured 2026-11-04.** The premise ("Phase A changed only where the ledger lives, so the expected delta is near zero") cannot be tested: `tool_calls` in `.codescout/usage.db` starts at **2026-10-06**, seven weeks after Phase A (2026-08-18), so no pre-Phase-A baseline exists in it. What it does give is the level today, read from `deliveries_json` (what the engines delivered, not text that merely quotes a guide): 91,635 rows carry the column, 4,007 are non-empty, over 584 distinct `cc_session_id`s. Of 5,212 ledger-key deliveries, **2,887 (55%) repeat a key already delivered to the same `cc_session_id`**. By bytes, **10,256,761 of 19,887,478 delivered bytes (51.6%)** went to repeat keys (block bytes split evenly across the keys of one delivery, so this is approximate). The most repeated keys: `project-activation-bootstrap` 770, `progressive-disclosure` 330, `symbol-navigation` 291, `librarian#Artifact Model` 123. **This is not a waste figure.** The ledger re-arms by design on compaction, `/clear`, a project switch and the anonymous-tier idle TTL, and this measurement does not separate those from a ledger that failed to remember. A first query over `output_json` (marker text) gave a different, inflated count, because tool results that quote guide text carry the same marker; do not use it. The classification is CIW-20.

## CIW-20 — Classify the 55% repeat deliveries by re-arm cause

**Status:** done (classified as far as `usage.db` allows; the residual needs CIW-21)
**Valid:** dated 2026-11-04

**Owner:** any. **Source:** CIW-19 S-02 measurement (this ledger).

**Observed.** In `.codescout/usage.db` (2026-10-06 to 2026-11-04), 2,887 of 5,212 ledger-key deliveries (55%) repeat a key already delivered to the same `cc_session_id`, and 51.6% of delivered bytes go to such repeats. The ledger re-arms by design on compaction, `/clear`, a project switch and the anonymous-tier idle TTL, so the number is an upper bound on waste, not a measurement of it.

**Next:** for each repeat, find the re-arm cause (a `SessionStart` source of `compact` or `clear`, a project switch, an idle gap over the TTL, or none of these). Only the "none of these" share is a ledger that failed to remember. `project-activation-bootstrap` alone repeats 770 times, so start there.

**Result 2026-11-04.** Same window (2026-10-06 to 2026-11-04), `deliveries_json`, rows with a `cc_session_id`. 2,916 repeat deliveries, 10,329,561 B (the count differs from CIW-19's 2,887 because this pass drops rows without a `cc_session_id`). Each repeat is assigned the FIRST cause that applies between the key's previous delivery and this one:

| cause | repeats | share | bytes | share |
|---|---|---|---|---|
| subagent re-arm (the delivering call has an `agent_id`) | 355 | 12.2% | 1,715,521 | 16.6% |
| compaction (a `workspace(post_compact=true)` call in between) | 1,028 | 35.3% | 3,504,873 | 33.9% |
| project switch (a `workspace(action="activate")` call in between) | 181 | 6.2% | 550,092 | 5.3% |
| a gap of 2 h or more between calls | 430 | 14.7% | 1,292,239 | 12.5% |
| **none of these (main thread)** | **922** | **31.6%** | **3,266,836** | **31.6%** |

So at most **3.27 MB in 30 days (about 109 KB a day, about 0.82 M tokens at 4 B per token) can be a ledger that failed to remember**, and that is an upper bound. Why: (1) `SessionStart:compact` can re-arm through a request file with no `workspace(post_compact)` call, and `usage.db` records only the tool call, so some of the 922 are compactions. (2) The 2 h gap is a PROXY, not a cause: the identified tier has no idle TTL (`idle_ttl` is anonymous-tier only), so a long gap marks a resume or a compaction, not a proven re-arm. (3) The subagent row is by design: the companion's `SubagentStart` hook writes re-arm requests (`src/tools/guide_rearm.rs`) so that a fresh subagent gets guides it never saw. Of the 922, `project-activation-bootstrap` is 230, `progressive-disclosure` 113, `symbol-navigation` 111. For scale, the programme opened against a measured ~900 K tokens of re-injection waste; the upper bound here is the same order of size, so this does not show a regression or its absence. The residual cannot be classified from `usage.db`: no re-arm event is recorded. That gap is CIW-21.

## CIW-21 — Record the re-arm cause where the ledger re-arms

**Status:** done
**Valid:** dated 2026-11-04

**Owner:** any. **Source:** CIW-20 result (this ledger).

**Observed.** `deliveries_json` records what each call delivered but not why a key that was already in the ledger was delivered again. Re-arms happen in at least five places (compaction through the `SessionStart:compact` request file or `workspace(post_compact)`, `/clear`, a project switch, a subagent dispatch or resume through `guide_rearm`, and the anonymous-tier idle TTL), and none writes an event. CIW-20 had to infer causes from neighbouring tool calls, and 922 repeats (31.6% of repeat bytes) stayed unexplained.

**Next:** record the re-arm cause where the ledger re-arms, so a repeat delivery can be tied to it. Smallest shape to try: a `rearm` field next to `ledger_keys` in the delivery record, or one row per re-arm in `usage.db`. The ledger is in memory and shared per process, so decide first whether the cause rides the next delivery or is written when the re-arm happens. Add a test that each re-arm path records its cause.

**Result (2026-11-05, `9ea0cb840`).** Ruling: the cause rides the next delivery. `GuideLedger` keeps `rearm_causes`; `clear`, `re_arm` and `expire_idle` add one `RearmCause` when they removed keys. `run_post_in` takes them once, for the first delivery it records, into `DeliveryRecord.rearm` (omitted when empty, so old rows keep their shape). A response that delivers nothing leaves the cause for the next one. Causes: `activate`, `post_compact`, `project_switch`, `subagent_rearm`, `idle_ttl`. `rekey` and `adopt` record nothing, because a conversation change cannot show as a repeat in `usage.db`. Tests: `every_rearm_path_records_the_cause_of_what_it_removed`, `a_rearm_that_removed_nothing_records_no_cause`, `causes_wait_for_the_next_delivery_and_are_taken_once`, `a_rearm_cause_rides_the_first_delivery_only_and_survives_an_empty_response`. Gate: lean 6,067 and default 8,935 passed.

**Limits.** Only new rows carry `rearm`; old rows cannot be classified this way. A `post_compact` that the `SessionStart:compact` request file handles without a `workspace(post_compact)` call may still record no cause. Re-run the CIW-20 classification on rows after this commit to see how much of the 31.6% remains.
## Resume — state before the live compaction test, 2026-11-05

**Session.** `732f1c09-df3f-48d4-be63-d9326860edda`, profile `~/.claude-kat`, project `/home/marius/work/claude/codescout`, branch `experiments`. Transcript `~/.claude-kat/projects/-home-marius-work-claude-codescout/732f1c09-df3f-48d4-be63-d9326860edda.jsonl`. Buddy state `.buddy/732f1c09-df3f-48d4-be63-d9326860edda/`. **CORRECTED after the compaction: the serving MCP process is NOT pid 268135.** It is pid 3194264 (instance `acf8`, started 2026-11-04 19:43:07Z, log `.codescout/diagnostic-acf8.log`). Its image has no `subagent_rearm` (`grep -c -a` on `/proc/3194264/exe` gives 0), so it predates CIW-21 (`9ea0cb840`, 2026-11-05 08:57 +02:00). Pid 268135 does contain it (count 1) but serves another session. The first version of this paragraph picked the process from `ps` without matching the instance id in the log.

**Why a live test.** CIW-21 was only tested in unit tests and the gate. CIW-18's probe ran headless, without the codescout server. This compaction is the first one where the shipped `RearmCause` code, the companion hooks and a real ledger meet.

**Baseline before compacting** (`.codescout/usage.db`, `cc_session_id` above): 837 rows, `max(id)` 267593 at 2026-11-05 14:37:39 UTC, 0 rows whose `deliveries_json` holds `"rearm"`. No re-arm has happened in this server's life.

**Prediction, to be held or falsified.**

1. The companion's `SessionStart` hook reports `source: compact`, and the first `workspace(post_compact=true)` clears the ledger (response says `"ledger": "cleared"`, not `"kept"`).
2. The row for that call has `rearm` equal to `["post_compact"]` on its first delivery in `deliveries_json`, and no later row in the session has `rearm`.
3. Main-thread rows keep `agent_id` NULL before and after.
4. A subagent dispatched after compaction gets its first delivery tagged `subagent_rearm`, with an `agent_id` that is not NULL.

**Check after compaction.**

```
sqlite3 .codescout/usage.db "select id, tool_name, agent_id, deliveries_json from tool_calls where cc_session_id='732f1c09-df3f-48d4-be63-d9326860edda' and id > 267593 and deliveries_json like '%\"rearm\"%'"
```

A missing row after a `"cleared"` response falsifies prediction 2 and means the cause was taken by a response that delivered nothing, or lost. A `rearm` on a row that is not the first delivery after the clear falsifies the take-once rule.

**Repo state.** codescout HEAD `ce69033c7` (peer commits on top of `6e85e1183`). Nothing of this session is uncommitted. Nothing is pushed in any repo (CIW-9). `prompt-engineering` carries `997b4cd`, `892d90c`, `bd67360`, `39672ce` unpushed; its untracked `scenarios/plan-opens-the-body/` and `.codescout/write.lock.holder` are not ours. The peer's staged files in codescout (`.codescout/memories/bug-triage-2026-10-27.md` and the hint-authority files) must not be touched.

**Open after this session.** CIW-14 (blocked on sample and desktop transcripts), CIW-15 (policy half is the operator's), CIW-18's CI half (design), CIW-1..9 (operator). The ADR `2026-09-14-a-subagent-is-a-principal.md` still says resume and compaction are "not measured"; amending it waits for the operator's word, as does keeping the probe scripts (they lived in the scratchpad and are gone).

**Result of the live compaction test (2026-11-05, 15:42 UTC).**

1. Held. `workspace(post_compact=true)` answered `"ledger": "cleared"`, and the log line `PostCompact: flushed all LSP clients ... ledger="cleared"` is in `diagnostic-acf8.log`.
2. NOT TESTED. Row 267892 (`workspace`, first delivery after the clear, engine `session-opener`) has no `rearm`, and no later row has one. The server that answered has no CIW-21 code, so this absence says nothing about the shipped code.
3. Held for what was observed. Rows 267892 and 267894 (main thread, after compaction) have `agent_id` NULL, as do the 10 rows before.
4. NOT TESTED, for the same reason as 2.

**What it takes to run the test for real.** Restart the MCP server for the session (`/mcp`) so it loads the current `target/release/codescout`. Confirm the serving process by matching `instance=` in `.codescout/diagnostic-*.log` to its pid, then `grep -c -a subagent_rearm /proc/<pid>/exe` must give at least 1. Only then compact again and run the check query. A `rekey` or `adopt` before the clear would leave `clear` nothing to remove, so no cause is recorded by design; with the right binary, check the log for `restored`/`adopted` lines before calling a missing `rearm` a failure.

**Result of the second compaction, on the right binary (2026-11-05, 15:53-15:54 UTC).** Serving process: pid 2749660, instance `0696`, log `.codescout/diagnostic-0696.log`, `subagent_rearm` count 1 in its image. The log has no `restored`/`adopted` line before the clear, so the empty-ledger exception did not apply.

1. Held. `workspace(post_compact=true)` answered `"ledger": "cleared"` (log line 35).
2. Held. Row 268049 (`workspace`, engine `session-opener`) has `rearm` equal to `["post_compact"]`. No later main-thread row has `post_compact`. Row 268000 has `["project_switch"]`: it is the first delivery after the server started, so the cause is correct, and it was a bonus observation, not a prediction.
3. Held. Rows 268011, 268049 and 268050-268053 (main thread) have `agent_id` NULL.
4. Held, with a correction to the prediction. A NEW subagent (`a90ee21e2388320ae`, row 268055, `get_guide`) has a non-NULL `agent_id` and a `session-opener` delivery, but NO `rearm`. The log shows the server did call `re_arm` for it (line 52), but its ledger was empty, so `re_arm` removed no key and `note_rearm` recorded nothing. This is by design. After `SendMessage` resumed the same subagent, row 268071 has `agent_id` `a90ee21e2388320ae` and `rearm` equal to `["subagent_rearm"]`. The log shows `restored the guide ledger of a previously served principal` (line 74) and then `re-arming guide topics` (line 75). So `subagent_rearm` marks a RESUMED subagent whose ledger holds the topic, not a first dispatch.

**Consequence for readers of `deliveries_json`.** A missing `rearm` on a subagent's first row is normal. Count `subagent_rearm` rows as resumes, never as dispatches.

## Template for new entries

```
## CIW-N — <one-line title>

**Status:** open | in-progress | done | declined | deferred
**Valid:** dated YYYY-MM-DD | invariant | conditional — <event>

**Owner:** operator | any. **Source:** <where this came from>

**Observed.** <what was found, with evidence>

**Next:** <the concrete action>
```
