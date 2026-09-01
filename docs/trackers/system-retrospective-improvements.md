---
id: '6f5ec09c63aef864'
kind: tracker
status: active
title: System Retrospective 2026-09 — Improvement Tasks
tags:
- retrospective
- improvements
- task-list
topic: birds-eye retrospective findings and the six structural improvement tasks derived from them
---

# System Retrospective 2026-09 — Improvement Tasks

## Why this initiative exists

Birds-eye retrospective (2026-09-01) synthesized from three sweeps: the session-intelligence trackers (9 OB classes, 17 IC clusters, 79 SDD rulings at ~12% wrong-rate), the bug corpus (531 files: 36 open / 495 archived, ~93% of archived genuinely fixed), and an architecture assessment (~56K LOC tools vs ~56.5K LOC librarian). Six improvement tasks fell out of it; this tracker owns them.

## Report summary (knowledge)

**The system is two products in one binary.** System A = code intelligence (`src/tools/`, ~56K LOC): LSP + tree-sitter navigation, hybrid semantic search, progressive disclosure. System B = the librarian (~56.5K LOC): artifact catalog, trackers, epistemics engine. They are now the same size; System B's hazards are different in kind and deserve their own discipline.

**Validated strengths:**
- Context economy at one choke point — `OutputGuard` + `@ref` buffers + `RecoverableError`/`isError:false` routing in the single dispatch pipeline (`src/server.rs`); every tool inherits it. Demonstrably works on its own author.
- Process designed so compliance disarms traps — the gate reorder (lean lane third, default last) converted "be careful" into "following instructions leaves nothing armed" (OB-2's mechanism, the template for the rest).
- Capture discipline compounds — the August step change (299 bug files, 56% of corpus) is what made IC-6 findable: the largest class (27 instances) sat at n=2 until the archive was counted.
- Agent-agnostic self-containment — enforcement in server, UX in plugin, boundary actively defended.
- Measurement beat intuition every time tried (A/B'd prompt candidate lost p≈0.47; `--workspace` swap verified by set-difference).

**The systematic failure signature:** ~62% of bugs are silent-failure shaped; the dominant species returns a **plausible value instead of an error**. Top clusters: addressing-without-an-escape-hatch (27), declared-not-wired (19), gate-keyed-on-unobservable-event (16), capped-result-presented-as-complete (16), accepted-parameter-silently-dropped (15), shared-resource-carries-no-owner (15). Archetype: `find` silently dropping `rel_path` and returning `count: 50` — the default page size wearing the costume of a match total. Second-order: **silences regenerate under repair** (the `update_entry` no-op success was introduced inside the fix for an earlier silent-write bug). Root context: an MCP server's consumer is an LLM, which always accepts a plausible number — the human-squinting feedback loop that normally kills silent failures is structurally absent, so the error-model bar must be higher.

**Biggest structural liability:** machine-local, silent-by-design state. Catalog, `cites` edges, augmentations, Qdrant index all live outside git; one measured pull lost 21/23 memories and 697/1117 edges with no error. `id = sha256(abs_path)` makes every file move a potential orphaning event. The vanished-rows bug (`docs/issues/2026-08-25-sdd-ledger-and-catalog-rows-vanished.md`, zombie) is root-cause-*undeterminable* because the catalog keeps no write audit trail. Discoverability gap: 380 hand-maintained `edit_markdown` calls on tracker tables while `entry_collection`/`state_at` sit near 0.1% usage.

**Meta-insight (converged on independently by OB, I-N, and the SDD ruling log):** for a whole family of defect classes the observer is disqualified by construction; only standing mechanisms that fire when nobody is worried actually catch them. Knowing the class prevented none of the instances. The ~12% SDD wrong-ruling rate — from a controller actively applying the lessons — calibrates how far vigilance can be trusted.

## T-1 — Catalog append-only audit trail (WAL of mutations)

**Why:** The catalog's most valuable failure (`sdd-ledger-and-catalog-rows-vanished`, zombie, high severity) is root-cause-undeterminable *by construction* — no record of which process wrote/deleted what, when. Same gap is OB-4's "which instrument actually ran". Highest-leverage change in the review: converts the entire silent-loss class from undeterminable to forensic.

**Shape:** a small append-only mutation log inside the catalog (who: pid/session/verb; when; what: table, row id, op, before-hash or delta). Written in the same transaction as the mutation so it cannot lie by omission. Needs a retention story and a query surface (doctor and/or a `librarian` action).

**Acceptance:** every catalog mutation path emits exactly one audit row in-transaction; a deliberate out-of-band deletion is reconstructible from the log alone; a test proves a mutation cannot commit without its audit row (guard must be shown able to fire — demand a deliberate break).

**Design settled 2026-09-01** (brainstorming, three decisions + phasing): capture via SQLite **triggers** so any writer — including raw `sqlite3` shells and foreign processes — is recorded and a mutation cannot commit without its audit row; payload = full OLD image on delete, changed-fields diff on update, id-only on insert; retention = keep forever, manual dry-run-by-default prune. Identity via per-connection `TEMP TABLE audit_ctx` seeded from `SessionKey::resolve`; out-of-band writers record as `'unknown'` — itself the forensic answer. Triggers reinstalled on every `Catalog::open` after migrations (table-copy migrations drop triggers; same-open reinstall closes the self-heal window). New surfaces: `librarian(action="audit_log")` + a doctor `audit` block. Full design: `docs/superpowers/specs/2026-09-01-catalog-audit-trail-design.md`. Git sharing of the trail is deliberately split out as T-7.
## T-2 — Unknown-field rejection as a workspace-wide invariant

**Why:** IC-15 (accepted-parameter-silently-dropped) has 15 instances; archetype is `find::Args` lacking `deny_unknown_fields`, returning page 1 as `count: 50`. The SDD lesson "an instance fixed is not a class enumerated" prescribes the fix shape: one deliberate enumeration, settled for good.

**Shape:** `deny_unknown_fields` (or an equivalent unknown-key `corrections` report, per repair-and-continue ADR 2026-07-10) on every tool Args struct, gated by a test that *enumerates* every Args type so a new tool cannot opt out silently.

**Acceptance:** enumeration test lists all Args structs and asserts the invariant; passing a bogus key to any tool yields either a refusal or a `corrections` note — never silence.

## T-3 — Name System B: librarian as a second product

**Why:** the librarian is now half the codebase and drifting toward a general agent-memory substrate. Its hazards (machine-local state, id-by-path, augmentation foot-guns) ride codescout's roadmap and release discipline, where they don't fit.

**Shape:** conceptual split first — own roadmap section, own hazard register, own release checklist (catalog migration story per release). Physical crate split only if/when warranted.

**Acceptance:** a librarian roadmap + hazard register exists and is referenced from `docs/ROADMAP.md`; release docs carry a librarian-specific checklist.

## T-4 — Escape hatches for IC-6 top offenders

**Why:** IC-6 (addressing-without-an-escape-hatch) is the largest class (27 instances, 5 subsystems). Flagship: `link_scan` binds any `[A-Z]{1,3}-\d+` token, backticks included — an entry id cannot be *mentioned* without being cited, so the ledger cannot describe its own bugs (`docs/issues/2026-08-31-an-entry-id-cannot-be-mentioned-without-citing-it.md`).

**Shape:** per the CLAUDE.md § Parsers Over a Namespace contract — every parser owes an escape and a disambiguator, answered *in the code*. Start with link_scan (inline escape syntax), then the identical-headings disambiguator gap (`occurrence` exists — verify the refusal sites actually prescribe it).

**Acceptance:** a token can be written literally near link_scan without minting an edge; each fixed refusal site names its escape in the error text.

## T-5 — Archive back-tagging sweep

**Why:** 377/531 bug files (71%) carry no `cluster/` tag. IC-6 jumped 2→27 when the archive was counted once; IC-11/13/14/15 sit at "spread unadjudicated". At least one more CLAUDE.md-section-sized class is plausibly hiding in the untagged 377.

**Shape:** batched classification sweep over `docs/issues/archive/`, tags written through the catalog (BL-48: direct frontmatter edits don't reach it). Track adjudication of the four "spread unadjudicated" clusters as part of the sweep.

**Acceptance:** archive tag coverage substantially complete (or files explicitly marked unclassifiable); each cluster's n and subsystem spread re-derived and recorded in `issue-clusters.md`.

## T-6 — Two-representations-one-truth seams

**Why:** prompt-surface triplication (3 surfaces, manual `ONBOARDING_VERSION` bump) and the dual symbol source (`symbols()` silently swapping LSP↔tree-sitter mid-session) are the same defect shape: two representations, one truth, drift silent. The gate reorder showed the winning remedy shape — make the default path end in the consistent state, don't add a checker.

**Shape:** per seam, find the single-source-of-truth restructure rather than another drift test. For symbols: at minimum, name the active source in the response so the swap is observable.

**Acceptance:** each seam either derives from one source or loudly names which representation answered.


**Named instances** (add to this list rather than opening a sibling task):

1. *Prompt-surface triplication* — 3 surfaces, manual `ONBOARDING_VERSION` bump. The original motivating case.
2. *Dual symbol source* — `symbols()` silently swapping LSP↔tree-sitter mid-session.
3. **`src/prompts/README.md`'s own figures, fixed 2026-09-01.** § *Surfaces* read `57,148 / 48,627 as of 2026-08-18` and § *The tool-surface budget* read `6,528 characters across 24 pinnable tools`; measured live the same day, the surface was **54,976 / 47,549** and the injection **3,818 across 23**. Wrong on five numbers, in a file whose own § budget section warns about exactly this. Remedy applied was the one this task prescribes — not a drift checker but a pointer to the deriving command (`cargo test --lib tool_surface_report_lengths -- --nocapture`), so the next reader re-derives instead of re-citing.
4. **`param_probe`'s `Spec::required` table vs the advertised schema** — closed by T-9. The `required()` helper stated what each action needs while the schema stated something different, and `graft` lived in the gap for as long as both existed. Note the remedy shape: the fix asserted the two existing representations against each other rather than adding a third.
## T-7 — Committed audit shards (phase 2 of T-1)

**Why:** the audit WAL (T-1) is machine-local like everything else in the catalog — the same liability class it exists to make forensic. Exported to committed per-host JSONL shards, one machine can answer "which session on the other machine deleted these rows".

**Shape:** `audit_log export` → `.codescout/audit/<host>-<YYYYMM>.jsonl`, append-only, `merge=union` gitattribute (different hosts write different files; same-host branch merges union cleanly — global order derives from `(host, seq, at_ms)`, never line position). Watermark `audit_exported_through_seq` + doctor's unexported-delta report keep the replica honest: a committed log must never read as complete when it is only as fresh as its last export (IC-13). Export folds into `reindex`/`merge_worktree` so it cannot be forgotten (IC-3); reindex churn filtered from export by default. Precedent: augmentation sidecar (`f565504a`).

**Acceptance:** a row mutated on host A is queryable on host B after pull; same-host two-branch shard merge needs no manual resolution; doctor names the unexported delta; a merged read labels each host's coverage window.

**Depends on:** T-1 (row format designed export-ready there: `seq` monotone/never-reused, watermark key reserved). Full design: `docs/superpowers/specs/2026-09-01-catalog-audit-trail-design.md` § Phase 2.

**Unblocked 2026-09-01 by T-13.** Three design decisions settled before planning, two of
them correcting the spec's Phase 2 sketch:

- **Read path: merge-on-query, stateless.** `audit_log` reads the local table and streams the
  shard files, merging on `(at_ms, host, seq)`. No import step and no second copy — an
  imported replica is the exact shape T-6 exists to remove, and shard filenames encode
  `host` + `YYYYMM` so `since`/`until` prunes files before opening them.
- **Shard scope: everything except `commits`-table rows.** The `commits` table is a cache of
  git, so committing an audit of it into git is circular; at ~192 rows/day it is also the
  largest remaining term. Still audited locally, just not exported.
- **Prerequisites first (T-13), not folded in.** The two volume defects are logically
  independent of an export feature, and fixing them first made the *local* trail usable
  immediately rather than holding that behind T-7 shipping.

Steady-state estimate after T-13, grounded on the catalog's own 98-day history (3,545 events
≈ 36/day, 4,446 artifacts): roughly **1–2 MB/month/host**. The 1.74-hour window this was
scoped in was a burst — an SDD run with 9 commits, several reindexes and a merge — and must
not be extrapolated as a rate.
## T-8 — Advertise `graft`'s required params

**Why:** `artifact(action="graft")` is advertised in the action enum (`src/librarian/tools/artifact.rs:35`) but its two required params are not in the schema. `src/librarian/tools/graft.rs:10-11` declares `Args { from_id: String, into_id: String }`; neither appears among `artifact`'s 53 advertised properties, verified against the live wire. Measured in `usage.db`: **1 attempt, 1 failure** (`missing_required_param`). The action is unusable as advertised.

**Shape:** add `from_id` / `into_id` to `input_schema` in `src/librarian/tools/artifact.rs`, action-labelled like their siblings (`src_id`/`dst_id` for `link`, `new_rel_path` for `move`).

**Acceptance:** T-9's guard goes green on `graft` without its `required()` helper supplying the params out-of-band; a live `artifact(action="graft")` with no params errors naming both fields, and both appear in `tools/list`.

## T-9 — Action→schema guard: required params must be advertised

**Why:** `every_action_labelled_schema_key_is_honored_by_that_action` (`src/librarian/tools/artifact.rs:350`) iterates `schema["properties"]` — it checks **schema→action** only. Nothing checks the reverse, which is why T-8's defect survived. Sharper: that test's own `required()` helper **hardcodes** `from_id`/`into_id` for `graft` (`artifact.rs:386-389`), so the knowledge that would have caught the bug was written down, out-of-band, inside the test that missed it — § *Two-representations-one-truth* (T-6) occurring inside the guard.

**Shape:** assert the reverse direction — every required (non-`Option`, no `serde(default)`) field of each action's `Args` appears in `schema["properties"]`. Derive one list from the other rather than maintaining a second hand-written table.

**This is worth MORE because the structural fix is deferred.** `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md` gates `schemars`-derived schemas on a third instance of *advertised ≠ accepted*. Until that fires, this guard is what **detects** instance 3 rather than waiting for someone to trip over it — CLAUDE.md § *Observer Blindness*'s "check that runs when nobody is worried".

**Acceptance:** written **before** T-8 so it goes red on `graft` naturally — that is the deliberate break, earned rather than staged. Then green after T-8, with a param deletion re-reddening it.

## T-10 — Correct the tool-surface-budget spec's instance count

**Why:** `0e0316e9036d7f16` § *Rejected* defers `schemars` derivation under a `tool-registration-rule-of-three` resting on *"one confirmed instance (F-1) plus one unverified (`query` / `title_contains` / `preview` passed by agents, unadvertised — origin not yet checked)"*. Both halves are now checkable, and the count moves in **both directions**:

- `graft` is a **new confirmed** instance (T-8) — the count rises to 2 confirmed.
- The unverified one is **the wrong class** and should be retired. Verified 2026-09-01: no `Args` struct under `src/librarian/tools/` accepts `query`, `title_contains` or `preview` (sole grep hit is an unrelated fn parameter, `get.rs:35`), and `find::Args` cannot carry `deny_unknown_fields` because the dispatcher passes sibling actions' keys through. So those three are neither advertised **nor** accepted — agents guessed the names and serde dropped them silently. That is IC-15 accepted-parameter-silently-dropped, i.e. **T-2's** class, not this one.

**Net: 2 confirmed, 1 live. The trigger has NOT fired.** Recording this is what keeps the gate honest — an inflated count fires the rewrite early exactly as a stale one never fires it at all.

**Shape:** update § *Rejected* and § *Revisit-when* through the catalog. Re-point the retired instance at T-2.

**Acceptance:** the spec names its instances with their verification status, and a reader can tell how far the trigger is from firing without re-deriving it.

## T-11 — Fix the bare missing-param error, then cut the prose compensating for it

**Why:** `missing_required_param` is the **#2 error family** across the librarian tools (41, behind `edit_stale_match` at 44). **12 of the 41 are `missing field 'patch'`** — the raw serde message from `artifact(action="update")`. Its siblings (`get`, `graph`, `link`, `append_entry`, `update_entry`) already name the action and carry a hint; `update` does not. The `artifact` tool description *already documents this exact failure in prose* — so the per-request surface pays, on every request of every session, to describe a defect a one-line fix removes.

**Shape:** bring `update` (and any other bare-message action) up to the sibling pattern via `RecoverableError::with_hint`. **Then** cut the compensating paragraph from the description.

**Acceptance:** gate green; the cut is the **only** change in this cohort that removes prose the model reads, so it ships behind pre-registered `A-35` with an observational window on `err_family` (P-4, precedent A-2).

## T-12 — `artifact_event` legibility

**Why:** worst error rate of any tool on the surface — **26.1% (6 of 23)** — and **100% of its failures are `missing_required_param`**, at 2,258 chars of per-request cost. The `kind`→required-payload-keys mapping (`note`→`text`, `status_change`→`to`, `verdict`→`outcome`, …) exists only as description prose, so it is not reachable by the machine at the moment of failure.

**Shape:** surface the mapping from prose into the schema, or make the error name the required keys **for the `kind` actually passed**.

**Acceptance:** a `create` call with a valid `kind` and a missing payload key errors naming that key. Re-judge its 7 never-passed params **after** the fix, never before — both time-travel actions (`artifact:state_at`, `librarian:workspace_state_at`) were attempted exactly once each and **both failed on the same missing param**, so a 0/2 discovery rate is what "never passed" can mean.

## T-13 — Audit volume prerequisites for T-7

**Valid:** dated 2026-09-01

**Why it exists:** scoping T-7 started by histogramming the live trail instead of reading
the spec's Phase 2 bullets, and the histogram said the spec's volume analysis was aimed at
the wrong term. The spec proposed filtering "pure reindex churn" at export; reindex churn is
**0.4%** of the rows. The dominant terms were two defects nobody had measured.

**Measured** (live catalog, 1.74-hour window, 27,914 rows):

| class | rows | share | payload chars | share |
|---|---:|---:|---:|---:|
| `commits` update, payload `{}` | 27,505 | 98.5% | 55,010 | 6% |
| `artifact_augmentation` update | 23 | 0.08% | 786,771 | **88%** |
| `artifact` update, churn keys only | 107 | 0.4% | 25,359 | 3% |
| genuine signal | 279 | 1.0% | ~22k | 3% |

Unfiltered that is ~380k rows/day into a git-committed file. T-7 was not shippable on it.

**Shipped** at `40ab56f6` (patch-id `d3021d83634be0f6b8d7c69200f241f80f9e5f96`): a `WHEN`
guard on the UPDATE triggers (`IS NOT`, never `<>`); a stand-in for oversized values in
UPDATE diffs only, leaving DELETE images verbatim because that is where the forensic value
is and not where the bytes are; and `payload_bytes` + `largest_payload_bytes` on
`audit::health`.

**Verified live, not just in tests.** Triggers are schema-level, so the fix reached the
still-running old-binary server the moment an upgraded process opened the catalog. A reindex
that previously wrote ~2,750 `commits`/`update` rows wrote **20 rows, 0 empty diffs**. A real
tracker append wrote **441 chars** where it would have written 7,364.

**Closed three bug files:** the empty-diff one (opened on notice during this measurement),
the augmentation-params one (its "measure first" step discharged by the table above), and the
writer-abort one — whose BLOB half got a test born red on the real production error, and
whose NULL-key half is closed by narrowing the module-doc invariant rather than by a guard
no caller can reach.

## History

### 2026-09-01 — Tracker created
Seeded from the birds-eye retrospective (3-agent sweep: trackers, bug corpus, architecture). T-1 selected as the starting task.

### 2026-09-01 — T-1 design approved; T-7 split out
Brainstorming settled T-1's three axes (any-writer triggers / full-on-delete+diff-on-update / keep-forever) and produced `docs/superpowers/specs/2026-09-01-catalog-audit-trail-design.md`. The git-sharing question raised during review became T-7 (committed audit shards), phased after T-1 with the row format made export-ready up front.


### 2026-09-01 — T-8..T-12 added from the tool-surface measurement

Measured the `tools/list` surface off the live wire (26 tools, 54,583 chars = 47,156 schema + 7,427 desc; budget 56,519, headroom 1,936), reconciled against `server::tests::tool_surface_report_lengths`. Found it **under budget but mis-allocated**: `graft` advertised without its two required params (T-8), no guard in that direction (T-9), and `missing_required_param` the #2 error family at 41 (T-11/T-12).

Three candidate tasks were **dropped as already adjudicated**, which is why this cohort is five and not eight: `library`'s zero calls is open experiment `prompt-surface-compaction-session-log:F-3`, re-running on/after 2026-09-02; narrowing `pinnable()` / trimming the injected `workspace` param was **rejected** in `0e0316e9036d7f16` ("a bursty capability, not a dead one"); and per-tool schema caps were rejected there too ("per-item caps do not bound a sum").

T-10 exists because the measurement **corrected the premise of a deferred decision in both directions at once** — see its section.


### 2026-09-01 — T-8 and T-9 shipped; T-10's premise established

T-9 written **first**, deliberately: it went red on `["graft:from_id", "graft:into_id"]` against the unfixed schema, so the deliberate break was earned from a real defect rather than staged by mutating a passing test. T-8 then made it green. Guard wired at all four `param_probe` sites (`artifact`, `librarian`, `artifact_event`, `artifact_refresh`) — one kill proves one site.

The guard adds **no third list**. `Spec::required` already states each action's required params, and `sweep` depends on that table being complete (its base call must survive deserialisation), so the two representations already existed and the assertion just makes them agree. Its own blind spot — an action absent from `required` contributes nothing and is passed over silently — is recorded at the function's doc comment, not only in the bug file.

Surface cost +393 chars (54,583 → 54,976; headroom 1,936 → 1,543): an **addition**, carried under P-3 by the measured deficit (1 attempt / 1 failure, plus `missing_required_param` ×41). Gate green: clippy clean, lean 3408/0, default 4984/0. Fix `6894b67d` on `experiments`, patch-id `3cb9bc68a685c46252388dc21a3dd8d7beff9098`. Bug archived as `2fbb59c9b84a0dcf`, tagged `cluster/declared-not-wired` through the catalog.

T-10's investigation is **done and its finding is the opposite of what was expected**: chasing whether `graft` was the third instance that trips the `schemars` rule-of-three showed the spec's second instance does not qualify. `query`/`title_contains`/`preview` are neither advertised nor accepted — no `Args` under `src/librarian/tools/` has those fields — so they are IC-15 silent-drop (T-2's class), not this one. Net **2 confirmed, 1 live; the trigger has NOT fired.** Writing that correction into the spec is what remains of T-10.


### 2026-09-01 — T-8..T-12 closed out, and two of the five were scoped on a stale premise

Final: **T-8, T-9, T-10, T-11 done; T-12 dropped.** Commits `6894b67d` (fix + guard),
`2e14e92a` (bug file + tasks), `1468c4b5` (probe + three stale counts), `0e48ef50` (T-11).

**The result worth keeping is not any of the fixes — it is that 2 of 5 tasks rested on a
number that had already been fixed.** Both were error aggregates over `usage.db`'s 29-day
retention window, and both windows *straddled the fix commit*:

| task | headline number | reality |
|---|---|---|
| T-11 | `missing field 'patch'` ×11, largest single cause in its family | all 11 predate `60df0d76` (2026-08-27 18:46); **zero after** |
| T-12 | `artifact_event` 26.1% error rate, worst on the surface | spans **two** fixes (`6ba720bc` 2026-08-16, plus the `with_hint` messages on 08-27); since 08-27, **12 calls / 0 errors** |

This defect class is nastier than a wrong query, because **the number is real and the query is
correct.** Nothing in `SELECT err_family, count(*)` hints that half the population is
historical. It survives review for the same reason the attribution errors that evening did —
it is assembled entirely from true parts.

**The check is one clause and it is now the probe's fourth documented trap:** group any error
aggregate by `date(called_at)` before scoping work on it. *A count that stops on a date is a
fixed bug.* T-11 survived as a real but **different** task once re-derived past the fix
timestamp (9 of 15 live failures were bare serde messages from the remaining bare-`?` sites);
T-12 did not survive at all.

Note what did **not** save us: T-11 and T-12 were the two tasks with the *most* quantitative
backing in the original plan. The measurement is what made them look strongest, and the
measurement is what was wrong.

### 2026-09-01 — T-1 landed on experiments
Catalog audit trail merged: 9 commits, fast-forward to `10972335` (patch-id `e2076003a9cd7cb3bedee3ff1e2ee7c944bed0bf` for the head commit; the branch was rebased twice mid-run, so cite by patch-id). Final gate on the merged tree: clippy clean, lean 0 failures, default 0 failures across 27 binaries (exit codes 0/0/0, `FAILED|panicked` grep = 0). SDD run: 5 tasks, 3 fix rounds + 1 final fix wave, 15 rulings (0 wrong so far) appended to `docs/trackers/sdd-ruling-log.md`. Two Opus reviews produced the load-bearing findings (non-atomic trigger install; prune-ignores-filters). Follow-ups owed and filed as bugs: audit-growth-via-augmentation-params, audit-trigger-can-abort-writer. Phase 2 (committed shards) = T-7.
