# Session Log — Template

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries via `edit_markdown(action="insert_before", heading="## Template
> for new entries", content=...)`. Add a row to the Index / Wins Index
> table for each new entry — the indexes are the eval surface, the
> sections are the evidence.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (CLAUDE.md, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-06-27 | med | architectural | fixed-verified | `debug_enforce_symbol_tools` redundant with always-on structural guard |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-06-27 | med | scout config+git+issues before defending a mechanism's retention | would have shipped "keep the knob" — a wall in an empty field | validated |
---

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

Copy this block when appending a new friction. Allocate the next free
ID. Add a matching row to the Index table.

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

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Copy this block when appending a new win. A win without a
**Counterfactual** is marketing — name what would have happened
without the pattern, with at least one piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

**Status:** validated | promoted-to-permanent-docs | archived

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
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. Session log keeps the pointer. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — `debug_enforce_symbol_tools` flag is redundant with the always-on structural guard

**Observed:** 2026-06-27, debugging the "can't add a `data` modifier without escaping to native Edit" friction in backend-kotlin; evolved into a DRY/SOLID review of `edit_file`'s structural-edit gate.

**When:** Deciding whether to keep or retire `debug_enforce_symbol_tools` while consolidating `edit_file`'s structural-edit enforcement to a single chokepoint.

**Expected (my initial recommendation):** Keep the flag as a parameter of the consolidated chokepoint — "preserves a deployment knob a deployment may want (force-symbol-tools as policy)."

**Got (scouted reality):** The flag has no distinct purpose. (1) `guard_structural_rewrite` (always-on guard at `edit_file/mod.rs:429` batch + `:583` single) blocks the *identical* set of structural edits regardless of the flag — proven by `batch_edit_blocks_new_symbol_introduction_via_new_string` (default config, blocks) vs `edit_file_blocked_on_source_file_when_debug_enforce_symbol_tools` (flag on, same block). (2) Commit `fbd8bbdc` narrowed the flag to structural-only, making it coextensive with that guard. (3) The flag is set `true` only in this repo's `.codescout/project.toml` for dogfooding — a purpose the always-on guard already serves. (4) Its only remaining distinct effects are a *worse* error message (the generic "blocked for structural edits" that preempts the rich "which indices were safe / single-line is allowed" guidance) and an earlier return — plus a documented deadlock failure mode (`docs/issues/2026-06-13-rust-lsp-mux-spawn-fail-deadlocks-source-editing.md`).

**Probable cause:** `fbd8bbdc` made the flag-gate diff-aware by reusing `guard_structural_rewrite` — the same predicate the always-on guard uses — leaving two enforcement entry points sharing one predicate and diverging only in message. The redundancy was born in the fix.

**Workaround:** Recommendation revised — retire the flag; fold enforcement into the single always-on chokepoint emitting the rich message. Remove the field + the `= true` line from `.codescout/project.toml` (no `deny_unknown_fields`, so a stale key is silently ignored — no migration break).

**Severity:** med — would have shipped a "keep the knob" design that preserves the exact moat-weakening message that caused the original native-Edit escape, leaving the divergent-surface defect half-fixed.

**Status:** fixed-verified — Commits 1 (retire flag) + 2 (single-line escape-hatch hint) landed on `experiments`; fmt + clippy `-D warnings` + full lib green (2813). Captured as `docs/issues/2026-06-27-edit-file-generic-structural-message-preempts-rich-guidance.md`. Archive after the fix reaches `master`.

**Fix idea / Pointer:** Pending consolidation plan (this work stream). Sub-findings to route: the generic-message-preempts-rich-message defect → open a new `docs/issues/` bug; the deadlock interaction → `docs/issues/2026-06-13-rust-lsp-mux-spawn-fail-deadlocks-source-editing.md`.

---

## W-1 — Scout a mechanism's real usage before defending its retention

**Observed:** 2026-06-27, when challenged ("why keep it? does it serve a purpose?") on a recommendation to retain `debug_enforce_symbol_tools`.

**Pattern:** Before recommending that a config flag / mechanism be *kept*, scout its actual usage across three surfaces — where it's set (`grep '<flag> = true'` repo-wide, configs included), when it was introduced/narrowed (`git log -S<flag>`), and what it's documented to break (issue tracker) — rather than asserting a purpose from its name.

**Counterfactual:** Without the grep that surfaced `docs/issues/2026-06-13-...` (the deadlock) and the `git log -S` that surfaced `fbd8bbdc` (the narrowing that made the flag coextensive with the guard), I'd have shipped the "keep the knob, it may serve a deployment policy" recommendation — a wall with no named change scenario, preserving the divergent inferior message. The grep + pickaxe converted a soft defense into an evidence-backed "retire."

**Confirming data points:** (1) F-1 this session — the flag's claimed purpose dissolved under the scout. (2) The architecture memory `agentic-surface-as-moat` predicted the cost (LLM-facing message weighs heavier than backend code), confirming message-divergence was the real defect, not line count.

**Impact:** med — prevented a net-regression design decision (keeping a redundant gate that weakens the agentic surface).

**Promote-when:** A second instance where scouting a mechanism's real usage (config + git + issues) before a keep/retire call overturns the initial instinct. At 2 datapoints, promote to CLAUDE.md as a pre-decision scout rule.

**Status:** validated

---

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
