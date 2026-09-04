# TAXONOMY — ID prefixes used in this repo

A single-page reference for every monotonic-ID ledger in the project. Use it
when (a) appending a new observation and you're not sure which tracker, or
(b) onboarding and trying to navigate the accumulated session intelligence.

The accumulated rules are spread across `CLAUDE.md`, individual tracker
templates, and skill SKILL.md files. This page is the index, not the spec —
follow the links for the controlling convention.

> **Note** — Some trackers below are *augmented artifacts* (the T-N row is
> the canonical example: data in catalog DB params, prose in markdown body,
> auto-synced via a render template). The body/params split changes how you
> append. See [`architecture/augmented-artifacts.md`](architecture/augmented-artifacts.md)
> for the mental model.

## Main taxonomy

A **declared ledger** carries `entry_prefix: <PREFIX>` in its frontmatter (committed,
so it survives a fresh clone) and gets server-assigned ids from
`doc(action="append_entry", id=…, id_prefix=…)`. **Twenty-three are declared today** (re-derived 2026-08-31 by
reading `entry_prefix:` out of every frontmatter block): AA, CAP, CM, CTX, DC, ET, F, FND,
FT, GF, GG, H, HY, **OB**, OP, R, S, SD, SV, TB, U, W, WP.

*Counting rule, stated because a bare integer inherits whichever rule the reader picks
(`observer-blindness:OB-1`):* distinct **prefixes**, not declaration lines. Excluded from
the raw grep are two worked examples inside `tracker-conventions.md`'s fenced blocks, one
`entry_prefix: null` (a tracker deliberately owning no namespace), and the six bare
`entry_prefix:` block-sequence headers — those contribute F and W, which are counted once
each rather than seven times.

*(This sentence read "Thirteen" until 2026-08-26 and "Fifteen" until 2026-08-28, and was
**exact when measured** each time — `CTX` was declared on 2026-08-21 by `711a25cf`, two
days after the first count, and nothing re-counted. Recorded as `claim-decay:DC-3`; a
count written into prose owns no repair trigger, so re-measure it whenever you add a
prefix. **The 2026-08-28 re-measure confirms the pattern a third time:** five resume-queue
prefixes were added that day, which would make fifteen-plus-five = twenty — but the real
count is twenty-one, because `OP` had been declared by the operator-rules stream in the
interim and, once again, nothing re-counted. Do not increment this number; re-derive it.
**Fourth firing, and it is the one to remember: seven minutes.** The corrected value
`twenty-one` was committed at 13:20 on 2026-08-28 in `068281af`. It was already false —
a concurrent session in the same checkout had declared `CM`
(`docs/trackers/resume-cross-machine-catalog-restore.md`) at **13:13**, seven minutes
earlier, and the measurement behind `068281af` was taken just before it. Corrected to
twenty-two at 13:25. No amount of care at write time fixes this: the number was true when
measured, false when committed, and nothing in between could have known. That is the
whole argument for re-deriving on read rather than trusting the figure — and for keeping
the count next to the command that produces it.)*

> ⚠️ **F and W now appear among the declared prefixes, which contradicts the F/W row
> below.** One session log declared `entry_prefix: [F, W]`. That is a real, unresolved
> contradiction between this document and the corpus — not a typo here. Decide it
> deliberately before adding more: declaring F/W makes them dangling-checked but gives one
> token many definers; leaving them undeclared keeps ambiguity. Measured 2026-08-19 across
> 10 repos in 2 umbrellas: **33% of cross-file entry citations are ambiguous** and F/W are
> the dominant contributors (`F-1` alone has 169 definers). codescout at 28% is among the
> healthiest — this is a property of the per-file `PREFIX-N` convention, not a local defect.

| Prefix | Lives in | Captures | Append tool | Promotes to |
|---|---|---|---|---|
| **F-N** | `docs/trackers/<topic>-session-log.md` | Per-work-stream friction observation: plan-vs-reality drift, surprise tool behavior, blocker | `doc(action="append_entry", id=<log's artifact id>, id_prefix="F", anchor_heading="## Template for new entries", title=…, body=…)` — one call writes the section and records the high-water mark; add the Index row *after*, with the returned id. **Deliberately NOT a declared ledger.** F/W are namespaced per work stream, so each of the 8+ live session logs owns its own `F-1..F-N`. Declaring them would give one token eight definers, and the resolver reports Ambiguous rather than resolving — which is already happening (`W-13` has two). **Decided 2026-08-17 — keep per-log numbering, cite qualified.** `link_scan` now resolves `<file-stem>:F-33` to that log's entry, so write `bug-fix-session-log:F-33` and not a bare `F-33` whenever you cite an F/W entry from outside its own log. No renumbering, no lost attribution. See **HY-10** | Bug file (`docs/issues/`) if reproducible; W-N pair if pattern caught it next time |
| **W-N** | Same file as F-N | Per-work-stream win: a discipline / scout / pattern that prevented a worse outcome with named counterfactual | Same | CLAUDE.md / ADR / skill SKILL.md after 2+ confirming datapoints |
| **R-N** | `docs/trackers/reconnaissance-patterns.md` (declared ledger, artifact `5696563f06b2c222`) | Meta — observations about the **recon skill itself**: hits, misses, vocabulary expansions | `doc(action="append_entry", id="5696563f06b2c222", id_prefix="R", anchor_heading="## Template for new entries", title=…, body=…)` — no `entry_collection`, because entries are `## R-N` body sections. **One call**: the server assigns the id atomically *and* writes `## R-N — <title>`, the only shape that defines a citable token. (The old two-step — reserve, then write via `body_edits` — is obsolete; an augmentation being present does not change this, per `append_entry.rs`'s `seed_prose` case.) Add the Index row after, with the returned id. Never compute or suffix an id. `edit_file`'s heading grammar is refused: the ledger is augmented | PR against `codescout-companion/skills/reconnaissance/SKILL.md` |
| **U-N** | `docs/trackers/codescout-usage-frictions.md` | Friction using codescout tools / MCP server: tool slips, prompt drift, hook false-positives | `doc(action="append_entry", id="c43df94e69ca915f", id_prefix="U", anchor_heading="## Template for new entries", title=…, body=…)` — no `entry_collection` (prose ledger). **One call**: the server assigns the id and writes the `### U-N — <title>` section at the ledger's own heading level. The old reserve-then-`body_edits` two-step is obsolete. `edit_file`'s heading grammar is refused: declared ledger | H-N hookify rule, CLAUDE.md note, or prompt-surface edit |
| **SKF-N** | `docs/trackers/skill-frictions.md` (declared ledger, artifact `09d45a9c7d7cf203`) | Rough edges found while **using a project skill**: a command that fails, documented behaviour that diverges from reality, a friction that recurs across sessions | `doc(action="append_entry", id="09d45a9c7d7cf203", id_prefix="SKF", anchor_heading="## \`/<skill-name>\`", title=…, body=…)` — no `entry_collection` (prose ledger). **One call**: the server assigns the id and writes the `### SKF-N — <title>` section. Renumbered from `F-NNN` on 2026-09-01 — that scheme restarted per skill section, so 23 entries shared 13 ids (`F-001` had **five** definers) and every one was uncitable | H-N hookify rule, or an edit to the skill's own SKILL.md |
| **H-N** | `docs/trackers/codescout-usage-hookify.md` | Hook design proposal: warn → deny criteria, new gate ideas, false-positive carve-outs | `doc(action="append_entry", id="e522954737601d13", id_prefix="H", anchor_heading="## Template for new entries", title=…, body=…)` — no `entry_collection` (prose ledger). **One call**: the server assigns the id and writes the `### H-N — <title>` section at the ledger's own heading level. The old reserve-then-`body_edits` two-step is obsolete. `edit_file`'s heading grammar is refused: declared ledger | Shipped hook in `claude-plugins/codescout-companion/hooks/` |
| **T-N** | `docs/trackers/tool-usage-patterns.md` (augmented artifact `f2ecdd76a6189efb`) | Tool-selection quality observation: legitimate / debatable / wrong-tool call with prompt gap | `doc(action="append_entry", id="f2ecdd76a6189efb", entry_collection="observations", id_prefix="T", entry={…})` — atomic id. Body prose via `doc(action="update", patch={body_edits:[…]})`. **Never** `doc(action="augment", merge=true, augment={params:{observations:[…]}})`: that REPLACES the array, and took this queue 19 → 1 on 2026-08-16 | `src/prompts/source.md` edits (server-instructions surface) |
| **WIN-N** | `docs/trackers/windows-platform-support.md` (augmented artifact `52451519052d207c`) | Windows-platform issue: process-spawn / lsp / platform-gated / path-handling / build-install / test-portability / companion defect, fix, or cfg-gate decision | `doc(action="append_entry", id="52451519052d207c", entry_collection="issues", id_prefix="WIN", entry={…})` — atomic id, then sync the body `## Issue index` table. **Never** `doc(action="augment", merge=true, augment={params:{issues:[…]}})`: that REPLACES the array | Bug file (`docs/issues/`) for new incidents; `status` flips in place as fixes land |
| **A-N** | `docs/trackers/prompt-hamsa-audit-log.md` (craft-level twin in `claude-plugins/docs/trackers/`) | Prompt-audit record from a Hamsa audit: named gap, recommended move, prediction, confidence, outcome (filled when evidence lands) | Per the tracker's maintenance convention (`## A-N — <title>` section + Index row) | Hamsa SKILL.md heuristic / buddy memory when the finding generalizes |
| **PV-N** | `docs/trackers/provenance-subsystem.md` (augmented artifact `e12cd7e0060ed9b8`) | Provenance/attribution **programme** state: measurement verdicts vs pre-registered kill conditions, standing design decisions, hazards not to rediscover, open decisions, buildable work. Typed `finding \| gap \| decision \| hazard \| task` | `doc(action="append_entry", id_prefix="PV", entry_collection="items", entry={...})` — atomic monotonic id; query with `entry_filter` | Implementation plan (`docs/plans/`) once phase moves past MEASUREMENT; a `decision` flips to `settled` in place |
| **OB-N** | `docs/trackers/observer-blindness.md` (declared ledger, artifact `3922c2a0fd0dfcfc`) | A defect **class** indexed by its observer structure: who structurally cannot see it, who can, and the check that runs whether or not anyone suspects a problem. The unit is a class, never an instance — an instance is a bug file / `F-N` / `R-N`. Admission test: *would a more careful version of the same party have caught it?* If yes it is **not** an `OB`; the test is structural inability, never observed failure. Second required property: the failure returns a **plausible answer rather than an error**, which is why it survives review chains that catch louder bugs | `doc(action="append_entry", id="3922c2a0fd0dfcfc", id_prefix="OB", anchor_heading="## Template for new entries", title=…, body=…)` — prose ledger, no `entry_collection`; one call writes `## OB-N — <title>` and records the high-water mark. Add the Index row after, with the returned id. `edit_file`'s heading grammar is refused: declared ledger | An `H-N` hook proposal or an `I-N` intervention once the mechanism is designable; a CLAUDE.md rule once two or more classes share one mechanism shape |
| **CAP-N** | `docs/trackers/capability-proposals.md` (augmented artifact `01291679a5ee4707`) | **Pre-plan** proposal for a codescout capability we do not have: the ask, a substrate check citing what exists today at `path:line` and what is genuinely missing, and the open decisions. Reflective — judgment, not gathering | Append a `## CAP-N` section above `## Anti-goals` + an Index row, via `doc(action="update", patch={body_edits: [...]})` | A spec + plan under `docs/superpowers/` once it has tasks and a file structure; or `rejected` in place with the reason kept |
| **HY-N** | `docs/trackers/tracker-hygiene-log.md` (declared ledger, artifact `7e498b6dcb45b924`) | Verdict on the tracker-hygiene skill's own detectors: `hit` (a detector fired correctly), `miss` (a defect class no detector sees), `proposal` (a new detector or convention change). Dated `## Sweep YYYY-MM-DD` sections sit alongside and own no id | **Two shapes, and they differ.** An `HY-N` entry: `doc(action="append_entry", id="7e498b6dcb45b924", id_prefix="HY", anchor_heading="## Template for new entries", title=…, body=…)` — prose ledger, no `entry_collection`; one call writes `## HY-N — <title>` and records the mark. A **sweep** entry is dated rather than numbered, so it goes through a plain `body_edits` insert with the `next-sweep-due` frontmatter bump riding along **in the same call**. `edit_file` is refused: declared ledger | A new detector in `codescout-companion/skills/tracker-hygiene/SKILL.md`, or a convention change in `get_guide("tracker-conventions")` |
| **OP-N** | `docs/trackers/operator-rules.md` (declared ledger, artifact `fa21bfb35684794d`) | A rule that holds across every project, tool and model for **this operator** — its text, binding, shape, status and evidence. Spec: `docs/superpowers/specs/2026-08-27-operator-rules-engine-design.md` | `doc(action="append_entry", id="fa21bfb35684794d", id_prefix="OP", anchor_heading="## Template for new entries", title=…, body=…)` — prose ledger, no `entry_collection` | Compiled into each Claude Code profile's `CLAUDE.md` by `codescout operator-rules compile`. A withdrawn rule is **retired in place** with its text kept verbatim (precedent: OP-5, *Conclude Last*, superseded by OP-1), never deleted |
| **BUG (slug)** | `docs/issues/YYYY-MM-DD-<slug>.md` | Per-bug investigation file: Symptom / Repro / Root cause / Fix / Workaround | Create from `docs/issues/_TEMPLATE.md`; status field in frontmatter | Archived to `docs/issues/archive/` once the fix is verified on `experiments` — reaching master is NOT required. Record the SHA **labelled with its branch** *and* its `git patch-id --stable` — the SHA is positional and dies when `experiments` is rebased; the patch-id is a content hash of the diff and survives rebase and cherry-pick. **No pending-master-SHA Resume line, and no later reconciliation** — record both once and the record stays resolvable whichever path the fix takes to `master`. Measured 2026-08-19: 10 of 63 archived files had already lost their SHA to a rebase, while zero patch-ids collided across 3594 commits. Move via `doc(action="move", …)`, never `git mv` |
| **DC-N** | `docs/trackers/claim-decay.md` (augmented artifact `7d813ce54cb9f1c5`) | A durable written claim that decayed where **no process event is scheduled to repair it** — typed `measured-value-drift \| premature-assertion \| stale-pointer \| deferral-rationale \| status-zombie \| freshness-gap \| undecidable-green \| prohibition-false`. Records the *missing trigger*, not the wrong text. Evidence only: the narrative stays in the F-N / R-N / bug file the row cites | `doc(action="append_entry", id="7d813ce54cb9f1c5", entry_collection="claims", id_prefix="DC", entry={…})` for the row, **then** a `## DC-N — <title>` section via `doc(action="update", patch={body_edits:[…]})`. Both writes, always — the row is what `entry_filter` reads, the heading is what makes it citable. **Never** `doc(action="augment", merge=true, augment={params:{claims:[…]}})`: that REPLACES the array | A `librarian(action="doctor")` check, an `audit_doc_refs` severity rule, or a test — flip the row to `check-shipped` via `update_entry` when it lands |
| **IC-N** | `docs/trackers/issue-clusters.md` (declared ledger, artifact `1b5a080fe2efcb6b`) | A **defect class** the bug corpus instantiates, stated as a claim that could be false. Membership is carried by a reserved `cluster/<slug>` tag in each bug file's frontmatter, so a class's instances are a **query** and never a list — that being the failure that retired this repo's hand-maintained bug index in 2026-05-18 and rotted the BL-N queue's *Sequencing notes*, whose two named clusters are now entirely `done`. The unit is a class; an instance is a bug file. Slugs are claim-shaped (`blast-radius-exceeds-visibility`), never topic-shaped (`concurrency`), and each bug carries **exactly one** — multi-membership makes counts non-additive, and the counts drive promotion. Promotion threshold: **≥3 instances spanning ≥2 subsystems**; three in one subsystem is a broken subsystem, not a mechanism | `doc(action="append_entry", id="1b5a080fe2efcb6b", id_prefix="IC", anchor_heading="## Template for new entries", title=…, body=…)` — prose ledger, no `entry_collection`; one call writes `## IC-N — <title>` and records the high-water mark. Add the Index row after, with the returned id. Tag the bug itself via `doc(action="update", id=…, patch={tags:[…]})` or `codescout artifact update <id> --tags …` — **never** a direct frontmatter edit, which never reaches the catalog (BL-48) and so is invisible to every `find` | Decided by the entry's `**Blind party:**` field: `OB` when a party structurally cannot see it, `H`/`I` when it needs a standing gate, `OP` when it holds operator-wide, `CLAUDE.md` for codescout-specific discipline, `DC` for a decayed written claim. Promotion does **not** close the cluster — the membership query stays live |

## Work-stream-specific prefixes (not durable taxonomy slots)

These appear inside individual session logs / specs and are scoped to one
work stream — not project-wide ID namespaces. The session log defines them
in its own header; they don't need slots here.

- **S-NN** — Session residuals: open follow-ups when a multi-session work
  stream wraps. Example: `docs/trackers/archive/2026-05-07-retrieval-session-residuals.md`.
- **D-NN** — Design decisions inside a spec. Example: a multi-decision
  spec uses D-1 through D-N for the decisions enumerated in that document.

**Resume queues (opened 2026-08-28).** One declared ledger per
partially-implemented work stream, each pickable by a fresh session. **This section
stores no count** — derive it, because three stored ones rotted here simultaneously
(2026-09-04: *"Six declared ledgers"* and *"all five are prose ledgers"* sat in
consecutive clauses, and *"five of the six are live"* followed the table; the *five*
never matched its own six-row table, so it shipped wrong rather than decayed):

```sh
# every declared ledger and its prefix — this is the population
grep -l '^entry_prefix:' docs/trackers/resume-*.md
```

**The filename glob is NOT the membership test** — `resume-*.md` over-matches. The real
marker is a declared `entry_prefix`, and at least one file matches the glob without
being a ledger (`resume-vacation-wrapup-2026-09-04.md` is prose: no prefix, no
`## PREFIX-N` sections, nothing in it citable). A count taken from the glob reads high,
which is how the last one broke.

Every ledger here is a **prose ledger** — `entry_prefix` +
`entry_high_water_<PREFIX>` in committed frontmatter, entries as
`## PREFIX-N — <title>` body sections, **no augmentation** (rationale:
`docs/conventions/cross-machine-catalog-resume.md` — a queue meant for another
session or machine cannot keep its state in a git-ignored catalog).

| Prefix | Tracker | Stream |
|---|---|---|
| **SV-N** | `resume-statement-validity-layers-3-5.md` | Statements: Layers 3c/5b — `asserted_at`, attestation storage, the decay default |
| **GG-N** | `resume-get-guide-section-grain-phases-2-3.md` | `get_guide` section grain: Phase 2 (`tracker-conventions`) and Phase 3 (8 topics) |
| **WP-N** | `resume-workspace-pinning-phase-4b-5.md` | Per-request workspace pinning: Phase 4b (per-`Workspace` locking, eviction) + Phase 5 |
| **ET-N** | `resume-embedding-transport-stages-1-3.md` | Embedding transport consolidation: Stages 1–3 |
| **CM-N** | `resume-cross-machine-catalog-restore.md` (`f4923e5e894de62f`) | Work left after a cross-machine catalog resume — what was restored, what was decided against, what is permanently lost. 9 entries, prose. Opened by a concurrent session the same day; row added here per its own `CM-9`, which specified it rather than editing this file while a peer held it |
| **TB-N** | `resume-tool-surface-budget.md` | Tool Surface Budget — **archived same day**: the stream had shipped in full on 2026-08-18 and the queue was opened on a false-negative grep. Kept as the only documentation of the live `TOOL_SURFACE_CHAR_BUDGET` gate |
| **SM-N** | `resume-tool-surface-structural-mechanisms.md` (`25633146506bd8b3`) | Tool Surface — structural mechanisms. **Row missing from this table until 2026-09-04**, which is the defect that exposed the three stale counts above |
| **AC-N** | `resume-artifact-chunk-grain-retrieval.md` (`9ba8a7a553b7a097`) | Artifact chunk-grain retrieval. `AC-1` states what a **wrong** result looks like alongside a right one — a stale MCP server returns *"changed nothing"* as a plausible number rather than an error, so its pre-flight is `readlink /proc/<pid>/exe` |
| **RQ-N** | `resume-queue-index.md` (`ea6e212a549f9972`) | **tracker → remaining work**, across all repos. The index *of* the queues rather than one of them; restates none of them by design, and carries its own known holes as declared holes |

**`TB-N` is the only closed record here; every other row is a live queue.** It is listed
because its prefix is declared and its entries are citable, not because there is work in
it. **No count is given deliberately** — run the `grep -l` above.

**Three sibling resume surfaces exist, answering different questions.** This table is
**prefix → file**, permanent, codescout only. `RQ-N` is **tracker → remaining work**, all
repos. `docs/trackers/resume-vacation-wrapup-2026-09-04.md` is **session → status** at
one instant, all repos — prose, not a ledger, which is why it has no row above.

Append to any of them with
`doc(action="append_entry", id=…, id_prefix="<PREFIX>", anchor_heading="## Template for new entries", title=…, body=…)`
— one call, from the main checkout. No `entry_collection`.

**Other declared ledgers scoped to one work stream** (registered 2026-08-28 — all eight
were declared and defining entries while this document listed none of them, which is what
`tracker-hygiene-log:HY-22` records). `HY` and `OP` earned main-table rows instead; the
rest are here. **`CM` is the ninth of that set** and sits in the resume-queue table above
instead, because it is literally a `resume-*.md` queue; see
`resume-cross-machine-catalog-restore:CM-9`, which found it independently and counted it
as the ninth.

| Prefix | Ledger | Captures | Entries | Append |
|---|---|---|---:|---|
| **AA-N** | `artifact-augmentation-followups.md` (`aabef87ec988dc1d`) | Enhancements to the `artifact_augmentation` feature — the followup roadmap after v1 | 21 | prose |
| **CTX-N** | `context-performance.md` (`5d7f5c0a41d0b6f3`) | Packing-constant measurements: a constant chosen once from intuition that outlived the corpus justifying it | 2 | prose |
| **FND-N** | `fable-tuning-findings.md` (`35de33286cd34f87`) | Fable tuning — findings | 18 | `entry_collection="findings"` |
| **FT-N** | `fable-tuning-tasks.md` (`ad1af8262fdce357`) | Fable tuning — tasks. **Owned `T` until `d3282868`**; surrendered it because `tool-usage-patterns` had the stronger claim (`CLAUDE.md` hard-codes `id_prefix="T"` for `f2ecdd76a6189efb`). `params_schema` now pins `^FT-\d+$`, so writing `T-` here is refused | 12 | `entry_collection="tasks"` |
| **GF-N** | `2026-08-16-iron-law-gate-firing-audit.md` (`ac8fbe339e66ade3`) | One empirical pass over codescout's own `usage.db` asking what the TU-N investigation did not | 8 | prose |
| **SD-N** | `structural-debt-refactor.md` (`38a17e4acf1f1fa1`) | Structural debt in the 690-commit `experiments` cohort. `status: draft` | 11 | prose |

`entry_collection` named ⇒ pass it to `append_entry` with an `entry={…}` row; `prose` ⇒
omit it and pass `anchor_heading` + `title` + `body` so the server writes the
`## PREFIX-N — <title>` section itself.

If you find yourself wanting to introduce a new project-wide prefix, ask first
whether it really earns a slot or whether it's a variant of one of the **eleven**
in the table above.

### Measured drift — 2026-08-19, re-derived 2026-08-28

**Re-derived 2026-08-28.** The 2026-08-19 figures below are kept because the *shape* they
describe is still right and the movement between them is the point.

| | 2026-08-19 | 2026-08-28 |
|---|---:|---:|
| prefixes defining entries | 29 | **38** |
| defining `## PREFIX-N — title` headings | not counted | **1,332** |
| defining prefixes with **no** row here | 17 | **11** (the scan says 12 — `BUG` is documented, see below) |

The count went *down* while the population went *up*, because this pass registered eight
declared-but-undocumented ledgers (`AA` `CTX` `FND` `FT` `GF` `SD` in the work-stream table
above, `HY` and `OP` in the main one) plus five resume queues. See
`tracker-hygiene-log:HY-22` for how the gap was found and why no detector reports it.

The eleven without a row, with the number of files defining each — **and none of them is a
defect**:

| Prefix | Files | Why no row |
|---|---:|---|
| `C` | 10 | Spec-local case numbering across the 2026-05-15/16 nav-eval rounds. Correctly local |
| `ADR` | 4 | The ADR numbering convention, not a ledger |
| `E` | 2 | Spec-local |
| `BL` | 1 | Self-documented in `open-issue-work-queue.md`'s own header, which explicitly says work-stream-scoped, *"not a project-wide namespace"* |
| `AB` `B` `DF` `I` `L` `TMR` `TU` | 1 each | One-off spec- or session-local namespaces |

`CM` was in this list for five minutes. It was a one-off when the row was written and a
declared resume-queue ledger with 9 entries by the time it was committed — see the
seven-minute note under § *Main taxonomy*. Re-derive; a characterisation decays exactly
like a count.

`BUG` defines 41 headings and **is** documented — as the `BUG (slug)` row in the main
table, whose format a `**PREFIX-N**` scan does not match. Do not "fix" it.

**Re-derive; do not trust these numbers.** Both a raw count and the reasoning above decay,
and this section has now been wrong twice (see the parenthetical under § *Main taxonomy*,
which records the same failure a third time for the declared-prefix count). The commands:

```bash
# prefixes that define entries, and how many headings each
grep -rhoE '^#{1,6} [A-Z]{1,3}-[0-9]+ *[—–-] ' --include='*.md' docs/ > /tmp/defs.txt
sed -E 's/^#+ ([A-Z]{1,3})-.*/\1/' /tmp/defs.txt | sort | uniq -c | sort -rn

# prefixes that DECLARE (frontmatter entry_prefix) — fence-aware caveat below
grep -rh '^entry_prefix:' --include='*.md' docs/
```

**The declaration grep is fence-naive and the guard is not.** `grep '^entry_prefix:'` also
matches a line inside a ```` ```yaml ```` block or a Rust raw-string literal, so it reports
`HY` in `docs/manual/src/concepts/entry-citations.md` and `OP` in
`docs/superpowers/plans/2026-08-27-operator-rules-phase-1.md` — neither of which is a
ledger. Probed 2026-08-28: `edit_file` on both returns *"heading not found"*, not the
ledger refusal, so the guard **does** skip fenced declarations. Filter by hand, or trust the
guard over the grep.
## Trackers that deliberately own no prefix

Not every tracker is a ledger. Measured 2026-08-17: 27 unaugmented trackers under
`docs/trackers/`, and only **three** owned an id namespace. The rest are design docs,
research notes and finished logs — they own no ids and stay directly editable.

Listed here so nobody "fixes" them by adding a prefix.

| Tracker | Why no prefix |
|---|---|
| [`sdd-ruling-log.md`](trackers/sdd-ruling-log.md) | Records decisions an agent made autonomously during a `subagent-driven-development` run, which the run's own ledger deletes at finish. The unit of value is the **ruling line**, not an index — nothing cites an individual ruling, and the table exists to be mined across runs (`verdict` is the column that matters: which rulings turned out wrong, and what they had in common). Giving it ids would add citation ceremony to rows nobody cites. |

## How to choose

```
You observed something. Where does it go?
│
├─ Is it a bug? (wrong output, silent failure, corrupt state)
│   → BUG file in docs/issues/  (open one per the CLAUDE.md trigger rules)
│   → if it is Windows-specific, ALSO add a WIN-N row to windows-platform-support.md
│
├─ Is it a Windows-platform defect / portability gap / cfg-gate decision?
│   → WIN-N in windows-platform-support.md (augmented artifact 42dfdfc8b1522192)
│
├─ Is it a friction with codescout / MCP / hooks / Iron Laws?
│   → U-N in codescout-usage-frictions.md
│
├─ Is it a design idea for a hook / gate / IL refinement?
│   → H-N in codescout-usage-hookify.md
│
├─ Is it a tool-selection observation worth reviewing later?
│   → T-N in tool-usage-patterns.md (artifact f2ecdd76a6189efb)
│
├─ Is it about the recon skill itself (hit / miss / proposal)?
│   → R-N in reconnaissance-patterns.md
│
├─ Is it a defect CLASS whose right party structurally cannot see it?
│   → OB-N in observer-blindness.md (artifact 3922c2a0fd0dfcfc)
│     Two admission tests, both required:
│       (a) would a MORE CAREFUL version of the same party have caught it?
│           yes → it is an F-N or a bug file, not an OB
│       (b) does it return a PLAUSIBLE ANSWER rather than an error?
│           no → it is loud, and something downstream already fires
│     Name the blind party, who CAN see it, and a candidate mechanism.
│     No mechanism yet? Write "none yet" — that is the worklist, not a gap.
│
├─ Is it a feature idea — a capability codescout does not have yet?
│   → CAP-N in capability-proposals.md (artifact 01291679a5ee4707)
│     Do the substrate check BEFORE writing it: what exists today, what is
│     actually missing. An entry without one is not ready.
│     Already has tasks + a file structure? Skip this — write a spec + plan.
│
└─ Is it a per-work-stream friction or win, scoped to one task / refactor?
    ├─ Friction → F-N in <topic>-session-log.md
    └─ Win      → W-N in same file (with counterfactual)
```

## Promotion ladder

```
F-N / W-N    (session log — per work stream, archived when wrapped)
  │
  ├──→ BUG file   if friction stabilizes into a reproducible bug
  │
  └──→ CLAUDE.md / ADR / SKILL.md   if win confirmed 2+ times across work streams
                                    (promote-when criterion fires)

U-N          (codescout usage frictions — durable across sessions)
  │
  ├──→ H-N hookify rule   if friction can be substrate-enforced
  │
  └──→ CLAUDE.md / prompt-surface edit   if convention or guidance fix

H-N          (hookify proposal)
  │
  └──→ Shipped hook in codescout-companion/hooks/  (PR + merge)

R-N          (recon-skill meta)
  │
  └──→ PR against codescout-companion skill SKILL.md   (promote-when fires)

T-N          (tool-usage patterns)
  │
  └──→ src/prompts/source.md edits   (server-instructions surface)

BUG          (per-bug investigation)
  │
  └──→ Fix lands on master + archive move to docs/issues/archive/
```

## SHA-citation rule

Every prefix above may cite git commits as evidence. **Cite the SHA *and* its patch-id.**

A SHA is positional: after cherry-pick + rebase the experiments-side original orphans and
`git branch --contains` returns empty. `git show <sha> | git patch-id --stable` is a content
hash of the diff and survives rebase **and** cherry-pick, so the pair stays resolvable —
with no promotion path to check and nothing owed later.

Full rationale, measurements and the recovery procedure:
[`docs/RELEASE.md` § *Citing a fix: SHA + patch-id*](RELEASE.md).

For cross-repo citations (e.g. tracker in codescout pointing at a fix in
codescout-companion), prefix with the repo name: `codescout-companion:0b75991`.
## Citation format (mandatory)

Every closure that cites a git commit must use one of these shapes — bare SHAs without branch scope are ambiguous and not allowed:

- `(master:<sha>)` — fix has shipped to master, `git branch --contains <sha>` from master returns master.
- `(experiments:<sha>, not-yet-on-master)` — fix exists on experiments only. The qualifier is mandatory.
- `(<repo>:<sha>)` — cross-repo fix; repo prefix names which repo's SHA. E.g. `(claude-plugins:bd20a8a)`. Branch context is master unless further qualified.
- `(in-place)` — for files outside git (e.g. `~/.claude/CLAUDE.md`). No SHA citation.

**A citation is never updated after the fix reaches `master`.** Record the SHA and its patch-id once, at the time of the fix:

```
git show <sha> | git patch-id --stable
```

The SHA is **positional** — `experiments` is rebased after every ship, so a cherry-picked commit's original is orphaned and eventually garbage-collected. The patch-id is a **content hash of the diff** and survives both rebase and cherry-pick, so it still finds the change after the SHA dies. Measured 2026-08-19: zero genuine collisions across 3594 commits, and all 104 duplicate patch-ids were the same change appearing on two branches — the anchor working, not failing.

Recording both is what **replaces** the former cherry-pick-vs-fast-forward decision and the "master-side SHA still owed" follow-up. There is no promotion path to check and nothing to come back for.

Policy: [`docs/trackers/archive-cadence-policy.md`](trackers/archive-cadence-policy.md) — surface 1.

## Archive cadence

Closed entries graduate to `docs/trackers/archive/<tracker>-<YYYY>-q<n>.md`:

- **Trigger**: status is closed AND (SHA is on master OR closure is `wontfix` / `by-design` / `substrate-caught` / cross-repo verified).
- **Cadence**: manual quarterly pass + accelerated by release cuts.
- **Archive file**: per-tracker, time-partitioned. Frontmatter `kind: tracker, status: archived`.
- **Recovery**: `doc(action="find", kind="tracker", include_archived=true)`.

Full policy: [`docs/trackers/archive-cadence-policy.md`](trackers/archive-cadence-policy.md).

## Status vocabularies (per prefix)

Different prefixes use slightly different status enums:

- **F-N statuses** — `open | mitigated | fixed-verified | wontfix-false-alarm | promoted-to-bug-tracker | pinned-as-eval-baseline` (canonical in `docs/templates/session-log.md`).
- **W-N statuses** — `validated | promotion-due | promoted-to-permanent-docs | archived`.
  (`promotion-due` means the `Promote-when` criterion has **fired** and the text is not
  yet on the target surface — an action item, not a resting state. It was missing from
  this list until 2026-08-20 while `docs/templates/session-log.md` defined it.)
- **U-N statuses** — `open | fixed-shipped | promoted | wontfix` (informal).
- **H-N statuses** — `warn | deny | shipped | rejected`.
- **R-N verdicts** — `hit | miss | proposal | promoted`.
- **T-N verdicts** — `legitimate | debatable | wrong-tool`.
- **WIN-N statuses** — `fixed | mitigated | open | deferred | wontfix` (canonical in the `params_schema` of `docs/trackers/windows-platform-support.md`).
- **BUG statuses** — `open | taken | investigating | fixed | mitigated | wontfix | zombie` (canonical in `docs/issues/_TEMPLATE.md`). `taken` means a live session holds it (`claimed_by: <sessionId>`, resolved by `doctor`'s `claim_liveness` check); `investigating` is the residue state — worked, no live owner.

When in doubt, mirror the existing entries in that file — consistency beats
correctness here.

### `**Valid:**` — the one field whose vocabulary is the same everywhere

`**Status:**` says where the *work* stands and varies per prefix. `**Valid:**` says
whether the *claim* is still true, and takes the same three values in every ledger:

```text
**Valid:** invariant                   a law; no expiry
**Valid:** dated 2026-08-20            true of an instant; every measured count
**Valid:** conditional — <event>       true until that named event fires
```

There is no fourth, and no exemption: **declaring nothing is not "no claim here"** —
absence is read as decay. What decides whether an undeclared entry is actually flagged
is *exposure*, the number of other files citing it, so an uncited entry is left alone
because nothing rests on it. A section the server writes via
`doc(action="append_entry")` is stamped `**Valid:** dated <today>` unless the
caller passes a class.

Malformed declarations are refused rather than accepted — a bare `conditional` naming
no event, an unknown class (`conditionally speaking` fails on a word boundary, not a
prefix), and a calendar-invalid date (`dated 2026-02-30`).
`librarian(action="doctor")` reports these as `entry_conditional_past_due`,
`entry_dated_stale`, `entry_cited_from_outside_but_undeclared` and
`validity_unparseable` — read-only worklists, never verdicts.

Its sibling `**Rests on:**` is one durable sentence naming the route back to the proof —
an ADR, a decision, a principle, not a `path:line`. Parsed and stored today; no consumer
reads it yet.

Full treatment: [Statement Validity](manual/src/concepts/statement-validity.md).
Authoring rules: `get_guide("tracker-conventions")` § Required fields.
