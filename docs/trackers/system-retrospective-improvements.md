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


## T-14 — artifact(create) stamps an `id:` that guard-locks the file

**Status:** open — **HIGH**, and the first of this batch to work.
**Valid:** dated 2026-09-01
**Bug:** `docs/issues/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md`

**Why:** `artifact(action="create")` writes `id: '<16-hex>'` into frontmatter;
`librarian_guard`'s `stamped` arm (`src/util/librarian_guard.rs:95`) reads that as "the
librarian owns this file", so **every artifact the tool creates is permanently refused by
`edit_markdown` / `read_markdown`** — whatever its `kind`, augmented or not, ledger or not.
Verified by contrast in one session: `docs/TEAM-ONBOARDING.md` (tool-created, `kind: doc`,
plain prose) refused; `CONTRIBUTING.md` and `README.md` (catalog rows, indexed not created)
edited fine. Membership is not the trigger — the stamp is.

Three things make it worth doing first. It contradicts a ruling that was tried, probed and
reverted (`bb9a94d7`, documented in `get_guide("tracker-conventions")` § *Make the tracker
guarded*). The guard's own pinned test
`a_catalogued_but_unaugmented_file_stays_directly_editable` **cannot see it** — its sample
holds no *created* artifact, which is the recording-filter law, and here widening the sample
is the fix. And it was **masked until a repair**: the guard learned to read quoted ids
(`docs/issues/archive/2026-08-16-librarian-guard-misses-quoted-frontmatter-ids.md`) and
`create` emits the quoted form, so making the guard correct on its stated axis switched the
defect on across the whole created population.

**Shape:** decide between (a) stop stamping on create and let `reindex` associate path→id,
and (b) narrow the `stamped` predicate to the property the guard actually cares about —
*does this file's state live somewhere other than this file?*, which `augmented` and
`ledger` already answer. **Enumerate what consumes the frontmatter id before choosing**:
`doctor`'s `frontmatter_id_mismatch` / `repair_frontmatter_id` read it, and the conventions
guide says a ledger's identity must travel with the repo.

**Acceptance:** a `kind: doc` file produced by `artifact(create)` stays directly editable;
the new row in `a_catalogued_but_unaugmented_file_stays_directly_editable` **reds against
today's tree before the fix** (demand a deliberate break); and the `stamped` arm names why
it fired — today its `why` is the empty string (`src/util/librarian_guard.rs:106-108`),
which makes the arm most likely to fire unintentionally the one that explains itself least.

## T-15 — Wire the eight unannotated guides into section-grain `get_guide`

**Status:** open — **RE-SCOPED 2026-09-01 by measurement, and the measurement refutes this
entry's own framing.** Split into T-15a (cheap, ready) and T-15b (authoring, needs a decision).
**Valid:** dated 2026-09-01

**Why:** the section-grain mechanism **shipped 2026-08-27** (`7579b32b1cd2362f` Phase 1) and
its biggest consumer is unwired. Measured 2026-09-01:

```
for f in src/prompts/guides/*.md; do printf "%s serves | %s words | %s\n" \
  "$(grep -c '<!-- serves:' "$f")" "$(wc -w < "$f")" "$f"; done
```

`librarian.md` — **13** annotations at 3,097 words. `tracker-conventions.md` — **0** at
**5,829 words**, the largest guide in the corpus and the one that auto-injects on the first
`artifact` call of *any* session. Eight of ten guides have zero.

### The constraint this entry originally missed — a hard 2,500 B cap

A section that carries a `serves:` declaration must be **≤ 2,500 bytes**
(`MAX_DECLARED_SECTION_BYTES`, `src/prompts/guide_index.rs`), enforced by a test that fails the
build and whose message says *"Decompose it at `###` and move the declaration onto the child
sections. A slice this large is the failure this feature exists to fix."*

So *"annotation only, no code change"* — this entry's original claim, and the reason it was
ranked as the cheapest lever — **is true of most of the corpus and false of the guide that
matters most.** Measured per guide with a fence-aware splitter mirroring
`guide_index.rs::split_sections` (a naive `^## ` split is wrong here: three earlier
measurements were wrong because a fence line inside `tracker-conventions` matched it):

| guide | sections | over 2,500 B | total | annotatable today |
|---|---:|---:|---:|---|
| `librarian.md` | 18 | **0** | 23,137 B | already done (13) |
| `error-handling.md` | 3 | 0 | 1,857 B | all |
| `progressive-disclosure.md` | 5 | 0 | 5,669 B | all |
| `project-activation-bootstrap.md` | 5 | 0 | 2,594 B | all |
| `symbol-navigation.md` | 3 | 0 | 3,145 B | all |
| `untrusted-content.md` | 5 | 0 | 5,317 B | all |
| `iron-laws-detail.md` | 7 | 1 | 12,007 B | 6 of 7 |
| `librarian-runtime.md` | 11 | 1 | 10,944 B | 10 of 11 |
| `workspace-state.md` | 9 | 1 | 10,355 B | 8 of 9 |
| **`tracker-conventions.md`** | 15 | **5** | **39,106 B** | 10 of 15 |

**Note the tension the table exposes and this entry asserted away:** `librarian.md` is
annotated *because* all 18 of its sections already fit the cap. `tracker-conventions.md` is the
biggest lever *and* the only guide where a third of the sections cannot carry a declaration
without being decomposed first. Those are not independent facts — the guide is unannotated
partly because it is the hard one.

### T-15a — BLOCKED on `selector_key`; the "easy half" label was wrong in three ways

**Status: blocked. Do not attempt as written.** Scouted 2026-09-01 before annotating anything;
every clause of the original scoping was refuted. Superseded text kept below for the record.

> ~~Annotate the sections that already fit: **all** of `error-handling`, `progressive-disclosure`,
> `project-activation-bootstrap`, `symbol-navigation`, `untrusted-content`; and the
> under-cap sections of `iron-laws-detail` (6/7), `librarian-runtime` (10/11), `workspace-state`
> (8/9), `tracker-conventions` (10/15). Pure annotation, build-safe, independent of both Phase 2
> blockers on `7579b32b1cd2362f`.~~

**1. "Pure annotation" — the unit is the TOPIC, not the section.** Gate 5
(`every_section_of_a_declaring_topic_is_reachable`) obliges every section of a topic once any
one of them declares. So the four topics holding an over-cap section cannot be partially
annotated at all, and the per-topic counts above describe a partition the gate forbids.

**2. "Build-safe" — inverted. It would REMOVE delivery.** `Shape::matches`
(`src/prompts/guide_index.rs:179`) opens `let Some(sel) = sel else { return false }`,
deliberately — *"do not turn it into a wildcard"*. Only five tools in the registry override
`selector_key` (the librarian family via `adapter.rs`, plus `create_file`, `edit_file`,
`memory`). **Every** tool routing to `progressive-disclosure` (nine), `symbol-navigation`
(three) and `workspace-state` (one) returns `None`; `project-activation-bootstrap` has no
`relevant_guide_topic` at all — it is the session-opening special case. Annotating any of them
replaces whole-guide delivery with the preamble alone. Reproduced in the delivered bytes.

**3. "Ready now" — four of the nine topics are inert targets.** `error-handling`,
`untrusted-content`, `iron-laws-detail` and `librarian-runtime` are in
`PULL_ONLY_GUIDE_TOPICS`, and `GetGuide` serves `topic_body()` without consulting the index, so
declarations there change nothing either way.

**What is left of T-15a: nothing annotatable.** `tracker-conventions` is the only topic that is
both triggered and selector-bearing, and it is the five-over-cap one — i.e. **T-15b**, the half
that was deferred. The easy half was empty.

**The real prerequisite** is implementing `selector_key` on the tools that trigger these topics
(~13 tools, via `crate::tools::core::types::action_selector_key`). That is a behavioural change,
not an annotation pass, and it is unscoped — treat it as a new T-N rather than as part of this
one.

**Gate now refuses the bad configuration.** `b769277b` adds Gate 7
(`every_declaring_topic_has_a_live_route_to_a_declared_section`), which fails with the mechanism
and remedy named. Note its live population is **one file** — `grep -l 'serves:'
src/prompts/guides/*.md` returns only `librarian.md` (verified 2026-09-01, independently by
`codescout-b7`) — so the gate is cheap to hold but thinly exercised: a bug in the declaring path
has almost nothing testing it, and a second declaring member would be a deliberate act rather
than something the corpus supplies.

**Also corrected here:** the claim that this would be "a silent regression passing all six
gates" was wrong. Mutating three topics separately showed three of four already have a guard —
`symbol-navigation` via `a_non_declaring_topic_is_byte_identical_to_today`,
`project-activation-bootstrap` via `session_opening_guide_never_declares_sections`,
`workspace-state` incidentally via Gate 3. Only `progressive-disclosure` had nothing but a
re-pointable fixture assertion. See `reconnaissance-patterns:R-158`.

**Cross-cutting lesson:** `reconnaissance-patterns:R-157` — a work-split is two claims, and the
"easy half" label is the one that tells you not to check.

### T-15b — the five over-cap `tracker-conventions` sections

These need decomposition at `###` before they can be declared, which is **exactly** the
*"`tracker-conventions` is really six topics"* item that bug explicitly labels an authoring
judgement needing re-costing. Do not let T-15a drift into this. It wants a deliberate decision
about how that guide is organised, not a mechanical split to satisfy a byte cap — and a bad
split here is worse than no annotation, because a section boundary is what a future reader
inherits.

**Acceptance (both parts):** each annotated section names the tool call(s) it serves; the
`serves:` shapes must be real `tool.action` identifiers, because `parse_shape` accepts a
syntactically valid shape that can never match a live call and the section then silently stops
being delivered — the exact failure this feature exists to prevent. And the delivered-bytes
figure must be **re-measured** afterwards rather than asserted: the mechanism can be wired
correctly and still deliver everything if the keys do not match real call names.

## T-16 — Overflow hint on a heading-scoped `artifact(get)` points at metadata

**Status:** open — small.
**Valid:** dated 2026-09-01
**Bug:** `docs/issues/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md`

**Why:** when a heading-scoped `get` overflows, the envelope's `hint` names
`json_path="$.preview.headings[*]"` — the heading map — though the caller passed `heading=`
and wants `$.body`. Following the hint costs a wasted call. `src/librarian/adapter.rs:471`
states the right principle already ("the answer the action was asked for"); the mapping is
fixed rather than argument-aware, which is BL-19's own failure mode in a narrower form.

**Shape:** branch the hint on the arguments — `heading` / `headings` / `start_line` present
→ lead with `$.body`, and where the body will itself overflow, name the line-slice form
directly so the caller skips a hop.

**Measure before extending it:** the envelope carries `preview.headings` even for a
heading-scoped read (20 heading objects for `issue-clusters.md`), competing with the body
for the ~9 KB inline budget. If sections routinely sit just over the budget because of that
metadata, suppressing it **removes calls** rather than re-pointing them — which changes the
value of this task. Not yet measured.

**Acceptance:** a table test over `(args, expected_hint_json_path)` whose `heading` row
fails against today's tree.

## T-17 — Both shell gates evaluate a per-command predicate over the whole command string

**Status:** open — small, but needs a design call on the splitter.
**Valid:** dated 2026-09-01
**Bug:** `docs/issues/2026-09-01-source-gate-refuses-the-whole-compound-command.md`

**Why:** one offending clause in a `;`- or `&&`-separated command refuses every unrelated
clause beside it. Verified by three probes: `echo x; wc -l Cargo.toml` runs;
`echo x; wc -l Cargo.toml; grep -c fn src/main.rs` refuses wholesale; and
`echo x; find . -name '*.toml' | head -2` refuses with the `echo` never running — so the
**pipe** gate shares the defect, not just the source-file one. Fired four times in one
session on measurement commands.

The two gates differ in the half that matters for repair: the pipe gate **names the
offending clause** verbatim in its message; the source gate emits only `"shell access to
source files is blocked"`. So the offending clause is demonstrably in hand at refusal time,
and the pipe gate is the model for the message fix.

**Shape:** split on top-level separators and evaluate both predicates per resulting command;
refuse the whole command still — partial execution of a refused command is a worse contract
— but name the clause to remove. **One shared splitter, not two**: CLAUDE.md § *Parsers Over
a Namespace* records four independent shell gates in this process each separately
mis-parsing a heredoc, and a fifth parser is how that count reached four. Whatever splits
the string owes the usual escape answer — a `;` inside quotes or a heredoc body is not a
separator.

**Acceptance:** the three probes as a table test, the two refusal rows asserting on the
**named clause** rather than merely on refusal. Keep the `allowed` row — it is the
discriminator that stops a refuse-everything gate from satisfying both refusal rows.

**Fifth firing, 2026-09-01, and it raises the severity rather than the count.** The refused
command was:

    git status --short && grep -c 'fn selector_key' src/librarian/adapter.rs && git log -3 …

One `grep` clause on a source file refused the whole thing, so `git status` and `git log`
never ran. This entry currently characterises the population as *"measurement commands"*,
which reads as low-stakes; this instance was a **peer-collision check under time pressure** —
I was establishing whether a peer's index had captured my uncommitted work before warning
them, and the two clauses the gate discarded were the ones carrying the answer. The
substantive clause was the innocent one and the offending clause was the afterthought, which
is the inverse of what the "measurement command" framing suggests.

So the cost is not only a wasted round-trip: it is a wasted round-trip **on the class of
command where latency is the whole point**. CLAUDE.md § *Reaching a Peer Session* prescribes
checking tree state before peer routing is load-bearing, and this gate taxes exactly that
check, because a peer-coordination probe naturally mixes `git` plumbing (bounded, allowed)
with a source-file content read (refused). **Acceptance addition:** one probe row of the
`git … && grep … src/*.rs` shape, asserting the refusal names the `grep` clause — the
existing three probes are all single-purpose and none mixes an allowed `git` clause with a
refused source read, so none of them exhibits the discarded-innocent-clause cost this row
would pin.
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
