---
title: "tracker" → "ledger" rename — eval-gated design
date: 2026-07-10
status: active
kind: spec
owners: [marius]
tags: [taxonomy, librarian, prompt-eval, rename]
---

# "tracker" → "ledger" rename — eval-gated design

## Problem / motivation

codescout's augmented librarian artifacts are called **trackers** (`kind=tracker`,
`docs/trackers/`, `tracker_design`, guide topic `tracker-conventions`). The claim
under test: **"ledger" is a more appropriate name than "tracker"** for what these
artifacts actually are.

The term is not neutral. The two words cue different agent behaviors:

- **ledger** → append-only accounting; cues *append a new entry, preserve prior rows*.
- **tracker** → monitor evolving state; cues *regenerate the current snapshot*.

codescout's artifacts span both shapes:

- **Append-only entry collections** — F-N / W-N / T-N rows, bug files, anything
  driven by `append_entry` with monotonic IDs. Genuinely *ledger*-shaped. (The
  activation-bootstrap guide already calls bugs "the known-bug ledger" informally.)
- **Live-projected snapshots** — goal trackers, the legibility backlog with
  before/after deltas, anything whose body is regenerated from `params` via
  `render_template`. Genuinely *tracker/dashboard*-shaped.

So "ledger" fits the first subset well and may *undersell* the second. Whether that
mismatch costs anything in real agent behavior is an empirical question, not an
aesthetic one — hence the eval gate.

## Decisions (locked in brainstorming, 2026-07-10)

1. **Intended end state: full rename** — migrate stored `kind` values, rename tool
   actions / guide topic / docs dir / prose. Not a surface-only prose shift.
2. **Eval-first gate** — build and run a prompt-tdd naturality eval *before* any
   rename work. If "ledger" does not measurably beat "tracker", reconsider.
3. **The eval decides wholesale-vs-split** — whether "ledger" replaces "tracker"
   everywhere, or only for append-only collections while snapshots keep a distinct
   name, is resolved by the measured snapshot-handling regression (or lack of it).
4. **Aliased-binary eval fidelity** — the "ledger" arm uses a binary whose tool
   schema accepts "ledger" as a synonym, so the guide and the tools agree. This
   avoids the vocabulary-mismatch confound of a guide-only A/B, and the alias code
   is a reusable first increment of the Phase 1 migration (not throwaway).

## Phase 0 — Naturality eval (the gate)

### Location and pattern

`prompt-engineering/scenarios/ledger-vs-tracker/`, following the existing
`scenarios/librarian-guide/` precedent exactly:

- Arm files (`arm-tracker.yaml`, `arm-ledger.yaml`) swap the binary (`mcp_command`)
  and the delivered guide; `registry: anthropic-mcp`.
- `mode: trace`, `runs: 10` for statistical power.
- `--ablate` negative control (strips the system-prompt guide; expect the suite to
  go RED) to prove the term is load-bearing, not incidental.
- Fixtures seeded into an isolated catalog via CLI `artifact create` (set
  `LIBRARIAN_DB` in the run env), so `artifact(find)` returns them.

### Minimal aliased binary (`codescout-ledger`)

Built into scratchpad `eval-bins/` alongside the existing `codescout-C`. A synonym
layer, NOT the full rename:

- **`kind` dual-read** — `a.kind == "tracker" || a.kind == "ledger"` at
  `src/librarian/tools/create.rs:180` and every other `kind` gate.
- **Action alias** — `ledger_design` dispatches to the existing `tracker_design`
  librarian action; `append_entry` / `find` descriptions say "ledger".
- **Vocabulary flip** — tool-description text and the delivered guide use "ledger".

The current `codescout` release binary serves the `tracker` arm unchanged (the
realistic production baseline).

### Scenarios

Each scenario runs as A/B (`tracker` arm vs `ledger` arm) plus `--ablate`.

1. **Append-shape task.** Seed an F-N entry-collection artifact (e.g. a session log
   with existing rows). Message asks the agent to record a new observation.
   - **Correct action:** append a single new entry (`append_entry` or a scoped
     `edit_markdown insert_*`); the existing body is **not** wholesale-overwritten.
   - **Assertions (trace):** `tool_called` for the append path; `tool_not_called`
     for a full-body overwrite (`artifact update` with a `body` replace, or a
     `create_file`/`Write` clobber).
   - **Hypothesis:** ledger ≥ tracker (higher append-rate, lower clobber-rate).

2. **Snapshot-shape task.** Seed a live goal/backlog artifact whose body is a
   projection from params. Message asks the agent to reflect updated state.
   - **Correct action:** regenerate / update the projection (params update +
     refresh, or a targeted section rewrite); **not** a blind append of a new row.
   - **Assertions (trace):** correct update path called; blind-append path NOT
     called.
   - **Hypothesis:** the split discriminator — the case where "ledger" may lose by
     over-cuing append.

### Decision rule

- **Ledger measurably beats tracker on (1) AND no significant regression on (2)**
  → proceed to Phase 1 **wholesale**.
- **Ledger beats on (1) but regresses on (2)** → proceed to Phase 1 **split**:
  ledger = append-only collections; a distinct name (retain "tracker", or a third
  term) = snapshots.
- **No win on (1)** → abandon the rename; record the negative result.

"Measurably" / "significant": pre-register the threshold before running (e.g. the
`librarian-guide` suite's convention of N≥10 with a clear pass-rate delta). Save a
prompt-tdd baseline so the result is reproducible and regressions are caught.

## Phase 1 — Full rename (contingent on the gate)

Sketched at tier granularity here; the detailed implementation plan is written only
*after* the gate passes — writing a full migration plan for a rename the eval might
reject would be wasted work.

Four blast-radius tiers, in dependency order:

1. **Stored data (hard).** A `migrate_v7.rs` flipping `kind: tracker → ledger` in
   the catalog column and in every managed frontmatter block. Ship with the
   dual-read alias (from Phase 0) still on for one release as a safety net; remove
   it a release later. Rename `docs/trackers/` → `docs/ledgers/` **through the
   librarian** (`artifact update` status/path + `artifact move`), never a bare
   `git mv` — `id = sha256(abs_path)`, so a hand-move orphans the catalog row's
   events and augmentation.
2. **Global config.** `~/.config/librarian/workspace.toml` classification-rule
   globs: `**/docs/trackers/**`, `**/*_TRACKER.md`, `**/*-tracker.md`. Cross-repo
   config, outside the codescout tree.
3. **API / tool surface.** `tracker_design` → `ledger_design`; guide topic
   `tracker-conventions` → `ledger-conventions`; `kind` param docs; the three
   prompt surfaces (`src/prompts/source.md` `server_instructions` +
   `onboarding_prompt` slices, and `build_system_prompt_draft()` in `builders.rs`);
   `CLAUDE.md`; codescout memories; and the **three sibling repos** in the
   `codescout-ecosystem` umbrella (`prompt-engineering`, `claude-plugins`,
   `llm-proxy`). Bump `ONBOARDING_VERSION`.
4. **Prose (soft).** Live docs only. Frozen historical records
   (`docs/superpowers/plans/*`, `docs/**/archive/*`) are left as-is — they document
   what was true at the time.

### Blast-radius counts (2026-07-10 snapshot)

| Surface | Approx. scale |
|---|---|
| Rust `[Tt]racker` matches | 519 in 69 files |
| Rust `"tracker"` string literal (stored `kind`) | 83 in 31 files |
| Markdown `tracker` matches | 2435 in 304 files (mostly frozen history) |
| Global config classification-rule globs | 3 globs |

## Testing strategy

- **Phase 0:** the eval *is* the test — prompt-tdd `run` red/green + baseline diff.
- **Phase 1:** existing librarian catalog tests (dual-read alias must keep them
  green through the transition), the `prompt_surfaces_reference_only_real_tools`
  and `claude_md_contains_no_deprecated_tool_names` gates (updated to the new
  vocabulary), a migration round-trip test (`migrate_v7`), and the
  `audit_doc_refs` gate for doc-link drift.

## Out of scope

- Rewriting frozen historical plans/specs/archived session logs.
- Renaming sibling-repo internals beyond their references to codescout's taxonomy.
- Any Phase 1 work before the Phase 0 gate passes.
