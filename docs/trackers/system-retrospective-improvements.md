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

## T-7 — Committed audit shards (phase 2 of T-1)

**Why:** the audit WAL (T-1) is machine-local like everything else in the catalog — the same liability class it exists to make forensic. Exported to committed per-host JSONL shards, one machine can answer "which session on the other machine deleted these rows".

**Shape:** `audit_log export` → `.codescout/audit/<host>-<YYYYMM>.jsonl`, append-only, `merge=union` gitattribute (different hosts write different files; same-host branch merges union cleanly — global order derives from `(host, seq, at_ms)`, never line position). Watermark `audit_exported_through_seq` + doctor's unexported-delta report keep the replica honest: a committed log must never read as complete when it is only as fresh as its last export (IC-13). Export folds into `reindex`/`merge_worktree` so it cannot be forgotten (IC-3); reindex churn filtered from export by default. Precedent: augmentation sidecar (`f565504a`).

**Acceptance:** a row mutated on host A is queryable on host B after pull; same-host two-branch shard merge needs no manual resolution; doctor names the unexported delta; a merged read labels each host's coverage window.

**Depends on:** T-1 (row format designed export-ready there: `seq` monotone/never-reused, watermark key reserved). Full design: `docs/superpowers/specs/2026-09-01-catalog-audit-trail-design.md` § Phase 2.
## History

### 2026-09-01 — Tracker created
Seeded from the birds-eye retrospective (3-agent sweep: trackers, bug corpus, architecture). T-1 selected as the starting task.

### 2026-09-01 — T-1 design approved; T-7 split out
Brainstorming settled T-1's three axes (any-writer triggers / full-on-delete+diff-on-update / keep-forever) and produced `docs/superpowers/specs/2026-09-01-catalog-audit-trail-design.md`. The git-sharing question raised during review became T-7 (committed audit shards), phased after T-1 with the row format made export-ready up front.
