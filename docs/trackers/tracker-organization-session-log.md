---
status: active
title: Session Log — Tracker Organization (lifecycle folders, per-entry ledgers)
tags:
- session-log
- tracker-organization
- lifecycle
topic: tracker organization
entry_prefix:
- F
- W
entry_high_water_F: 6
entry_high_water_W: 6
---
# Session Log — Tracker Organization

> **Work stream:** reorganize `docs/trackers/` into lifecycle folders (layer 1) and topic or
> unit folders (layer 2); every ledger becomes one file per entry with issues-style identity
> (date+slug stem is the key, no sequential number). Brainstorm started 2026-11-11.
> Measurements: `tracker-hygiene-log` HY-34 (temp/ policy, parked) and HY-35 (why trackers go
> stale). Identity layer: `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md`
> (owned by a parallel session).
>
> **Exit:** archive when the design is implemented and the migration is done, or when the
> work stream is dropped. Record which one.

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries with:
>
> ```
> doc(action="append_entry", id="<artifact id>", id_prefix="F",
>          anchor_heading="## Template for new entries",
>          title="<one-line title>", body="**Observed:** ...",
>          index_row="| {id} | <date> | <sev> | <cat> | open | **<title>** — … |",
>          index_after_line="<the table line your row goes AFTER>")
> ```
>
> One call, one write — **and the index row belongs in that same call.**
> The server allocates the next id, formats the heading as `## F-N —
> <title>` (the only shape `link_scan` accepts as a definition), records
> the ledger's high-water mark, stamps `**Valid:** dated <today>` unless
> your body declares a class, and writes `index_row` with `{id}` replaced
> by the id it just allocated. The indexes are the eval surface, the
> sections are the evidence.
>
> **A row needs an anchor from SOMEWHERE, and there are two sources.**
> Best, once per ledger: declare `snapshot_anchor` in this file's own
> frontmatter, naming the index table's header line verbatim —
> `doc(action="update", id=…, patch={"extra": {"snapshot_anchor": "| ID |
> Date | … |"}})`. After that, `index_row` **alone** places every row at
> the block's **TAIL**, which is where a newest-last ledger wants it, and
> nobody chooses an anchor again. Otherwise pass `index_after_line` per
> call; it WINS when both are given. They are **not** symmetric —
> `index_after_line` with no `index_row` is refused, and passing neither
> anchor with a row is refused by name.
>
> **Choosing that per-call anchor fails silently in both directions, which
> is the reason to prefer the declaration above.** It inserts after the
> **first** line equal to what you pass: a non-unique line writes your row into the wrong
> table, and a line that does not exist writes nothing at all and
> allocates no id. Neither raises an error. **Prefer the target table's
> last existing row** — unique by construction, because ids are — over
> the `|---|---|` separator, which anchors to the table's **top** and is
> repeated 9 times in one live ledger. Top is right for a newest-first
> ledger like `bug-fix-session-log.md` and wrong for the oldest-first
> ones; the corpus does not agree, so your ledger decides. A declared
> `snapshot_anchor` sidesteps this whole paragraph.
>
> **Do not hand-allocate ids, and do not pre-write index rows.** A max-id
> is a fact about an instant, and a peer session in the same checkout can
> take the number between your scan and your write. Pre-written rows are
> worse: the allocator counts an id claimed by an index row, so rows
> written ahead of their sections consume the ids they name — which is why
> codescout's `statement-validity-session-log` starts at `statement-validity-session-log:F-2`/`statement-validity-session-log:W-3`
> rather than `statement-validity-session-log:F-1`/`statement-validity-session-log:W-1` (see `statement-validity-session-log:F-3` there).
>
> **`index_row` is not a pre-written row, and that warning does not reach
> it.** The server substitutes `{id}` after allocating, in the same write
> as the section, so no instant exists at which the row names an id the
> section does not. The warning is about a row you type yourself ahead of
> the append — that one still consumes the id it names.
>
> **A copy of this file is a ledger from its first moment.** The frontmatter
> above declares `entry_prefix: [F, W]`, which is why the recipe at the top
> is accepted on a fresh copy with nothing to declare first — and, because
> a declared file is a ledger, the librarian guard refuses direct edits to
> it from the start (`edit_file` says so by name). Retitle it and edit its
> prose and index tables with `doc(action="update", id=<the copy's artifact
> id>, patch={body_edits: [...]})` once the copy is catalogued; add entries
> only with `append_entry`, never by hand.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (`docs/conventions/`, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-11-11 | med | stale-claim | open | **frictions/ is not one-file-per-entry** — FR-N are defined in index.md; only issue-clusters/ has the shape |
| F-2 | 2026-11-12 | med | stale-claim | mitigated | **A plan and its spec claimed three things the code contradicts** — "as it does today", a one-finding expectation, and "cannot be shown failing" |
| F-3 | 2026-11-14 | med | stale-claim | open | **"L-B2 is blocked until identity Plan C exists" named the wrong plan** — the key-addressed tools need the Plan E flip; every doc action still takes a 16-hex id |
| F-4 | 2026-11-14 | med | process-gap | open | **Ten green task reviews, red gate** — the read-door scanner flags a write-only `OpenOptions` and runs only in the full gate |
| F-5 | 2026-11-14 | med | stale-claim | mitigated | **The routing rule "place(rel_path) gives Role::Entry" held only for rows directly under a managed root** — a nested-worktree row skipped the lifecycle route silently; fixed for Unix, Windows parked |
| F-6 | 2026-11-15 | med | process-gap | mitigated | **A second gate-only test missed by six green task reviews** — `result_caps` wants a `cap-class` line on `KEY_HOT_CAP`; per-task reviews run no suite (see F-4) |
| F-<n> | YYYY-MM-DD | low/med/high | <category> | open | <one-line title> |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-11-11 | med | read the gate before writing the surface | guide naming docs/templates/ would fail no_guide_names_a_path_or_field_only_this_repo_has | validated |
| W-2 | 2026-11-11 | med | measure the repo under the new model with an ignored census test before building tools on it | a migration sized from the estimate of about 76 trackers would have missed 81 flat files plus 261 in subfolders, and the loose temp tracker would surface only at migration time | validated |
| W-3 | 2026-11-11 | med | measure the ledger before ruling on two spec ambiguities | a 14-day default grace would have made doctor report 69 stale terminal entries on day one against a ledger swept about monthly | validated |
| W-4 | 2026-11-12 | med | an Opus plan review with the code open, before the first dispatch | a correct implementation would have failed the plan's own first check test, and a behaviour change of an existing check would have shipped unannounced | validated |
| W-5 | 2026-11-14 | med | a whole-branch Opus review after ten task reviews, with the deferred Minors and the Review Focus in the brief | I-1 would have shipped as a silent skip of the lifecycle route for nested-worktree rows, and I-2 as a lost body edit on `extra:{}` | validated |
| W-6 | 2026-11-15 | high | an Opus plan review with the code open before the first dispatch, and ranking fixtures whose two dates are set independently | the cap would have hidden the stale entries, three tests would have passed against a sort by `created`, and a huge `stale_after` would have panicked the hook's command | validated |
| W-<n> | YYYY-MM-DD | low/med/high | <pattern> | <what-would-have-happened> | open |

---

## Promotion status

**Audited:** <YYYY-MM-DD>, against the target surface itself — opened and read,
not recalled.

One line per `W-N` (and any `F-N` with a `Fix idea` bound for a permanent
surface). Check the **target**, not the entry: a `Promote-when` that fired is
invisible from inside the tracker, because `Status: validated` reads as healthy
either way. Record one of:

- **already promoted, no action** — quote the promoted text verbatim and name
  where it landed, so the next reader verifies instead of re-deriving.
- **UNFIRED, carried forward** — restate the criterion and the current datapoint
  count.
- **FIRED but not yet applied** — the one that leaks. Name the exact target
  surface and the exact text to add. This is an action item, not a note; set the
  entry's `Status:` to `promotion-due` so a query can find it.

> ⚠️ **Name every instance of the target, not the target's type.** This machine
> runs three Claude Code profiles (`~/.claude`, `~/.claude-sdd`,
> `~/.claude-kat`), each with its own `CLAUDE.md`. An audit that concluded
> *"not found in the user's global CLAUDE.md"* — singular — led to a promotion
> that reached one file of three on 2026-08-18. The session that found the gap
> was running on a profile **without** the rule, and applied it only because
> another profile's copy happened to be injected as project instructions. Three
> files that should be byte-identical have an md5; compare them.

> ⚠️ **For an INSTALLED artifact the target is the SERVING copy — not the repo
> source, and not the other copies.** Measured 2026-08-20: three rules promoted
> into a plugin skill were byte-identical across all three profile caches *and*
> stale against source, because the commit never bumped the version the cache is
> keyed on. Comparing the copies to each other reads **green** there — only
> comparing each copy to the claim catches it. And the session that made the edit
> is the **least representative observer**: its own reload resolved the skill from
> the repo source, so the confirming evidence sitting in front of it was evidence
> about the wrong artifact.

> ⚠️ **Anchor on a back-citation, not a verbatim quote.** A quote goes red when the
> promoted rule is legitimately reworded — a false positive produced by the
> promotion working as intended, observed 2026-08-20 when `codescout:R-89`'s bullet was
> rewritten and the tracker's stored quote had to be edited to match. The durable
> form is the promoted text citing its own entry id —
> *"(codescout:R-1 + codescout:R-7 in codescout's `docs/trackers/reconnaissance-patterns.md`.)"* — so
> verification is a `grep` for the id and survives every rewording. Keep the quote
> as a reading aid; do not make it the predicate.

Run this when the work stream wraps, **and** whenever a criterion fires
mid-stream — an audit that only happens at archive time is one that happens
after the lesson was needed. Prior art:
`eduplanner-ui:docs/trackers/archive/calendar-insight-panel-session-log-2026-08-18.md`, whose
audit correctly caught its own `calendar-insight-panel-session-log-2026-08-18:W-4` as fired-and-unapplied and named the exact
text to promote.

## Category conventions

Use a short kebab-case category to group similar frictions. Prior
sessions have used:

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

Add a new category by writing it as a kebab-case string; no central registry needed.

---

## F-N entry template

Pass this block as `append_entry`'s `body` (without the `## F-N — <title>`
line — the server writes the heading from `title`). Put the matching Index
row in the same call, as `index_row` (see *How to use* above); writing it
afterwards is the second write that call exists to remove. Do not allocate the id
yourself.

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates>

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Pass this block as `append_entry`'s `body`, with `id_prefix="W"` — F-N and
W-N have separate counters. A win without a **Counterfactual** is marketing
— name what would have happened without the pattern, with at least one
piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (`docs/conventions/<topic>.md`, ADR, etc.)>

**Promoted-to:** <surface + section, one per line, line-start — omit until it lands>

**Status:** validated | promotion-due | promoted-to-permanent-docs | archived

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates>

---
```

---

## Status vocabulary

Codified so the Index column means the same thing across sessions.

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. (`fixed` alone is too weak — verification is part of the status.) |
| `promoted-to-bug-tracker` | Moved to a formal tracker (`docs/issues/*`, `docs/TODO-*`, GitHub issue). The session log keeps the pointer; the formal tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. Do NOT close — its job is to remain comparable. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. Default for entries with evidence. |
| `promotion-due` | `Promote-when` has **fired** and the text is not yet on the target surface. An action item, not a resting state. Exists because `validated` cannot distinguish "criterion not yet met" from "criterion met, nobody harvested it" — and both read as healthy, which is how a lesson sits unpromoted while the failure it describes recurs. |
| `promoted-to-permanent-docs` | Moved into a `docs/conventions/` page, an ADR, a skill, or another permanent surface (a `CLAUDE.md` line only when it passes that file's residency test). Session log keeps the pointer — and, for a multi-instance target, names every instance it landed in. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — I claimed frictions/ is one file per entry; its 16 FR-N are defined in index.md

**Valid:** dated 2026-11-11

**Severity:** med · **Category:** stale-claim · **Status:** open

**Observed:** During the brainstorm I told the operator, and the parallel identity session
(message 2026-11-11), that `docs/trackers/frictions/` and `docs/trackers/issue-clusters/`
"already have one file per entry". The scout shows that only `issue-clusters/` has that shape:
each `IC-N-<slug>.md` defines its own `## IC-N — <title>` heading (for example
`IC-3-declared-not-wired.md` line 14). `frictions/index.md` defines all 16 `FR-N` entries as
headings inside the index (`grep -cE '^#{2,3} FR-[0-9]+ [—–-] '` → 16), and the files beside it
are evidence files ("Ledger entry: FR-5 in index.md"). Their names carry no `FR-N`, although
the index says they live at `FR-N-<slug>.md`.

**Cost:** The identity spec (280e77b90, § Key rule) uses `frictions/FR-3-<slug>.md` as its
example of a per-entry ledger folder. The rule it derives (a folder key owns the tokens of every
file inside the folder) is still right for the planned layout, but the example describes a
shape that does not exist yet. If a builder used `frictions/` as a fixture, the test would not
match reality.

**Fix:** Tell the identity session that `frictions/` is index-defined today, and that
`issue-clusters/` is the real per-entry example. In the migration, `frictions/` must move its
16 definitions out of `index.md` into the entry files.

## W-1 — The guide gate allows only server-defined paths, so the lifecycle layout must be a server convention

**Valid:** dated 2026-11-11

**Impact:** med · **Pattern:** read-the-gate-before-writing-the-surface · **Status:** validated

**Scout:** The operator wants the lifecycle system "promoted in get_guide". Before drafting any
guide text I read the test that constrains guides. `src/prompts/mod.rs`
`no_guide_names_a_path_or_field_only_this_repo_has` refuses, in EVERY guide, the needles
`docs/templates/`, `docs/conventions/`, `docs/issues`, `docs/superpowers/` and others. Paths
the server itself defines are allowed: the test's own doc comment names `docs/trackers/` and
`docs/augmentations` as constants in `src/librarian` (`LEDGER_DIR` in
`src/librarian/tools/append_entry.rs:3772`, `SIDECAR_DIR` in `augmentation_sidecar.rs:33`).
`tracker_conventions_names_no_repo_specific_artifact_kind` also refuses `docs/issues`,
`_TEMPLATE`, `kind: bug` and "bug file" in `tracker-conventions`.

**Consequence for the design:** the guide can teach the lifecycle layout by name only if the
server defines it: the lifecycle folder names under `docs/trackers/` and the `lifecycle:` key
become server constants that doctor checks. Per-kind templates stay in the repo, and no guide
names their path. A repo-specific procedure goes to the repo's own template.

**Counterfactual:** A guide section that pointed to `docs/templates/tracker-<kind>.md`, or that
said "like docs/issues", would fail the gate at the next run. The design would then have to
be reshaped after the plan was written.

## W-2 — A census of this repo under the new model gave the migration its worklist before any tool existed

**Valid:** dated 2026-11-11

**Observed:** 2026-11-11, building Plan L-A (the lifecycle core). Task 7 added an ignored test that walks `git ls-files '*.md'`, builds a `LifecycleTree` and prints counts. Run at tree `d5e6abc96e7a76df9a956b8b27321cc40096396c` on branch `feat/tracker-lifecycle-plan-a-20261111`: 0 collections, 0 entries, 1 placement problem, 81 flat `docs/trackers/*.md` files and 261 files in other `docs/trackers/` subfolders (343 documents in all under `docs/trackers/`).

**Pattern:** measure the repo under the new model with a throwaway test before any tool or migration depends on the model, and keep the test as an `#[ignore]`d census so the figure can be re-taken.

**Counterfactual:** the plan carried the estimate "about 76 legacy trackers"; the census counted 81 flat files plus 261 files in subfolders, so a migration sized from the estimate would have missed more than two thirds of the files. The one placement problem is the first temp tracker (`docs/trackers/temp/artifact-identity-refactor.md`): a loose file in `temp/`, outside any collection folder, which the model refuses. Without the census it would have surfaced at migration time, after the design was frozen.

**Confirming data points:** the same census shape found the identity session's figures (45 invalid keys and 37 duplicate-key groups over 2704 documents), so both migrations are sized from measurement; the final whole-branch review found that the lifecycle checks did not report a loose file in `temp/`, which the census had already shown as the only misplaced file.

**Impact:** med

**Promote-when:** the migration command (plan L-E) exists and reuses the census as its dry run.

**Status:** validated

**Rests on:** a design is checked against the real corpus before tools are built on it.

---

## W-3 — Measuring the ledger before ruling on two spec ambiguities gave the grace default a number

**Valid:** dated 2026-11-11

**Observed:** Plan L-B left two ambiguities for the operator: whether `terminal_in_live_dir` fires for terminal entries of a live stream, and whether `cited_target_closed` fires for blocked and deferred entries. Asked to "measure first", a throwaway script read `docs/issues` (1,641 files) as a ledger on 2026-11-11, with `fixed`, `wontfix`, `done`, `superseded` and `archived` as terminal.

- 212 terminal entries sit outside `archive/`, all with a `closed:` date. 69 are older than 14 days, 1 older than 30, 0 older than 90.
- No terminal entry lacks `closed:`.
- 135 `mitigated` entries carry `closed:` and 50 sit in `archive/`. In this ledger `mitigated` is a finished state, so a readme mapping it to `terminal` removes them. 16 entries with `zombie`, `investigating`, `open` or `taken` plus `closed:` are real `ClosedOnNonTerminal` inconsistencies.
- No stream collection exists yet, so the stream question has no data. Only 3 files in `docs/issues` have a `**Rests on:**` line and none is in `[[link]]` form, so the relation question has no data either.

**Ruling:** the default grace becomes 30 days (69 findings at 14 days against 1 at 30). `terminal_in_live_dir` applies to ledgers only, because a stream is archived whole and `stream_all_terminal` covers it. `cited_target_closed` keeps covering blocked and deferred entries, because a closed target is the signal to unblock. The spec was amended in `1ad993e5a`; the code follows in the next commit.

**Counterfactual:** keeping the 14-day default would have made `doctor` report 69 stale terminal entries on day one against a ledger that is already swept about monthly, and the first migration would have trained everyone to ignore the check.

**Limits:** one corpus, one date, and a status-word mapping chosen by hand. The two spec rulings rest on the spec text, not on data.

## F-2 — A plan and its spec claimed three things the code contradicts

**Valid:** dated 2026-11-12

**Observed:** I wrote the lifecycle-cleanup spec (`f01327c4f92f9a72`) and plan (`011a4f5022eea4c0`) after reading `LifecycleTree::build`. An Opus plan review then found three places where the text disagreed with the code. I checked each one myself.

- The spec said that when the first readme of a folder is broken, "the collection fails as it does today". It does not. A broken first readme pushes a `Frontmatter` or `Schema` problem and inserts the folder into `failed`, but a later good readme still reaches `collections.entry(dir).or_insert(..)`, builds the collection, and its entries get every entry check. The change in Task 1 therefore alters existing output.
- The plan's first check test expected one `CollectionInvalid` finding for two good ledger readmes. `testkit::LEDGER` declares `review_every: 30d` and the kept collection has no entry, so `check_collection` also emits `LedgerQuiet` (`(None, _) => true`). A correct implementation would have failed that test.
- Task 4 said the locale test "cannot be shown failing against a German git". On this machine git's German catalog is installed: `LANGUAGE=de LANG=en_US.UTF-8 git -C / rev-parse --show-toplevel` prints `Schwerwiegend: Kein Git-Repository ...`, and `LC_ALL=C` makes git ignore `LANGUAGE`.

**Cost:** each would have cost at least one implementer retry; the first would also have shipped an undisclosed behaviour change.

**Resolution:** spec and plan rewritten and committed as `a8475d2a9`. Ruling R1 in the SDD ledger names the behaviour change for the operator.

**Cause:** I described what `build` does for the usual case (a good first readme) and did not trace the broken-first path I had already read.

## W-4 — An Opus plan review before dispatch caught three false claims in the plan and spec

**Valid:** dated 2026-11-12

**Observed:** Before dispatching the five-task lifecycle-cleanup plan, an Opus reviewer read the spec, the plan and the code it touches, and checked each claim of the plan against the code. It reported 1 critical, 5 important and 7 minor findings. I verified the critical one and two of the important ones by running the code or the command (F-2 lists them). The plan was rewritten before any implementer started.

**Counterfactual:** the first implementer would have met a plan test that a correct implementation fails (`LedgerQuiet` is also emitted), and would have had to retry or bend the test. The spec's false "as it does today" would have shipped a changed behaviour of an existing check with no mention to the operator. The locale test would have been pinned by a probe that falls back to C and prints English, which suggests git is unaffected when it is not.

**Limits:** one plan, one reviewer, five tasks. The reviewer's other findings (a missing test of the duplicate key, the gate step, a three-readme case) were not verified one by one; they were taken as plan text and will meet their own reviewers at task time.

## F-3 — The note "L-B2 is blocked until identity Plan C exists" named the wrong plan: the key-addressed tools need Plan E

**Valid:** dated 2026-11-14

**Observed:** The parked-items memory said Plan L-B2 (the lifecycle tools) was blocked until identity Plan C exists. Plan C1 has landed (`7db510676`, in `experiments`). I read the code and the identity spec before planning L-B2:

- Every `doc` action takes a 16-hex `id` (`get.rs`, `update.rs`, `mv.rs` and the others each name `id="<16-hex>"` in their arg hint). None takes a key.
- The L-B2 tools in the lifecycle spec (§ Tools) address a collection by key, and the identity spec forbids a hex id in tool output.
- The identity spec's plan order puts "the read and write flip, `doc(move)` as a rename" in Plan E, not in C. The C1 execution-state doc lists N-5: `doc(move)` on a stem rename leaves `[[oldkey]]` cites stale.
- C1 does give the lifecycle layer a safe base for writes: dual-write of entries and cites, the CLI write lock, and `rekey_prefix` that refuses grammar citers.

**Expected:** that the end of Plan C unblocks all of L-B2.

**Cost:** I nearly planned L-B2 against an interface that does not exist. An L-B2 built on hex ids would need re-addressing at Plan E.

**Rests on:** the arg hints in `src/librarian/tools/*.rs`, the identity spec's "Plan order" paragraph and the C1 execution-state doc, all read on 2026-11-14.

**Fix:** correct the memory. The decision on how to sequence L-B2 against Plan E goes to the operator.

**Follow-up (2026-11-14):** the addressing-free write rules now exist as pure library functions in `src/librarian/lifecycle/write.rs` (merged, spec `0904b912ffaffdf6`). Only the wiring to `doc` waits on Plan E. The open design inputs for L-B2 are in that spec's § Open for the L-B2 plan.

**Follow-up 2 (2026-11-14):** tools part 1 is built on the id-addressed `doc` tools (spec `afc72d5722d1d601`, plan `67870a11f0ed7a0e`, landed on `integration/identity-lifecycle-20261114` at `18e0ad3c2`, not on `experiments`): `doc(create)` for an entry and a collection, `doc(update)` on an entry, `doc(get)` table on a readme. The key-addressed form and `doc(move)` as a key rename still wait on Plan E. See F-4, F-5 and W-5 for what the build taught.

## F-4 — Ten per-task reviews were green and the gate was red: a test that only the full gate runs caught a write-only OpenOptions

**Valid:** dated 2026-11-14

**Observed:** Tools part 1 (plan `67870a11f0ed7a0e`) ran ten tasks, each with an Opus task review that passed. The first full gate on the unmerged branch (`3c74bb536`) was red in both nextest lanes with one failure: `librarian::read_door::tests::no_librarian_production_read_bypasses_the_read_door`. The scanner matches the path `OpenOptions::new` and cannot tell a write from a read. Task 6's exclusive create (`create.rs`, `.write(true).create_new(true)`) tripped it. The fix was one `ALLOWED` entry in `read_door.rs`, the same shape as the existing `workspace.rs` and `catalog/audit/shard.rs` entries.

**Expected:** that ten green task reviews, each with focused tests run by the implementer, left only integration issues for the whole-branch review.

**Cost:** one fix-wave item and one extra gate cycle (about 7 minutes of gate time plus the dispatch). No code reached `experiments` or `integration` red. The task reviewers were told not to re-run the suite, so no per-task review could have seen it.

**Rests on:** the gate output (`GATE EXITS -> FMT=0 CLIPPY=0 LEAN=100 DEFAULT=100`, one FAIL line per lane, panic text at `read_door.rs:1500`), read on 2026-11-14.

**Fix:** an implementer who adds an `OpenOptions`, `fs::read` or `File::open` call under `src/librarian/` should run the read-door test alone at the task, not at the gate. `scripts/with-slot.sh cargo test --lib no_librarian_production_read_bypasses_the_read_door` takes about two seconds. Not yet written into the implementer brief template.

## W-5 — A whole-branch Opus review after ten task reviews found the two cross-task defects the task reviews could not see

**Valid:** dated 2026-11-14

**Pattern:** After the last task review, one Opus reviewer read the whole branch (21 commits) with the ledger's deferred Minors and the plan's Review Focus. It found two Important issues. Each one lives across tasks, so no task-scoped review could see it.

- I-1: a main-checkout session that updates an entry (or reads a collection table) whose catalog row sits in a nested worktree (`<main>/.worktrees/x/docs/trackers/...`) made the relative path from the first containing root, `place()` returned None, and the update took the generic path silently. It combined Task 9's route with Task 10's route and the write door's `door_root`.
- I-2: `patch={extra:{}, body_edits:[…]}` on an entry was refused as an empty update. The same patch worked before Task 9. The reviewer also triaged the 30 deferred Minors into fix and park, and ruled on the one the controller flagged.

**Counterfactual:** without the whole-branch pass, I-1 would have shipped as a silent skip of the lifecycle route. It becomes live as soon as Plan D creates entries in worktrees. I-2 would have shipped as a regression for entries: an agent sending an empty `extra` would lose its body edit and read a false message. The fix wave (6 commits, one scoped re-review) cost about 40 minutes.

**Rests on:** the final review report and the re-review report, both read on 2026-11-14; the fix wave commits `d37c2756b`..`fedc27ccc`.

## F-5 — The plan's routing rule held only for rows directly under a managed root: a nested-worktree entry skipped the lifecycle route

**Valid:** dated 2026-11-14

**Observed:** The plan's routing rule was "doc(update) routes by `layout::place(rel_path)` giving `Role::Entry`". The implementation derived `rel_path` from a catalog row's `abs_path` and the first containing managed root, `managed_roots` lists the session's git root first. For a row in a nested worktree (`<main>/.worktrees/x/docs/trackers/<collection>/<entry>.md`; this repo nests its worktrees under `.worktrees/`) the relative path became `.worktrees/x/docs/trackers/...`. `place` needs the path to start with `docs/trackers/`, so it returned None and the generic path ran, which wrote an undeclared status with no `status_since`. The write door did not stop it: `door_root` narrows to the nearest `.git`, and a worktree's `.git` file counts. The Task 9 review found the Windows spelling form of this class (`//?/C:/..` against `\\?\C:\..`) and fixed that one. Task 10 reused the helper. The whole-branch review found the nested-worktree form.

**Expected:** that a lifecycle-looking file always takes the lifecycle route or is refused loudly (the Task 9 ruling against silent fallback).

**Cost:** no data lost, and no worse than before the branch. It would have become live when Plan D creates entries in worktrees. The fix (`5bfeeee11`) narrows the root with `util::door_root` when the first root does not place but a nearer repository does. Two limits remain: a lifecycle-looking path with no `.git` between the root and the file still takes the generic path, and on Windows the narrowing probably does not apply (same spelling problem; unverified, no test).

**Rests on:** the final review report (I-1), the re-review report (F2 verdict and Minor 3), `src/librarian/tools/mod.rs` `root_and_rel_path` and `util.rs` `door_root`, read on 2026-11-14.

**Fix:** done for the Unix case. The Windows case is parked in the parked-items memory `tracker-lifecycle-parked-items` (§ Parked from tools part 1).

## F-6 — Six green task reviews, red gate again: a cap-class scanner flagged KEY_HOT_CAP, the second test that only the full gate runs

**Valid:** dated 2026-11-15

**Observed:** The collection hot view branch (plan `docs/superpowers/plans/2026-11-15-collection-hot-view.md`) ran six independent Opus task reviews, all green. The first full gate (`./scripts/gate.sh`) on head `8d9b3402b` was red in both Rust lanes: `tests/result_caps.rs every_cap_constant_is_classified` — `src/librarian/lifecycle/schema.rs:159 KEY_HOT_CAP — no cap-class annotation`. A scanner over every constant in the repository wants a `// cap-class: RESULT_CAP …` or `// cap-class: NOT_A_CAP — <reason>` line on a constant whose name looks like a cap. `KEY_HOT_CAP` is the name of a readme key, not a bound. Python (1836), fmt and clippy were green.

**Cost:** One red gate run (7 minutes) and a place in the single fix wave. No wrong code shipped: the gate caught it before the landing. Same class as F-4 (the read-door scanner): a check that only the full gate runs, which no per-task review or per-task test command exercises.

**Fix:** One annotation line (fix wave, `356803a85`). The structural gap stays: per-task reviews run no suite. A task that adds a constant, a file read or an `OpenOptions` should run `scripts/with-slot.sh cargo test --test result_caps` and the read-door test before its review.

**Rests on:** gate output `@bg_00000032` of 2026-11-15 (lean and default lanes, one failure each); the green re-run `GATE EXITS FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0` on `436cafd19` and `5d655bfef`.

## W-6 — An Opus plan review before the first dispatch found six Important defects, among them tests that could not fail

**Valid:** dated 2026-11-15

**Observed:** Before the first dispatch of the collection hot view plan, an independent Opus reviewer read the plan against the code at `fddfe01f2` with the codescout tools open. It found 0 Critical and 6 Important issues, all folded into the plan before any task started: (1) the rank rule (R3) made the cap hide the oldest entries first, which are exactly the stale and long-parked revisit-due ones; (2) three Task 5 tests could pass with wrong code, because `table::rows` already sorts by `created` and every entry helper in the tree writes `status_since` equal to `created`; (3) `hot_view(tree, key)` was ambiguous on a tree with two nodes of one key and rendered `0 of 0 hot` for an archived collection; (4) the stale rule `status_since + n days < today` panics on `stale_after: 4294967295d`, which the parser accepts; (5) the exit-code contract the hook relies on had no test of the built binary and left the root-resolution failure at exit 1; (6) the `--json` shape, `HotReason::RevisitDue` as a struct variant, and the JSON failure text were not pinned. It also corrected two code facts (`key_as_str` was already `pub(super)`; a second `EntryRow` literal in a test).

**Pattern:** An Opus plan review with the code open before the first dispatch, plus a fixture rule it produced: set the two dates of a ranking test independently, so that the wrong order and the right order do not coincide.

**Counterfactual:** The cap would have hidden the stale entries it exists to surface; the order, cap and stale tests would have gone green against a sort by `created`; a readme with a huge `stale_after` would have crashed the session-start command; the hook would have met exit 2 with empty stdout under version skew with a doc that promised a reason. The whole-branch review later found the last one anyway (the doc was still false), which shows the plan review did not replace it.

**Rests on:** the review report of 2026-11-15 (read against `fddfe01f2`); the plan's § Self-review notes and § Outcome; the later whole-branch review (second occurrence of the pattern of W-5).

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     doc(action="append_entry", id="<artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...",
              index_row="| {id} | <date> | <sev> | <cat> | open | **<t>** — … |",
              index_after_line="<the table line your row goes AFTER>")

     The server allocates the id, writes `## F-N — <title>` at the ledger's
     own level, records the high-water mark, stamps `**Valid:** dated
     <today>` AND writes the index row with `{id}` filled in — one write.
     A row needs an anchor from SOMEWHERE: `index_after_line` per call, or
     a `snapshot_anchor` declared once in this file's frontmatter, after
     which `index_row` alone lands the row at the block's TAIL. Passing
     `index_after_line` with no `index_row` is refused; they are not
     symmetric. A per-call anchor must be UNIQUE (first match wins,
     silently) and decides top vs bottom — prefer the last existing row.
     Do not hand-allocate ids; do not type the row yourself ahead of the
     call. -->
