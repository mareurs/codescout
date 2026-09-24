---
id: '1b770cb6462acde6'
kind: tracker
status: active
title: Deep-agent workflow observations and session coverage
tags:
- deep-agent
- observation
- prospective
- workflow
topic: deep-agent-observation
time_scope: '2026-09-18_to_2026-10-02'
entry_prefix:
- DWF
- DCS
snapshot_anchor: '| ID | Date UTC | Kind | Sampling | Capture key |'
entry_high_water_DWF: 7
entry_high_water_DCS: 7
---

# Deep-agent workflow observations and session coverage

**Status: collecting, 18 September–2 October 2026 (UTC).** First review: 25 September. Stop routine capture at 00:00 UTC on 2 October unless extended by the user. Companion: [context timing observations](deep-agent-context-observations.md). Design: [codescout deep agent](local-semantic-evaluator-design.md). Starting evidence: [baseline measurement](../research/2026-09-18-deep-agent-observation-baseline.md).

## Question and scope

Which current investigation/test/fix workflows are repeatable enough to delegate to a lightweight codescout worker, and what evidence would let it verify completion? Observe ordinary host-agent/human work before building the worker. A successful human-assisted sequence is not proof a local or API model can reproduce it. Record handoffs, authority, tool results and actual completion checks; keep that claim distinct from automation potential.

## Sampling protocol

The session's coordinating host agent owns one **routine-first** sample: the first substantive investigation, test/verification, or fix expected at initiation to need at least two task tool actions. Logging calls do not count as task steps. Select it when the task enters that loop, before the outcome; keep the sample even if it aborts after one action. Include workflows that complete normally or need no intervention. Capture pre-action evidence before the next dependent step; do not halt legitimate work to construct a full retrospective plan. Additional material failures, recoveries or unexpectedly effective workflows are **enrichment** and must be analyzed separately. Historical seeds are excluded from both.

One case may appear in both ledgers; link it and treat it as one grouped incident, not independent training examples. Delegate results flow to the session collector; record the actual actor, not an invented principal. Carry the capture key over compaction. Link canonical incident records instead of reproducing them. If collection starts after the outcome is known, declare `retrospective`; never reconstruct missing pre-decision facts from the later fix. Do not introduce new model calls, extra test runs or permission requests solely to populate this study.

## Workflow episode fields

| Field | Record |
|---|---|
| Status / Valid | `pending-outcome`, `observed`, or `needs-adjudication`; dated validity |
| Sampling / capture mode | `routine-first`, `enrichment`, or `historical-seed`; `prospective` or `retrospective` |
| Identity / key / times | Stable session+task key, actor/principal and collector, observed provider/model or unknown; start, capture and finish UTC (unknown when absent) |
| Task / authority / substrate | Objective; allowed work as actually instructed; workspace, base revision and dirty/worktree state; target paths |
| Pre-action evidence | Source spans/revisions or sanitized observations available then; constraints and uncertainty; later outcome kept separate |
| Initial next action / completion check | Intended next action and the independently observable check needed for this task; do not invent a detailed plan after execution |
| Trajectory | Ordered actual tools/actions with safe args or references, raw result slices/exit states; failed attempts and guidance; linked DCX context timing; gaps explicitly marked |
| Effects / recovery | Diff or artifact revisions, test/probe command and observed result, retries, intervention, cancellation/concurrency, rollback attempted/result or not needed |
| Outcome / basis | `good`, `bad`, `mixed`, or `unknown`; `verified-complete`, `partial`, `blocked`, `abandoned`, or `unknown`; name what the check actually established |
| Delegation candidate | Candidate bounded workflow, deterministic alternative, missing inputs/checks; proposal only, never a measured autonomous success |
| Rests on / grouping / overhead | Canonical source/incident, durable evidence, session/task/lineage grouping, actual or estimated capture time explicitly distinguished |

Green tests establish only the behavior they exercise. A fix episode should retain its observed failing baseline and post-fix check when the task produced them; if either is missing, mark that fact rather than run unrelated tests for a score. Investigation completion may be a well-supported unresolved finding, not a code patch. The collector's interpretation is provisional unless a named check supports it; do not use confidence or a tracker status as the completion oracle.

Freeze pre-action facts and append dated outcome/correction paragraphs. Do not overwrite a failed attempt with the eventual success. A usage row with outcome `success` is a tool-record outcome, not task verification; native tools and child principals may be missing from that instrument. Keep minimal sanitized durable excerpts, hashes and exact locators when feasible. Full prompts/transcripts, credentials and raw databases do not belong in this repository. A transient output-buffer handle alone is insufficient for later adjudication.

## Session coverage receipts

DCS entries are **coverage records, not workflow episodes**. At session end/handoff, the coordinating collector writes one receipt, including a zero/unknown receipt when there was no selected episode. A resumed session updates the same receipt/capture key rather than counting another session.

Record: session/principal/collector identity (or unknown); UTC observed start/end; workspace; coverage `complete-observed-session` or `partial`; DCX routine/enrichment IDs and DWF routine/enrichment IDs; for each missing routine sample, `none-observed`, `missed-capture`, or `unknown`; native/delegated/unobserved gaps; collection overhead and how measured; unresolved pending entries. `complete-observed-session` means the collector covered its stated session interval, not all machine activity. Do not invent an eligible-opportunity total without a contemporaneous tally. No DCS receipt is unknown participation, not zero failures. usage.db session IDs are a separate recorded population and cannot establish complete coverage of host-native activity.

These receipts make sampling compliance inspectable. Day-7/day-14 analysis reports participation/missingness first and never treats DCS rows as good/bad workflow cases.

## Append and outcome update

This prose ledger owns DWF episodes and DCS coverage receipts. `params` holds configuration, not duplicated observation rows. Server-allocated IDs and one atomic section/index write avoid duplicate numbering.

```python
doc(action="append_entry", id="1b770cb6462acde6", id_prefix="DWF",
    anchor_heading="## Template for new entries", title="<objective and observed result>",
    body="**Status:** pending-outcome\n**Valid:** dated YYYY-MM-DD\n\n<episode fields>",
    index_row="| {id} | YYYY-MM-DD | workflow | routine-first | <capture key> |")
```

For a session receipt use the same call with `id_prefix="DCS"`, a title identifying the observed session interval, the coverage fields as body, and `index_row="| {id} | YYYY-MM-DD | coverage | session-receipt | <capture key> |"`. The declared `snapshot_anchor` places the row at the index tail. For updates, use heading-scoped `doc(action="update", patch={"body_edits":[...]})`; preserve pre-action fields and add a dated outcome or correction. Index rows are static sample identity, not outcome status. No hand-built params arrays and no separate index mutation.

## Historical examples, excluded from prospective counts

[U-40](codescout-usage-frictions.md) records a discriminating edit-miss investigation; [PR-review F-4 and W-3](pr-review-session-log.md) are two accounts of the same adversarial review and belong to one incident group. [W-1](pr-review-session-log.md) shows a scope comparison changing a review path. These seeds identify data worth capturing; their later explanations are not pre-action training input. Neither repeated citations nor multiple records of one incident create independent examples.

## Review at day 7 and day 14

Use the frozen baseline query and declared UTC bounds for new usage aggregates; report schema/build/workload changes and retention limits. Then enumerate actual DWF/DCX episodes and DCS receipts by record type, sampling mode, capture mode and missingness. Adjudicate routine and enriched cases separately against their recorded checks. Inspect ordinary completions, unnecessary intervention, failed checks, effective recovery, and incomplete outcomes. Group linked records before any evaluation split. Identify a concrete workflow with usable initial evidence and a completion check; if those are absent, improve capture or extend deliberately. Do not start training or implementation merely because two weeks elapsed.

## Index

| ID | Date UTC | Kind | Sampling | Capture key |
|---|---|---|---|---|
| DWF-1 | 2026-09-18 | workflow | historical-seed | seed-usage-U40 |
| DCS-1 | 2026-09-18 | coverage | setup / partial | setup-2026-09-18-root |
| DWF-2 | 2026-09-20 | workflow | enrichment | s48d1f0c8-round3-fixer-dispatch |
| DWF-3 | 2026-09-23 | workflow | enrichment | 571eb3d6/fork-route |
| DWF-4 | 2026-09-24 | workflow | routine-first (retrospective) | 3b4fae98/rtk-eval |
| DWF-5 | 2026-09-24 | workflow | enrichment | 09093108:prefix-uniqueness |
| DCS-2 | 2026-09-24 | coverage | session-receipt | 571eb3d6/post-compaction-2026-09-23 |
| DCS-3 | 2026-09-24 | coverage | session-receipt | 09093108/whole-session |
| DWF-6 | 2026-09-24 | workflow | routine-first | 774ba049/guide-rearm-debug |
| DCS-4 | 2026-09-24 | coverage | session-receipt | 774ba049/post-compaction |
| DCS-5 | 2026-09-24 | coverage | session-receipt | 571eb3d6/post-compaction-2026-09-24 |
| DWF-7 | 2026-09-24 | workflow | routine-first | sebf651ec-open-bug-verify-sweep |
| DCS-6 | 2026-09-24 | coverage | session-receipt | sebf651ec-open-bug-verify-sweep |
| DCS-7 | 2026-09-24 | coverage | session-receipt | 571eb3d6/post-compaction-3-2026-09-24 |

## DWF-1 — Historical seed — discriminate an edit-miss hypothesis

**Status:** needs-adjudication
**Valid:** dated 2026-08-17

**Sampling / capture mode:** historical-seed / retrospective; excluded from prospective collection counts.
**Identity / key / times:** original principal and exact timestamps unknown; seed-usage-U40; source dated 2026-08-17, captured here 2026-09-18.
**Task / authority / substrate:** investigate a scoped multiline edit miss; original grants, HEAD and worktree state not reconstructed.
**Pre-action evidence:** no frozen packet. The original request bytes and later diagnosis would need separate reconstruction; do not insert the diagnosis into a training prefix.
**Initial next action / completion check:** source reports a scratch multiline probe distinguishing unsupported multiline behavior from literal escape corruption; exact initial plan is not reconstructed.
**Trajectory:** the canonical U-40 record describes repeated reads, a scratch two-line edit with actual newlines, and a subsequent successful single-line anchor edit. Complete ordered raw calls are not preserved here.
**Effects / recovery:** successful scratch probe refuted the broad multiline-unsupported hypothesis; the original edit recovered through a single-line anchor. No claim of a newly fixed tool or autonomous worker run.
**Outcome / basis:** good, provisional historical-source label; reported recovery with a discriminating probe, not independently rerun in this collection. Task disposition: partial evidence of completion.
**Delegation candidate:** bounded reproduce-and-discriminate investigation; missing raw inputs, revision and independent terminal evidence must be addressed before evaluation.
**Rests on / grouping:** [codescout-usage-frictions:U-40](codescout-usage-frictions.md). All later summaries of U-40 remain one incident group.
**Overhead:** not measured.

## DCS-1 — Collection setup — partial-session coverage

**Valid:** dated 2026-09-18

**Status:** recorded

**Capture key:** setup-2026-09-18-root

**Coverage:** partial observed session; rules were introduced during this session. Coordinating principal `/root`; host session attribution not independently verified for this receipt. Terra research and Sol protocol review contributed to setup; this is not a prospective task sample.

**Routine context sample:** unknown — no contemporaneous selection under this protocol before setup.

**Routine workflow sample:** unknown — no contemporaneous selection under this protocol before setup.

**Historical seeds only:** DCX-1 and DWF-1; excluded from routine denominators.

**Observed work:** reproduced frozen usage aggregates and created collection ledgers and project rules. See [baseline](../research/2026-09-18-deep-agent-observation-baseline.md).

**Capture gaps:** pre-rule work and native-tool activity cannot be reconstructed as complete prospective coverage. Missing receipt or missing sample must not be interpreted as zero eligible work.

**Recording effort:** not measured; no numeric estimate.

**Handoff:** future sessions follow first-eligible sampling and preserve pre-action evidence; review 25 September, routine collection ends 2 October at 00:00 UTC.

## DWF-2 — Five-agent fixer dispatch built from each bug file's own Fix ruling — pre-action packet

**Status:** pending-outcome
**Valid:** dated 2026-09-20

**Sampling / capture mode:** enrichment / prospective. **NOT routine-first**: this session's first substantive episode ran before a context compaction and was never selected under this protocol, so the routine slot is declared `missed-capture` in this session's DCS receipt rather than backfilled here. Outcome genuinely unknown at capture — the five agents were still running when this was written.

**Identity / key / times:** collector and coordinating principal = session `48d1f0c8-9f60-43bb-a15e-17ec7995813a`, profile `~/.claude-kat`, observed model Opus 5 (1M context). Five delegated subagent actors under that session: three Sonnet, two Opus (provider observed; no per-actor session ids are minted, so they are not independently attributable — see `90d32f37ef2d8fc8`, which is that gap filed as a bug). Capture key `s48d1f0c8-round3-fixer-dispatch`. Dispatch and capture 2026-09-20; finish unknown at capture.

**Task / authority / substrate:** objective — fix five filed bugs drawn from the open ledger, one agent each. Authority as actually instructed: the user's standing *"continue autonomously; when in doubt, check trackers and/or measure first"*, under CLAUDE.md's constraints (no push absent an explicit ask; never `git add -A`; `experiments` only). Workspace `/home/marius/work/claude/codescout`, base revision `170eac15`, tree carrying one modified file owned by another session (`.codescout/audit/ripper-65e654-202609.jsonl`, untouched). Target paths declared disjoint per agent: `scripts/probe_augmentation_restore.py`; `scripts/install-hooks.sh` + `tests/hooks-discrimination.sh`; `src/librarian/tools/audit_doc_refs/**`; `src/prompts/README.md` + `src/prompts/mod.rs`; `src/librarian/tools/doctor.rs`.

**Pre-action evidence:** (a) `doc(find, kind="bug", status in open/taken/investigating/zombie)` returned **62 rows** at dispatch. (b) Ten candidates' own `## Fix` sections read before any brief was written, and **five ruled a fixer OUT** and were not dispatched: `3cee1969ae4e9d57` (*"the right answer is a decision, not a patch"*), `f47274c162774e8e` (*"not choosing between them here"*), `863fb5cf6bf011ef` (irreversible host-local `DELETE` wanting an operator decision), `c04e0d83106045d7` (the fix lives in a different repo), and `aa1110786cd2a8a4` / `1a0a6887f5998777` (both *"not designed"*). (c) `./scripts/gate.sh` observed green at `170eac15` **before** dispatch — `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0` — so no agent would be handed a peer's red as its own. (d) File-ownership overlap **measured, not assumed**: `grep mode="files"` returned `src/librarian/tools/doctor.rs` for both `fix_anchor` (63 matches) and `docs/augmentations` (8), and `src/prompts/mod.rs` in three separate searches; `ls src/librarian/tools/` then established `audit_doc_refs` is a separate directory module, which is what made two of the five safe to run concurrently. Constraint and uncertainty held at capture: whether briefs built from each file's own `## Fix` section reduce the prior round's defect rate (4 of 11 briefs there prescribed remedies the bug files explicitly ruled out) is the open question; n is far too small to answer it and this episode is not designed to.

**Initial next action / completion check:** next action — receive five hand-backs and integrate. Independently observable check, fixed **before** the outcome: (1) `./scripts/gate.sh` green at the integrated tree, read as its four printed exit codes rather than as an absence of output — the `;`-chained form ends in `echo`, so its shell status is 0 whatever happened; (2) each agent's claimed new test present **by name** in the DEFAULT lane's output, since `--no-default-features` compiles no librarian code and a lean green there is silence, not a pass; (3) `git show --stat <sha>` per commit showing only that agent's declared paths — a clean `git status --short` is explicitly **not** accepted as evidence of exclusion, being equally what inclusion produces.

**Trajectory:** five concurrent `Agent` dispatches issued in one turn after the scouting above. Ordered per-agent trajectories not yet observed; to be appended on hand-back. Gaps marked rather than inferred. No linked DCX entry — no separate context-timing decision was selected for this episode.

**Effects / recovery:** none observed at capture. Five bug files moved `open` → `taken` with `claimed_by` set to this session id, **verified at the bytes** by grepping the written frontmatter rather than trusting five `updated: true` responses — an earlier call in this same session returned `updated: true` for eleven files while writing a wrong deletion sentinel into all of them.

**Outcome / basis:** unknown at capture, deliberately left unset. The protocol's instruction not to overwrite pre-action facts with the eventual result is precisely why this paragraph is written now rather than after the hand-backs.

**Delegation candidate:** the bounded repeatable step here is **not** the fix — it is the triage gate in front of it: *read a filed bug's `## Fix` section and classify whether it prescribes a patch, a decision, or an out-of-repo change.* Five of ten candidates classified "not a patch" on their own text, and each classification is checkable against the file that produced it. Missing input for automating it: **no machine-readable marker distinguishes a prescriptive `## Fix` from a deliberative one** — today the difference is carried entirely in prose. Proposal only; no autonomous run was measured, and this episode establishes nothing about whether a lightweight worker could reproduce the classification.

**Rests on / grouping:** one grouped incident spanning this session and its five dispatched agents. The five bug files are the canonical records and are not reproduced here: `41c3978d6372edf9`, `28b4927cf6253635`, `6ed22167cef9d224`, `43d63e9f2e6c7cc0`, `de46d402441e1e2b`.

**Overhead:** capture ~8 minutes, actual. Distinguished from the scouting above, which the task required regardless and is not collection overhead.

**Correction — 2026-09-20T14:00Z (citation only, no observed fact changed):** the bug cited in *Identity / key / times* was fixed and archived the same day, and `id = sha256(abs_path)` re-keyed it on the move, so the id recorded at capture no longer resolves. The citation now names the post-archive id. The dead id is deliberately not restated here — an artifact id cannot be mentioned without being cited, and `git log` holds the prior value. Nothing about the pre-action packet's observed content is altered by this.

## DWF-3 — Phase-2 replay moved to a subscription fork route — five probes, each exposing a contamination mechanism

**Status:** pending-outcome
**Valid:** dated 2026-09-23

| Field | Record |
|---|---|
| Sampling / capture mode | `enrichment`, `retrospective`. Captured after the probes ran; not shown to be this session's first eligible episode |
| Identity / key / times | session `571eb3d6-c879-43f6-b3f9-5a51e744e1af`, principal = the operator via this coordinating session, collector = same; model claude-opus-5.5; 2026-09-23, times approximate |
| Task / authority | Operator: "it should run on subscription not API", then "lets do them all" (stripped-CLAUDE.md test, end-to-end, RTD-3). Authority: build and run phase-2 replays under the subscription; public repo, data sharing approved earlier |
| Substrate | `experiments` at `95e48217`, then `a044ac8d`; shared checkout with peer uncommitted edits (CLAUDE.md and others, not touched) |
| Pre-action evidence | Replays had used the paid Messages API (`phase2-replay.py:156`) and stopped at its usage cap. `claude -p --resume --fork-session` existed. Not known before probing: what a resume adds to context, and how to keep tools inert |
| Initial next action / check | One arm-0 fork at transcript cut 1780. Check: the first action is the same `mcp__codescout__doc` write as original record 1788, with no contamination in the fork's own transcript |
| Trajectory | Five probes, each exposing one mechanism before the next: (1) plugin SessionStart:fork bootstrap made the agent call `workspace` first; (2) resume re-delivered killed background agents from `toolUseResult` metadata; (3) a scratch cwd produced "environment has changed", and the agent stopped; (4) toy probes: `--max-turns 1` still executes a tool; `permissions.deny` hides the tool; the model reads an attachment's `rendered`, not `files[].content`; (5) final design: mount namespace with bind-mounted settings (plugins off, deny-all hook) and transcript-derived CLAUDE.md files |
| Effects / recovery | `scripts/phase2-fork.py` and a pre-registration amendment in `a044ac8d`, before any registered arm ran. One stale fork file (own, id-named) removed from the shared projects dir. The first cleanup design deleted every new .jsonl in that dir, which would have taken peers' transcripts; it was caught and narrowed to id-named files before any run |
| Outcome / basis | Probe 5: first action `mcp__codescout__doc` (update), text contains the RTD-9 claim "fired **once, ever**"; the tool_result reads "hook error: replay: tool execution disabled". Establishes that the route reproduces the action shape on 1 sample. It does NOT establish reproduction of the violation rate; that is the registered arm-0 criterion (RTD-8 ≥ 0.3), pending |
| Delegation candidate | Fork replay as a bounded workflow: cut index + arm text → first-turn row. Deterministic parts: seeding, shadow mounts, cleanup. Missing check: automated contamination diff of fork-added records against a whitelist |
| Rests on / overhead | `docs/evals/rule-injection-timing-preregistration.md` § Amendments (fork route); capture ~10 min, estimated |

**Outcome — 2026-09-23 (pre-action fields above unchanged):** `observed`, `mixed`, `partial`. The registered route-validity check PASSED: fork arm 0 reached RTD-8 **5/10** (criterion ≥ 0.3), with 10/10 forks going straight to the doc write. It was not the fifth probe that ended the trajectory, though: a **sixth** contamination surfaced after this entry's capture. The first registered launch opened 3/3 forks by auditing the git tree. The cause was the resume's *"The date has changed"* notice (transcript day 2026-09-21, fork day 2026-09-23). It was fixed by setting the seed's last `date` attachment to the fork's day (`d8f465e1`), and those 3 rows were discarded, not scored. Killing that launch also left orphaned forks writing into the shared projects directory. Six own files were identified by content and removed, and a peer's file was left untouched. The driver now cleans up on SIGTERM, verified with 0 forks and 0 owned files left. Results are recorded in `docs/evals/rule-tell-scoring-2026-09-23.md` § *Phase 2 — fork route on the subscription*. **What the check established:** the route reproduces the decision point's action and a violation rate above the floor, on one decision point. **What it did not:** that fork-route rates equal API-route rates. They are compared only within the route.

## DWF-4 — rtk evaluation for codescout run_command — corpus replay plus live guard probes; verdict do-not-adopt

**Status:** observed
**Valid:** dated 2026-09-24

| Field | Record |
|---|---|
| Sampling / capture mode | `routine-first`; **retrospective**. Written after the measurements, so the pre-action row below is reconstructed from the opening request, not frozen before it |
| Identity / key / times | session 3b4fae98-500a-4fa9-8127-b16642a8c23d, profile ~/.claude-sdd, model claude-opus-5-5; collector = same session; ~06:50–07:25 UTC 2026-09-24 |
| Task / authority / substrate | User: "research and measure if rtk-ai/rtk would help codescout and how". Research only, no code changes. Workspace `experiments` @36999188, dirty tree (peer edits); rtk v0.49.0 binary + fixture crate in session scratchpad only |
| Pre-action evidence | rtk README: it hooks only the Bash tool. CLAUDE.md: native Bash denied since 2026-09-20. codescout run_command already buffers anything over ~10 KB behind a summary |
| Initial next action / completion check | Replay the recorded run_command corpus (usage.db) through `rtk rewrite` and `rtk pipe -f`; check: bytes before/after on real outputs plus live fidelity probes |
| Trajectory | README fetch → download + sha256 verify → corpus family table → offline replay (7m41s; 23,641 calls) → 14 live raw-vs-rtk pairs → fixture crate for cargo test/clippy → two pipe-mode outliers re-checked live (they turned out to be pipe-mode artifacts, so those figures are upper bounds) → guard probes |
| Effects / recovery | No repo writes except codescout memory `research/rtk-evaluation` and this entry. One IL-3 block on my own grep pipe; reran with a redirect |
| Outcome / basis | `good` / `verified-complete` for the question asked. Inline savings upper bound 0.63% of codescout tool output; `empty_test_selection_diagnostic` silenced and an IL-3 bypass that masks exit codes, both observed live. Recommendation: do not adopt |
| Delegation candidate | Corpus replay against usage.db is deterministic and scriptable. Judging fidelity needed live probes the replay could not substitute for |
| Rests on / grouping / overhead | memory `research/rtk-evaluation`; capture effort ~3 min |

## DWF-5 — Ledger prefix uniqueness enforced at declaration — pre-action packet

**Status:** observed
**Valid:** dated 2026-09-24

| Field | Record |
|---|---|
| Sampling / capture mode | `enrichment`, `prospective`. This session's routine-first sample (the `external_prefix` fix, commit `7513f2de`) was NOT snapshotted before action; it is `missed-capture` in this session's DCS receipt, not back-filled here. |
| Identity / key / times | session `09093108-1425-4f6d-9695-a9e3bb98ea0d`, collector = same session (Opus 5.5, main loop); capture key `09093108:prefix-uniqueness`; captured 2026-09-24 before implementation; finish unknown |
| Task / authority / substrate | Operator instruction, verbatim: "uniqueness is what I'm after so librarian should be able to validate if an index exists and deny a new ledger with conflicting names. it should also be part of the guide". Base `5103f818` on `experiments`, shared checkout, peer-dirty files present (this ledger among them — this entry is appended, not committed with their hunk). |
| Pre-action evidence | Measured 2026-09-24 over tracked markdown: 49 ledgers declare `entry_prefix`, 37 distinct prefixes; only `F`/`W` are multiply declared (x15 each, the session-log template). Four write paths can declare a prefix: `create.rs:384` and `update.rs:552` (both already validate `extra` at the boundary), `catalog/rekey.rs` (`rekey_prefix_rows` refuses only a prefix the SAME ledger reserves), and hand-edited frontmatter (no write-time hook exists). `link_scan`'s `prefix_conflicts` already DETECTS a declared prefix with >1 active definer after the fact; nothing PREVENTS it. |
| Initial next action / completion check | Next: a shared helper in `catalog/augmentation.rs` that reads every declaration under the declaring file's git root (both `entry_prefix` and `external_prefix` count as owners; `F`/`W` exempt as the one shared family), refusing at create / update / rekey; then a `doctor` check for the hand-edit path; then the guide. Completion check: each refusal observed RED before the fix, one mutation per refusal site killed, gate green, and the live corpus still passes (no false refusal on today's 49 ledgers). |
| Uncertainty at capture | Whether `F`/`W` should stay exempt was stated to the operator as an assumption, not ruled. Repo scope (not catalog-wide) is a design choice, not measured against cross-repo citation practice. |

**Outcome 2026-09-24:** `good` / `verified-complete` for the stated completion check. Committed `45d49a10`. What the check established, separately: (1) each refusal observed RED before its fix -- 12 tests; (2) 17 guarded sites mutated via `scripts/mutation-probe.sh`, all KILLED at the end, with FOUR surviving on the first pass (alternatives-free, rival-excludes-self, repo grouping, per-artifact dedupe), each read as `untested` and closed by a fixture detail; (3) gate `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, the 12 test names read out of the default lane; (4) live corpus: the new doctor check reports 0, agreeing with an independent `git ls-files` scan (only `F`/`W` shared), so no existing ledger is refused. NOT established: the write-time guard is not live in any running MCP server until a rebuild; the unindexed-sibling and linked-worktree cases are false accepts by design, covered only by doctor after reindex/merge.

**Trajectory notes worth keeping:** one mid-task compile break reached the shared tree (a replaced stub left a duplicate definition for ~2 minutes; the tool's own compile check reported it and it was removed before any peer build was observed). A gate run exited 1 on `fmt-mine` refusing a LIVE peer's uncommitted files -- the guard working, not a defect of this change. A fixture edit silently failed to apply because `fmt-mine` had reformatted the bytes between read and write, leaving two new assertions checking rows that did not exist -- caught only because the corresponding mutations still SURVIVED. Mid-task a peer (sessionId `3b4fae98-500a-4fa9-8127-b16642a8c23d`) independently hit the same side-bug this session filed and handed over its IC-6 member entry.

## DCS-2 — Session 571eb3d6 — rule-tell eval campaign, post-compaction interval

**Valid:** dated 2026-09-24

| Field | Record |
|---|---|
| Session / principal / collector | session `571eb3d6-c879-43f6-b3f9-5a51e744e1af`; the operator via this coordinating session; collector = same; model claude-opus-5.5 |
| Observed interval (UTC) | this receipt covers the post-compaction stretch 2026-09-23 ~14:30 to 2026-09-24 ~08:30, approximate. Earlier parts of the session, before compaction, are not covered here |
| Workspace | `/home/marius/work/claude/codescout`, branch `experiments`, shared checkout with 5+ peer sessions |
| Coverage | `partial`. The collector covered this interval only, and captured retrospectively |
| DCTX routine / enrichment | none recorded. Routine sample: `missed-capture`. Several context choices were made (the injection channel, stripping CLAUDE.md, a date-attachment rewrite), but none was snapshotted before action |
| DWF routine / enrichment | DWF-3 (enrichment, retrospective). Routine sample: `missed-capture`. The first substantive multi-step episode of this interval (moving the judge to the subscription) was not snapshotted before action |
| Native / delegated / unobserved gaps | no subagents. Subscription `claude -p` forks and judge calls do not appear in `usage.db`: about 150 Opus forks and about 1,000 Haiku judge calls ran outside the MCP recorder |
| Unresolved pending entries | none of this session's. DWF-3 carries a dated outcome |
| Collection overhead | about 15 min across DWF-3, its outcome, and this receipt; estimated, not measured |

## DCS-3 — Session 09093108 — prefix uniqueness, #59 and the DCX rekey, the withheld-commit rule, caveat markers

**Valid:** dated 2026-09-24

| Field | Record |
|---|---|
| Session / principal / collector | session `09093108-1425-4f6d-9695-a9e3bb98ea0d`; the operator via this coordinating session; collector = same (Opus 5.5, main loop) |
| Observed interval (UTC) | 2026-09-24, the whole session across one compaction; start and end times were not recorded, so no interval is claimed |
| Workspace | `/home/marius/work/claude/codescout`, branch `experiments`, shared checkout; peers observed committing or staging during the interval: `3b4fae98` (codescout-64), `774ba049` (codescout-88), `571eb3d6` (codescout-0e) |
| Coverage | `partial` — pre-compaction work is covered only through DWF-5 and its own account; the rest was captured retrospectively here |
| DCX routine / enrichment | none recorded. Routine sample: `missed-capture` — context choices were made without a pre-action snapshot (which records to load into a delegated classifier, which guide sections to read before each edit) |
| DWF routine / enrichment | DWF-5 (enrichment, prospective, outcome recorded). Routine sample: `missed-capture` — the `external_prefix` fix (`7513f2de`), as DWF-5 records. Post-compaction episodes were not snapshotted: #59's allocator refusal plus the `DCTX` → `DCX` rekey (`c8d4e0d6`), the withheld-commit rule (`a3dcff72`), the `STANDING` / `TRACKED` caveat markers |
| Native / delegated / unobserved gaps | one delegated subagent (Opus; read-only caveat classification; about 10 min, 74 tool uses) — its MCP calls land in `usage.db`, and it activated the project read-only process-wide as a side effect, observed as a refused write here. Gate, `mutation-probe` and `cargo` child processes are outside `usage.db` |
| Noteworthy, not separately sampled | `rekey_prefix` moved the body but left the frontmatter declaration behind (filed and fixed in `c8d4e0d6`); a temporary-index commit raced a peer's commit between `read-tree` and `commit`, and the foreign-index guard refused it before it could silently revert their file; a guide edit moved a line-keyed test exemption and redded the gate |
| Unresolved pending entries | none of this session's; DWF-5 carries its outcome |
| Collection overhead | about 10 min for DWF-5's outcome and this receipt; estimated, not measured |

## DWF-6 — Guide re-delivery debug → three ledger mechanisms fixed/filed, one fix verified live across a real restart

**Status:** observed
**Valid:** dated 2026-09-24

- **Sampling / capture mode:** routine-first, **retrospective** — the session's first substantive workflow began before a context compaction, and its pre-action facts were not captured beforehand. Per this ledger's protocol they are NOT reconstructed from the later fix; they are marked unknown.
- **Identity / key / times:** session `774ba049-d97c-443a-b31d-f662a9cb6a1e` (host agent, collector = same); capture key `774ba049/guide-rearm-debug`; provider/model: Claude Code, Opus-class host (subagent probe on Sonnet). Start: unknown (pre-compaction); capture 2026-09-24 ~08:45Z; finish pending the gate.
- **Task / authority / substrate:** user: debug why `project-activation-bootstrap` "arms continuously" in session `571eb3d6`, then "file the bugs, write regression tests and start fixing", later "ok, go" on fixing the re-arm scoping. Workspace `/home/marius/work/claude/codescout`, branch `experiments`, shared checkout (peers committing concurrently).
- **Pre-action evidence:** unknown (lost to compaction) — not reconstructed.
- **Initial next action / completion check:** unknown for the first cycle. Second cycle (post-compaction, prospective in substance): verify fix `971ed73f` live through the real hook chain and a real `/mcp` restart; check = the probe principal's on-disk ledger rewritten by the NEW process still carrying the OLD process's timestamp.
- **Trajectory (summary; canonical records linked):** forensic transcript read → refuted shared-server hypothesis → three candidate mechanisms → one filed and fixed (`971ed73f`, regression test RED→GREEN) → rebuild + live probe (dispatch, resume, `/mcp`, resume) → live check passed → verification surfaced two more mechanisms, both measured directly (`6d671794cd4da970` companion restore strips parent marks; `5201164f55ec16ff` re-arm consumed by any principal) plus a zombie recurrence (`523233935cc53bc4`) and stale served guides (`d01eaef12cce9b8a`) → re-arm scoping fixed test-first (new test RED at the intended assertion, GREEN 66/66; three isolated mutations each KILLED by exactly the intended test).
- **Effects / recovery:** commits `971ed73f`, `09f7b2c2`, and the re-arm fix pending the gate. One self-caught error: a pre-compaction retraction of the re-arm mechanism as an accepted design tradeoff was reversed by direct measurement; recorded in the bug file.
- **Outcome / basis:** good / partial — fixes verified by observed RED→GREEN and a live check; the companion-side restore fix (`6d671794cd4da970`) is awaiting an operator decision.
- **Delegation candidate:** "live-verify a guide-ledger fix across a restart" is a bounded workflow: dispatch probe → read per-principal ledger file → operator `/mcp` → resume probe → compare stamps. Missing input for a worker: the `/mcp` step needs the operator. Proposal only.
- **Rests on / grouping / overhead:** bug files named above; one incident group with `571eb3d6`. Capture overhead ~5 min, estimated.

## DCS-4 — Session 774ba049 — guide-ledger debug, fixes and live verification, post-compaction interval

**Valid:** dated 2026-09-24

| Field | Record |
|---|---|
| Session / principal / collector | session `774ba049-d97c-443a-b31d-f662a9cb6a1e`; coordinating host agent, collector = same; post-compaction model claude-opus-5-5, pre-compaction model not recorded here |
| Observed interval (UTC) | post-compaction only: ~07:35 (serving PID 1405510 start) to ~09:15, 2026-09-24, approximate. The pre-compaction part of the session is not covered |
| Workspace | `/home/marius/work/claude/codescout`, branch `experiments`, shared checkout with concurrent peers (two peers' uncommitted entries sat in this ledger during the interval) |
| Coverage | `partial` — interval above only; captured retrospectively |
| DCX routine / enrichment | none recorded. Routine sample: `missed-capture` — context choices were made (what to brief a probe subagent with; which topic to test, chosen to avoid a hook confound) without a pre-action snapshot |
| DWF routine / enrichment | DWF-6 (routine-first, retrospective). No enrichment entry |
| Native / delegated / unobserved gaps | one delegated probe subagent (principal `…/a3ba615808d91a57d`, Sonnet) — its codescout calls are in `usage.db` under its `agent_id`; native `Glob`/`Read` calls used deliberately (they do not poll codescout's re-arm inbox) are absent from `usage.db`; `scripts/mutation-probe.sh` runs happened in isolated worktrees |
| Unresolved pending entries | none in this ledger. Open outside it: `5201164f55ec16ff` awaits a live check after rebuild; `6d671794cd4da970` and `f6a748bcbeee1652` await operator decisions |
| Collection overhead | ~10 min across DWF-6 and this receipt, including separating two peers' unstaged hunks from this session's; estimated, not measured |

**Update 2026-09-24 (same session, after rebuild):** `5201164f55ec16ff`'s live check passed on PID 2968670 — a parent call left a subagent's pending request and the parent's ledger untouched, and the named subagent's own call consumed it. Archived. Still open outside this ledger: `6d671794cd4da970`, `f6a748bcbeee1652`.


**Update 2026-09-24 (later, same session 774ba049):**

- `f6a748bcbeee1652` was verified live, both halves.
- Its live check exposed two more mechanisms:
  - `92deba12cd82aaf0`: a nested `claude -p` hijacks its ancestor session's server. Shown to have happened in `571eb3d6`, where 51 parent calls were logged under a nested id with identical per-tool counts.
  - `798f69a248d72298`: interactive SessionStart runs before the server's slot exists, so 0 of 3 interactive starts were stamped.
- Both were fixed test-first (`claude-plugins:1cc83fbf`, `3a069d5d`), with mutations killed per site and per signal, and verified live on their original reproductions.
- All four ledger bugs from this investigation are archived.

**Capture gap:** no new DWF entry was opened for this second half. This receipt covers it.

## DCS-5 — Session 571eb3d6 — phase-1 selector gates, Score A/B, the judge-channel contamination, second post-compaction interval

**Valid:** dated 2026-09-24

| Field | Record |
|---|---|
| Session / principal / collector | session `571eb3d6-c879-43f6-b3f9-5a51e744e1af`; the operator via this coordinating session; collector = same; model claude-opus-5.5. The session moved from profile `~/.claude-kat` to `~/.claude-sdd` on resume |
| Observed interval (UTC) | second post-compaction stretch, 2026-09-24 ~06:45 to ~10:30, approximate. DCS-2 covers the stretch before it |
| Workspace | `/home/marius/work/claude/codescout`, branch `experiments`, shared checkout; peers 09093108 and 3aa55c01 interacted |
| Coverage | `partial`, retrospective |
| DCTX routine / enrichment | none recorded. Routine sample: `missed-capture`. Context choices made without a pre-action snapshot: the judge's config dir (clean channel), the move of the judge account to `~/.claude`, and the selection of injections |
| DWF routine / enrichment | none recorded. Routine sample: `missed-capture`. The first substantive multi-step episode (the per-rule selector's registration and gate) was not snapshotted before action. Outcomes are recorded in the eval docs, commits `bbba4aa2`…`575aafdf` |
| Native / delegated / unobserved gaps | two background research subagents (general-purpose, web and docs only). Every `claude -p` judge call (about 3,000 Haiku and Sonnet calls) and the 20 Opus forks ran outside `usage.db` |
| Notable, for the review | the `claude -p` judge channel loaded 5 plugins, SessionStart hooks and the user CLAUDE.md, at 2,778 tokens for "Say OK." (clean: 249). On the clean channel the `rtd8` checker failed its gate, and phase 2's RTD-8 claim was withdrawn; the other three checkers reproduced exactly |
| Unresolved pending entries | none. `rtd8c` (the re-worded RTD-8 checker) was scoring at handoff; its outcome goes to the scoring doc, not here |
| Collection overhead | about 10 min for this receipt; estimated |

## DWF-7 — Open-bug verify sweep: four read-only verifier batches over the 34 bugs older than 14 days — pre-action packet

**Status:** pending-outcome
**Valid:** dated 2026-09-24

**Sampling / capture mode:** routine-first / prospective for the dispatch below. The session's first substantive multi-step work, closing `2fc50a3d` (now `863b801018a947e4`), completed before this capture. It is summarised here as retrospective context and is not claimed as a prospective sample.

**Identity / key / times:** collector and coordinating principal = session `ebf651ec-5ab7-42d9-a526-dcf9758692e1`, profile `~/.claude`, model Opus 5.5 (1M). Four delegated general-purpose subagents (Opus), not independently attributable beyond agent id. Capture key `sebf651ec-open-bug-verify-sweep`. Capture 2026-09-24T11:40Z; finish unknown at capture.

**Task / authority / substrate:** the user asked "lets check open issues" and then chose options 1 and 2. Option 1: close `2fc50a3d` as fixed. Option 2: run `librarian(doctor)` and verify the open bugs older than 14 days against current code. Delegates are authorised read-only (no edits, no git index/HEAD changes, no lean-lane builds). Workspace `/home/marius/work/claude/codescout`, HEAD `436a8ff6`, tree dirty with peers' files plus this session's uncommitted archive of `2fc50a3d` and re-pointed citations.

**Pre-action evidence:** (a) At about 11:30Z, `doc(find, kind=bug, status in open/taken/investigating/zombie)` returned 110 rows, and the disk had 111; the one-row gap was `b586243d`, committed after the query. 44 of those 110 are `RESIDUAL:` rows routed today by `cb54d062`. (b) `doctor` at `436a8ff6` returned 81 violations, including 5 `open_bug_cited_from_source` (`523233935`, `7579b32b`, `e421be68`, `f47274c1`, `bfdfeebd`). (c) The population is the 33 non-RESIDUAL active bugs filed on or before 2026-09-10, plus `bfdfeebd`: 34 files. (d) Part 1 retrospective: `2fc50a3d` looked like an unclosed duplicate. Reading it showed two failure modes, closed by `2caf55c5` + `02a86104` (mode 1) and `074b749e` (mode 2). Patch-ids were re-derived, and the two already recorded elsewhere matched.

**Initial next action / completion check:** dispatch four read-only verifier batches, each returning a per-bug verdict (FIXED with SHA+patch-id / PARTIAL / STILL-OPEN / EXTERNAL / CANNOT-DETERMINE) backed by lines read. The check: this collector spot-verifies every FIXED verdict before changing any status, and the user decides on closures.

**Outcome update — 2026-09-24T12:30Z.** Four verifiers returned 34 verdicts: 2 FIXED, 1 probably fixed, 9 partial or mitigated, 6 external/not-a-code-defect, 3 not recurred, 12 still open, 1 cannot-determine, and 1 zombie that came back. The per-batch totals reconcile to 34; the 12 still-open includes the design-limit and structural cases. The collector re-derived the spot checks before any status change: `05fceb57`'s two anchors (reachable, non-merge, patch-ids matched), and the zombie `d25aa6db`'s two recurrences (`usage.db` rows 79110 and 126947; `git grep` at each row's `project_sha` confirmed both symbols existed). Status changes were not applied; they were handed to the user. **Unplanned finding (enrichment):** the dispatch itself exposed a concurrent guide-ledger race, filed as `c161cc27ddff5672`. Batch B reported three injections of one topic; the collector counted each ledger's stamps against each transcript's injections and traced it to `src/server.rs:679`/`:1204`. A side effect: one verifier activated the project read-only, which blocked the coordinator's reindex until it was re-run with `workspace=` pinned. **Outcome:** good / partial. The investigation is complete as verdicts; closures await the user.

## DCS-6 — Session ebf651ec — open-issue review, 34-bug verify sweep, and the guide-ledger race it exposed

**Status:** observed
**Valid:** dated 2026-09-24

**Session / interval:** `ebf651ec-5ab7-42d9-a526-dcf9758692e1`, profile `~/.claude`, from session start (about 11:20Z) to the commit of this sweep (about 13:45Z). Model: Sonnet 5 at first, then Opus 5.5 from the user's first task onward. No compaction in the interval.

**Selected IDs:** `DWF-7` (routine-first; prospective for the dispatch, with the earlier 2fc50a3d closure summarised retrospectively inside it) and `DCX-2` (routine-first, prospective).

**Coverage:**
- **Missed captures, declared rather than backfilled:** (1) the first context decision of the session, whether to trust a peer's slot-file claim and what to put in the fork brief for the a5054d13 live-check, was not captured before acting; (2) the first substantive multi-step episode, closing `2fc50a3d`, ran before selection, so it appears in DWF-7 only as retrospective context.
- **Enrichment:** the concurrent guide-ledger race filed as `c161cc27ddff5672` was found during the DWF-7 dispatch. It is recorded in DWF-7's outcome update and in DCX-2's delivery outcome, not as a separate DCX entry.
- **Delegates:** 1 fork (the a5054d13 diagnostic) and 4 verifier subagents (DWF-7). Their principals are observable only as CC agent ids.

**Capture gaps:** the verifier transcripts are process-local task files and are not retained. The durable evidence is the per-bug notes written into each bug file, the ledger-vs-transcript injection table in `c161cc27ddff5672`, and the `usage.db` row ids cited there. The fork's report was relayed to the peer, and its one error (which ledger `post_compact` cleared) was corrected on the peer's prompt.

**Recording effort:** three ledger appends and three updates, about 6 tool calls of roughly 250 in the interval. No task was displaced.

## DCS-7 — Session 571eb3d6 — rtd8c, the form-3 ablation, JevK5 Stage 1 and the API-route clean re-score, third post-compaction interval

**Valid:** dated 2026-09-24

| Field | Record |
|---|---|
| Session / principal / collector | Session `571eb3d6-c879-43f6-b3f9-5a51e744e1af`. Principal: the operator, via this coordinating session. Collector: the same session. Model: claude-opus-5.5 |
| Observed interval (UTC) | Third post-compaction stretch, 2026-09-24, about 11:00 to 15:00, approximate. DCS-2 and DCS-5 cover the earlier stretches |
| Workspace | `/home/marius/work/claude/codescout`, branch `experiments`, shared checkout. Peers ebf651ec and 3b4fae98 had uncommitted entries in this ledger |
| Coverage | `partial`, retrospective |
| DCTX routine / enrichment | None recorded. This session's routine sample was already recorded as `missed-capture` in DCS-2. Context choices made this interval without a pre-action snapshot: running the generic-clause ablation blind rather than first reading the judge's reasons on the corpus (to avoid tailoring), and running JevK5's determinism check on a neutral text outside every gate |
| DWF routine / enrichment | None recorded. Outcomes are in the eval docs, commits `d8e6f4d5` to `f3321e40`: the rtd8c result, S0 form 3, JevK5 Stage 1, the API-route clean re-score, and the data bundle |
| Native / delegated / unobserved gaps | No subagents. About 2,300 `claude -p` judge calls (Sonnet and Haiku) ran outside `usage.db`, as did the local JevK5 GPU runs and the background shell jobs that launched them |
| Notable, for the review | Two of this session's own claims were retracted before anything was built on them: a Stage-1 probability range, and the reading that recall is lost to rule assignment (it is mostly silence). A third, a gate-precision cost attributed to form 3, was withdrawn once form 2b's log showed it predated the change. Each was caught by recounting from the rows. At the operator's direction, peer entries DWF-4 and DWF-7 were committed with DCS-2 and DCS-5 in `502265fc`, with their authors named in the commit body. The live peer ebf651ec was told |
| Unresolved pending entries | None. Local-route Stage 2 has not started |
| Collection overhead | About 10 minutes for this receipt, estimated |

## Template for new entries

Choose either the workflow field table or the session coverage fields. This is the append anchor, not a recorded episode.
