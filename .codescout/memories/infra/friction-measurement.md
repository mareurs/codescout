# Friction Measurement — instruments, measured rates, and what not to re-derive

Established 2026-08-19/20 by a five-agent reconnaissance into "how do we measure agent
friction". Read this **before** measuring anything about tool-call friction — three of the
instruments involved were found to be wrong, and two of them are still wrong.

## READ THIS FIRST — frequency is not friction, and the project has said so three times

**Before ranking `err_family` by count and reporting the top family as a friction problem,
read `2026-08-15-tool-usage-investigation:TU-7`.** It exists to stop exactly that:

> *"Ranking error families by count puts `il3_pipe_to_trimmer` (262) near the top. It is
> **healthy**... So frequency is not the signal — recovery is."* — TU-7, 2026-08-15, whose
> augmentation prompt adds that it was *"recorded specifically so it is not re-litigated by
> someone ranking error families by count."*

Sequence, because it is the cautionary tale: TU-7 records it (2026-08-15) → the Iron-Law
Gate Firing Audit replicates it, 96%/3%, and repeats the warning (2026-08-16) →
`tool-usage-patterns:T-25` ranks by count anyway and calls IL-3 "the single largest friction
family in the corpus" (2026-08-19) → a five-agent pass re-derives TU-7 from scratch at ~150k
tokens (2026-08-20). **A well-written negative result with an explicit do-not-re-litigate
note did not survive four days.** The decay was a stale *warning*, not a stale fact, which no
freshness check flags. Live datapoint for `capability-proposals:CAP-7`.

## Instrument health — check before trusting a number

| Instrument | State | Consequence |
|---|---|---|
| `usage.db` `cc_session_id` | **BROKEN** (filed, unfixed) | Resolved once at `src/server.rs:115-125`, cloned per call at `:974-977`. 30.9% of rows (8,980/29,103) sit in pools where one server `session_id` carries 2+ `cc_session_id` labels. **Never group by `cc_session_id` alone** — group by `session_id`, disambiguate by `called_at` window |
| `usage.db` subagent attribution | **MISSING** (filed, unfixed) | 72.7% of calls and 68.3% of errors originate in dispatched subagents, filed under the parent. No `agent_id`/`is_sidechain` exists (`grep` → 0 matches in `src/`). Every per-session figure is a parent/subagent blend |
| `usage.db` `friction_target` | **PARTIAL** (filed, unfixed) | Key list omits `command`/`file_path`/`id`/`topic`, so `run_command` errors are 431/431 NULL and both largest families are 0% attributable — 38% of errors. Also gated on `is_friction` (`src/usage/mod.rs:81`), so 0 of 25,696 non-overflowed successes have one |
| `cc.py` cost/token output | **BROKEN** (filed, cross-repo) | Sums `message.usage` per JSONL line; one completion is 2-3 lines each carrying an identical usage dict. **2.1-2.6x inflation** (222 entries → 87 distinct `message.id`). Its call/turn/tool counts are fine; its costs are not |
| `cc.py` profile discovery | **BROKEN** (filed) | `PROJECTS_DIR` hardcoded to `~/.claude`, ignores `CLAUDE_CONFIG_DIR` — silently blind to `~/.claude-sdd` and `~/.claude-kat` |
| `usage.db` `input_json`/`output_json` | **GOOD, richer than documented** | ~100% populated on this deployment (not the sparse debug-only case the docs imply). Targets are reconstructable retroactively |
| `err_family` + backfill | **GOOD** | 26 families, fingerprint-versioned via `PRAGMA user_version` (`src/usage/db.rs:225`, `:444-483`, `:485-525`). Adding a family re-classifies history on next open |
| CC `toolDenialKind` | **GOOD, unused** | Structural field on `type:"user"` entries; `is_error:true` on 153/153 denials; stable across CC 2.1.233-235. Prefer it to text-grepping denial messages, which overcounts ~4x |

## Measured friction rates — the 2026-08-20 replication of TU-7, with a base rate

Ground truth: 840 matched errors = 74.5% of the 30-day error corpus, transcript-joined by
exact `(tool_name, input_json)` equality. Base friction rate **27.0%** (label = retry of the
same mistake, or ≥3 calls to recover, or no recovery within 25 calls — the pass's own
construct; absolute rates move with the threshold, ordering does not).

- **Iron-Law gates are the healthiest errors AND the highest-volume.** `il2_structural_edit`
  0.16x lift, `il3_shell_on_source` 0.50x, `il1_read_overlaps_symbol` 0.68x,
  `il3_pipe_to_trimmer` 0.71x — all **below** base rate. Median calls-to-recovery corpus-wide
  is **1**. (TU-7 reached the same conclusion on recovery/repeat; this adds the base rate.)
- **`err_family IS NULL` is the high-friction bucket** — 53.2%, **1.97x lift**.
- IL violations concentrate **75-88% in dispatched subagents**, which never receive the
  parent's injected guides. Dispatch-briefing problem (Iron Law 6), not a `source.md` slice
  problem.
- Note TU-7's own contrast still holds and is sharper than volume: `write_scope_denied` had
  the corpus's **highest** immediate-repeat rate (26%) on only 42 hits, versus
  `il3_pipe_to_trimmer` at 3% on 262 — the difference being a message that names a concrete
  corrective action.

## Detector validation — what failed and what replaced it

| Predicate | Precision | Lift | Verdict |
|---|---|---|---|
| fire on every error | 27.0% | 1.00x | baseline |
| `repeat_family` (best variant) | 31.2% | 1.16x | reshaped |
| `target_thrash` (≥3 on one target) | 27.1% | **1.00x** | **dropped** — friction-random |
| `route_around` (different tool succeeds on same target) | 21.5% | **0.79x** | **dropped** — firings are agents correctly obeying the gate |
| ~~`S-A OR S-B`~~ | ~~48.0%~~ | ~~1.78x~~ | **S-B FALSIFIED 2026-08-20 — see below** |

`S-A` = consecutive-error run in one session with no intervening success. `S-B` =
`err_family IS NULL`.

**S-B does not survive measurement against `usage.db` itself.** Re-measured with
`scripts/friction-probe.py` (calibrated against TU-7 first — ratios 0.9 / 0.82 / 0.63,
ordering preserved):

| Predicate (usage.db only, 30-day corpus) | NULL | classified | ratio |
|---|---:|---:|---:|
| immediate repeat (TU-7's discriminator) | **2.8%** (2/71) | ~4-5% avg | NULL is **better** |
| same tool succeeds later | 15.5% | 11.2% | 1.38x |
| `calls_to_recovery` | mean **1.13**, 0% unrecovered | mean 1.0–1.67 | mid-pack |
| *(POV1's transcript-joined label)* | *53.2%* | *27.0%* | *1.97x* |

The NULL bucket is **49% librarian/artifact API-shape errors + 31% one worktree-activate
write gate** — one uncovered *surface*, not a general untaught population. Classifying it is
worth doing for taxonomy reasons; it is not a friction detector. **`S-A` survives**: 84 runs
of length ≥2 over 30 days, 16.0% of errors inside one.

Caveat recorded so the mistake is not repeated: one test run during this refutation was
itself invalid — "recovery = a later success sharing `friction_target`" returns ~99%
friction for *both* arms, because `friction_target` is only populated when `is_friction` is
true, so a success essentially never carries one. The divergence from POV1's label is
therefore **unexplained, not attributed**.

## Before/after comparison — not yet possible, and what it would take

Use `scripts/friction-probe.py --split-at "<UTC>" --clean-only`. On a 2026-08-18 22:30
cutoff: adjusted effect **0.86pp** against **±0.40pp** Poisson noise (**2.15σ**), needing
**~8,071 clean calls per arm** for 80% power against **1,740** available — **4.6× more
data**. Re-run when that closes; do not quote the delta before then.

Three confounds must be handled every time, and a fourth is unhandled:

1. **Workload mix** — across that cutoff `run_command` went 34.8% → 48.9% of calls while
   `symbols`/`edit_code`/`read_file`/`grep` all fell. Per-tool rates span 0.31% (`grep`) to
   6.91% (`edit_code`), so the aggregate moves with no behaviour change. Handled by direct
   standardisation; `mix_coverage` flags an extrapolating adjustment.
2. **Build identity** — `codescout_sha` is per call and builds run *concurrently*, so a time
   split mixes builds and a build split mixes time. `codescout_dirty` went **4% → 22%**;
   `--clean-only` drops those.
3. **Reconnects** — the 24h window held **15 builds across 23 processes**. "After" is not
   one condition; the probe says so on every run.
4. **UNHANDLED — mix confounding is fractal.** `read_markdown` looked 2.4× worse after the
   cutoff with its error *composition* unchanged (`librarian_managed_artifact` 68% before,
   67% after) — the within-tool workload moved. The unit that would fix it is
   (tool × target-kind), which needs `friction_target`, NULL on 96.6% of rows. A third
   independent argument for that bug.

## Signal availability — what lives where

- **`thinking` blocks are empty**: 0 of 4,906 carry text (Opus-5/Sonnet-5 emit an encrypted
  `signature` only; only `claude-sonnet-4-6` emits plaintext). Do not build on them.
- **`is_error` is bimodal**: absent on codescout `RecoverableError`s *by design*
  (`RecoverableError → isError:false`), present on 100% of hook denials. A transcript-level
  error scan cannot see codescout's largest error population — count from `usage.db.outcome`.
- **Narration self-reports work**: first-person error admissions (`I was wrong`, `I blurred`,
  `misread`) 12 hits at 100% precision; self-correction verbs 39 at 100%. Guessed families are
  the weak ones — `actually` 337 hits at ~25%, `wait` fired **once** in 4,180 blocks.
- **User corrections do not text-match**: 67 keyword hits over 154 user entries, **0 genuine**
  (all synthetic wrappers). Only 3 real across 4 large sessions. The agent is the better
  instrument for this class.
- **Transcript joins**: `sessionId` == `cc_session_id` always, but subagents live in
  `projects/<enc>/<sid>/subagents/agent-*.jsonl` (291 files across 3 profiles here) — top-level
  counting undercounts by up to 47%. 3 of 22 sessions have no transcript at all. `2c518eb6`
  exists in two profiles, the shorter copy stale — prefer the longest.
- **`promptId`** is the turn key (0/126 contiguity violations) but is **not resume-stable**;
  `cc_session_id` is the durable one. Compaction summaries and task-notifications mint fresh
  `promptId`s (system-triggered pseudo-turns).
- **Turn/token axes are transcript-only by construction** — codescout sees one
  `call_content()`; `promptId` is never sent to the server and tokens live in the API response.

## Where the work lives

- **Design**: `capability-proposals:CAP-9` — five items, ordered, with a substrate check and
  four open questions. Items 1-2 (attribution fix, `S-A OR S-B`) are prerequisites; 3-5 each
  have an open decision. **Start at the attribution bug, not the detector.**
- **Bugs**: six filed 2026-08-20 in `66654f53` (patch-id
  `8ddbe5e10a9851ec7b7db241e4590b51956a09df`), all `status: open` under `docs/issues/2026-08-20-*`.
- **Tool-usage lessons**: `tool-usage-patterns:T-25` (corrected, with the decay story) and
  `T-26` (grep-vs-structured-telemetry, with false-positive rates).
- **Prior art that must be read first**: `2026-08-15-tool-usage-investigation:TU-7` and
  `2026-08-16-iron-law-gate-firing-audit`. Both are dated snapshots whose own contracts forbid
  extending them — new evidence opens a new dated document.
- **`pika_observations`** exists in `usage.db` with a full typed schema, **0 rows, 0 references
  in `src/`** — created by a buddy plugin skill, and orphaned rather than deleted by the 30-day
  retention sweep because `usage.db` never enables `PRAGMA foreign_keys`.

## One claim that did NOT reproduce

An analysis pass reported an uncatalogued error family `"another codescout instance is writing
to this project"` from concurrent subagent dispatch. **Zero rows match it**; a wider
instance/locked/concurrent sweep returns only the IL-3 families. Do not hunt for it. The real
unclassified head is the worktree/activate write block (22 hits).
