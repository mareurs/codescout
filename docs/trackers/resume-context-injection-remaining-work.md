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
entry_high_water_CIW: 19
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
| CIW-10 | any | M | in-progress | Attribution scan: store each operation's write targets in the cache entry (approach 2) |
| CIW-11 | any | S–M | open | Route `Onboarding::call_content` through `run_post` |
| CIW-12 | any | S–M | open | Emission ceiling coverage: the session opener and a real p50 session |
| CIW-13 | any | S | open | GG-11: `librarian` § Reference sits 14 B under its 2,500 B cap |
| CIW-14 | any | S | open | GG-8: re-run the section-use probe against its pre-registered predictions |
| CIW-15 | any | M | open | A write-time check on an `entry_prefix` declaration (F-1's owed mechanism) |
| CIW-16 | any | S | open | Eval runner: re-link the credential cell every round |
| CIW-17 | any | S | open | A-46's "median run < 5 s" health rule misfires on short control prompts |
| CIW-18 | any | M–L | open | Principal identity: the edges the ADR left unmeasured |
| CIW-19 | any | S | open | Guide-ledger residuals S-01, S-02 and S-04 |

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

**Status:** in-progress — design conversation started 2026-11-04
**Valid:** dated 2026-11-04

**Owner:** any (operator chose this option on 2026-11-04). **Source:** bug `20927ccfe293c34e` § Remaining items (reopen trigger fired: a real red took 19,917 ms against a 15,000 ms trigger); `docs/superpowers/specs/2026-11-01-policy-loop-speedup-design.md` § Approaches (approach 2, deferred).

**Observed.** A warm `scan_provenance` takes about 13 s, of which 5.81 s is `write_targets` (phase timers, estimated not measured). Approach 2's recorded costs: the cache digest must cover `file-provenance.py`, so an edit to that file invalidates every entry (2,355 files here); a cold refill was measured at four calls killed at 30 s before the fifth answered; doc-tool targets stay dynamic; equivalence must be re-proved on the real corpus.

**Next:** finish the design (brainstorming), get it approved, then a plan.

## CIW-11 — Route `Onboarding::call_content` through `run_post`

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** any. **Source:** `src/engines/mod.rs` `POST_PHASE_BYPASSES` (added `4516d674c`).

**Observed.** `Onboarding` overrides `call_content` and never calls `run_post`, so an `onboarding` call gets no session opener, guide block, operator rule or ledger stamp. It is now declared, "recorded as found, not ruled deliberate".

**Next:** route it through `run_post` and remove the entry; `every_call_content_override_reaches_run_post_or_is_declared` then fails on a stale entry, which is the check that it worked.

## CIW-12 — Emission ceiling coverage: the session opener and a real p50 session

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** any. **Source:** `src/server.rs` `a_p50_session_stays_under_the_committed_emission_byte_ceiling` (`CEILING = 12_244`) and its comment (≈L17926–17934).

**Observed.** The fixture warms the ledger, which masks the session opener; the fixture triggers no operator rule; and the comment estimates a real p50 session at about 15,351 B, over the ceiling, with no test. Re-deriving the ceiling against real transcripts is called "a spec amendment".

**Next:** a fixture variant without `warm_ledger`; one probe run of p50 bytes on real transcripts.

## CIW-13 — GG-11: `librarian` § Reference sits 14 B under its 2,500 B cap

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** any. **Source:** `docs/trackers/resume-get-guide-section-grain-phases-2-3.md` GG-11.

**Observed.** § Reference ≈2,486 B (naive byte count, not the production parser); § doctor repairs grew to ≈2,338 B, so `librarian.doctor` draws ≈4,824 B. GG-11 lists three remedies, none adjudicated. Raising the cap is allowed only for a harness reason (`src/prompts/README.md` § Caps follow the harness).

**Next:** re-measure with the production parser, then pick a remedy.

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

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** any (prompt-engineering). **Source:** `prompt-hamsa-audit-log:A-44` runner repair; `src/prompt_tdd/profiles.py` `link_credentials` docstring.

**Observed.** On 2026-11-04 the flat `profiles/plugin-free` cell held an expired credential copy (every run exited 1 in about 1 s). After the repair, Claude Code 2.1.318 replaced the account cell's symlink with a regular file once (11:18), which the docstring says cannot happen (it was written against 2.1.252). Tokens were still identical, and `prompt-tdd profiles link` restored the link.

**Next:** call `link_credentials` at the start of every `run_arms.py` round, and correct the docstring.

## CIW-17 — A-46's "median run < 5 s" health rule misfires on short control prompts

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** any (prompt-engineering). **Source:** `prompt-hamsa-audit-log:A-46` outcome.

**Observed.** Two control cells ran at a 3.9 s and 4.3 s median with 8–10 distinct genuine answers, so the frozen dead-instrument rule fired on healthy cells.

**Next:** key the duration rule on distinct answers plus an auth/refusal pattern, or apply it to treatment cells only, in `scenarios/operator-rules/` before the next run.

## CIW-18 — Principal identity: the edges the ADR left unmeasured

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** any. **Source:** `docs/adrs/2026-09-14-a-subagent-is-a-principal.md` § Deliberately out of scope, § Revisit-when.

**Observed.** Not measured: whether `agent_id` survives a subagent's own compaction or resume; nested dispatch beyond the fork-seed guard (companion `3b94e5f`); clients other than Claude Code. The cross-repo key-drift guard is local only: codescout now pins the full literal (`4516d674c`), but no CI job sees both repos.

**Next:** a live probe of `agent_id` across a subagent resume (S); a CI job that checks out both repos (M–L).

## CIW-19 — Guide-ledger residuals S-01, S-02 and S-04

**Status:** open
**Valid:** dated 2026-11-04

**Owner:** any. **Source:** `docs/trackers/2026-08-18-guide-ledger-residuals.md`.

**Observed.** S-01 and S-02 (measurements) are recorded nowhere; 2,328 ledger files are on disk against an estimate of about 1,050. S-04: GC still filters on `.json` only (`src/tools/guide_ledger.rs`), with 0 `.tmp` files live.

**Next:** take the two measurements; decide whether S-04 needs code.

## Template for new entries

```
## CIW-N — <one-line title>

**Status:** open | in-progress | done | declined | deferred
**Valid:** dated YYYY-MM-DD | invariant | conditional — <event>

**Owner:** operator | any. **Source:** <where this came from>

**Observed.** <what was found, with evidence>

**Next:** <the concrete action>
```
